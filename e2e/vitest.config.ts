import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    globals: true,
    environment: 'node',
    testTimeout: 120000, // 2 minutes for container startup
    hookTimeout: 60000,
    teardownTimeout: 30000,
    pool: 'forks', // Better for testcontainers
    maxWorkers: 1, // Sequential to avoid port conflicts
    reporters: ['verbose'],
    coverage: {
      provider: 'v8',
      reporter: ['text', 'json', 'html'],
      exclude: ['node_modules/', 'dist/']
    }
  },
  resolve: {
    alias: {
      '@': './src'
    }
  }
});
