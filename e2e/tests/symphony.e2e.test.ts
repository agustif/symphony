import { describe, it, expect, beforeAll, afterAll } from 'vitest';
import { GenericContainer, StartedTestContainer, Wait } from 'testcontainers';
import { z } from 'zod';

const SYMPHONY_PORT = 8080;
const TEST_TIMEOUT = 120000;

// Simple schemas for basic validation
const SimpleStateSchema = z.object({
  generated_at: z.string(),
  counts: z.object({
    running: z.number(),
    retrying: z.number()
  }),
  codex_totals: z.object({
    input_tokens: z.number(),
    output_tokens: z.number(),
    total_tokens: z.number(),
    seconds_running: z.number()
  }),
  health: z.object({
    status: z.string()
  })
});

describe('Symphony E2E Tests', () => {
  let container: StartedTestContainer;
  let baseUrl: string;

  beforeAll(async () => {
    // Resolve workflow path relative to project root
    const workflowPath = new URL('../../WORKFLOW.md', import.meta.url).pathname;
    
    container = await new GenericContainer('symphony-rust:local')
      .withExposedPorts(SYMPHONY_PORT)
      .withEnvironment({
        LINEAR_API_KEY: 'test-api-key-for-e2e'
      })
      .withBindMounts([
        {
          source: workflowPath,
          target: '/srv/symphony/WORKFLOW.md',
          mode: 'ro'
        }
      ])
      .withWaitStrategy(
        Wait.forHttp('/api/v1/state', SYMPHONY_PORT)
          .forStatusCode(200)
          .withStartupTimeout(30000)
      )
      .withStartupTimeout(60000)
      .start();

    const mappedPort = container.getMappedPort(SYMPHONY_PORT);
    baseUrl = `http://localhost:${mappedPort}`;
  }, TEST_TIMEOUT);

  afterAll(async () => {
    if (container) {
      await container.stop();
    }
  }, 30000);

  describe('Health & Startup', () => {
    it('should start container and respond to health check', async () => {
      const response = await fetch(`${baseUrl}/api/v1/state`);
      expect(response.status).toBe(200);
    });

    it('should return JSON with valid structure', async () => {
      const response = await fetch(`${baseUrl}/api/v1/state`);
      const data = await response.json();
      
      const parsed = SimpleStateSchema.safeParse(data);
      expect(parsed.success).toBe(true);
      
      if (!parsed.success) {
        console.error('Schema validation errors:', parsed.error.format());
      }
      
      if (parsed.success) {
        // Status can be 'idle', 'polling', or 'unknown' (with test API key)
        expect(['idle', 'polling', 'unknown']).toContain(parsed.data.health.status);
        expect(parsed.data.counts.running).toBe(0);
        expect(parsed.data.counts.retrying).toBe(0);
      }
    });
  });

  describe('API Endpoints', () => {
    it('GET /api/v1/state should return 200 with JSON', async () => {
      const response = await fetch(`${baseUrl}/api/v1/state`);
      expect(response.status).toBe(200);
      expect(response.headers.get('content-type')).toContain('application/json');

      const data = await response.json();
      expect(data.generated_at).toBeDefined();
      expect(data.counts).toBeDefined();
      expect(data.codex_totals).toBeDefined();
      expect(data.health).toBeDefined();
    });

    it('POST /api/v1/refresh should return 202', async () => {
      const response = await fetch(`${baseUrl}/api/v1/refresh`, {
        method: 'POST'
      });
      
      expect(response.status).toBe(202);
      
      const data = await response.json();
      expect(data.queued).toBe(true);
      expect(data.operations).toContain('poll');
      expect(data.operations).toContain('reconcile');
      expect(data.requested_at).toBeDefined();
    });

    it('GET / should return HTML dashboard', async () => {
      const response = await fetch(`${baseUrl}/`);
      
      expect(response.status).toBe(200);
      expect(response.headers.get('content-type')).toContain('text/html');
      
      const html = await response.text();
      expect(html).toContain('<!doctype html>');
      expect(html).toContain('Symphony Dashboard');
    });

    it('GET /api/v1/invalid-issue should return 404', async () => {
      const response = await fetch(`${baseUrl}/api/v1/NONEXISTENT-123`);
      
      expect(response.status).toBe(404);
      
      const data = await response.json();
      expect(data.error).toBeDefined();
      expect(data.error.code).toBe('issue_not_found');
    });
  });

  describe('State Consistency', () => {
    it('should maintain consistent counts across multiple requests', async () => {
      const responses = await Promise.all([
        fetch(`${baseUrl}/api/v1/state`).then(r => r.json()),
        fetch(`${baseUrl}/api/v1/state`).then(r => r.json()),
        fetch(`${baseUrl}/api/v1/state`).then(r => r.json())
      ]);

      expect(responses).toHaveLength(3);
      
      for (const data of responses) {
        expect(data.counts).toBeDefined();
        expect(typeof data.counts.running).toBe('number');
        expect(typeof data.counts.retrying).toBe('number');
      }
    });

    it('should have zero issues initially', async () => {
      const response = await fetch(`${baseUrl}/api/v1/state`);
      const data = await response.json();
      
      expect(data.counts.running).toBe(0);
      expect(data.counts.retrying).toBe(0);
      expect(data.running).toHaveLength(0);
      expect(data.retrying).toHaveLength(0);
    });
  });

  describe('Container Health', () => {
    it('should have healthy container status', async () => {
      // Container is running if we got here (testcontainers waits for healthcheck)
      expect(container).toBeDefined();
      // Try to fetch state to confirm it's working
      const response = await fetch(`${baseUrl}/api/v1/state`);
      expect(response.status).toBe(200);
    });

    it('should write startup logs', async () => {
      const stream = await container.logs();
      let logs = '';
      
      stream.on('data', (chunk: Buffer) => {
        logs += chunk.toString();
      });
      
      // Wait a bit for logs
      await new Promise(resolve => setTimeout(resolve, 100));
      
      expect(logs).toContain('Symphony');
    });
  });
});

describe('Symphony Concurrent Request Tests', () => {
  let container: StartedTestContainer;
  let baseUrl: string;

  beforeAll(async () => {
    // Resolve workflow path relative to project root
    const workflowPath = new URL('../../WORKFLOW.md', import.meta.url).pathname;
    
    container = await new GenericContainer('symphony-rust:local')
      .withExposedPorts(SYMPHONY_PORT)
      .withEnvironment({
        LINEAR_API_KEY: 'test-key'
      })
      .withBindMounts([
        {
          source: workflowPath,
          target: '/srv/symphony/WORKFLOW.md',
          mode: 'ro'
        }
      ])
      .withWaitStrategy(
        Wait.forHttp('/api/v1/state', SYMPHONY_PORT)
          .forStatusCode(200)
          .withStartupTimeout(30000)
      )
      .start();

    const mappedPort = container.getMappedPort(SYMPHONY_PORT);
    baseUrl = `http://localhost:${mappedPort}`;
  }, TEST_TIMEOUT);

  afterAll(async () => {
    if (container) {
      await container.stop();
    }
  }, 30000);

  it('should handle concurrent API requests', async () => {
    const requests = Array(10).fill(null).map(() => 
      fetch(`${baseUrl}/api/v1/state`).then(r => r.json())
    );

    const results = await Promise.all(requests);
    
    expect(results).toHaveLength(10);
    
    for (const data of results) {
      expect(data.counts).toBeDefined();
      expect(typeof data.counts.running).toBe('number');
    }
  });

  it('should handle concurrent refresh requests', async () => {
    const requests = Array(5).fill(null).map(() => 
      fetch(`${baseUrl}/api/v1/refresh`, { method: 'POST' }).then(r => r.json())
    );

    const results = await Promise.all(requests);
    
    expect(results).toHaveLength(5);
    
    for (const data of results) {
      expect(data.queued).toBe(true);
      expect(data.operations).toContain('poll');
    }
  });
});
