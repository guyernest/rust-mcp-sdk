---
phase: 103
reviewers: [codex]
reviewed_at: 2026-06-30T21:28:36Z
plans_reviewed: [103-01-PLAN.md, 103-02-PLAN.md, 103-03-PLAN.md, 103-04-PLAN.md, 103-05-PLAN.md, 103-06-PLAN.md]
---

# Cross-AI Plan Review — Phase 103

## Codex Review

## Summary

The phase is generally well-planned: the wave structure matches the dependency graph, the two real technical risks are correctly identified, and the plans preserve the key scope fences: no changes to `examples/wasm-client`, no full browser OAuth SDK, no `tasks/*` wire changes. The biggest issues are around a few implementation assumptions that could break execution: `getrandom` must be available as a normal dependency if `pkce.rs` is target-agnostic, the `PendingSlot` abstraction may silently overwrite responses unless guarded, the demo server Cargo feature split is underspecified and likely tricky, and the OAuth route-merge spike needs to produce reusable implementation shape rather than only notes.

## Strengths

- Correctly treats `WasmHttpTransport` as a fix to an existing broken implementation, not a new transport.
- Good dependency ordering: PKCE, transport, and spike run before server/browser implementation; release gate comes last.
- Strong validation coverage for the pure PKCE helper: RFC vector, property tests, fuzz harness.
- The plans respect important fences: browser OAuth orchestration stays in the example, `examples/wasm-client` remains unchanged, `tasks/*` contract is frozen.
- The security model is thoughtfully covered: PKCE S256, state validation, bearer rejection, owner-scoped tasks, sessionStorage tradeoff documented.
- `make quality-gate` plus explicit `make wasm-build` is the right final gate, since wasm is not covered by the normal quality gate.

## Concerns

- **HIGH: `getrandom` dependency availability may be wrong in 103-01.**  
  If `getrandom` is only declared under a wasm target dependency, an ungated host-compiling `src/shared/pkce.rs` that directly imports `getrandom::fill` will fail on host. The plan says “already a wasm dep,” but target-agnostic SDK code needs `getrandom` available for host too, or a cfg-specific implementation.

- **HIGH: 103-04 Cargo feature split is under-specified.**  
  One example package containing both a wasm `cdylib` depending on `pmcp` with `default-features = false, features = ["wasm"]` and a native demo server needing `pmcp` with `full` / `streamable-http` is hard to express cleanly in one `Cargo.toml`, because dependency features are unified per package build. This can easily cause wasm builds to pull native HTTP stacks.

- **HIGH: 103-04 server route composition may require deeper API changes than planned.**  
  If `StreamableHttpServer` does not expose a composable router builder publicly, “merge onto one origin” may not be possible without modifying SDK internals. The spike plan identifies this, but 103-04 assumes the chosen mechanism will be implementable.

- **MEDIUM: `PendingSlot::put` should not silently overwrite.**  
  The proposed `put(&mut self, msg)` overwrites any existing pending response. If `send()` is accidentally called twice before `receive()`, the first response is lost. Even if `Client` is serial today, the transport should reject this state with an internal error.

- **MEDIUM: 103-02 host tests prove the buffer, not the transport.**  
  A host-testable `PendingSlot` is useful, but it does not prove `WasmHttpTransport::send()` actually stores `do_request()` output or that `receive()` returns it in wasm. At least one wasm-targeted test or a mockable `do_request` seam would better prove WEBCH-02.

- **MEDIUM: The delayed task discovery heuristic is race-prone.**  
  `store.list(owner)` plus “most recent Working task” is acceptable for a single-user demo, but the plan should require filtering by a unique task variable or request marker if possible. Otherwise concurrent clicks can complete the wrong task.

- **MEDIUM: 103-03 says no throwaway probe remains, but gives no durable test artifact.**  
  The spike resolves real risks, but deleting the probe means the result can regress. If route composition and owner resolution are critical, some minimized test or helper should survive into 103-04.

- **MEDIUM: 103-06 blocks on human decision despite `autonomous: false`, but earlier plans assume autonomous execution.**  
  That is reasonable, but it means the overall phase is not fully autonomous. Make sure workflow tooling understands the blocking checkpoint.

- **LOW: `pending_slot` likely should not be public API.**  
  Adding `pub mod pending_slot` widens the public surface for internal plumbing. Prefer `pub(crate) mod pending_slot;` if module visibility supports tests, or keep tests inside the module.

- **LOW: Fuzz command may be brittle.**  
  `cargo +nightly fuzz build pkce_helper` is not the usual cargo-fuzz form in many setups. Prefer documenting `cargo fuzz build pkce_helper` / `cargo fuzz run pkce_helper`, with nightly only if the repo already requires it.

## Suggestions

- In 103-01, explicitly update dependency placement if needed: make `getrandom` a normal dependency, or implement native verifier generation with an already-normal dependency and wasm with `getrandom`. Do not rely on a wasm-only target dependency for host code.

- Change `PendingSlot::put` to return `Result<()>` and error when the slot is occupied:
  `send()` should fail if called before the prior response is received.

- Keep `pending_slot` internal:
  use `pub(crate) mod pending_slot;` or place it under `wasm_http.rs` with `#[cfg(test)]` host-testable logic if feasible.

- Add one WEBCH-02 test that exercises the transport contract closer to reality. If browser Fetch is hard to test, introduce a small internal response-buffering method tested directly, or a wasm-bindgen test that stubs `fetch`.

- For 103-04/05, strongly consider splitting the example into two packages under `examples/web-channel-client/`, such as:
  `client/` for wasm and `server/` for native, or a workspace with separate manifests. That avoids feature unification between wasm and native server dependencies.

- Strengthen the delayed task pattern by attaching a unique marker in task variables or tool args, then have the background updater find the matching Working task. If not possible, document the single-user/concurrency limitation directly in the example README.

- In 103-03, have the spike output include concrete implementation snippets or a small retained test helper for route composition and owner resolution, not only prose.

- In 103-04, add an explicit test for state/PKCE failure during token exchange if the IdP routes are implemented there. The browser state check is in 103-05, but server-side PKCE rejection is also worth pinning.

- In 103-05, specify token exchange request format exactly: content type, fields, and expected JSON response. OAuth token endpoints are often where browser demos fail due to form-vs-JSON mismatches.

- In 103-06, run `make wasm-release` if that is truly part of phase validation. The task text mentions it, but acceptance criteria only require `make wasm-build`.

## Risk Assessment

**Overall risk: MEDIUM-HIGH.**

The core SDK work is low-to-medium risk: PKCE is straightforward, and the transport fix is small. The higher risk is integration complexity: OAuth route composition with `StreamableHttpServer`, the native/wasm Cargo feature split, and the delayed task updater pattern. The plans identify these risks well, but a few assumptions need to be converted into concrete implementation constraints before execution. If the example package is split cleanly and `getrandom` dependency placement is fixed up front, the phase drops closer to **MEDIUM** risk.

---

## Consensus Summary

Only one external reviewer (Codex) was run, so "consensus" here is a single
independent perspective rather than cross-model agreement. Codex's assessment
**aligns with the internal plan-checker** on structure (correct wave/dependency
ordering, D-08-as-fix, frozen fences) but surfaces **implementation-assumption
risks the checker did not flag** — these are the actionable additions.

### Agreed Strengths (Codex ↔ internal plan-checker)
- `WasmHttpTransport` correctly modeled as a **fix to an existing broken impl**, not a greenfield transport.
- Dependency ordering is sound: PKCE / transport / spike precede server + browser work; release gate is last.
- Strong ALWAYS coverage for the pure PKCE helper (RFC vector + property + fuzz harness).
- Scope fences respected: `examples/wasm-client` untouched, browser OAuth stays in the example, `tasks/*` contract frozen.
- Security model is thoughtful (S256, state validation, bearer rejection, owner-scoped tasks, sessionStorage tradeoff documented).
- Final gate runs BOTH `make quality-gate` AND `make wasm-build` (SC-4).

### Top Concerns to Address (Codex — NEW, not caught internally)
1. **HIGH — `getrandom` dependency placement (103-01).** A target-agnostic `src/shared/pkce.rs` calling `getrandom::fill` will fail to compile on the HOST target if `getrandom` is declared only as a wasm-target dep. Must be a normal dependency (or cfg-specific impl). This is the single most likely "plan looks fine, build breaks immediately" failure.
2. **HIGH — example Cargo feature unification (103-04/05).** One example package can't cleanly carry both a wasm `cdylib` (`pmcp` `default-features=false, features=["wasm"]`) and a native demo server (`pmcp` `full`/`streamable-http`) — per-package feature unification will pull native HTTP stacks into the wasm build. Recommend splitting into `client/` (wasm) + `server/` (native) sub-packages/manifests.
3. **HIGH — OAuth route composition feasibility (103-04).** "Merge IdP routes onto one origin" assumes `StreamableHttpServer` exposes a composable router builder publicly; if it doesn't, this needs SDK-internal changes. The 103-03 spike must *prove implementability* (and ideally leave a durable helper/test), not just produce prose notes.
4. **MEDIUM — `PendingSlot::put` should reject double-write.** Make it `Result<()>` and error when the slot is occupied, so a double `send()` before `receive()` surfaces an internal error instead of silently dropping the first response.
5. **MEDIUM — 103-02 host test proves the buffer, not the transport.** Add a wasm-targeted test or a mockable `do_request` seam so WEBCH-02 proves `send()` actually stores the response and `receive()` returns it.
6. **MEDIUM — delayed-task discovery race.** `store.list(owner)` + "most recent Working task" can complete the wrong task under concurrent clicks; filter by a unique task-variable/request marker, or document the single-user limitation in the README.
7. **LOW — keep `pending_slot` internal** (`pub(crate)`), and **normalize the fuzz command** (`cargo fuzz build/run pkce_helper`, drop `+nightly` unless the repo requires it).

### Divergent Views
None — single reviewer. Worth noting Codex rates overall risk **MEDIUM-HIGH** (vs the internal checker's PASS), driven entirely by *integration* risk (Cargo feature split, OAuth route-merge, delayed-task updater), not by plan-structure defects. Fixing concerns #1–#3 up front drops it toward MEDIUM.
