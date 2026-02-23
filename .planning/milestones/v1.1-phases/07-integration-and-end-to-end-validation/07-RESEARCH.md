# Phase 7: Integration and End-to-End Validation - Research

**Researched:** 2026-02-23
**Domain:** Rust/MCP SDK integration testing, builder API validation, end-to-end example design
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Scenario: multi-step data processing (fetch-data + transform + store), NOT the deploy pipeline from the existing example
- Pause trigger: missing dependency -- a step depends on output from a step that requires client action, showing the data-dependency handoff
- Client continuation demonstrated via direct `ServerCore` calls -- construct JSON requests with `_task_id` in `_meta`, no transport layer
- Print the full message list from the handoff so the reader sees: user intent, assistant plan, tool call/result pairs, and the handoff narrative
- Replace the existing `62_task_workflow_opt_in.rs` with the full lifecycle example
- The existing API is sufficient: `with_task_support(true)` on the workflow + `with_task_store()` + `prompt_workflow()` -- no new builder methods needed
- No convenience methods -- keep the 3-piece API clean and explicit
- Add explicit backward compatibility integration tests: register both task and non-task workflows on the same server, verify non-task workflows are unaffected
- Happy path: create-execute-handoff-continue-complete full lifecycle
- Error cases: step failure with handoff, retry with `_task_id` (overwrite behavior), cancel-with-result for explicit completion
- Tests call handler methods directly (handler-level), not through ServerCore JSON-RPC
- Tests live in `crates/pmcp-tasks/tests/` alongside existing pmcp-tasks tests
- Filename: `62_task_workflow_lifecycle.rs` (replaces `62_task_workflow_opt_in.rs`)
- Synchronous: keep `fn main()` (no `#[tokio::main]`), consistent with current example style
- Heavy inline comments: comment each lifecycle stage ("// Stage 1: Invoke workflow prompt", etc.) -- this is a teaching example
- Module-level `//!` doc explaining the full lifecycle

### Claude's Discretion
- Exact data processing scenario (what tools and data shapes)
- Number of workflow steps (enough to show the lifecycle, not more)
- Integration test file name and organization within `crates/pmcp-tasks/tests/`
- How to handle the sync-vs-async tension if handler calls require async (may need `tokio::runtime::Runtime::new()` block)

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| INTG-01 | ServerCoreBuilder provides API to register task-aware workflow prompts | Already implemented in `prompt_workflow()` method (builder.rs lines 620-693). Phase 7 validates it works end-to-end, no new code needed. |
| INTG-02 | Existing non-task workflows continue to work unchanged (backward compatibility) | Requires explicit integration tests that register BOTH task-aware and non-task workflows on the same server and verify non-task workflows return standard `GetPromptResult` without `_meta`. |
| INTG-03 | Working example (62_tasks_workflow.rs) demonstrates complete task-prompt bridge lifecycle | New file `examples/62_task_workflow_lifecycle.rs` replacing existing `62_task_workflow_opt_in.rs`. Must demonstrate: workflow invocation, handoff inspection, `_task_id` continuation, `tasks/result` polling. |
| INTG-04 | Integration tests validate create-execute-handoff-continue-complete flow through real ServerCore | Tests in `crates/pmcp-tasks/tests/` using `InMemoryTaskStore` + `TaskRouterImpl` + `ServerCoreBuilder` to exercise the full lifecycle. |
</phase_requirements>

## Summary

Phase 7 is a validation and demonstration phase -- it builds NO new production code. Everything needed for the task-prompt bridge was implemented in Phases 4-6. The phase produces: (1) a comprehensive lifecycle example that replaces the existing opt-in example, (2) backward compatibility integration tests, and (3) end-to-end integration tests for the full workflow lifecycle.

The key technical challenge is the sync-vs-async tension in the example. The existing `62_task_workflow_opt_in.rs` uses `fn main()` (synchronous) and only tests builder compilation. The full lifecycle example needs to invoke `handle_request` which is async. The established pattern from `60_tasks_basic.rs` is `#[tokio::main]`, but the CONTEXT.md says "keep `fn main()`". The resolution is to use `tokio::runtime::Runtime::new().unwrap().block_on(async { ... })` inside `fn main()`, which maintains the non-attribute style while supporting async operations.

**Primary recommendation:** Design a 3-step data processing workflow (fetch_data -> transform_data -> store_data) where `transform_data` depends on `fetch_data`'s output via a binding. Make `fetch_data` fail on first invocation (or be unreachable) to trigger an `UnresolvedDependency` pause, demonstrating the data-dependency handoff. Then show client continuation via `_task_id`.

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| pmcp | 1.10.3 | MCP SDK server core | The crate under test |
| pmcp-tasks | 0.1.0 | Task lifecycle + task router | Provides `InMemoryTaskStore`, `TaskRouterImpl` |
| tokio | workspace | Async runtime for handler invocation | Required for `ProtocolHandler::handle_request` |
| serde_json | workspace | JSON construction and inspection | Standard for MCP protocol value manipulation |
| async_trait | workspace | Async trait bounds | Required for `ToolHandler` implementations |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| No new dependencies | - | - | This phase adds zero new dependencies |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `tokio::runtime::Runtime::new().block_on()` | `#[tokio::main]` attribute | CONTEXT.md says keep `fn main()` for consistency with existing style. `block_on()` achieves the same result without the attribute. |

## Architecture Patterns

### Recommended Project Structure
```
examples/
  62_task_workflow_lifecycle.rs  # Replaces 62_task_workflow_opt_in.rs (INTG-03)

crates/pmcp-tasks/tests/
  workflow_integration.rs        # New integration test file (INTG-02, INTG-04)
```

### Pattern 1: Example as Teaching Document
**What:** The lifecycle example serves as documentation. Each lifecycle stage is a clearly commented section with printed output showing the full message list and task state.
**When to use:** For examples that demonstrate complex multi-step flows.
**Example:**
```rust
// Stage 1: Invoke the workflow prompt
// The server creates a task, runs server-executable steps,
// pauses at the data dependency, and returns a handoff.
let prompt_request = Request::Client(Box::new(ClientRequest::GetPrompt(GetPromptParams {
    name: "data_pipeline".to_string(),
    arguments: HashMap::from([("source".to_string(), "api_endpoint".to_string())]),
    _meta: None,
})));
let response = server.handle_request(RequestId::from(1i64), prompt_request, None).await;
```

### Pattern 2: Handler-Level Integration Tests
**What:** Tests instantiate `ServerCoreBuilder` with real `InMemoryTaskStore` + `TaskRouterImpl`, register workflows, and call `handle_request` directly. This tests the full wiring without transport overhead.
**When to use:** For integration tests that need to verify the complete request path.
**Example:**
```rust
fn build_test_server() -> (ServerCore, Arc<InMemoryTaskStore>) {
    let store = Arc::new(
        InMemoryTaskStore::new()
            .with_security(TaskSecurityConfig::default().with_allow_anonymous(true)),
    );
    let router = Arc::new(TaskRouterImpl::new(store.clone()));

    let server = ServerCoreBuilder::new()
        .name("test-workflow-integration")
        .version("1.0.0")
        .tool("fetch_data", FetchDataTool)
        .tool("transform_data", TransformDataTool)
        .tool("store_data", StoreDataTool)
        .with_task_store(router)
        .prompt_workflow(workflow_with_task_support)
        .expect("workflow should register")
        .prompt_workflow(workflow_without_task_support)
        .expect("non-task workflow should register")
        .stateless_mode(true) // Skip initialization check
        .build()
        .expect("server should build");

    (server, store)
}
```

### Pattern 3: Backward Compatibility via Dual Registration
**What:** Register both a task-enabled workflow and a non-task workflow on the SAME server. Invoke both via `handle_request` and verify the non-task workflow returns a standard `GetPromptResult` with `_meta: None`.
**When to use:** For INTG-02 backward compatibility verification.
**Verified in codebase:** The `prompt_workflow()` method already branches on `has_task_support` (builder.rs lines 668-682). The test verifies this branching works at the integration level.

### Pattern 4: Task ID Extraction from CreateWorkflowTask
**What:** The `create_workflow_task` method returns a `CreateTaskResult` serialized as `Value`. The task ID is at path `["task"]["taskId"]`.
**CRITICAL NOTE:** The current `task_prompt_handler.rs` extracts the task ID with `value.get("id")` (line 633), which returns `None` because `CreateTaskResult` wraps the task under `result["task"]["taskId"]`. This means task creation always falls to the graceful degradation path (inner handler without task tracking). This is a pre-existing bug/issue that should be noted but is OUT OF SCOPE for Phase 7 integration tests -- the example uses `ServerCore::handle_request` with `GetPrompt` which goes through `task_prompt_handler`, so it will exhibit the same behavior. The integration tests should work AROUND this by directly setting up the workflow and testing the continuation path independently, OR this could be flagged as something to fix.

**UPDATE after re-examining:** Actually, examining more carefully -- the task_prompt_handler calls `self.task_router.create_workflow_task()` which returns a `Value`. Looking at `TaskRouterImpl::create_workflow_task()` in router.rs (line 318-358), it builds `CreateTaskResult { task: record.task, _meta: None }` and serializes to Value. So the JSON structure is `{"task": {"taskId": "...", ...}}`. The handler extracts with `value.get("id")` which indeed returns None.

This means the `handle()` method in `TaskWorkflowPromptHandler` always falls through to the inner handler path. The full lifecycle test must account for this. The fix would be to change `value.get("id")` to `value.get("task").and_then(|t| t.get("taskId")).and_then(|v| v.as_str()).map(String::from)`. This is a **Phase 7 bug fix** that should be included.

### Anti-Patterns to Avoid
- **Transport-level testing:** Do NOT test through stdio/HTTP transport. Use `handle_request` directly.
- **Mocking the task store:** Use the real `InMemoryTaskStore`, not mocks. This is integration testing.
- **Ignoring the task_id extraction bug:** The example WILL NOT WORK as a full lifecycle demo without fixing the `value.get("id")` bug in `task_prompt_handler.rs`.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| JSON request construction | Manual JSON-RPC framing | `Request::Client(Box::new(ClientRequest::...))` | Existing enum construction from `src/types/protocol.rs` handles all framing |
| Task store setup | Custom in-memory store | `InMemoryTaskStore::new().with_security(TaskSecurityConfig::default().with_allow_anonymous(true))` | Real store with real security config -- this is integration, not unit |
| Response extraction | Manual JSON parsing | Pattern-match on `ResponsePayload::Result(v)` / `ResponsePayload::Error(e)` | Matches the pattern used in `lifecycle_integration.rs` and `60_tasks_basic.rs` |

**Key insight:** The existing codebase has well-established patterns for building test servers, constructing requests, and extracting results (see `crates/pmcp-tasks/tests/lifecycle_integration.rs` and `examples/60_tasks_basic.rs`). Phase 7 should reuse these patterns exactly.

## Common Pitfalls

### Pitfall 1: Task ID Extraction Bug in TaskWorkflowPromptHandler
**What goes wrong:** `TaskWorkflowPromptHandler::handle()` extracts task_id with `value.get("id")` (line 633), but `create_workflow_task` returns `CreateTaskResult` which serializes as `{"task": {"taskId": "..."}}`. The `get("id")` always returns `None`.
**Why it happens:** The original implementation likely assumed a different JSON structure from `create_workflow_task`.
**How to avoid:** Fix the extraction to `value.get("task").and_then(|t| t.get("taskId")).and_then(|v| v.as_str())` before writing the lifecycle example. Without this fix, the workflow always falls through to the graceful degradation path (no task tracking).
**Warning signs:** The example will print `_meta: None` on the `GetPromptResult` even though a task-enabled workflow was invoked.

### Pitfall 2: Sync-Async Tension in Example
**What goes wrong:** `handle_request` is async but CONTEXT.md requires `fn main()` (not `#[tokio::main]`).
**Why it happens:** Consistency requirement with existing example style.
**How to avoid:** Use `tokio::runtime::Runtime::new().unwrap().block_on(async { ... })` inside `fn main()`.
**Warning signs:** Compilation error if async code appears outside an async context.

### Pitfall 3: Stateless Mode Required for Direct handle_request Calls
**What goes wrong:** `handle_request` returns error if server is not initialized AND not in stateless mode.
**Why it happens:** The server checks `is_initialized()` before processing client requests (core.rs line 709).
**How to avoid:** Always call `.stateless_mode(true)` on the builder for examples and tests that call `handle_request` directly without sending an `Initialize` request first.
**Warning signs:** `-32002 "Server not initialized"` error on first request.

### Pitfall 4: Owner ID Resolution in Anonymous Context
**What goes wrong:** When `auth_context` is `None` (no auth), the owner resolves to `"local"` via `resolve_task_owner`. Task operations must use the same owner.
**Why it happens:** The `handle_request` call passes `None` for auth_context in tests/examples.
**How to avoid:** Ensure all task operations (create, get, result, cancel) use `"local"` as the owner, which happens automatically when `allow_anonymous: true` on the store and `auth_context: None` on requests.
**Warning signs:** "not found" errors when polling a task that was just created -- usually an owner mismatch.

### Pitfall 5: Cargo.toml Example Registration
**What goes wrong:** The example file exists but cargo doesn't discover it.
**Why it happens:** The `62_task_workflow_opt_in.rs` example is NOT registered in `Cargo.toml` as an `[[example]]` entry. Cargo auto-discovers files in the `examples/` directory, but only if they match naming conventions and there are no conflicting entries.
**How to avoid:** Verify the example compiles with `cargo check --example 62_task_workflow_lifecycle` after creating/renaming the file. If auto-discovery fails, add an explicit `[[example]]` entry.
**Warning signs:** The existing `62_task_workflow_opt_in.rs` compiles fine via auto-discovery, so renaming it should work.

### Pitfall 6: GetPromptResult _meta Field Extraction
**What goes wrong:** Tests try to access `_meta` on the response but the field serializes differently depending on whether it's `Some(map)` or `None`.
**Why it happens:** `_meta` on `GetPromptResult` is `Option<Map<String, Value>>`. When the task ID extraction bug is fixed, the response should have `_meta: Some(...)`. Without the fix, it's `None`.
**How to avoid:** After fixing Pitfall 1, verify `_meta` is present in the response and contains `task_id`, `task_status`, `steps`, and optionally `pause_reason`.
**Warning signs:** `_meta` absent or empty in the serialized response.

## Code Examples

Verified patterns from the existing codebase:

### Constructing a GetPrompt Request
```rust
// Source: src/types/protocol.rs (ClientRequest::GetPrompt variant)
use pmcp::types::{ClientRequest, Request, RequestId};
use std::collections::HashMap;

let prompt_request = Request::Client(Box::new(ClientRequest::GetPrompt(
    pmcp::types::GetPromptParams {
        name: "data_pipeline".to_string(),
        arguments: HashMap::from([
            ("source".to_string(), "api_endpoint".to_string()),
        ]),
        _meta: None,
    },
)));
let response = server.handle_request(RequestId::from(1i64), prompt_request, None).await;
```

### Constructing a CallTool Request with _task_id in _meta
```rust
// Source: src/types/protocol.rs (CallToolRequest with RequestMeta)
use pmcp::types::{CallToolParams, ClientRequest, Request, RequestId, RequestMeta};

let tool_request = Request::Client(Box::new(ClientRequest::CallTool(CallToolParams {
    name: "fetch_data".to_string(),
    arguments: serde_json::json!({ "source": "api_endpoint" }),
    _meta: Some(RequestMeta {
        progress_token: None,
        _task_id: Some(task_id.to_string()),
    }),
    task: None, // NOT a task-augmented call; just continuation
})));
let response = server.handle_request(RequestId::from(2i64), tool_request, None).await;
```

### Constructing a tasks/cancel with Result (Workflow Completion)
```rust
// Source: crates/pmcp-tasks/tests/lifecycle_integration.rs (pattern for tasks/cancel)
let cancel_request = Request::Client(Box::new(ClientRequest::TasksCancel(serde_json::json!({
    "taskId": task_id,
    "result": { "summary": "all steps completed", "output": final_result }
}))));
let response = server.handle_request(RequestId::from(5i64), cancel_request, None).await;
```

### Building a Data-Dependency Workflow
```rust
// Source: pattern from existing test code + workflow DSL
let workflow = SequentialWorkflow::new("data_pipeline", "Fetch, transform, and store data")
    .argument("source", "Data source identifier", true)
    .step(
        WorkflowStep::new("fetch", ToolHandle::new("fetch_data"))
            .arg("source", DataSource::prompt_arg("source"))
            .bind("raw_data")
    )
    .step(
        WorkflowStep::new("transform", ToolHandle::new("transform_data"))
            .arg("input", DataSource::from_step("raw_data"))  // <-- dependency on fetch output
            .bind("transformed")
    )
    .step(
        WorkflowStep::new("store", ToolHandle::new("store_data"))
            .arg("data", DataSource::from_step("transformed"))
    )
    .with_task_support(true);
```

### Extracting Response from handle_request
```rust
// Source: crates/pmcp-tasks/tests/lifecycle_integration.rs (unwrap_result helper)
use pmcp::types::jsonrpc::ResponsePayload;

fn unwrap_result(response: pmcp::types::JSONRPCResponse) -> serde_json::Value {
    match response.payload {
        ResponsePayload::Result(v) => v,
        ResponsePayload::Error(e) => panic!("Expected success but got error: {}", e.message),
    }
}
```

### Sync main() with Async block_on
```rust
// Pattern: fn main() wrapping async operations
fn main() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        // All async operations here
        let response = server.handle_request(id, request, None).await;
        // ...
    });
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| No task-prompt bridge | TaskWorkflowPromptHandler composes with WorkflowPromptHandler | Phase 4 (v1.1) | Workflow prompts can now create tasks |
| No handoff messages | Structured handoff with _meta + narrative | Phase 6 (v1.1) | Clients know exactly what to do next |
| No continuation path | _task_id in _meta enables fire-and-forget recording | Phase 6 (v1.1) | Tool results reconnect to workflow tasks |

**Deprecated/outdated:**
- `62_task_workflow_opt_in.rs`: Will be replaced by `62_task_workflow_lifecycle.rs`. The opt-in example only proves compilation, not the full lifecycle.

## Open Questions

1. **Task ID extraction bug -- fix scope**
   - What we know: `task_prompt_handler.rs` line 633 extracts `value.get("id")` which returns None for `CreateTaskResult` JSON (`{"task": {"taskId": "..."}}`)
   - What's unclear: Should the fix be a separate commit before the example, or bundled into the first task?
   - Recommendation: Fix it as the first task in the plan. Without it, the entire lifecycle example is non-functional. The fix is a one-line change: `value.get("id")` -> `value.get("task").and_then(|t| t.get("taskId")).and_then(|v| v.as_str())`. This is technically a bug fix from Phase 4, surfaced during Phase 7 integration.

2. **Example scenario: what makes fetch_data fail?**
   - What we know: The pause trigger must be a data dependency (step depends on output from a step requiring client action)
   - What's unclear: How to make the fetch_data step NOT complete server-side so the UnresolvedDependency pause fires
   - Recommendation: Design `fetch_data` to intentionally fail (return `Err(...)`) on server-side execution. This triggers `PauseReason::ToolError` for the fetch step, then `PauseReason::UnresolvedDependency` for the transform step (which depends on fetch's output). The client then calls `fetch_data` with `_task_id` to provide the data, then `transform_data` to continue. Actually, a cleaner approach: make `fetch_data` succeed but have 3 steps where step 1 succeeds, step 2 needs an external credential (param from step 1 + a client-provided value), triggering `UnresolvableParams` or just design it so step 2's input binding depends on step 1 which fails. The simplest: step 1 tool handler returns an error, causing `PauseReason::ToolError { retryable: true }`, then step 2 gets `UnresolvedDependency`. This shows both pause reason types AND the data-dependency handoff.

3. **Integration test handler-level vs ServerCore-level**
   - What we know: CONTEXT.md says "Tests call handler methods directly (handler-level), not through ServerCore JSON-RPC"
   - What's unclear: "Handler-level" could mean calling `PromptHandler::handle()` directly or calling `ServerCore::handle_request()` (which is still handler-level, just dispatched through the server)
   - Recommendation: Interpret "handler-level" as "no transport layer" -- use `ServerCore::handle_request()` because it exercises the full wiring (builder -> handler -> task router -> store). This matches the existing test pattern in `lifecycle_integration.rs`. If CONTEXT.md literally means call `PromptHandler::handle()` directly, that would bypass the builder wiring which is precisely what INTG-01 and INTG-04 need to validate.

## Sources

### Primary (HIGH confidence)
- `src/server/builder.rs` -- Builder API with `prompt_workflow()`, `with_task_store()`, task support branching (lines 620-693)
- `src/server/workflow/task_prompt_handler.rs` -- `TaskWorkflowPromptHandler` implementation including step loop, progress tracking, handoff generation
- `src/server/core.rs` -- `handle_request` dispatch, fire-and-forget continuation recording (lines 764-800)
- `crates/pmcp-tasks/src/router.rs` -- `TaskRouterImpl` with `create_workflow_task`, `handle_workflow_continuation`, cancel-with-result
- `crates/pmcp-tasks/tests/lifecycle_integration.rs` -- Established integration test patterns for task lifecycle
- `examples/62_task_workflow_opt_in.rs` -- Current example to be replaced
- `examples/60_tasks_basic.rs` -- Reference pattern for task example structure

### Secondary (MEDIUM confidence)
- Phase 6 verification report (`06-VERIFICATION.md`) -- Confirms all 13 behavioral truths for handoff and continuation
- Phase 6 plan summaries (`06-01-SUMMARY.md`, `06-02-SUMMARY.md`) -- Documents all wiring decisions and their implementation

### Tertiary (LOW confidence)
- None

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all dependencies are existing workspace members, no new crates
- Architecture: HIGH -- patterns directly extracted from existing codebase (lifecycle_integration.rs, 60_tasks_basic.rs, builder.rs)
- Pitfalls: HIGH -- task_id extraction bug verified by reading source code; sync-async tension documented in CONTEXT.md decisions
- Bug discovery: HIGH -- the `value.get("id")` vs `value["task"]["taskId"]` mismatch is verified by reading `CreateTaskResult` struct (task.rs lines 278-289) and `create_workflow_task` implementation (router.rs lines 318-358) and the extraction line (task_prompt_handler.rs line 633)

**Research date:** 2026-02-23
**Valid until:** 2026-03-23 (stable -- no external dependencies or fast-moving APIs)
