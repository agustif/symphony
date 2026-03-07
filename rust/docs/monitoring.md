# Symphony Monitoring Guide

## Health Endpoint

### Endpoint

```
GET /api/v1/state
```

### Response Schema

```json
{
  "status": "<string>",
  "active_agents": <integer>,
  "queue_depth": <integer>,
  "uptime_secs": <integer>,
  "last_poll": "<ISO8601 timestamp>",
  "retry_backlog": <integer>,
  "memory_mb": <float>
}
```

### Example Response

```json
{
  "status": "busy",
  "active_agents": 5,
  "queue_depth": 12,
  "uptime_secs": 86400,
  "last_poll": "2024-01-15T10:30:00Z",
  "retry_backlog": 2,
  "memory_mb": 1024.5
}
```

---

## Health Status Values

| Status | Description | Action Required |
|--------|-------------|-----------------|
| `idle` | System ready, no active work | None - normal state |
| `busy` | Agents actively processing issues | None - normal state |
| `polling` | Currently fetching new issues from Linear | None - transient state |
| `unknown` | System state cannot be determined | Investigate immediately |

### Status Transitions

```
┌─────────┐     poll start     ┌──────────┐
│  idle   │ ──────────────────▶│ polling  │
└─────────┘                    └──────────┘
     ▲                              │
     │                              │ poll complete + work found
     │ no work                      ▼
     │                         ┌─────────┐
     └────────────────────────│  busy   │
           work complete      └─────────┘
                                   │
                                   │ error
                                   ▼
                              ┌──────────┐
                              │ unknown  │
                              └──────────┘
```

---

## Metrics to Monitor

### Primary Metrics

| Metric | Endpoint Field | Description |
|--------|---------------|-------------|
| Health Status | `status` | Current orchestration state |
| Active Agents | `active_agents` | Number of agents processing |
| Queue Depth | `queue_depth` | Issues waiting to be processed |
| Retry Backlog | `retry_backlog` | Failed operations pending retry |
| Memory Usage | `memory_mb` | Current memory consumption |
| Last Poll | `last_poll` | Timestamp of last Linear API poll |

### Derived Metrics

| Metric | Calculation | Description |
|--------|-------------|-------------|
| Poll Latency | `now - last_poll` | Time since last successful poll |
| Throughput | Issues processed / minute | Work completion rate |
| Error Rate | Retries / hour | Failure frequency |

---

## Recommended Alert Thresholds

### Critical Alerts (Immediate Action)

| Alert | Condition | Threshold | Duration |
|-------|-----------|-----------|----------|
| Poll Stall | `now - last_poll` | > 5 minutes | Immediate |
| High Retry Backlog | `retry_backlog` | > 10 | 5 minutes |
| Memory Critical | `memory_mb` | > 1800 MB | 2 minutes |
| Status Unknown | `status == "unknown"` | Any | Immediate |
| Health Check Fail | HTTP response | Non-200 | 3 failures |

### Warning Alerts (Investigation Needed)

| Alert | Condition | Threshold | Duration |
|-------|-----------|-----------|----------|
| Poll Delayed | `now - last_poll` | > 2 minutes | 5 minutes |
| Elevated Retries | `retry_backlog` | > 5 | 10 minutes |
| Memory High | `memory_mb` | > 1500 MB | 5 minutes |
| Queue Backlog | `queue_depth` | > 50 | 15 minutes |
| Agent Saturation | `active_agents == max` | Continuous | 30 minutes |

### Info Alerts (Awareness)

| Alert | Condition | Threshold |
|-------|-----------|-----------|
| Queue Growing | `queue_depth` increasing over time | Trend |
| Long Uptime | `uptime_secs` | > 30 days (consider restart) |

---

## Dashboard Access

### Local Access

```
http://localhost:8080/api/v1/state
```

### Remote Access

```
http://<symphony-host>:8080/api/v1/state
```

### Grafana Dashboard

If using Prometheus + Grafana, import the following dashboard configuration:

```json
{
  "dashboard": {
    "title": "Symphony Monitoring",
    "panels": [
      {
        "title": "Health Status",
        "type": "stat",
        "targets": [
          {
            "expr": "symphony_status",
            "legendFormat": "Status"
          }
        ]
      },
      {
        "title": "Active Agents",
        "type": "gauge",
        "targets": [
          {
            "expr": "symphony_active_agents"
          }
        ]
      },
      {
        "title": "Queue Depth",
        "type": "graph",
        "targets": [
          {
            "expr": "symphony_queue_depth"
          }
        ]
      },
      {
        "title": "Memory Usage",
        "type": "graph",
        "targets": [
          {
            "expr": "symphony_memory_mb"
          }
        ]
      }
    ]
  }
}
```

### Prometheus Scrape Config

```yaml
scrape_configs:
  - job_name: 'symphony'
    static_configs:
      - targets: ['localhost:8080']
    metrics_path: '/api/v1/metrics'
    scrape_interval: 15s
```

---

## Log Monitoring

### Key Log Patterns

| Pattern | Level | Meaning |
|---------|-------|---------|
| `Agent started for issue` | INFO | New agent spawned |
| `Issue completed successfully` | INFO | Work finished |
| `Poll cycle completed` | DEBUG | Normal operation |
| `Retry attempt` | WARN | Transient failure |
| `Linear API error` | ERROR | API connectivity issue |
| `Agent panic` | ERROR | Critical failure |

### Journalctl Commands

```bash
# Follow logs
journalctl -u symphony -f

# Filter by level
journalctl -u symphony | grep -E "ERROR|WARN"

# Last 100 lines
journalctl -u symphony -n 100

# Since time
journalctl -u symphony --since "1 hour ago"
```

### Docker Logs

```bash
# Follow logs
docker compose logs -f symphony

# Last 200 lines
docker compose logs --tail=200 symphony

# Filter errors
docker compose logs symphony 2>&1 | grep -E "ERROR|WARN"
```

---

## Health Check Script

```bash
#!/bin/bash
# /usr/local/bin/symphony-health-check

ENDPOINT="http://localhost:8080/api/v1/state"
RESPONSE=$(curl -sf "$ENDPOINT" 2>/dev/null)

if [ $? -ne 0 ]; then
    echo "CRITICAL: Health endpoint unreachable"
    exit 2
fi

STATUS=$(echo "$RESPONSE" | jq -r '.status')
POLL_AGE=$(echo "$RESPONSE" | jq -r '.last_poll')

if [ "$STATUS" == "unknown" ]; then
    echo "CRITICAL: Status unknown"
    exit 2
fi

echo "OK: Status=$STATUS"
exit 0
```

---

## Quick Reference

| Check | Command |
|-------|---------|
| Health Status | `curl -s localhost:8080/api/v1/state \| jq .status` |
| Queue Depth | `curl -s localhost:8080/api/v1/state \| jq .queue_depth` |
| Memory Usage | `curl -s localhost:8080/api/v1/state \| jq .memory_mb` |
| Last Poll | `curl -s localhost:8080/api/v1/state \| jq .last_poll` |
