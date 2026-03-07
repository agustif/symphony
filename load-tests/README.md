# Load Testing

Load testing for Symphony using [k6](https://k6.io/).

The scripts in this directory use fixed arrival-rate executors for fairer cross-runtime comparisons. That keeps offered load constant instead of letting a slower implementation silently issue fewer requests.

## Prerequisites

- [k6](https://k6.io/docs/get-started/installation/) installed
- Symphony running locally or remotely

## Running Tests

### Basic Load Test

Standard fixed-rate load test:

```bash
k6 run load-test.js
```

With custom base URL:

```bash
BASE_URL=http://symphony.example.com RATE=100 DURATION=45s k6 run load-test.js
```

### Stress Test

Push toward saturation with a ramping offered load:

```bash
START_RATE=50 STAGE1_RATE=150 STAGE2_RATE=250 STAGE3_RATE=350 k6 run stress-test.js
```

### Soak Test

Sustained fixed-rate load:

```bash
DURATION=10m RATE=50 k6 run soak-test.js
```

### Spike Test

Sudden traffic burst at fixed offered load:

```bash
BASE_RATE=20 SPIKE_RATE=250 RECOVERY_RATE=20 k6 run spike-test.js
```

## Test Scenarios

### Load Test
- **Executor**: `constant-arrival-rate`
- **Default offered load**: 80 requests/sec
- **Purpose**: Validate steady-state latency at a fixed arrival rate

### Stress Test
- **Executor**: `ramping-arrival-rate`
- **Default offered load**: 50 → 100 → 200 → 300 requests/sec
- **Purpose**: Find breaking point

### Soak Test
- **Executor**: `constant-arrival-rate`
- **Default offered load**: 40 requests/sec for 5 minutes
- **Purpose**: Check for memory leaks, resource exhaustion

### Spike Test
- **Executor**: `ramping-arrival-rate`
- **Default offered load**: 20 → 200 → 20 requests/sec
- **Purpose**: Test recovery from sudden traffic bursts

## CI/CD Integration

Run load tests in CI:

```yaml
- name: Run load tests
  run: |
    docker run --rm \
      --network host \
      -e BASE_URL=http://127.0.0.1:8080 \
      -v "$(pwd)/load-tests:/tests" \
      grafana/k6 run /tests/load-test.js
```

If you cannot use host networking, pass a reachable hostname such as `host.docker.internal` instead of `127.0.0.1`.

## Interpreting Results

### Key Metrics

- **http_req_duration**: Request latency
  - p(95) < 500ms is good
  - p(99) < 1000ms is acceptable

- **http_req_failed**: Error rate
  - Should be < 0.1% for production
  - < 1% is acceptable for stress tests

- **vus / maxVUs**: Virtual user capacity used to sustain the configured arrival rate

- **iterations**: Total requests completed under the configured load profile
  - Compare this together with the configured arrival rate and failure rate

## Performance Targets

| Metric | Target | Acceptable |
|--------|--------|------------|
| p95 Latency | < 100ms | < 500ms |
| p99 Latency | < 500ms | < 1000ms |
| Error Rate | < 0.01 | < 0.05 |
| Sustained Arrival Rate | hold configured target | minor backpressure only |
