---
phase: 73
slug: typed-client-helpers-list-all-pagination-parity-client-01
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-21
---

# Phase 73 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) + proptest + cargo-fuzz |
| **Config file** | root `Cargo.toml`, `fuzz/Cargo.toml` |
| **Quick run command** | `cargo test -p pmcp --lib client:: --features full` |
| **Full suite command** | `make quality-gate` |
| **Estimated runtime** | ~45 seconds (quick) / ~6 minutes (full) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p pmcp --lib client:: --features full`
- **After every plan wave:** Run `cargo test -p pmcp --features full && cargo clippy --workspace --all-targets --all-features -- -D warnings`
- **Before `/gsd-verify-work`:** `make quality-gate` must be green
- **Max feedback latency:** 60 seconds for quick, 360 seconds for full

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 73-01-XX | 01 | 1 | CLIENT-01 (typed helpers) | — | Serialize errors returned as `Error::validation`, never panic | unit | `cargo test -p pmcp --lib client::tests::typed_helpers --features full` | ❌ W0 | ⬜ pending |
| 73-01-XX | 01 | 1 | CLIENT-01 (ClientOptions) | — | `ClientOptions::default()` has `max_iterations: 100` | unit + property | `cargo test -p pmcp --lib client::options --features full` | ❌ W0 | ⬜ pending |
| 73-02-XX | 02 | 2 | CLIENT-01 (pagination) | T-73-01 | `max_iterations` cap enforced; infinite-cursor servers cannot DoS the client | property + integration | `cargo test -p pmcp --lib client::tests::list_all --features full && cargo test -p pmcp --test list_all_pagination --features full` | ❌ W0 | ⬜ pending |
| 73-02-XX | 02 | 2 | CLIENT-01 (delegation equivalence) | — | Typed + list_all helpers return the same data as their Value counterparts | property | `cargo test -p pmcp --lib client::tests::delegation_equiv --features full` | ❌ W0 | ⬜ pending |
| 73-02-XX | 02 | 2 | CLIENT-01 (cursor fuzz) | T-73-01 | Arbitrary cursor strings never cause panics or infinite loops | fuzz | `cargo +nightly fuzz run list_all_cursor_loop -- -runs=10000` | ❌ W0 | ⬜ pending |
| 73-03-XX | 03 | 3 | CLIENT-01 (example) | — | Example compiles, runs, and prints typed results | example | `cargo run --example 09_typed_client_helpers --features full` | ❌ W0 | ⬜ pending |
| 73-03-XX | 03 | 3 | CLIENT-01 (doctest) | — | All new rustdoc examples on helpers + `ClientOptions` pass | doctest | `cargo test -p pmcp --doc --features full client::` | ✅ | ⬜ pending |
| 73-03-XX | 03 | 3 | CLIENT-01 (CI gate) | — | Full CI-matching gate green on HEAD | integration | `make quality-gate` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

Exact Task IDs (e.g., `73-01-03`) are finalized by gsd-planner and written back here before execution begins.

---

## Wave 0 Requirements

- [ ] `src/client/options.rs` — new module with `ClientOptions` struct (created Wave 1, needed by all waves)
- [ ] `src/client/mod.rs::tests::typed_helpers` — unit-test module for typed helpers
- [ ] `src/client/mod.rs::tests::list_all` — unit-test module for list_all helpers using existing `MockTransport` (line 1847 of `src/client/mod.rs`)
- [ ] `src/client/mod.rs::tests::delegation_equiv` — property tests asserting typed helpers match Value-path results
- [ ] `tests/list_all_pagination.rs` — integration test wiring a paginated MockTransport
- [ ] `fuzz/fuzz_targets/list_all_cursor_loop.rs` — new fuzz target for `max_iterations` + cursor safety
- [ ] `examples/09_typed_client_helpers.rs` — new example (filename avoids c08 collision with Phase 74's `c08_oauth_dcr.rs`)

No framework install needed — `cargo test`, `proptest`, and `cargo-fuzz` are already wired in the workspace.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Rustdoc rendering on docs.rs | CLIENT-01 | docs.rs build only triggers after release publish | After `pmcp 2.6.0` publishes, verify https://docs.rs/pmcp/latest/pmcp/client/struct.Client.html shows `call_tool_typed`, `list_all_tools`, `ClientOptions::with_client_options` (or chosen name) without rendering warnings |
| CHANGELOG accuracy | CLIENT-01 | Human judgement on wording/placement | Reviewer reads CHANGELOG v2.6.0 entry for completeness and semver-appropriateness |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 60s (quick) / 360s (full)
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
