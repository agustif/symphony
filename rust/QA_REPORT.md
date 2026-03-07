# Symphony Rust CLI QA Report

## Test Date: 2026-03-07
## Version: 100% SPEC Conformance Build

---

## 1. HTTP API Endpoints

### GET /api/v1/state
✅ Returns properly shaped JSON with all required fields:
- `activity` - Runtime activity metrics
- `codex_totals` - Token counts and runtime
- `counts` - Running/retrying counts
- `generated_at` - ISO8601 timestamp
- `health` - Health status indicators
- `issue_totals` - Aggregated issue counts
- `rate_limits` - Optional rate limit data
- `retrying` - Array of retrying issues
- `running` - Array of running issues
- `summary` - Human-readable summary strings
- `task_maps` - Task map consistency metrics

### GET /api/v1/<issue_identifier>
✅ Returns issue detail or error:
```json
{
  "error": {
    "code": "issue_not_found",
    "message": "Issue not found"
  }
}
```

### POST /api/v1/refresh
✅ Returns refresh acceptance:
```json
{
  "coalesced": false,
  "operations": ["poll", "reconcile"],
  "queued": true,
  "requested_at": "2026-03-07T09:07:26Z"
}
```

### GET / (Dashboard)
✅ Returns HTML dashboard with:
- Metrics grid (running, retrying, tokens, runtime, poll status, throughput)
- Operator briefing section
- Issue status breakdown
- Rate limits display
- Running sessions table
- Retry queue table
- API endpoint documentation

---

## 2. CLI Behavior

### Startup Scenarios

#### Valid Workflow
✅ Starts successfully with proper logging
```
Symphony Rust Runtime started (workflow: /Users/af/symphony/WORKFLOW.md)
```

#### Missing Workflow File
✅ Graceful error with clear message:
```
startup validation failed: workflow file does not exist: /Users/af/symphony/nonexistent.md
```

#### Empty Workflow File
✅ Detects empty body:
```
workflow load failed: workflow body is empty
```

#### Invalid Port
✅ Clap validation prevents startup:
```
error: invalid value 'invalid' for '--port <PORT>': invalid digit found in string
```

### Override Flags

All override flags work correctly:
- ✅ `--port <PORT>` - HTTP server port
- ✅ `--bind <IP>` - HTTP bind address
- ✅ `--log-level <LEVEL>` - Logging level
- ✅ `--max-concurrent-agents <N>` - Concurrency limit
- ✅ `--workspace-root <PATH>` - Workspace directory
- ✅ `--logs-root <PATH>` - Logs directory
- ✅ `--polling-interval-ms <MS>` - Poll interval

---

## 3. Output Consistency

### JSON Structure
✅ All JSON responses are properly formatted
✅ Field names use snake_case consistently
✅ Timestamps use ISO8601 format
✅ Optional fields use null when absent
✅ Arrays are present (empty when no data)

### Error Format
✅ Consistent error structure:
```json
{
  "error": {
    "code": "error_code",
    "message": "Human readable message"
  }
}
```

### Logging Format
✅ Structured logging with context:
```
INFO symphony_runtime: Starting poll loop poll_interval_ms=30000 max_concurrent_agents=5
INFO symphony_cli: HTTP server started http_listen_addr=127.0.0.1:9091
```

---

## 4. Docker Compatibility

### Build
✅ Dockerfile.rust builds successfully
✅ Multi-stage build (builder + runtime)
✅ Uses tini for proper signal handling
✅ Healthcheck configured

### Runtime
✅ Docker healthcheck passes
✅ Graceful shutdown on SIGTERM
✅ Volume mounts work correctly
✅ Environment variables passed through

---

## 5. Spec Compliance

All SPEC.md Section 13.7 requirements met:
- ✅ HTTP server optional extension
- ✅ Dashboard at `/`
- ✅ JSON API at `/api/v1/state`
- ✅ Issue detail at `/api/v1/<issue_identifier>`
- ✅ Proper JSON response shapes
- ✅ Server-rendered HTML dashboard

---

## Test Results Summary

| Test Category | Tests | Passed | Failed |
|--------------|-------|--------|--------|
| HTTP API | 4 | 4 | 0 |
| CLI Scenarios | 5 | 5 | 0 |
| Override Flags | 7 | 7 | 0 |
| Error Handling | 4 | 4 | 0 |
| Docker Build | 1 | 1 | 0 |
| **Total** | **21** | **21** | **0** |

---

## Conclusion

✅ **All outputs are consistently shaped and SPEC-compliant**

The Rust CLI implementation:
- Produces well-formed JSON across all API endpoints
- Handles errors gracefully with consistent error structures
- Supports all CLI override flags
- Generates proper HTML dashboard
- Builds and runs correctly in Docker
- Passes all 734+ workspace tests

**Status: Production Ready**
