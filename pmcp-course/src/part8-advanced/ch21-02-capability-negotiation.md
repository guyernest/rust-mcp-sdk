# Capability Negotiation

Task support in MCP is negotiated per-request, not per-session. This design choice has deep implications for how you architect servers, especially stateless ones. In this section, you will learn the negotiation model, understand the three client profiles, and see why no server-side session state is needed.

## Learning Objectives

By the end of this section, you will be able to:

- Explain why MCP uses per-request capability signals instead of per-session negotiation
- Describe how `req.task` serves as the capability signal
- List the three client profiles and the code path each takes
- Compare task negotiation with HTTP content negotiation
- Identify when Lambda timeout limitations make tasks mandatory vs optional

## The Principle: Capability Inspection, Not Client Sniffing

Servers never need to know which client is calling them. They do not check User-Agent headers. They do not maintain a session table mapping client IDs to feature sets. Instead, they inspect the request itself.

The signal is the `task` field in the `tools/call` request parameters:

```json
// Task-aware client request:
{
  "method": "tools/call",
  "params": {
    "name": "analyze_imagery",
    "arguments": { "image_uri": "s3://bucket/image.tif" },
    "task": { "ttl": 300000 }
  }
}

// Non-task-aware client request:
{
  "method": "tools/call",
  "params": {
    "name": "analyze_imagery",
    "arguments": { "image_uri": "s3://bucket/image.tif" }
  }
}
```

The only difference is the presence or absence of `"task"`. Your handler sees this through `extra.is_task_request()`:

```rust
async fn handle(args: AnalyzeArgs, extra: RequestHandlerExtra) -> Result<Value> {
    if extra.is_task_request() {
        // Client understands tasks -- return CreateTaskResult
        create_task_and_return(&args, &extra).await
    } else {
        // Client does not understand tasks -- return sync result
        run_and_return_inline(&args).await
    }
}
```

This is the entire negotiation. No handshake. No feature flags stored in session state. The request carries its own capability declaration.

## How the Signal Flows

Here is the full path from client request to handler branch:

```
Client                         Server Core                    Handler
  |                               |                              |
  |-- tools/call { task: {} } --> |                              |
  |                               |-- parse CallToolRequest -->  |
  |                               |   req.task = Some({})        |
  |                               |                              |
  |                               |-- build RequestHandlerExtra  |
  |                               |   .with_task_request(        |
  |                               |       req.task)              |
  |                               |                              |
  |                               |-- call handler(args, extra)->|
  |                               |                              |
  |                               |         extra.is_task_request()
  |                               |         == true              |
  |                               |                              |
  |<-- CreateTaskResult ----------|<-- json!({task: ...}) -------|
```

The server core deserializes `CallToolRequest`, extracts `req.task`, and passes it into `RequestHandlerExtra` via `.with_task_request()`. Your handler never touches raw JSON -- it calls `extra.is_task_request()` and gets a boolean.

## Why No Session State Is Needed

In many protocol designs, capabilities are negotiated once during session initialization and stored server-side. MCP takes a different approach for tasks:

```
Session-based negotiation (NOT how MCP Tasks work):

  Client                    Server
    |-- initialize -------->|
    |   { supportsTasks }   |-- store in session map --|
    |<-- capabilities ------|                          |
    |                       |                          |
    |-- tools/call -------->|-- lookup session ------->|
    |                       |   "does this client      |
    |                       |    support tasks?"       |
    |                       |<-- yes ------------------|
    |<-- CreateTaskResult --|

Per-request negotiation (how MCP Tasks actually work):

  Client                    Server
    |-- tools/call -------->|
    |   { task: {} }        |-- req.task.is_some()? -->|
    |                       |   yes                    |
    |<-- CreateTaskResult --|
```

Per-request negotiation is better for stateless servers because:

1. **No session store required.** Lambda functions do not share memory between invocations. Storing "client X supports tasks" requires an external database. Per-request signals avoid this entirely.

2. **Mixed-mode clients are supported.** A client might want tasks for `analyze_imagery` (slow) but not for `list_models` (fast). Per-request signals let the client choose on every call.

3. **Proxy transparency.** If a proxy or gateway sits between the client and server, it can add or remove the `task` field without the server needing to know about the proxy.

## The Three Client Profiles

Your server will encounter three types of clients. The dual-path handler with `get_task_result` fallback handles all three without any client-specific code.

### Profile 1: Task-Native Client

Understands the MCP Tasks specification. Sends `task` in the request. Polls `tasks/get` and retrieves results with `tasks/result`.

```
Client                    Server                    TaskStore
  |                         |                          |
  |-- tools/call            |                          |
  |   { task: {ttl: 300k} } |                          |
  |                         |-- store.create() ------->|
  |<-- CreateTaskResult ----|                          |
  |                         |                          |
  |-- tasks/get {taskId} -->|-- store.get() ---------->|
  |<-- {status: working} ---|                          |
  |                         |                          |
  |-- tasks/get {taskId} -->|-- store.get() ---------->|
  |<-- {status: completed} -|                          |
  |                         |                          |
  |-- tasks/result -------->|-- (return stored result) |
  |<-- CallToolResult ------|                          |
```

**What the handler does:** Takes the task path (`extra.is_task_request() == true`), creates a task, spawns background work, returns `CreateTaskResult`.

### Profile 2: Tool-Polling Client

Does not understand MCP Tasks, but is smart enough to call `get_task_result` when told to. This covers LLM-powered clients that can follow instructions in tool descriptions.

```
Client                    Server
  |                         |
  |-- tools/call            |
  |   (no task field)       |
  |                         |-- sync path: run inline
  |<-- CallToolResult ------|  (or timeout with task hint)
  |                         |
  |-- tools/call            |
  |   get_task_result       |
  |   {task_id: "t-abc"}    |
  |                         |
  |<-- "still working,      |
  |     try again in 5s" ---|
  |                         |
  |-- tools/call            |
  |   get_task_result       |
  |   {task_id: "t-abc"}    |
  |                         |
  |<-- result --------------|
```

**What the handler does:** Takes the sync path (`extra.is_task_request() == false`). If the operation completes within the timeout, returns inline. If it cannot, the `get_task_result` tool provides a manual polling path.

### Profile 3: Sync-Only Client

No task support. No ability to poll. Expects a synchronous result. This is the simplest client and the fallback for everything.

```
Client                    Server
  |                         |
  |-- tools/call            |
  |   (no task field)       |
  |                         |-- sync path: run inline
  |<-- CallToolResult ------|
```

**What the handler does:** Takes the sync path. If the operation is too slow, the client gets a timeout error. For tools with `TaskSupport::Required`, this client cannot use the tool at all -- and that is the correct behavior, because the operation genuinely cannot complete synchronously.

## Comparison with HTTP Content Negotiation

If this pattern feels familiar, it should. HTTP has used per-request capability signaling for decades via the `Accept` header:

```
HTTP Content Negotiation:
  Accept: application/json        -->  Server returns JSON
  Accept: text/html               -->  Server returns HTML
  Accept: */*                     -->  Server returns default format

MCP Task Negotiation:
  task: { ttl: 300000 }           -->  Server returns CreateTaskResult
  (no task field)                  -->  Server returns CallToolResult
```

Both patterns let the server adapt its response format based on what the client declares it can handle. Neither requires the server to remember what format a particular client prefers across requests.

The analogy extends further:

| HTTP | MCP Tasks |
|------|-----------|
| `Accept: application/json` | `task: { ttl: 300000 }` |
| `Content-Type: application/json` | Response is `CreateTaskResult` |
| `406 Not Acceptable` | Error when `TaskSupport::Required` but no `task` sent |
| `Content-Negotiation: ...` | `execution.taskSupport` in `tools/list` |

## Implications for Serverless

Lambda functions have a hard timeout of 15 minutes. Most practical deployments use 30 seconds to 5 minutes. This creates a clear decision framework:

```
+----------------------------------------------------------------------+
|                Task Support Decision Matrix                           |
+----------------------------------------------------------------------+
|                                                                       |
|  Expected Duration    Lambda Timeout    Recommended TaskSupport        |
|  ==================== ================= ============================  |
|  < 5 seconds          Any               Forbidden (no benefit)        |
|  5-30 seconds         30s               Optional (sync might work)    |
|  30s - 5 minutes      5min              Optional (sync risky)         |
|  > 5 minutes          Any               Required (sync impossible)    |
|  Variable/unknown     Any               Optional (let client decide)  |
|                                                                       |
+----------------------------------------------------------------------+
```

When the operation might exceed the Lambda timeout, `Required` is the honest declaration. The server cannot guarantee synchronous completion, so it should not pretend it can.

For operations with variable duration -- sometimes 2 seconds, sometimes 2 minutes -- `Optional` is the right choice. The handler runs the operation synchronously when possible and falls back to task creation when it detects the operation will be slow (e.g., by checking input size or upstream service latency).

```rust
if extra.is_task_request() {
    // Always use task path if client requests it
    create_task_and_spawn(&args, &extra, &store).await
} else if estimate_duration(&args) > Duration::from_secs(25) {
    // Sync path, but operation is too slow -- return helpful error
    Err(Error::validation(
        "This analysis is estimated to take >25 seconds. \
         Use task mode or call get_task_result after the operation."
    ))
} else {
    // Sync path, operation should complete in time
    run_inline(&args).await
}
```

## Key Takeaways

- Task negotiation is per-request, not per-session -- the `task` field in the request is the only signal
- `extra.is_task_request()` is the single branch point in your handler code
- No session state, no client registry, no feature flag database needed
- Three client profiles (task-native, tool-polling, sync-only) are all served by the same dual-path handler plus `get_task_result`
- For serverless: match `TaskSupport` level to your Lambda timeout and expected operation duration

---

*Continue to [Chapter 21 Exercises](./ch21-exercises.md) ->*
