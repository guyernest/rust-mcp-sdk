# MCP Tasks: Long-Running Operations

Some operations take too long to fit inside a single request-response cycle. A satellite imagery analysis tool might need 45 seconds. A compliance report generator might need two minutes. An ETL pipeline might need ten. When the work outlasts the transport timeout, you need a different pattern.

MCP Tasks solve this with a two-phase model: the server accepts the request immediately (Phase 1), then the client polls for the result (Phase 2). This chapter teaches you how to build servers that support both synchronous and task-based execution, letting the client decide which path to take.

## Learning Objectives

By the end of this chapter, you will be able to:

- Explain why polling is preferred over SSE for serverless deployments
- Configure a `TaskStore` on your server builder
- Declare `TaskSupport::Optional` on tools using `.with_execution()`
- Write a dual-path tool handler that branches on `extra.is_task_request()`
- Build a `get_task_result` fallback tool for clients that do not support tasks natively
- Describe how capability negotiation works at the per-request level

## Why Tasks Matter for Enterprise MCP

Enterprise MCP deployments face a fundamental tension: many valuable operations are inherently slow, but the MCP protocol's default behavior assumes synchronous request-response.

```
+---------------------------------------------------------------------+
|                 The Timeout Problem                                   |
+---------------------------------------------------------------------+
|                                                                       |
|  Operation                        Typical Duration    Sync Feasible?  |
|  ================================ =================== ==============  |
|  Database query                   50ms - 2s           Yes             |
|  API call with retry              1s - 10s            Maybe           |
|  Report generation (PDF)          10s - 60s           Unlikely        |
|  Satellite imagery analysis       30s - 120s          No              |
|  ETL pipeline stage               1min - 30min        No              |
|  ML model training checkpoint     5min - hours        No              |
|                                                                       |
|  Lambda timeout: 15 minutes max                                       |
|  Streamable HTTP timeout: varies by proxy/CDN                         |
|  Client patience: varies by UX expectations                           |
|                                                                       |
+---------------------------------------------------------------------+
```

Without tasks, a slow tool call either times out (bad UX, wasted compute) or forces the server to use WebSockets with server-sent events (incompatible with serverless). Tasks give you a third option: accept immediately, work in the background, let the client poll.

## The Polling Model vs SSE Notifications

MCP defines two mechanisms for tracking long-running work:

| Mechanism | Transport | Serverless Compatible | Client Complexity |
|-----------|-----------|----------------------|-------------------|
| **Polling** (`tasks/get`) | Any (HTTP, stdio, WS) | Yes | Low -- just retry on interval |
| **SSE notifications** | Streamable HTTP only | No -- requires persistent connection | Higher -- must hold open connection |

Polling wins for serverless because each poll is a stateless HTTP request. The server does not need to hold a connection open. It does not need to remember which client is listening. It reads task state from the store, returns it, and shuts down.

```
Polling model (serverless-friendly):

Client                    Lambda                    DynamoDB
  │                         │                          │
  │── tools/call ──────────>│                          │
  │                         │── create task ──────────>│
  │<── CreateTaskResult ────│                          │
  │                         │  (Lambda exits)          │
  │                         │                          │
  │── tasks/get ───────────>│  (new Lambda)            │
  │                         │── read task ────────────>│
  │<── {status: working} ───│                          │
  │                         │  (Lambda exits)          │
  │                         │                          │
  │── tasks/get ───────────>│  (new Lambda)            │
  │                         │── read task ────────────>│
  │<── {status: completed} ─│                          │
  │                         │                          │
  │── tasks/result ────────>│  (new Lambda)            │
  │                         │── read result ──────────>│
  │<── CallToolResult ──────│                          │
```

Each arrow to Lambda is a fresh invocation. The `TaskStore` (backed by DynamoDB, Redis, or another external store) provides the continuity that the stateless compute layer cannot.

## TaskStore: The External State Bridge

The `TaskStore` trait is the abstraction that makes tasks work across stateless invocations. It manages the lifecycle of task records -- creation, status transitions, pagination, expiration, and cleanup.

```rust
use pmcp::server::task_store::{InMemoryTaskStore, TaskStore, StoreConfig};
use std::sync::Arc;

// For development and testing: in-memory store
let store = Arc::new(InMemoryTaskStore::new());

// For production: use pmcp-tasks crate with DynamoDB or Redis
// let backend = DynamoDbBackend::new(client, table_name);
// let store = Arc::new(GenericTaskStore::new(backend));
```

The SDK provides `InMemoryTaskStore` for development. For production, the `pmcp-tasks` crate provides DynamoDB and Redis backends that survive Lambda cold starts and container restarts.

| Backend | Crate | Use Case |
|---------|-------|----------|
| `InMemoryTaskStore` | `pmcp` (core) | Development, testing, single-process servers |
| `GenericTaskStore<DynamoDbBackend>` | `pmcp-tasks` | AWS Lambda, serverless, multi-instance |
| `GenericTaskStore<RedisBackend>` | `pmcp-tasks` | Low-latency, container-based deployments |

**Key point:** The `TaskStore` is not just storage -- it enforces the MCP state machine. Transitions like `completed -> working` are rejected at the store level, not in your handler code. This means you cannot accidentally corrupt task state regardless of how your handler logic is structured.

## Chapter Contents

This chapter has two hands-on sections and an exercise set:

1. **[Task Lifecycle and Polling](./ch21-01-lifecycle.md)** -- Set up a TaskStore, declare task support on tools, write a dual-path handler, and implement the `get_task_result` fallback tool pattern

2. **[Capability Negotiation](./ch21-02-capability-negotiation.md)** -- Understand how per-request capability signals work, the three client profiles, and why no session state is needed for task negotiation

3. **[Chapter 21 Exercises](./ch21-exercises.md)** -- Practice adding task support to existing tools, building dual-path handlers, and designing task support strategies

## Knowledge Check

Before continuing, make sure you can answer:

- What problem does the two-phase task model solve that synchronous tool calls cannot?
- Why is polling better than SSE notifications for serverless deployments?
- What role does the `TaskStore` play in a stateless server architecture?

---

*Continue to [Task Lifecycle and Polling](./ch21-01-lifecycle.md) ->*
