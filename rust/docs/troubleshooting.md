# Symphony Troubleshooting Guide

## Exit Codes

| Exit Code | Name | Description | Resolution |
|-----------|------|-------------|------------|
| 0 | Success | Normal shutdown, no errors | N/A |
| 1 | General Error | Unspecified error occurred | Check logs for details |
| 2 | Configuration Error | Invalid config or missing environment | Verify configuration and environment variables |
| 10 | Linear API Error | Failed to authenticate or connect to Linear API | Check `LINEAR_API_KEY` validity |
| 11 | Network Error | Network connectivity failure | Verify network access, DNS, firewall |
| 12 | Resource Exhaustion | Out of memory or file descriptors | Increase resource limits |
| 13 | State Corruption | Persistent state is corrupted | Clear state directory, restart fresh |
| 14 | Permission Denied | Insufficient permissions for required operations | Check file/directory permissions |

### Exit Code Categories

```
0-2   : General/Configuration
10-14 : Runtime Errors
```

---

## Startup Failure Decision Tree

```
Symphony fails to start
         │
         ▼
    Exit code?
         │
    ┌────┴────┬─────────┬──────────┐
    ▼         ▼         ▼          ▼
   Code 2   Code 10   Code 12    Code 14
    │         │         │          │
    ▼         ▼         ▼          ▼
 Check:    Check:    Check:     Check:
 ┌─────┐  ┌────────┐ ┌──────┐  ┌────────┐
 │ENV  │  │API Key │ │Memory│  │Perms   │
 │vars │  │valid?  │ │limits│  │on dirs │
 └──┬──┘  └────┬───┘ └──┬───┘  └────┬───┘
    │          │        │           │
    ▼          ▼        ▼           ▼
 Set         Regen    Increase   chown/chmod
 missing    key in    memory     data dir
 vars       Linear    limit      to user
            settings
```

### Step-by-Step Diagnosis

1. **Check Exit Code**
   ```bash
   systemctl status symphony
   # Or check logs
   journalctl -u symphony -n 50
   ```

2. **Verify Environment**
   ```bash
   # Check required variables
   grep -E "LINEAR_API_KEY|RUST_LOG" /etc/symphony/config.env
   
   # Test API key
   curl -H "Authorization: Bearer $LINEAR_API_KEY" \
        https://api.linear.app/graphql \
        -d '{"query": "{ viewer { id } }"}'
   ```

3. **Check Resources**
   ```bash
   # Memory
   free -h
   
   # File descriptors
   ulimit -n
   
   # Disk space
   df -h /var/lib/symphony
   ```

4. **Check Permissions**
   ```bash
   ls -la /var/lib/symphony
   # Should be owned by symphony user
   ```

---

## Common Runtime Issues

### Issue: Linear API Errors

**Symptoms:**
- Exit code 10
- Log: `Linear API error: Unauthorized`
- Status: `unknown`

**Causes:**
- Invalid or expired API key
- API key lacks required scopes
- Linear service outage

**Fixes:**
```bash
# 1. Verify API key
curl -H "Authorization: Bearer $LINEAR_API_KEY" \
     https://api.linear.app/graphql \
     -d '{"query": "{ viewer { id } }"}'

# 2. Regenerate key in Linear settings
# Settings → API → Personal Access Tokens → Regenerate

# 3. Update config
sudo systemctl edit symphony
# Add: Environment=LINEAR_API_KEY=lin_api_newkey

# 4. Restart
sudo systemctl restart symphony
```

---

### Issue: High Memory Usage

**Symptoms:**
- Exit code 12
- OOM killer triggered
- Performance degradation

**Causes:**
- Too many concurrent agents
- Memory leak in agent
- Large git repository clones

**Fixes:**
```bash
# 1. Reduce max agents
sudo systemctl edit symphony
# Add: Environment=SYMPHONY_MAX_AGENTS=5

# 2. Increase memory limit
sudo systemctl edit symphony
# Add: MemoryMax=4G

# 3. Restart
sudo systemctl restart symphony

# 4. Monitor memory
watch -n 5 'curl -s localhost:8080/api/v1/state | jq .memory_mb'
```

---

### Issue: Poll Stall

**Symptoms:**
- `last_poll` > 5 minutes ago
- Queue not growing
- Status stuck in `polling`

**Causes:**
- Network connectivity issue
- Linear API rate limiting
- Blocked thread

**Fixes:**
```bash
# 1. Check network
ping api.linear.app

# 2. Check for rate limits in logs
journalctl -u symphony | grep -i "rate limit"

# 3. Restart to unstick
sudo systemctl restart symphony
```

---

### Issue: Retry Backlog Growing

**Symptoms:**
- `retry_backlog` continuously increasing
- Issues not being processed
- WARN logs for retries

**Causes:**
- Transient network issues
- Linear API instability
- Invalid issue data

**Fixes:**
```bash
# 1. Check logs for specific errors
journalctl -u symphony | grep "Retry attempt"

# 2. Test Linear connectivity
curl -I https://api.linear.app/graphql

# 3. If backlog persists, reset state
sudo systemctl stop symphony
rm -rf /var/lib/symphony/retry_queue.json
sudo systemctl start symphony
```

---

### Issue: Permission Denied

**Symptoms:**
- Exit code 14
- Log: `Permission denied`
- Cannot write to state directory

**Causes:**
- Wrong ownership on data directory
- Running as wrong user
- SELinux/AppArmor blocking

**Fixes:**
```bash
# 1. Fix ownership
sudo chown -R symphony:symphony /var/lib/symphony

# 2. Fix permissions
sudo chmod 750 /var/lib/symphony

# 3. Check SELinux (if applicable)
sudo ausearch -m avc -ts recent
# Create policy if needed

# 4. Restart
sudo systemctl restart symphony
```

---

## Recovery Procedures

### Procedure: Refresh

**Use when:** Transient errors, stale state, minor issues

**Steps:**
```bash
# 1. Check current status
curl -s localhost:8080/api/v1/state | jq .

# 2. Send SIGHUP for graceful refresh (if supported)
sudo systemctl reload symphony

# 3. If reload not available, restart
sudo systemctl restart symphony

# 4. Verify recovery
sleep 10
curl -s localhost:8080/api/v1/state | jq .status
```

**Expected Result:** Status returns to `idle` or `busy`

---

### Procedure: Restart

**Use when:** Unresponsive, memory issues, configuration changes

**Steps:**
```bash
# 1. Stop gracefully
sudo systemctl stop symphony

# 2. Wait for full shutdown
sleep 5

# 3. Verify stopped
sudo systemctl status symphony

# 4. Start fresh
sudo systemctl start symphony

# 5. Monitor startup
journalctl -u symphony -f

# 6. Verify health
sleep 10
curl -s localhost:8080/api/v1/state | jq .
```

**Expected Result:** Clean startup, status becomes `idle`

---

### Procedure: Reset

**Use when:** State corruption, persistent errors, exit code 13

**Steps:**
```bash
# 1. Stop service
sudo systemctl stop symphony

# 2. Backup existing state (optional)
sudo cp -r /var/lib/symphony /var/lib/symphony.backup.$(date +%s)

# 3. Clear state directory
sudo rm -rf /var/lib/symphony/*

# 4. Verify clean
ls -la /var/lib/symphony

# 5. Start service
sudo systemctl start symphony

# 6. Monitor for issues
journalctl -u symphony -f
```

**Expected Result:** Fresh start, all state rebuilt from Linear API

**Warning:** Reset will lose any local state not persisted to Linear

---

## Diagnostic Commands

### Quick Health Check

```bash
#!/bin/bash
echo "=== Symphony Diagnostics ==="

# Service status
echo -e "\n[Service Status]"
systemctl is-active symphony

# Health endpoint
echo -e "\n[Health State]"
curl -s localhost:8080/api/v1/state | jq .

# Recent errors
echo -e "\n[Recent Errors]"
journalctl -u symphony -p err -n 10 --no-pager

# Resource usage
echo -e "\n[Resource Usage]"
ps aux | grep symphony | grep -v grep | awk '{print "CPU: "$3"% | MEM: "$4"% | RSS: "$6" KB"}'

# Network connectivity
echo -e "\n[Network Test]"
curl -sf -o /dev/null -w "Linear API: %{http_code}\n" https://api.linear.app/graphql
```

### Log Analysis

```bash
# Error count by hour
journalctl -u symphony --since today | grep -c ERROR

# Most common errors
journalctl -u symphony --since "1 week ago" | grep ERROR | sort | uniq -c | sort -rn | head

# Agent activity
journalctl -u symphony --since today | grep -c "Agent started"
journalctl -u symphony --since today | grep -c "Issue completed"
```

---

## Getting Help

### Information to Collect

```bash
# System info
uname -a
cat /etc/os-release

# Service logs
journalctl -u symphony --since "1 hour ago" > symphony-logs.txt

# State snapshot
curl -s localhost:8080/api/v1/state > symphony-state.json

# Configuration (sanitize keys first!)
systemctl cat symphony > symphony-service.txt
cat /etc/symphony/config.env | sed 's/API_KEY=.*/API_KEY=***REDACTED***/' > symphony-config.txt

# Resource limits
ulimit -a > symphony-limits.txt
```

### Support Channels

- GitHub Issues: Report bugs and request features
- Documentation: Check deployment and monitoring guides
- Logs: Always include relevant log excerpts
