---
phase: 102-http-task-dispatch
reviewed: 2026-06-22T00:00:00Z
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
  critical: 0
  warning: 5
  info: 2
  total: 7
status: issues_found
---

# Phase 102: Code Review Report

**Reviewed:** 2026-06-22T00:00:00Z
**Depth:** standard
**Files Reviewed:** 8
**Status:** issues_found

## Summary

Phase 102 extracts the `tasks/*` lifecycle into a single shared `task_dispatch` unit and wires it into both `Server` (mod.rs) and `ServerCore` (core.rs), making the HTTP path capable of serving tasks for the first time. The architecture is sound and the IDOR isolation gate works correctly. No critical security vulnerabilities were found: cross-owner `tasks/get|list|cancel` correctly rejects unauthorized access (it does return an error response, even if the error code is imprecise), and the gate never leaks task content across owners.

Five warning-class issues were found: three bare `.unwrap()` panic points on infallible-in-practice `serde_json::to_value` calls in the `route_tasks_get|list|cancel` success paths (the prior review flag that `.unwrap_or_default()` is used everywhere else is confirmed correct and these three are the only remaining deviants), one doc-vs-code inconsistency in `resolve_owner` (the doc says priority is OAuth subject first, but the `TaskStore` branch picks `client_id` first — a semantic bug masked because the HTTP proxy always leaves `client_id: None`), and a missing documentation warning about the stdio no-auth owner collapse. Two info items cover a redundant `ServerCore::resolve_task_owner` wrapper and a test helper that rebuilds a full `Server` inside every proptest case.

---

## Warnings

### WR-01: Bare `.unwrap()` on `serde_json::to_value` in `route_tasks_get` success path

**File:** `src/server/task_dispatch.rs:407`
**Issue:** The success arm calls `serde_json::to_value(result).unwrap()`. Every other `to_value` call in this file uses `.unwrap_or_default()` (lines 275, 359, 372, 413, 447, 441, 473, 478). `GetTaskResult` is a plain serializable struct so the panic is unreachable today, but the divergence from the file's own convention is a latent risk if the type gains a non-serializable field (e.g., a `Box<dyn Trait>` for extensibility).
**Fix:**
```rust
// line 407 — replace:
success_response(id, serde_json::to_value(result).unwrap())
// with:
success_response(id, serde_json::to_value(result).unwrap_or_default())
```

### WR-02: Bare `.unwrap()` on `serde_json::to_value` in `route_tasks_list` success path

**File:** `src/server/task_dispatch.rs:441`
**Issue:** Same pattern as WR-01. `ListTasksResult` is likewise a plain serializable struct but `.unwrap()` diverges from every surrounding call site.
**Fix:**
```rust
// line 441 — replace:
success_response(id, serde_json::to_value(result).unwrap())
// with:
success_response(id, serde_json::to_value(result).unwrap_or_default())
```

### WR-03: Bare `.unwrap()` on `serde_json::to_value` in `route_tasks_cancel` success path

**File:** `src/server/task_dispatch.rs:472`
**Issue:** Same pattern as WR-01 and WR-02. `CancelTaskResult` is a plain serializable struct. The `.unwrap()` is inconsistent with all surrounding call sites in the file.
**Fix:**
```rust
// line 472 — replace:
success_response(id, serde_json::to_value(result).unwrap())
// with:
success_response(id, serde_json::to_value(result).unwrap_or_default())
```

### WR-04: `resolve_owner` doc comment states `subject`-first priority but the `TaskStore` branch is `client_id`-first

**File:** `src/server/task_dispatch.rs:162-179`
**Issue:** The doc comment at line 162 reads "priority chain: OAuth subject, then client ID, then session ID, then 'local'" — this describes the `TaskRouter` branch at line 171 where `subject` is the first positional argument to `router.resolve_owner(Some(&ctx.subject), ctx.client_id.as_deref(), None)`.

The `TaskStore` branch at line 179 implements the **opposite** priority:
```rust
Some(ctx) => ctx.client_id.clone().unwrap_or_else(|| ctx.subject.clone()),
```
Here `client_id` is tried first and falls back to `subject`. When `client_id` is `Some`, the owner becomes the OAuth application's client identity (e.g., `"my-spa-app"`), not the authenticated user's `sub`. Every task from every user of that application would be owned by a single identity, collapsing per-user isolation to per-application isolation.

The HTTP proxy (`extract_auth_from_proxy_headers`, streamable_http_server.rs line 880) always sets `client_id: None` — so the cross-owner isolation tests pass because the proxy never populates `client_id`. A server using a JWT `AuthProvider` that extracts `client_id` from the token's `azp`/`client_id` claim would silently lose per-user task isolation.

**Fix:** Align the `TaskStore` branch to subject-first as documented, and update the doc comment to be explicit about both branches:
```rust
// line 159-184 — corrected doc + implementation for the TaskStore branch:

/// Resolve the owner ID from the authentication context.
///
/// Returns `None` if no backend is configured. With a `TaskRouter`,
/// delegates to [`TaskRouter::resolve_owner`] (priority: OAuth subject >
/// client ID > session ID > "local"). With only a `TaskStore`, uses
/// `subject` as the primary owner identity (OIDC `sub` claim), falling
/// back to `"local"` when no auth context is present. Owner is ALWAYS
/// derived from auth/router, NEVER from client params (IDOR mitigation,
/// T-102-01).
pub(crate) fn resolve_owner(&self, auth_context: Option<&AuthContext>) -> Option<String> {
    if let Some(router) = self.task_router {
        return Some(match auth_context {
            Some(ctx) => {
                router.resolve_owner(Some(&ctx.subject), ctx.client_id.as_deref(), None)
            },
            None => router.resolve_owner(None, None, None),
        });
    }
    if self.task_store.is_some() {
        return Some(match auth_context {
            Some(ctx) => ctx.subject.clone(),   // subject is the user identity (OIDC sub)
            None => "local".to_string(),
        });
    }
    None
}
```

### WR-05: Stdio transport always passes `auth_context: None` — all tasks land in `"local"` owner bucket with no per-user isolation, and this is undocumented

**File:** `src/server/mod.rs:1049`
**Issue:** `handle_request_message` (the path invoked by `run_stdio` and `run`) always passes `None` as the auth context:
```rust
let response = server.handle_request(id, request, None).await;
```
With `auth_context: None`, `resolve_owner` returns `Some("local")` for the `TaskStore` branch. ALL tasks created over stdio from any source are therefore owned by `"local"` and visible to any other `"local"` request. There is no isolation.

For the standard single-user CLI use case (one process, one user) this is correct. The danger is undocumented: a developer who adds a `TaskStore` to a `Server::run_stdio()` server (perfectly valid API) will get no warning that all tasks share a single owner. If the server is later wrapped in a multiplexer or exposed over a non-HTTP transport that reuses the same `Server`, task isolation is silently absent.

**Fix:** Add a security callout to `run_stdio()` (and optionally `run()`) documentation:
```rust
/// # Security: task isolation
///
/// The stdio transport processes all messages with `auth_context = None`.
/// If a [`TaskStore`] is configured, all tasks are assigned to the synthetic
/// owner `"local"` — there is no per-user task isolation on this transport.
/// For multi-tenant or per-user task isolation use [`StreamableHttpServer`]
/// with an authentication provider, which derives owner identity from the
/// incoming request's auth context.
```

---

## Info

### IN-01: `ServerCore::resolve_task_owner` is a one-line wrapper over `self.task_dispatch().resolve_owner` with no added logic

**File:** `src/server/core.rs:968-972`
**Issue:** The method body is entirely:
```rust
self.task_dispatch().resolve_owner(auth_context)
```
It is called only at lines 1079 and 1134 and predates the shared dispatch unit. Now that the dispatch unit owns the resolution logic, this private wrapper adds a layer of indirection without contributing any logic, contrary to the "single source of truth" design goal.
**Fix:** Inline the two call sites (`self.task_dispatch().resolve_owner(auth_context.as_ref())`) and delete `resolve_task_owner`. Not urgent but reduces surface area.

### IN-02: `proptest_task_branch_gate::grid_server()` is called inside each of 64 proptest cases, rebuilding a full `Server` every time

**File:** `src/server/task_dispatch_tests.rs:634`
**Issue:** `gate_never_misfires` calls `grid_server()` as the first statement inside the proptest closure body. `grid_server()` calls `Server::builder()...build()` which allocates fresh `HashMap`s, `Arc`s, `RwLock`s, `CancellationManager`, and 7 tool registrations. The test already shares a `PROPTEST_RT` to avoid repeated runtime allocation; the server should be shared similarly. At 64 cases it is harmless; at higher case counts for regression strengthening it becomes wasteful.
**Fix:** Hoist the server construction before the `proptest!` macro:
```rust
// before proptest! block:
let server = std::sync::Arc::new(grid_server());
proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    #[test]
    fn gate_never_misfires(...) {
        let server = server.clone();
        // ... rest of body unchanged
    }
}
```
`Server::handle_request` is `&self` and `Arc<Server>` is `Clone`, so this is safe.

---

_Reviewed: 2026-06-22T00:00:00Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
