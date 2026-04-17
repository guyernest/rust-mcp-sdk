---
phase: 70
slug: add-extensions-typemap-and-peer-back-channel-to-requesthandl
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-04-16
---

# Phase 70 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo test` + `proptest 1.7` + `cargo-fuzz` (dev-deps at `Cargo.toml:131, 140`) |
| **Config file** | Default proptest config; `.cargo/config.toml` controls test threads |
| **Quick run command** | `cargo test --lib server::cancellation -- --test-threads=1` |
| **Full suite command** | `cargo test --features "full" -- --test-threads=1` |
| **Estimated runtime** | Quick: ~5s · Full: ~2–3 min · Quality gate: ~5 min |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib server::cancellation -- --test-threads=1`
- **After every plan wave:** Run `cargo test --features "full" -- --test-threads=1`
- **Before `/gsd-verify-work`:** `make quality-gate` must be green (matches CI exactly)
- **Max feedback latency:** 5s (per-task) / 180s (per-wave)

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 70-01-01 | 01 | 1 | PARITY-HANDLER-01 | — | Extensions typemap field compiles into both `RequestHandlerExtra` structs with `Default`/`Debug`/`Clone` preserved | compilation | `cargo check --features "full"` | ✅ existing | ⬜ pending |
| 70-01-02 | 01 | 1 | PARITY-HANDLER-01 | — | Insert then get of a typed value yields the same value (proptest round-trip) | property | `cargo test --test handler_extensions_properties prop_extensions_insert_get_roundtrip -- --test-threads=1` | ❌ W0 | ⬜ pending |
| 70-01-03 | 01 | 1 | PARITY-HANDLER-01 | T-70-01 (Information Disclosure) | Inserting same type twice returns `Some(old)`; `Debug` prints type names only | unit | `cargo test --lib server::cancellation::tests::test_extensions_insert_overwrite_returns_old -- --test-threads=1` | ❌ W0 | ⬜ pending |
| 70-01-04 | 01 | 1 | PARITY-HANDLER-01 | — | `extra.clone()` preserves extensions key set | property | `cargo test --test handler_extensions_properties prop_extra_clone_preserves_extensions -- --test-threads=1` | ❌ W0 | ⬜ pending |
| 70-01-05 | 01 | 1 | PARITY-HANDLER-01 | — | 6 positional struct-literal sites in `src/server/workflow/prompt_handler.rs` compile after `#[non_exhaustive]` marker | compilation | `cargo check --features "full"` | ✅ existing | ⬜ pending |
| 70-02-01 | 02 | 2 | PARITY-HANDLER-01 | T-70-02 (Tampering — session routing) | `PeerHandle` trait defined; `DispatchPeerHandle` impl routes by session_id | unit | `cargo test --lib server::cancellation::tests::test_peer_handle_trait_shape -- --test-threads=1` | ❌ W0 | ⬜ pending |
| 70-02-02 | 02 | 2 | PARITY-HANDLER-01 | T-70-02 | Two peers in parallel: sample on peer A does NOT deliver to peer B | integration | `cargo test --test handler_peer_integration test_sample_session_routing -- --test-threads=1` | ❌ W0 | ⬜ pending |
| 70-02-03 | 02 | 2 | PARITY-HANDLER-01 | — | `peer.progress_notify` no-ops when no progress token present | unit | `cargo test --lib server::cancellation::tests::test_peer_progress_notify_noop_without_reporter -- --test-threads=1` | ❌ W0 | ⬜ pending |
| 70-02-04 | 02 | 2 | PARITY-HANDLER-01 | T-70-05 (DoS — timeout) | `peer.sample()` honors configurable timeout; default matches ElicitationManager | unit | `cargo test --lib server::cancellation::tests::test_peer_sample_respects_timeout -- --test-threads=1` | ❌ W0 | ⬜ pending |
| 70-02-05 | 02 | 2 | PARITY-HANDLER-01 | — | 9 ServerCore dispatch sites wire the new peer field; remaining 6 struct-literal sites updated | compilation | `cargo check --features "full"` | ✅ existing | ⬜ pending |
| 70-02-06 | 02 | 2 | PARITY-HANDLER-01 | — | WASM build compiles without peer field (cfg-gate honored) | compilation | `cargo check --target wasm32-unknown-unknown --features schema-generation` | ✅ existing | ⬜ pending |
| 70-03-01 | 03 | 3 | PARITY-HANDLER-01 | — | `examples/s42_handler_extensions.rs` compiles and runs in <5s, demonstrating cross-middleware insert/retrieve | example | `cargo run --example s42_handler_extensions` | ❌ W0 | ⬜ pending |
| 70-03-02 | 03 | 3 | PARITY-HANDLER-01 | — | `examples/s43_handler_peer_sample.rs` compiles and runs in <5s, demonstrating in-handler `peer.sample()` round-trip | example | `cargo run --example s43_handler_peer_sample` | ❌ W0 | ⬜ pending |
| 70-03-03 | 03 | 3 | PARITY-HANDLER-01 | T-70-04 (DoS — malformed input) | Fuzz target survives ≥100 iterations without panic | fuzz | `cargo +nightly fuzz run fuzz_peer_handle -- -max_total_time=30` | ❌ W0 | ⬜ pending |
| 70-03-04 | 03 | 3 | PARITY-HANDLER-01 | — | rustdoc on both `cancellation.rs` files passes with `-D warnings`; migration prose present | doc | `RUSTDOCFLAGS='-D warnings' cargo doc --no-deps --features full` | ✅ existing | ⬜ pending |
| 70-03-05 | 03 | 3 | PARITY-HANDLER-01 | — | Full CI gate green (fmt + clippy pedantic+nursery + build + test + audit) | gate | `make quality-gate` | ✅ existing | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `tests/handler_extensions_properties.rs` — proptest file covering insert/get roundtrip + clone preservation
- [ ] `tests/handler_peer_integration.rs` — integration test with in-process client transport (pattern from `tests/typed_tool_transport_e2e.rs`)
- [ ] `fuzz/fuzz_targets/fuzz_peer_handle.rs` — new fuzz target for CLAUDE.md ALWAYS-fuzz compliance
- [ ] `examples/s42_handler_extensions.rs` — new example file
- [ ] `examples/s43_handler_peer_sample.rs` — new example file
- [ ] No framework install needed — proptest (Cargo.toml:131), quickcheck, mockito, insta already in dev-deps; `cargo-fuzz` installed via `cargo install cargo-fuzz` (optional for local, required for CI fuzz gate)

---

## Manual-Only Verifications

*None — all phase behaviors have automated verification (proptest, unit, integration, fuzz, example-smoke, rustdoc, compile-check for WASM).*

---

## Validation Sign-Off

- [ ] All 16 tasks have `<automated>` verify commands or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify (longest gap: 1)
- [ ] Wave 0 covers all ❌ MISSING file references
- [ ] No watch-mode flags (`--watch`, `--loop`)
- [ ] Feedback latency <5s (per-task quick run) / <180s (per-wave full run)
- [ ] `nyquist_compliant: true` set in frontmatter after Wave 0 files land

**Approval:** pending
