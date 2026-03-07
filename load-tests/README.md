# Load Testing

Load testing for Symphony using [k6](https://k6.io/).

## Prerequisites

- [k6](https://k6.io/docs/get-started/installation/) installed
- Symphony running locally or remotely

## Running Tests

### Basic Load Test

Standard load test ramping up to 200 concurrent users:

```bash
k6 run load-test.js
```

With custom base URL:

```bash
BASE_URL=http://symphony.example.com k6 run load-test.js
```

### Stress Test

Push to breaking point with 1000 concurrent users:

```bash
k6 run stress-test.js
```

### Soak Test

Sustained load over 1 hour:

```bash
k6 run soak-test.js
```

### Spike Test

Sudden traffic burst:

```bash
k6 run spike-test.js
```

## Test Scenarios

### Load Test
- **Duration**: ~16 minutes
- **Users**: 100 → 200 concurrent
- **Purpose**: Validate performance under expected load

### Stress Test
- **Duration**: ~19 minutes
- **Users**: 500 → 1000 concurrent
- **Purpose**: Find breaking point

### Soak Test
- **Duration**: ~80 minutes
- **Users**: 100 concurrent
- **Purpose**: Check for memory leaks, resource exhaustion

### Spike Test
- **Duration**: ~70 seconds
- **Users**: 10 → 1000 → 10
- **Purpose**: Test recovery from sudden traffic bursts

## CI/CD Integration

Run load tests in CI:

```yaml
- name: Run load tests
  run: |
    docker run -v $(pwd)/load-tests:/tests grafana/k6 run /tests/load-test.js
```

## Interpreting Results

### Key Metrics

- **http_req_duration**: Request latency
  - p(95) < 500ms is good
  - p(99) < 1000ms is acceptable

- **http_req_failed**: Error rate
  - Should be < 0.1% for production
  - < 1% is acceptable for stress tests

- **vus**: Virtual users
  - Shows how many concurrent users the system handled

- **iterations**: Total requests made
  - Higher is better

## Performance Targets

| Metric | Target | Acceptable |
|--------|--------|------------|
| p95 Latency | < 100ms | < 500ms |
| p99 Latency | < 500ms | < 1000ms |
| Error Rate | < 0.01% | < 0.1% |
| RPS | > 1000 | > 500 |
