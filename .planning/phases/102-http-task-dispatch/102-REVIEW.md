---
phase: 102-http-task-dispatch
reviewed: 2026-06-22T12:00:00Z
depth: standard
files_reviewed: 8
files_reviewed_list:
  - src/server/task_dispatch.rs
  - src/server/task_dispatch_tests.rs
  - src/server/mod.rs
  - src/server/core.rs
  - src/server/builder.rs
  - tests/tool_as_task_lifecycle_http.rs
  - examples/s46_http_tool_as_task.rs
  - Cargo.toml
findings:
  critical: 2
  warning: 4
  info: 3
  total: 9
status: issues_found
---

# Phase 102: Code Review Report

**Reviewed:** 2026-06-22
**Depth:** standard
**Files Reviewed:** 8
**Status:** issues_found

## Summary

Phase 102 extracts the `tasks/*` lifecycle into a shared `src/server/task_dispatch.rs`
unit and wires it into the high-level `Server`/`ServerBuilder` so HTTP-hosted servers
can serve task-based tools without a `ServerCore` shim. The architecture is sound:
the single-source invariant is maintained, the capability rule is correctly shared, and
the create-path gate's self-enforcing structure is well-designed and well-tested.

The cross-owner IDOR tests are present at both the in-crate and live HTTP layers, and
the auth flow is correctly threaded from `StreamableHttpServer` through
`handle_client_request`. The primary security risk identified is a moderate info-leakage
issue in error-code selection for cross-owner access denials. Two critical findings
concern a panic risk from bare `.unwrap()` calls in result-type serialization on the
hot response path, and an owner-resolution precedence ambiguity that could produce
unexpected task isolation behavior. Four warnings cover secondary quality issues.

## Critical Issues

### CR-01: Bare `.unwrap()` on `serde_json::to_value` in three hot response paths

**File:** `src/server/task_dispatch.rs:409`, `src/server/task_dispatch.rs:446`,
`src/server/task_dispatch.rs:480`

**Issue:** `route_tasks_get`, `route_tasks_list`, and `route_tasks_cancel` each call
`serde_json::to_value(result).unwrap()` on the success path. `serde_json::to_value`
can return `Err` when the type contains non-string map keys or other non-serializable
constructs. While the SDK's own `GetTaskResult`, `ListTasksResult`, and
`CancelTaskResult` types are almost certainly serde-safe today, a bare `.unwrap()` in
a `pub(crate) async fn` on the JSON-RPC response-assembly path converts a serialization
failure into a process panic rather than a `-32603 Internal Error`. The same pattern
is used safely as `.unwrap_or_default()` in every other serialization in this file;
these three are inconsistent exceptions.

Contrast the safe pattern used for the envelope serialization at line 277 (which uses
`.unwrap_or_default()`) and for the router fallbacks (which also use
`.unwrap_or_default()`).

```rust
// CR-01 affected locations — replace .unwrap() with .unwrap_or_default():

// route_tasks_get:409
success_response(id, serde_json::to_value(result).unwrap_or_default())

// route_tasks_list:446
success_response(id, serde_json::to_value(result).unwrap_or_default())

// route_tasks_cancel:480
success_response(id, serde_json::to_value(result).unwrap_or_default())
```

If the intent is to be strictly correct rather than silently return empty JSON, convert
to an `error_response` on serialization failure:

```rust
// Alternatively, for correctness:
match serde_json::to_value(result) {
    Ok(v) => success_response(id, v),
    Err(e) => error_response(id, -32603, format!("serialization error: {e}")),
}
```

---

### CR-02: `client_id` takes priority over `subject` in `resolve_owner` — diverges from documentation and creates cross-tool owner inconsistency

**File:** `src/server/task_dispatch.rs:179`

**Issue:** On the standard `TaskStore`-only path (no `TaskRouter`), `resolve_owner`
derives the owner as:

```rust
ctx.client_id.clone().unwrap_or_else(|| ctx.subject.clone())
```

This means when `client_id` is present, it is used as the task owner — but `client_id`
is a parameter that can differ per connection session even for the same authenticated
user. The function's own doc comment states the priority chain is
"OAuth subject, then client ID, then session ID" (subject first), and `T-102-01` is
cited as ensuring owner derives from auth/router and never from client params. However,
`client_id` in `AuthContext` is populated from the MCP protocol's client info (it is
client-supplied, not server-derived from an OAuth token), making it a weaker
isolation boundary than `subject`.

This creates a concrete isolation gap: two different MCP client sessions for the same
authenticated user (same `subject`) but different `client_id` values will be treated
as different task owners, meaning user A cannot see their own tasks across reconnections
unless `client_id` is stable. Conversely, if `client_id` is spoofable (which it is in
the no-auth-provider path that the tests exercise), it widens the scope of who can
access tasks.

The `TaskRouter` path (line 171) passes `client_id` to `router.resolve_owner()` as the
second priority argument, after `subject` — consistent with the documented "subject
first" priority. The `TaskStore` path inverts this, giving `client_id` priority over
`subject`, which is both inconsistent with `TaskRouter` behavior and inconsistent with
the documented priority chain.

**Fix:**

```rust
// src/server/task_dispatch.rs:179 — subject takes priority, client_id is tiebreak
Some(ctx) => ctx.subject.clone(),
// If client-session-scoped isolation is intentional, document that explicitly
// and update the priority-chain doc; if not, use subject as the stable owner.
```

If `client_id`-first is intentional for some use case, the doc comment and T-102-01
threat note must be corrected to match — currently they claim subject-first priority
which the code does not implement on the `TaskStore` path.

---

## Warnings

### WR-01: Cross-owner access returns `-32603 Internal Error` instead of a not-found error — error-code leakage

**File:** `src/server/task_dispatch.rs:411`, `task_dispatch.rs:480`, `task_dispatch.rs:448`

**Issue:** When owner B tries to access owner A's task via `tasks/get`, `tasks/cancel`,
or `tasks/list`, the `TaskStore` returns `TaskStoreError::NotFound` (per the store's
documented non-disclosure behavior: "If the task belongs to a different owner, the
store returns `NotFound`"). However, `route_tasks_get`, `route_tasks_list`, and
`route_tasks_cancel` map ALL store errors — including `NotFound` — to `-32603`:

```rust
Err(e) => error_response(id, -32603, e.to_string()),
```

The JSON-RPC error body then contains the store's `"task not found: <task_id>"`
message. This is not an IDOR (the task content is not leaked), but the distinction
matters: `-32603` conventionally means "Internal Error" (a server fault), not "not
found". The more appropriate code is `-32601` (Method Not Found / resource not found),
or a domain-specific code. The tests currently only assert `ResponsePayload::Error(_)`
without checking the code, so this mis-classification would not be caught by the
current test suite.

Contrast `handle_tasks_result` (line 365) which correctly distinguishes
`TaskStoreError::NotFound` by letting it fall through rather than returning an error
directly — `route_tasks_get` and the others could do the same for `NotFound`.

**Fix:**

```rust
// In route_tasks_get, route_tasks_cancel, route_tasks_list — distinguish NotFound:
match store.get(&params.task_id, &owner_id).await {
    Ok(task) => {
        let result = crate::types::tasks::GetTaskResult::new(task);
        success_response(id, serde_json::to_value(result).unwrap_or_default())
    },
    Err(crate::server::task_store::TaskStoreError::NotFound { .. }) => {
        error_response(id, -32601, "task not found".to_string())
    },
    Err(e) => error_response(id, -32603, e.to_string()),
}
```

---

### WR-02: `create_response` in `Server` always uses `-32603` for any error from the create path

**File:** `src/server/mod.rs:1228-1247`, `mod.rs:1460-1466`

**Issue:** The create-path bolt-on in `handle_call_tool` (lines 1455-1467) decomposes
the `JSONRPCResponse` from `maybe_build_task_created` back into `Result<Value>`:

```rust
crate::types::jsonrpc::ResponsePayload::Error(err) => {
    Err(crate::Error::Protocol {
        code: crate::error::ErrorCode(err.code),
        message: err.message,
        data: err.data,
    })
},
```

This `Err` then flows into `create_response` at line 1228, which unconditionally maps
any `Err` to code `-32603`:

```rust
Err(e) => JSONRPCResponse {
    payload: crate::types::jsonrpc::ResponsePayload::Error(
        crate::types::jsonrpc::JSONRPCError {
            code: -32603,   // ← always -32603, ignores e's code
            message: e.to_string(),
            data: None,
        },
    ),
},
```

The comment at line 1458 notes "the caller's `create_response` re-wraps `Err` as
`-32603`, so the code is preserved" — but this is incorrect: the `Protocol` error's
`code` field is NOT preserved through `create_response`. The `to_string()` includes
the message but not the numeric code. For the current create path this only affects
store errors (which are `-32603` anyway), but any future path that emits a different
code (e.g., `-32002`) through `maybe_build_task_created` will have it silently
overwritten to `-32603`. The comment is misleading documentation.

**Fix:** Either document this clamping behavior explicitly as intentional, or route
the error through a code-preserving path:

```rust
// In create_response, preserve the error code from Protocol errors:
Err(crate::Error::Protocol { code, message, data }) => JSONRPCResponse {
    payload: crate::types::jsonrpc::ResponsePayload::Error(
        crate::types::jsonrpc::JSONRPCError {
            code: code.0,
            message,
            data,
        }
    ),
    ..
},
Err(e) => JSONRPCResponse { /* -32603 fallback */ },
```

Or update the comment at line 1458 to accurately state "the code is always remapped to
-32603 by create_response regardless of the gate's error code".

---

### WR-03: Stdio transport path passes `auth_context: None` unconditionally — tasks owned by "local" regardless of who called

**File:** `src/server/mod.rs:1049`

**Issue:** The `handle_request_message` function used by the stdio transport loop
always calls `handle_request(id, request, None)`. When a `task_store`-backed server
is run over stdio (via `Server::run_stdio()`), all tasks are owned by `"local"` (the
`resolve_owner` fallback for `auth_context: None`). This means any caller can access
any task. While stdio servers are typically single-user and this is arguably acceptable,
it creates an undocumented behavioral divergence: the same `Server` struct has strict
owner-scoped isolation over HTTP (where `StreamableHttpServer` resolves `AuthContext`)
and completely open access over stdio.

This is not new to Phase 102 — it predates the phase — but Phase 102 is the first time
the tasks path is reachable over `Server`, so the risk surface is newly active. A
future developer adding `task_store` to a stdio-transported `Server` (perhaps to use
the SDK's task polling infrastructure) would have no isolation between callers.

**Fix:** The fix is documentation: `ServerBuilder::task_store` should warn that
owner-scoped isolation requires an HTTP transport with an `AuthContext`; over stdio,
all tasks share the `"local"` owner namespace. Alternatively, the stdio path could
derive a stable owner from the session identifier if one is available.

---

### WR-04: `resolve_owner` with `task_router` always returns `Some(String)` — caller `unwrap_or_else` is dead code and may mask silent "local" fallback in future changes

**File:** `src/server/task_dispatch.rs:166-182`

**Issue:** When `task_router` is present, `resolve_owner` returns
`Some(router.resolve_owner(...))` — it can never return `None` in this branch.
When `task_store` is present but no router, it returns `Some(...)`. `None` is only
returned when BOTH are absent. The call sites therefore apply `.unwrap_or_else(||
"local".to_string())` on the result, but this fallback only fires in the no-backend
case — which is a configuration state the caller already verified cannot happen (the
dispatch methods only reach `resolve_owner` after confirming a backend is present in
the `if let Some(store)` / `else if let Some(router)` branches).

The dead `unwrap_or_else` is not a current bug, but it masks the contract: if a future
refactor changes `resolve_owner` to return `None` in some new branch, the silent
"local" fallback at the call site would pass all existing tests while silently merging
all tasks into a single owner namespace. The function signature should instead
communicate that "no backend" is impossible at these call sites.

**Fix:** Use `expect()` with a descriptive message at the call sites that are inside
`if let Some(store)` branches — impossible-to-reach panics are better than silent
misclassification:

```rust
// Inside route_tasks_get, after `if let Some(store) = self.task_store`:
let owner_id = self
    .resolve_owner(auth_context)
    .expect("resolve_owner returns Some when task_store is present");
```

Or refactor `resolve_owner` to return `String` (not `Option<String>`) and move the
`"local"` fallback inside.

---

## Info

### IN-01: `with_task_store` naming is confusing and undiscoverable — it accepts a `TaskRouter`, not a `TaskStore`

**File:** `src/server/mod.rs:3748`

**Issue:** `ServerBuilder::with_task_store` accepts an `Arc<dyn TaskRouter>` despite
its name containing "task_store". The correct `TaskStore` setter is `task_store`. The
method has a long doc comment warning about this naming mismatch, but the name itself
will confuse API consumers in autocomplete and at a glance. The SUMMARY notes this is
"naming debt documented, not renamed — additive-only API" — accepted, but worth
flagging for eventual cleanup in a future breaking-change window.

**Fix:** Defer to a future minor semver break: rename to `with_task_router` and
deprecate `with_task_store` with a `#[deprecated]` attribute pointing to the new name.
No action required in this phase.

---

### IN-02: `task_shaped_value` in `gate_tests` includes a `result` field — tests the synchronous-completion path only, missing a pending-only shaped value fixture

**File:** `src/server/task_dispatch.rs:543-549`

**Issue:** The `gate_tests` module's `task_shaped_value()` fixture always includes a
`"result"` field, meaning every `Some`-case gate test exercises the synchronous
completion branch inside `build_task_created_response`. The pending case (a task-shaped
value with `status: "working"` and no `result`) is never tested by `gate_tests` — only
by the separate `pending_tasks_result_preserves_minus_32002` test in
`task_dispatch_tests.rs`. The truth-table is therefore incomplete for the pending-store
branch of `build_task_created_response`.

**Fix:** Add a `pending_task_shaped_value()` fixture to `gate_tests` and a separate
truth-table row:

```rust
fn pending_task_shaped_value() -> Value {
    serde_json::json!({
        "taskId": "tool-fabricated",
        "status": "working",
    })
}

// Additional gate_tests row:
async fn gate_accepts_required_task_shaped_pending() { ... }
```

---

### IN-03: `proptest_task_branch_gate` creates a new `tokio::runtime::Runtime` per proptest iteration — 64 runtimes spawned per test run

**File:** `src/server/task_dispatch_tests.rs:629`

**Issue:** Inside the `proptest!` macro, each test iteration executes:

```rust
let rt = tokio::runtime::Runtime::new().unwrap();
```

With 64 cases, this spawns 64 separate multi-threaded Tokio runtimes. Each runtime
creates thread pools. The total overhead is acceptable for 64 iterations, but the
pattern is an anti-pattern that will scale poorly if `with_cases` is increased. The
runtime is also not explicitly shut down; it drops at the end of each closure, which
does trigger shutdown, but `block_on` inside proptest closures is a known footgun when
the tested async code itself spawns tasks.

**Fix:** Construct the runtime once outside the `proptest!` block and reuse it. The
current proptest version (in this codebase) supports this pattern:

```rust
#[test]
fn gate_never_misfires() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let server = grid_server();
    proptest!(|(support_ix in 0u8..4, with_task in any::<bool>(), shaped in any::<bool>())| {
        let name = tool_name(support_ix, shaped);
        let resp = rt.block_on(server.handle_request(...));
        // ...
    });
}
```

Note: `server` must be `Send + Sync` for this to work; since `Server` is already
`Arc`-able and used across threads in the HTTP tests, this should be fine.

---

_Reviewed: 2026-06-22_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
