---
phase: 102
reviewers: [codex]
reviewed_at: 2026-06-22T16:30:40Z
plans_reviewed: [102-01-PLAN.md, 102-02-PLAN.md, 102-03-PLAN.md]
model: gpt-5.5
---

# Cross-AI Plan Review — Phase 102

## Codex Review

## Summary

The plans are directionally strong and clearly target the real failure mode: `ServerCore` has task lifecycle support while the HTTP-facing `Server` rejects it. The wave ordering is sensible: extract shared machinery, wire `Server`, then prove over real HTTP. The biggest risks are in the exact seam design: `maybe_build_task_created` is underspecified, response-shape adaptation may get awkward, and auth/owner handling for newly reachable `tasks/*` needs sharper tests. I would approve the architecture, but not the plans as-is without tightening the create-path contract and HTTP/auth edge coverage.

## 102-01 Review

### Strengths

- Correctly prioritizes SHARE-not-duplicate by extracting `task_dispatch` before touching `Server`.
- Good recognition that `apply_tasks_capability_rule` must be a free function because `tool_infos` is built at different times.
- Keeps Phase 101 `ServerCore` tests as the regression oracle.
- Calls out PMAT/cognitive-complexity and WASM gating early.

### Concerns

- **HIGH:** `maybe_build_task_created` is introduced in Plan 01 but not proven there. It is described as “primarily for Server wiring,” which means the riskiest helper may compile without behavioral coverage until Plan 02.
- **HIGH:** The helper signature omits an explicit `task_requested: bool`. The plan relies on “caller precondition.” That is fragile and makes the gate easier to misuse later.
- **MEDIUM:** Moving or duplicating `success_response` / `error_response` helpers risks subtly changing JSON-RPC envelope shape. The plan says duplicate trivial bodies is acceptable, but the phase’s whole point is avoiding drift.
- **MEDIUM:** `build_task_created_response` signature drops explicit `task_id` and terminal `result` parameters from the described current shape. If it re-extracts internally, say so; otherwise result persistence can regress.
- **LOW:** “Byte-identical behavior” is claimed, but the acceptance tests are behavior-level, not byte-level. That wording overpromises.

### Suggestions

- Make `maybe_build_task_created` take `task_requested: bool`; let the helper enforce the complete gate.
- Add unit tests for `TaskDispatch::maybe_build_task_created` inside `src/server/task_dispatch.rs` under `#[cfg(test)]`, since external integration tests cannot call `pub(crate)` cleanly.
- Prefer moving shared JSON-RPC response builders into `task_dispatch` once, then have `ServerCore` delegate, rather than duplicating helper bodies.
- Preserve the existing `ServerCoreBuilder::default_tasks_capability` API if anything internal/tests may reference it; delegate instead of removing.

### Risk Assessment

**MEDIUM.** The extraction is the right architecture, but small signature choices here determine whether Plan 02 becomes clean or invasive.

## 102-02 Review

### Strengths

- Directly addresses the actual HTTP-facing gap: `ServerBuilder` gets task backends, capabilities advertise, `Server::handle_request` stops rejecting `tasks/*`.
- Includes important non-task regression coverage to prevent task envelope leakage.
- Correctly separates endpoint routing from create-path handling.
- Requires the capability rule to be shared, not re-derived.

### Concerns

- **HIGH:** Auth on `tasks/*` endpoint routing is not explicit enough. `Server::handle_call_tool` validates auth, but `tasks/get|result|list|cancel` may bypass equivalent auth validation depending on where the new outer dispatch is inserted.
- **HIGH:** Adapter option `(b)` converting `JSONRPCResponse` back into `Result<Value>` is dangerous. It can lose JSON-RPC error codes like `-32002` or double-wrap errors unless implemented very carefully.
- **HIGH:** The plan says non-task tools with client-sent task field must return normal results. That may conflict with expected validation if a tool declares `TaskSupport::Forbidden`. The plan should explicitly test Required/Optional/Forbidden/None semantics.
- **MEDIUM:** Adding `with_task_store` to `ServerBuilder` reuses confusing legacy naming: `with_task_store` actually accepts `TaskRouter`, while `task_store` accepts `TaskStore`. Existing precedent exists, but this is API debt being expanded.
- **MEDIUM:** The proptest through public `Server::handle_request` may be expensive and hard to write correctly because generating arbitrary tools/handlers dynamically is awkward.
- **MEDIUM:** `tasks_dispatch_shared` only drives `Server::handle_request`, not HTTP. That is fine for Plan 02, but do not count it toward HTASK-03.

### Suggestions

- For tasks endpoint routing, strongly prefer adapter `(a)`: intercept `ClientRequest::Tasks*` at the JSONRPCResponse assembly layer and return `TaskDispatch::route_tasks_endpoint` directly.
- Add explicit tests for auth/owner scoping: two different `AuthContext` owners, task created by owner A, `tasks/get/result/cancel` by owner B returns not found or equivalent non-leak.
- Add explicit tests for `TaskSupport::Forbidden` and `TaskSupport::Optional` with and without the request `task` field.
- Rename or heavily document `with_task_store` on `ServerBuilder`; if additive API allows, consider `task_router(...)` as the clearer new high-level setter while retaining legacy naming only where already present.
- Make the property test target the extracted gate in module tests, then keep one integration regression through `Server::handle_request`.

### Risk Assessment

**HIGH.** This is the highest-risk plan. It changes the public high-level dispatcher, auth reachability, response shaping, and task creation behavior in one wave.

## 102-03 Review

### Strengths

- Correctly uses a real `StreamableHttpServer` + `StreamableHttpTransport` loopback as the phase acceptance gate.
- Mirrors the important Phase 101 invariants: advertised capability, store-minted id, pending `-32002`, terminal result retrieval.
- Keeps the old in-process lifecycle test untouched.
- Adds a worked example, which is valuable for the pmcp.run-shaped use case.

### Concerns

- **HIGH:** Binding `127.0.0.1:0` is only “preferred,” but the plan does not require proving how to retrieve the assigned port. Falling back to a fixed port invites flaky tests.
- **HIGH:** The example runs a server and client in one process, but shutdown behavior is not specified. A spawned HTTP server can hang the example/test if not cancelled or if background task errors are ignored.
- **MEDIUM:** The live HTTP test asserts happy-path owner behavior but not cross-owner isolation over HTTP.
- **MEDIUM:** `cargo run --example s46_http_tool_as_task --features full` can be brittle if the example uses sleeps instead of readiness signaling.
- **LOW:** “No `ServerCore::handle_request` shim” is checked by grep in the example, but the stronger proof is that it builds only `Server::builder()` and uses `StreamableHttpServer`.

### Suggestions

- Require an ephemeral-port helper, even if that means binding a `TcpListener` first and passing the actual address if the server API supports it.
- Add deterministic readiness signaling instead of fixed `sleep(500ms)` where possible.
- Ensure both test and example cancel/abort the spawned server task after the client completes.
- Add one HTTP-level negative test for pending result and one for owner isolation if auth context can be injected through the transport.
- In the example, keep output concise and make failure assertions hard, not just printed.

### Risk Assessment

**MEDIUM.** The acceptance target is correct, but test reliability and shutdown details need tightening.

## Overall Assessment

The plan set mostly achieves the phase goal: one shared lifecycle unit, high-level `Server` support, and real HTTP validation. The most important blind spot is that “shared dispatch” is only partially enforced if the create-path remains split between `ServerCore` and `Server` with a helper that depends on caller discipline. Put the full task gate inside the shared helper, including `task_requested`, `TaskSupport`, backend presence, and task-shaped value detection.

Overall risk: **MEDIUM-HIGH**. The architecture is sound, but Plan 02 sits on several sharp edges: auth reachability, JSON-RPC error preservation, and create-path response shaping. Tighten those contracts before execution.

---

## Consensus Summary

Single reviewer (Codex / gpt-5.5). Overall verdict: **architecture approved; tighten Plan 02 contracts before execution. Overall risk MEDIUM-HIGH.**

### Agreed Strengths
- SHARE-not-duplicate is correctly sequenced: extract `task_dispatch` first, wire `Server` second, prove over real HTTP third.
- `apply_tasks_capability_rule` as a free function (tool_infos built at different lifecycle points) is the right call.
- Phase 101 `ServerCore` tests retained as the no-regression oracle.
- Real `StreamableHttpServer` + `StreamableHttpTransport` loopback as the acceptance gate; Phase 101 invariants mirrored (advertised cap, store-minted id, `-32002`, terminal result).

### Agreed Concerns (highest priority — feed into `--reviews` replan)
- **[HIGH] The create-path gate depends on caller discipline.** `maybe_build_task_created` relies on the caller passing `req.task.is_some()` as a precondition. Move the FULL gate inside the shared helper — pass `task_requested: bool` explicitly and let the helper enforce `task_requested && backend present && TaskSupport ∈ {Required,Optional} && value has taskId+status`. This is the difference between "shared dispatch" and "shared-ish dispatch."
- **[HIGH] `maybe_build_task_created` is introduced in Plan 01 but only behaviorally proven in Plan 02.** Add `#[cfg(test)]` unit tests for it inside `src/server/task_dispatch.rs` in Plan 01 (external `tests/` can't reach the `pub(crate)` helper cleanly).
- **[HIGH] Auth reachability on the newly-served `tasks/*` path (Plan 02).** `Server::handle_call_tool` re-validates auth, but `tasks/get|result|list|cancel` may bypass equivalent validation depending on where the outer dispatch is inserted. Prefer adapter **(a)** (intercept `ClientRequest::Tasks*` at the JSONRPCResponse-assembly layer, return `route_tasks_endpoint` directly) over **(b)** — option (b)'s `JSONRPCResponse → Result<Value>` conversion can drop `-32002` or double-wrap errors.
- **[HIGH/MEDIUM] TaskSupport matrix is under-tested.** Add explicit tests for `Forbidden` / `Optional` / `Required` / `None` × (task field present/absent), not just the non-task regression. A `Forbidden` tool with a client-sent task field is an untested edge.
- **[HIGH] Cross-owner isolation is asserted only at the `Server::handle_request` layer, not over HTTP.** Add an owner-A-creates / owner-B-reads (get/result/cancel) IDOR test; ideally one at the HTTP level in Plan 03 if auth context can be injected through the transport.
- **[HIGH] Plan 03 test reliability:** ephemeral `:0` port is only "preferred" — require a concrete port-readback mechanism (bind a `TcpListener` first if needed); replace `sleep(500ms)` with readiness signaling; and ensure both the test and the example abort/cancel the spawned server task after the client completes (otherwise hang risk).
- **[MEDIUM] Avoid drift in JSON-RPC envelope builders.** Don't duplicate `success_response`/`error_response` bodies into `task_dispatch`; move them once and delegate, or the phase's anti-drift goal leaks at the envelope layer.
- **[MEDIUM] `build_task_created_response` signature dropped explicit `task_id`/terminal `result` params** — if it re-extracts internally, document it; otherwise terminal-result persistence can regress.
- **[MEDIUM] API debt:** `ServerBuilder::with_task_store` accepting a `TaskRouter` (while `task_store` takes a `TaskStore`) expands confusing legacy naming onto the high-level surface — at minimum document heavily.
- **[LOW] "byte-identical behavior"** is claimed for the ServerCore refactor but only behavior-level tests back it — soften the wording or add a stronger assertion.

### Divergent Views
None — single reviewer.

### Recommended next step
Incorporate this feedback with: `/gsd:plan-phase 102 --reviews`
The HIGH items cluster on Plan 02 (auth reachability, the create-path gate contract, adapter (a) vs (b)) and Plan 03 (port readback + shutdown). These are contract-tightening edits, not a re-architecture — the SHARE-via-Option-A design itself is endorsed.
