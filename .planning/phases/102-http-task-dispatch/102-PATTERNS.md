# Phase 102: Lift the task lifecycle onto the high-level `Server` / HTTP path - Pattern Map

**Mapped:** 2026-06-21
**Files analyzed:** 6 (2 NEW source, 3 MODIFIED source, 1 NEW test, 1 NEW example, +Cargo.toml)
**Analogs found:** 6 / 6 (every file has an in-tree analog; this is a move-and-share refactor, not greenfield)

> **Phase nature:** Pure intra-crate refactor. The research (`102-RESEARCH.md`) already verified
> every analog at exact line numbers. Recommended sharing strategy is **Option A** (a free-standing
> `src/server/task_dispatch.rs` unit called by BOTH dispatchers). The failure mode is *re-implementing*
> (drift), not missing logic — Phase 101 already wrote every piece correctly on `ServerCore`.

---

## File Classification

| New/Modified File | Role | Data Flow | Closest Analog | Match Quality |
|-------------------|------|-----------|----------------|---------------|
| NEW `src/server/task_dispatch.rs` | service (shared dispatch unit) | request-response / CRUD | `src/server/core.rs` task methods (`resolve_task_owner`, `build_task_created_response`, `handle_tasks_result`, the `tasks/*` arms) + `src/server/builder.rs:881` capability rule | exact (lift-and-share) |
| MODIFIED `src/server/mod.rs` (`Server` + `ServerBuilder`) | controller (HTTP dispatcher) + builder | request-response | `src/server/core.rs` (`ServerCore` fields + `impl ProtocolHandler` dispatch) | role-match (sibling dispatcher) |
| MODIFIED `src/server/builder.rs` (`apply_tasks_capability_rule` → free fn) | builder | transform (caps mutation) | itself (builder.rs:881-904, in-place lift to free fn) | exact |
| NEW `tests/tool_as_task_lifecycle_http.rs` | test (integration) | request-response (HTTP loopback) | `tests/workflow_prompt_e2e_test.rs:54-97` (HTTP harness) + `tests/tool_as_task_lifecycle.rs` (invariants) | exact (two analogs compose) |
| NEW `examples/s46_http_tool_as_task.rs` | example | request-response (HTTP loopback) | `examples/s45_tool_as_task_lifecycle.rs` (lifecycle) + `tests/workflow_prompt_e2e_test.rs:54-97` (HTTP) | exact (compose) |
| MODIFIED `Cargo.toml` (`[[example]]` block) | config | — | `Cargo.toml:541-544` (`s45` block) | exact |

---

## Pattern Assignments

### `src/server/task_dispatch.rs` (NEW — shared dispatch unit, Option A)

This is the core deliverable. It hosts (a) the shared **free** capability rule both builders call,
and (b) the shared **task-lifecycle** logic both dispatchers call. Gate the whole module
`#[cfg(not(target_arch = "wasm32"))]` (every analog item is non-wasm-gated).

**Module registration analog** (`src/server/mod.rs:77-82`) — register the new module next to `task_store`:
```rust
/// SDK-level task store trait and in-memory implementation.
#[cfg(not(target_arch = "wasm32"))]
pub mod task_store;
/// Task routing trait for MCP Tasks integration.
#[cfg(not(target_arch = "wasm32"))]
pub mod tasks;
// ADD (mirror this shape):
// /// Shared task-lifecycle dispatch unit used by both Server and ServerCore.
// #[cfg(not(target_arch = "wasm32"))]
// pub(crate) mod task_dispatch;
```

#### Part A — the shared capability rule (HTASK-01)

**Analog (verbatim source to lift):** `src/server/builder.rs:881-904` — currently a `ServerCoreBuilder`
method that reads `self.task_store`/`self.task_router`/`self.tool_infos`/`self.capabilities`. Per the
research (Pitfall 2), the two builders hold `tool_infos` at different lifecycle points, so this MUST
become a **free function** over explicit params:

```rust
// SOURCE — src/server/builder.rs:881-904 (current ServerCoreBuilder method, lift to free fn):
fn apply_tasks_capability_rule(&mut self) -> Result<()> {
    use crate::types::tools::TaskSupport;

    let has_backend = self.task_store.is_some() || self.task_router.is_some();
    let has_required_task_tool = self.tool_infos.values().any(|info| {
        info.execution
            .as_ref()
            .and_then(|e| e.task_support)
            .is_some_and(|ts| matches!(ts, TaskSupport::Required))
    });

    if has_required_task_tool && !has_backend {
        return Err(Error::validation(
            "a tool declares TaskSupport::Required but no TaskStore or TaskRouter \
             is configured to back the tasks/* endpoints",
        ));
    }

    if self.capabilities.tasks.is_none() && has_backend {
        self.capabilities.tasks = Some(Self::default_tasks_capability());
    }

    Ok(())
}
```

**Target free-fn shape** (signature confirmed by research §Pattern 1; keep error string byte-identical):
```rust
pub(crate) fn apply_tasks_capability_rule(
    capabilities: &mut ServerCapabilities,
    tool_infos: &HashMap<String, ToolInfo>,
    has_backend: bool, // task_store.is_some() || task_router.is_some()
) -> Result<()> { /* body lifted verbatim, reading the params instead of self */ }
```

**Default capability shape (FROZEN — do not re-derive):** `src/server/builder.rs:848-859`
`default_tasks_capability()`. This is the exact `ServerTasksCapability` the client `assert_capability`
expects. Either move it alongside the free fn or keep it in builder.rs and call it.

#### Part B — owner resolution (shared seam helper)

**Analog (lift):** `src/server/core.rs:972-990` — note research finding #3: this is an inherent
`ServerCore` method, NOT a pre-existing free fn. It reads `self.task_router` then `self.task_store`:
```rust
// SOURCE — src/server/core.rs:972-990:
fn resolve_task_owner(&self, auth_context: Option<&AuthContext>) -> Option<String> {
    if let Some(ref router) = self.task_router {
        return Some(match auth_context {
            Some(ctx) => router.resolve_owner(Some(&ctx.subject), ctx.client_id.as_deref(), None),
            None => router.resolve_owner(None, None, None),
        });
    }
    if self.task_store.is_some() {
        return Some(match auth_context {
            Some(ctx) => ctx.client_id.clone().unwrap_or_else(|| ctx.subject.clone()),
            None => "local".to_string(),
        });
    }
    None
}
```
The shared unit must own this (it depends only on `&task_store`, `&task_router`, `&AuthContext`). The
research recommends a `TaskDispatch<'a>` borrow-struct holding `&Option<Arc<dyn TaskStore>>` +
`&Option<Arc<dyn TaskRouter>>` exposing `resolve_owner`.

#### Part C — create-path response (HTASK-02, the hard part)

**Analog (lift verbatim, preserve the D-STORE-MINTS-ID invariant):** `src/server/core.rs:1006-1064`
`build_task_created_response`. The three-way-id invariant tail (the HTASK-03 assert):
```rust
// SOURCE — src/server/core.rs:1053-1063 (tail — preserve byte-for-byte semantics):
let create_result = crate::types::tasks::CreateTaskResult::new(final_task);
let mut envelope = serde_json::to_value(create_result).unwrap_or_default();
if let Some(obj) = envelope.as_object_mut() {
    obj.insert(
        "_meta".to_string(),
        serde_json::json!({ RELATED_TASK_META_KEY: { "taskId": store_id } }),
    );
}
Self::success_response(id, envelope)
// Invariant: task.taskId == _meta.relatedTask.taskId == store_id (store-minted).
```

**Terminal-result extraction (lift):** `src/server/core.rs:457-468` `extract_terminal_result` — pulls the
nested `result` or top-level `content` array into a `CallToolResult`.

**Create-path GATE (the trigger logic to share):** `src/server/core.rs:589-640` inside
`handle_call_tool`. The gate is `req.task.is_some() && task_store.is_some() && tool_task_support ∈
{Required, Optional} && value has taskId + status`. Research Pattern 2 recommends a
`maybe_build_task_created(id, tool_name, value, task_support, auth) -> Option<JSONRPCResponse>` helper:
`Some` = task creation, `None` = "fall through to a normal `CallToolResult`". The CALLER must pass
`req.task.is_some()` (today `Server::handle_call_tool` ignores `req.task` entirely — see divergence table).

#### Part D — `tasks/result` precedence (lift verbatim)

**Analog:** `src/server/core.rs:1076-1126` `handle_tasks_result`. FROZEN error-code precedence
(store-first → router → `-32002` pending → `-32601` no-backend). The round-trip asserts `-32002`:
```rust
// SOURCE — src/server/core.rs:1115-1125 (the specified pending vs no-backend distinction):
if self.task_store.is_some() {
    Self::error_response(id, -32002, "task result not available: task not completed".to_string())
} else {
    Self::error_response(id, -32601, "tasks/result not supported".to_string())
}
```

#### Part E — `tasks/get | list | cancel` routing (lift verbatim, the easy part)

**Analog:** `src/server/core.rs:1310-1421`. Each arm is a pure function of `(task_store, task_router,
resolve_owner, params, auth)` — store-first, `TaskRouter` fall-through, `-32601 "Tasks not enabled"`
when neither. Lift into the shared unit (e.g. `route_tasks_endpoint(id, ClientRequest::Tasks*, auth)`).
`TasksGet` template:
```rust
// SOURCE — src/server/core.rs:1312-1344 (TasksGet arm — representative of all three):
ClientRequest::TasksGet(params) => {
    if let Some(ref store) = self.task_store {
        let owner_id = self.resolve_task_owner(auth_context.as_ref())
            .unwrap_or_else(|| "local".to_string());
        match store.get(&params.task_id, &owner_id).await {
            Ok(task) => {
                let result = crate::types::tasks::GetTaskResult::new(task);
                Self::success_response(id, serde_json::to_value(result).unwrap())
            },
            Err(e) => Self::error_response(id, -32603, e.to_string()),
        }
    } else if let Some(ref task_router) = self.task_router {
        /* router fall-through */
    } else {
        Self::error_response(id, -32601, "Tasks not enabled".to_string())
    }
}
```
> **Complexity note (research §Environment Availability):** the combined `tasks/*` routing block is large.
> If a single `route_tasks_endpoint` exceeds PMAT cog 25, split per-endpoint (one fn per `Tasks*` variant).

---

### `src/server/core.rs` (MODIFIED — refactor `ServerCore` to call the shared unit)

**Role:** controller (the existing, working dispatcher). **Action:** delegate its task methods to the new
`task_dispatch` unit so the Phase 101 tests (`tests/tool_as_task_lifecycle.rs`) prove no regression.

- `ServerCore` fields to read (`src/server/core.rs:256-262`): `task_router: Option<Arc<dyn TaskRouter>>`,
  `task_store: Option<Arc<dyn TaskStore>>` — both `#[cfg(not(target_arch = "wasm32"))]`. These become the
  inputs the shared unit borrows.
- `ToolCallOutcome::TaskCreated` enum (`src/server/core.rs:299-314`) and its dispatch branch
  (`src/server/core.rs:1219-1235`) stay in `core.rs`; only the *body* of `build_task_created_response`
  moves to the shared unit (or `core.rs` calls the shared fn).

---

### `src/server/mod.rs` — `Server` + `ServerBuilder` (MODIFIED, HTASK-01 + HTASK-02)

**Role:** controller (HTTP-facing dispatcher) + builder. **Analog:** the `ServerCore` field layout and
`impl ProtocolHandler` dispatch in `core.rs`.

**1. Add task fields to `Server` struct** (`src/server/mod.rs:315-355`). Mirror the `ServerCore` field
declarations EXACTLY (`src/server/core.rs:256-262`), same `#[cfg(not(target_arch = "wasm32"))]` gating:
```rust
// ADD to Server (mirror core.rs:256-262):
#[cfg(not(target_arch = "wasm32"))]
task_router: Option<Arc<dyn TaskRouter>>,
#[cfg(not(target_arch = "wasm32"))]
task_store: Option<Arc<dyn crate::server::task_store::TaskStore>>,
```

**2. Add fields + setters to `ServerBuilder` struct** (`src/server/mod.rs:1878-1914`). Mirror the
`ServerCoreBuilder` setters verbatim (`src/server/builder.rs:757-767` `with_task_store` for the legacy
`TaskRouter`; `:829-838` `task_store` for the standard path):
```rust
// SOURCE — src/server/builder.rs:829-838 (task_store setter — copy onto ServerBuilder):
#[cfg(not(target_arch = "wasm32"))]
pub fn task_store(mut self, store: Arc<dyn crate::server::task_store::TaskStore>) -> Self {
    // Capability advertisement is centralized in build(); registering a store
    // records the backend, never clobbering an explicit capability.
    self.task_store = Some(store);
    self
}
```
> Copy the rustdoc (incl. the `# Examples` doctest at builder.rs:798-828) so HTASK-04's doctest coverage
> is satisfied for the new `ServerBuilder` methods. Adjust the doctest path to `ServerBuilder`.

**3. Wire the capability rule into `ServerBuilder::build`** (`src/server/mod.rs:3529-3646`). `tool_infos`
is built locally at `src/server/mod.rs:3568-3582`. Call the shared free fn AFTER that and BEFORE moving
fields into `Server` (research Pitfall 2):
```rust
// In ServerBuilder::build, right after the tool_infos cache (mod.rs:3582):
#[cfg(not(target_arch = "wasm32"))]
let has_backend = self.task_store.is_some() || self.task_router.is_some();
#[cfg(not(target_arch = "wasm32"))]
crate::server::task_dispatch::apply_tasks_capability_rule(
    &mut self.capabilities, &tool_infos, has_backend)?;
// then thread task_store/task_router into the returned Server { .. } literal (mod.rs:3612-3645).
```
> `ServerCoreBuilder::build` (`src/server/builder.rs:1066-1067`) already calls the rule first thing — keep
> that call site, just route it through the new free fn.

**4. Replace the `tasks/*` hard-reject** (`src/server/mod.rs:1166-1172`). This is the HTASK-02 target:
```rust
// SOURCE — src/server/mod.rs:1166-1172 (CURRENT — to be replaced):
ClientRequest::TasksGet(_)
| ClientRequest::TasksResult(_)
| ClientRequest::TasksList(_)
| ClientRequest::TasksCancel(_) => Err(crate::Error::protocol(
    crate::ErrorCode::METHOD_NOT_FOUND,
    "Tasks not supported: no task router configured",
)),
// AFTER: one delegation per Tasks* variant to the shared route_tasks_endpoint.
```
> Note: `process_client_request` (mod.rs:1126) returns `Result<serde_json::Value>` and wraps via
> `create_response` (mod.rs:1177). The shared unit returns a full `JSONRPCResponse` (`success_response`/
> `error_response`). The planner must reconcile this: either have the shared unit expose a
> `Result<Value>`-returning variant for the `Server` path, or call the shared `JSONRPCResponse` builder
> from a sibling of `handle_client_request`. Flag for the planning spike (research Open Question 1).

**5. Wire the create-path into `Server::handle_call_tool`** (`src/server/mod.rs:1207-1355`). Today it
returns a bare `Value` (mod.rs:1354) and IGNORES `req.task`. Per research Pattern 2 / Pitfall 1, after the
tool produces its `Value` (`src/server/mod.rs:1330`, before `CallToolResult` wrapping at :1346-1352),
branch through `maybe_build_task_created` keyed on `req.task.is_some()` + the tool's `TaskSupport`
(`self.tool_infos.get(&req.name).execution.task_support`) + `taskId`+`status` in the value. **Keep
`Server`'s own auth-revalidation (mod.rs:1226-1236) and widget-enrichment (mod.rs:1350-1352) paths
intact** — see the divergence table below.

---

### `src/server/builder.rs` (MODIFIED — `apply_tasks_capability_rule` → delegate to free fn)

**Action:** Replace the body of the `ServerCoreBuilder` method (`src/server/builder.rs:881-904`) with a
delegation to the new free fn in `task_dispatch.rs`, preserving identical behavior. `default_tasks_capability`
(`:848-859`) either moves with it or is called by the free fn — keep the exact `ServerTasksCapability` shape.

---

### `tests/tool_as_task_lifecycle_http.rs` (NEW — HTASK-03 live HTTP round-trip)

**Role:** integration test. **Two composed analogs:**

**Harness analog** — `tests/workflow_prompt_e2e_test.rs:6-95` (HTTP loopback setup):
```rust
// SOURCE — tests/workflow_prompt_e2e_test.rs:6-92 (cfg gate + StreamableHttpServer + client):
#![cfg(all(feature = "streamable-http", not(target_arch = "wasm32")))]

use pmcp::server::streamable_http_server::StreamableHttpServer;
use pmcp::shared::streamable_http::{StreamableHttpTransport, StreamableHttpTransportConfig};
use pmcp::{Client, ClientCapabilities, Result, Server};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};
use url::Url;

// ... build server via Server::builder() ...
let server = Arc::new(Mutex::new(server));
let addr: SocketAddr = "127.0.0.1:18765".parse().unwrap();   // see Pitfall 5: prefer :0
let http_server = StreamableHttpServer::new(addr, server);
tokio::spawn(async move { let _ = http_server.start().await; });
sleep(Duration::from_millis(500)).await;

let config = StreamableHttpTransportConfig {
    url: Url::parse(&format!("http://{}", addr)).map_err(|e| pmcp::Error::Internal(e.to_string()))?,
    extra_headers: vec![], auth_provider: None, session_id: None,
    enable_json_response: true, on_resumption_token: None, http_middleware_chain: None,
};
let transport = StreamableHttpTransport::new(config);
let mut client = Client::new(transport);
let init_result = client.initialize(ClientCapabilities::default()).await?;
```
> **Pitfall 5 (research):** the harness hardcodes `127.0.0.1:18765` + a 500ms sleep. CONTEXT/PRD say `:0`.
> Prefer binding `:0` and reading back the assigned port; otherwise keep a distinct high port and rely on
> CI's `--test-threads=1`. Flag the choice; do not silently inherit `18765`.

**Invariants/assertions analog** — `tests/tool_as_task_lifecycle.rs:128-266` (mirror these over HTTP):
- task tool builder shape (`completing_task_tool`, lines 135-156): returns `taskId`+`status`+nested
  `result.content`; `.with_execution(ToolExecution::new().with_task_support(TaskSupport::Required))`.
- init asserts `init.capabilities.tasks.is_some()` (line 219-222) — the `tasks`-advertised invariant.
- `call_tool_with_task` → assert wire id `!= "tool-fabricated"` (store-minted) (lines 225-236).
- poll `client.tasks_get(&id)` until `status.is_terminal()`, assert polled id == client id (lines 240-255).
- `client.tasks_result(&id)` → assert `!result.content.is_empty()` (lines 258-265).
- pending case (`pending_task_tool`, lines 161-179) → assert `-32002` from `tasks/result` before completion.
- `_meta.relatedTask.taskId == task.taskId` three-way assert (lines 268-279, against the raw envelope).

> Build the server with the high-level path: `Server::builder().name(..).version(..).tool("summarize",
> task_tool).task_store(store).build()?` (this is the NEW capability the phase adds; mirrors the
> `ServerCoreBuilder` call at `tests/tool_as_task_lifecycle.rs:188-199` but on `Server`).

---

### `examples/s46_http_tool_as_task.rs` (NEW — HTASK-04 worked example)

**Role:** example. **Two composed analogs:** `examples/s45_tool_as_task_lifecycle.rs` (the lifecycle +
task-tool body + the 4 numbered round-trip steps, lines 125-228) and the `StreamableHttpServer` harness
from `workflow_prompt_e2e_test.rs`. Replace s45's in-process `DuplexTransport` (s45:50-123) with the real
HTTP transport. Header conventions to mirror (`examples/s45_tool_as_task_lifecycle.rs:1-47`):
```rust
//! # Server example: HTTP tool-as-task lifecycle (Phase 102)
//! ... run line:
//! Run with: `cargo run --example s46_http_tool_as_task --features full`
#![cfg(not(target_arch = "wasm32"))]
```
The task-tool body to reuse verbatim (`examples/s45_tool_as_task_lifecycle.rs:132-153`): a `TypedTool`
returning `{ taskId, status:"completed", ttl, createdAt, lastUpdatedAt, result:{content:[...]} }` with
`.with_execution(ToolExecution::new().with_task_support(TaskSupport::Required))`. The 4 lifecycle steps
(initialize/assert tasks → `call_tool_with_task` → poll `tasks_get` → `tasks_result`) lift from s45:169-225.
Emphasize in the doc comment: NO `ServerCore::handle_request` shim — only `Server` + `StreamableHttpServer`.

---

### `Cargo.toml` (`[[example]]` block)

**Analog (verbatim, bump `s45`→`s46`):** `Cargo.toml:541-544`:
```toml
[[example]]
name = "s45_tool_as_task_lifecycle"
path = "examples/s45_tool_as_task_lifecycle.rs"
required-features = ["full"]
```
Add the `s46_http_tool_as_task` block immediately after, same `required-features = ["full"]`.

---

## Shared Patterns

### Capability advertisement (HTASK-01, cross-cutting both builders)
**Source:** `src/server/builder.rs:881-904` (`apply_tasks_capability_rule`) + `:848-859`
(`default_tasks_capability`). **Apply to:** `ServerCoreBuilder::build` (already) AND `ServerBuilder::build`
(new) — via the SAME free fn (research Pitfall 2). The client checks `tasks` capability CLIENT-side
(`src/client/mod.rs:557,576 assert_capability("tasks", ...)`), so a store-backed `Server` MUST advertise
`tasks` or the HTTP round-trip fails before any request (Pitfall 3).

### Owner scoping (security — preserve, never relax)
**Source:** `src/server/core.rs:972-990` (`resolve_task_owner`). **Apply to:** every `tasks/*` arm and the
create-path in the shared unit. Owner derives from `AuthContext` (OAuth subject / client id / "local"),
NEVER from client input (IDOR mitigation, research §Security V4).

### Store-mints-id invariant (D-STORE-MINTS-ID)
**Source:** `src/server/core.rs:1031-1063`. **Apply to:** the create-path response. The wire
`task.taskId` AND `_meta.relatedTask.taskId` are ALWAYS the store-minted id, never the tool-fabricated one
(`tests/tool_as_task_lifecycle.rs:233-236` asserts `!= "tool-fabricated"`).

### Error-code precedence (FROZEN)
**Source:** `src/server/core.rs:1115-1125` (`-32002` pending vs `-32601` no-backend) and the `tasks/*` arms
(`-32601 "Tasks not enabled"`, `-32603` on store error). **Apply to:** the shared `tasks/*` routing — do
not change any code or message; the round-trip asserts `-32002`.

### WASM gating (HTASK-04 — preserve exactly)
**Source:** EVERY task item is `#[cfg(not(target_arch = "wasm32"))]` (core.rs:253-262, the `TaskCreated`
variant core.rs:308, all `tasks/*` arms core.rs:1311+, module decls mod.rs:78-82). **Apply to:** the new
`task_dispatch` module body, the new `Server`/`ServerBuilder` fields+setters, and the new test/example
`#![cfg(...)]` headers (Pitfall 4). Add a wasm `cargo check` to verification.

### Auth path divergence (preserve per-dispatcher — DO NOT unify)
**Source — the divergence table (research §Runtime State Inventory):** the two `handle_call_tool` bodies
ALREADY differ. The create-path seam bolts onto each WITHOUT merging their auth/render paths:

| Concern | `ServerCore::handle_call_tool` (core.rs:471) | `Server::handle_call_tool` (mod.rs:1207) |
|---------|----------------------------------------------|------------------------------------------|
| Returns | `ToolCallOutcome` (Result \| TaskCreated) | bare `Value` (serialized `CallToolResult`) |
| Auth | expects pre-validated `auth_context` | RE-validates via `auth_provider.validate_request` (mod.rs:1226-1236) |
| Result text | `summarize_structured_output` / pretty JSON (core.rs:642-650) | `result.to_string()` (mod.rs:1347) |
| Widget enrich | `with_widget_enrichment` (core.rs:646) | `with_widget_enrichment` (mod.rs:1351) — same |
| Task detection | YES (core.rs:596-640) | **NONE** — `req.task` ignored — ADD here |
| `ToolRejected` → `CallToolResult::rejected` | core.rs:580-584 | mod.rs:1339-1343 — same |

**Regression guard (Pitfall 1):** a non-task `tools/call` over HTTP must still return a plain
`CallToolResult` (no `CreateTaskResult` leakage); a task tool called WITHOUT a `task` field must fall
through (core.rs:633-639). Add a `server_call_tool_non_task` regression test.

---

## No Analog Found

None. Every file has at least one strong in-tree analog (this is the defining property of a move-and-share
refactor). The only genuinely-new code is the *wiring/parameterization* of the shared seam, whose target
shape is specified in research §Pattern 1/2/3.

---

## Metadata

**Analog search scope:** `src/server/{core,mod,builder,task_store}.rs`, `src/client/mod.rs`,
`tests/{workflow_prompt_e2e_test,tool_as_task_lifecycle}.rs`, `examples/s45_tool_as_task_lifecycle.rs`,
`Cargo.toml`.
**Files scanned:** 9 (all cited at verified line numbers; cross-checked against `102-RESEARCH.md` which
independently verified the same lines).
**Pattern extraction date:** 2026-06-21
