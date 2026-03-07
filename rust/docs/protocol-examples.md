# Protocol Examples

This document provides concrete examples of the app-server JSON-RPC protocol messages
used by Symphony. These examples are aligned with the `symphony-agent-protocol` crate
implementation.

## Message Envelope

All protocol messages are JSON objects with the following envelope structure:

### Request Envelope

```json
{
  "id": 1,
  "method": "thread.start",
  "params": { ... }
}
```

### Response Envelope

```json
{
  "id": 1,
  "result": { ... },
  "error": null
}
```

### Event Notification

```json
{
  "method": "turn.completed",
  "params": { ... }
}
```

## Lifecycle Sequence

### 1. Initialize

Sent at session startup to establish protocol capabilities.

**Request:**
```json
{
  "id": 1,
  "method": "initialize",
  "params": {
    "protocolVersion": "2",
    "capabilities": {}
  }
}
```

**Response:**
```json
{
  "id": 1,
  "result": {
    "protocolVersion": "2",
    "capabilities": {
      "supportsTurnReuse": true
    }
  }
}
```

### 2. Initialized Notification

Sent after initialize response to signal readiness.

```json
{
  "method": "initialized",
  "params": {}
}
```

### 3. Thread Start

Begins a new agent thread for an issue.

**Request:**
```json
{
  "id": 2,
  "method": "thread.start",
  "params": {
    "prompt": "You are working on issue SYM-123...",
    "cwd": "/tmp/symphony_workspaces/SYM-123",
    "approvalPolicy": "suggest",
    "sandbox": "workspace-write"
  }
}
```

**Response:**
```json
{
  "id": 2,
  "result": {
    "thread": {
      "id": "thread_abc123"
    }
  }
}
```

### 4. Turn Start

Begins a new turn within an existing thread.

**Request:**
```json
{
  "id": 3,
  "method": "turn.start",
  "params": {
    "threadId": "thread_abc123",
    "prompt": "Continue working on the issue..."
  }
}
```

**Response:**
```json
{
  "id": 3,
  "result": {
    "turn": {
      "id": "turn_def456"
    }
  }
}
```

### 5. Turn Progress Events

Emitted during turn execution.

**Agent Message:**
```json
{
  "method": "agent.message",
  "params": {
    "threadId": "thread_abc123",
    "turnId": "turn_def456",
    "content": "I'll start by examining the codebase..."
  }
}
```

**Tool Call:**
```json
{
  "method": "agent.tool_call",
  "params": {
    "threadId": "thread_abc123",
    "turnId": "turn_def456",
    "toolCallId": "call_789",
    "toolName": "read_file",
    "arguments": {"path": "src/main.rs"}
  }
}
```

**Tool Result:**
```json
{
  "method": "agent.tool_result",
  "params": {
    "threadId": "thread_abc123",
    "turnId": "turn_def456",
    "toolCallId": "call_789",
    "result": "fn main() { ... }"
  }
}
```

### 6. Turn Completed

Signals turn completion with usage statistics.

```json
{
  "method": "turn.completed",
  "params": {
    "threadId": "thread_abc123",
    "turnId": "turn_def456",
    "status": "completed",
    "usage": {
      "inputTokens": 1234,
      "outputTokens": 567,
      "totalTokens": 1801
    }
  }
}
```

### 7. Rate Limit Event

Emitted when rate limits are encountered.

```json
{
  "method": "rate_limit.hit",
  "params": {
    "threadId": "thread_abc123",
    "resetAt": 1709600000,
    "limitType": "tokens"
  }
}
```

### 8. Thread End

Signals thread termination.

```json
{
  "method": "thread.end",
  "params": {
    "threadId": "thread_abc123",
    "reason": "completed"
  }
}
```

## Error Handling

### Protocol Error Response

```json
{
  "id": 2,
  "result": null,
  "error": {
    "code": -32602,
    "message": "Invalid params: missing required field 'prompt'"
  }
}
```

### Agent Error Event

```json
{
  "method": "agent.error",
  "params": {
    "threadId": "thread_abc123",
    "error": {
      "type": "timeout",
      "message": "Turn exceeded maximum duration"
    }
  }
}
```

## Stderr Lines

Non-JSON lines on stderr are captured as log output:

```
panic: file missing
```

Parsed as:
```rust
ParsedLine::Stderr(StderrLine { message: "panic: file missing" })
```

## Method Categories

| Category | Methods |
|----------|---------|
| Lifecycle | `initialize`, `initialized` |
| Thread | `thread.start`, `thread.end` |
| Turn | `turn.start`, `turn.completed` |
| Agent | `agent.message`, `agent.tool_call`, `agent.tool_result`, `agent.error` |
| System | `rate_limit.hit` |

## Reference

See `symphony-agent-protocol` crate for:
- `AppServerEvent` - Event parsing and field extraction
- `StreamLineParser` - Incremental line parsing with buffering
- `ProtocolSequenceValidator` - Startup/turn sequence validation
- `build_*_request` functions - Request payload construction
