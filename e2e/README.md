# Symphony E2E Tests

End-to-end tests for Symphony using Vitest and Testcontainers.

## Prerequisites

- Node.js 20+ 
- Docker running locally
- The `symphony-rust:local` Docker image built

## Setup

```bash
cd e2e
npm install
```

## Build Required Image

```bash
cd ..
docker build -f Dockerfile.rust -t symphony-rust:local .
cd e2e
```

## Run Tests

```bash
# Run all tests
npm test

# Run with watch mode
npm run test:watch

# Run with UI
npm run test:ui

# Run specific test file
npx vitest run tests/symphony.e2e.test.ts
```

## Test Coverage

The E2E suite covers:

### API Endpoints
- `GET /api/v1/state` - Returns complete state snapshot
- `POST /api/v1/refresh` - Queues refresh operation
- `GET /` - HTML dashboard
- `GET /api/v1/<issue>` - Issue detail (404 for missing)

### Scenarios
- Container startup and health checks
- State consistency across requests
- Activity metrics validation
- Token tracking
- Concurrent request handling
- CLI override flags
- Error handling (missing files, invalid args)

### Validation
- All responses validated with Zod schemas
- Type-safe test assertions
- JSON schema validation for API contracts
- HTML structure validation

## Test Structure

```
e2e/
├── tests/
│   ├── symphony.e2e.test.ts       # Main E2E tests
│   ├── symphony.edge-cases.test.ts # Edge cases & errors
│   ├── schemas.ts                  # Zod validation schemas
│   └── fixtures.ts                 # Test fixtures
├── package.json
├── tsconfig.json
└── vitest.config.ts
```

## Configuration

Tests run sequentially (maxWorkers: 1) to avoid port conflicts.
Default timeouts:
- Test timeout: 2 minutes
- Hook timeout: 1 minute  
- Container startup: 1 minute

## CI Integration

```yaml
# Example GitHub Actions
- name: Run E2E Tests
  run: |
    docker build -f Dockerfile.rust -t symphony-rust:local .
    cd e2e
    npm ci
    npm test
```

## Debugging

```bash
# Run with verbose logging
DEBUG=testcontainers* npm test

# Run single test
npx vitest run --reporter=verbose tests/symphony.e2e.test.ts -t "should return valid state"

# Keep containers after test for inspection
KEEP_CONTAINERS=true npm test
```
