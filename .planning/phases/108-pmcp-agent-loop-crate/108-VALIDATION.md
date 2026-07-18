---
phase: 108
slug: pmcp-agent-loop-crate
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-07-17
---

# Phase 108 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `#[test]` + `proptest` 1.7 + doctests |
| **Config file** | none (cargo default); workspace `Cargo.toml` |
| **Quick run command** | `cargo test -p pmcp-agent -- --test-threads=1` |
| **Full suite command** | `make quality-gate` (fmt --all, clippy pedantic+nursery -D warnings, build, workspace test, audit) |
| **Estimated runtime** | ~60s quick / ~10min full gate |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p pmcp-agent -- --test-threads=1` (plus `cargo test -p pmcp --test in_tool_peer_roundtrip` for D-106-A tasks)
- **After every plan wave:** Run `make quality-gate`
- **Before `/gsd:verify-work`:** Full suite must be green (incl. PMAT cog ≤25 in CI)
- **Max feedback latency:** 600 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| TBD | TBD | TBD | AGNT-01 | — | N/A | unit (compile) | `cargo test -p pmcp-agent seams::` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | AGNT-02 | — | N/A | unit | `cargo test -p pmcp-agent loop::decide` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | AGNT-03 | — | N/A | property | `cargo test -p pmcp-agent replay_safety` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | AGNT-04 | — | N/A | integration (D-03 real loop) | `cargo test -p pmcp-agent --test real_loop_sampling` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | AGNT-05, AGNT-06 | — | HTTP sources feature-gated; no secrets logged | unit (mock HTTP) | `cargo test -p pmcp-agent --features openai-compat,anthropic sources::` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | AGNT-07 | — | N/A | integration + wasm compile gate | `cargo build -p pmcp-agent --target wasm32-unknown-unknown` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | AGNT-08 | — | N/A | integration | `cargo test -p pmcp-agent invoker::task_augmented` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | AGNT-09 | — | Env resolver reads secrets from env, never persists them | unit | `cargo test -p pmcp-agent config::resolver` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | D-106-A | — | Request serialization preserved (D-02: pump responses only) | integration (D-03 real loop) | `cargo test -p pmcp --test in_tool_peer_roundtrip` | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | ALWAYS-fuzz | — | N/A | fuzz | `cargo fuzz run agent_digest` (or proptest equivalent) | ❌ W0 | ⬜ pending |
| TBD | TBD | TBD | ALWAYS-example | — | N/A | example | `cargo run -p pmcp-agent --example s50_standalone_vs_sampled` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*
*Task IDs filled by planner; requirement→command map is authoritative from RESEARCH.md Validation Architecture.*

---

## Wave 0 Requirements

- [ ] `crates/pmcp-agent/Cargo.toml` — new workspace member (mirror `crates/pmcp-tasks` isolation precedent); add to root `[workspace].members`
- [ ] `crates/pmcp-agent/tests/real_loop_sampling.rs` — D-03 real-loop harness (extend `tests/common/duplex.rs` convention → real `Server::run` + real `Client` with `on_sampling`)
- [ ] `tests/in_tool_peer_roundtrip.rs` (pmcp core) — D-106-A proof (sampling + elicitation + roots round-trips on stock server loop)
- [ ] `crates/pmcp-agent/tests/replay_safety.rs` + `tests/fixtures/*.json` golden traces — AGNT-03 property tests over `EffectTrace`
- [ ] Fuzz target `fuzz/fuzz_targets/agent_digest.rs` (or proptest equivalent) — CLAUDE.md ALWAYS requirement
- [ ] `examples/s50_standalone_vs_sampled.rs` — ALWAYS requirement (verify next free example number at plan time)
- [ ] CI wasm32 compile gate entry for `pmcp-agent` default features (D-13)

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Standalone example against a live local model (e.g., Ollama) | AGNT-05 | Requires a running LLM endpoint; CI uses mock HTTP | `ollama serve` then `cargo run -p pmcp-agent --features openai-compat --example s50_standalone_vs_sampled -- --standalone` |
| Shape-compatibility mapping vs durable-agent-lambda (D-09) | AGNT-02/03 design validation | Private-repo reference; human review of the mapping artifact | Review the D-09 mapping doc against `pmcp-run/.../iteration.rs` |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 600s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
