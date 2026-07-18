---
phase: 109
slug: team-reference-servers
status: ready
nyquist_compliant: true
wave_0_complete: true
created: 2026-07-18
updated: 2026-07-18
---

# Phase 109 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) + proptest for property tests + cargo-fuzz targets |
| **Config file** | `crates/pmcp-team-servers/Cargo.toml` (Wave 1 / 109-01 creates crate) |
| **Quick run command** | `cargo test -p pmcp-team-servers --all-features` |
| **Full suite command** | `make quality-gate` |
| **Estimated runtime** | ~60 seconds (quick) / ~10 minutes (full gate) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p pmcp-team-servers --all-features`
- **After every plan wave:** Run `make quality-gate`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 120 seconds

---

## Per-Task Verification Map

Every task across the 8 plans carries a `<verify><automated>` command — no MISSING references, so no Wave-0 test-scaffold work is outstanding (Nyquist-compliant).

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 109-01-T1 | 109-01 | 1 | TEAM-01 | T-109-01-SC | Wasm-clean manifest, path-dep only default build | auto | `cargo build -p pmcp-team-servers` | task-created | ⬜ pending |
| 109-01-T2 | 109-01 | 1 | TEAM-01 | T-109-01-SC | Fuzz sub-package excluded from published tarball | auto | `cargo build -p pmcp-team-servers --all-features` (+ fuzz target files) | task-created | ⬜ pending |
| 109-01-T3 | 109-01 | 1 | TEAM-01 | T-109-01-01 | derive_attachment total/pure over adversarial pkg | tdd/proptest | `cargo test -p pmcp-team-servers --test derive_props` | task-created | ⬜ pending |
| 109-01-T4 | 109-01 | 1 | TEAM-01 | — | Additive contract rev keeps 19-tool conformance | auto | `cargo test --test team_contracts_conformance` | exists | ⬜ pending |
| 109-02-T1 | 109-02 | 2 | TEAM-02 | T-109-02-01 | Path containment (no `..`/absolute escape) + fuzz | tdd+fuzz | `cargo test -p pmcp-team-servers fs::local::tests fs::backend` | task-created | ⬜ pending |
| 109-02-T2 | 109-02 | 2 | TEAM-02 | T-109-02-01 | Exact 11-tool surface; HTTP DNS-rebinding via SDK | auto | `cargo build -p pmcp-team-servers --features "team-fs http" --bin team-fs && cargo test -p pmcp-team-servers fs::server` | task-created | ⬜ pending |
| 109-03-T1 | 109-03 | 2 | TEAM-04 | T-109-03-* | Zero-dep BM25 scorer; bounded memory backend | tdd | `cargo test -p pmcp-team-servers mem::bm25 mem::backend` | task-created | ⬜ pending |
| 109-03-T2 | 109-03 | 2 | TEAM-04 | T-109-03-* | Exact 6 mem__* surface; HTTP via SDK | auto | `cargo build -p pmcp-team-servers --features "mem-mcp http" --bin mem-mcp` | task-created | ⬜ pending |
| 109-04-T1 | 109-04 | 2 | TEAM-03 | T-109-04-* | Console channel offline; webhook feature-gated | auto | `cargo test -p pmcp-team-servers approval::channels && cargo build -p pmcp-team-servers --features "approval-mcp webhook http"` | task-created | ⬜ pending |
| 109-04-T2 | 109-04 | 2 | TEAM-03 | T-109-04-* | subject-ref linkage; static/dynamic approval tools | auto | `cargo test -p pmcp-team-servers approval && cargo build -p pmcp-team-servers --features "approval-mcp http" --bin approval-mcp` | task-created | ⬜ pending |
| 109-05-T1 | 109-05 | 2 | TEAM-05 | T-109-05-01/02/04 | Strict depth parse + self/ancestor guards + fuzz | tdd+fuzz | `cargo test -p pmcp-team-servers --test team_props && cargo test -p pmcp-team-servers team::guards` | task-created | ⬜ pending |
| 109-05-T2 | 109-05 | 2 | TEAM-05 | T-109-05-03/05 | Task-augmented hop; tight verbatim re-emit; keys redacted | tdd | `cargo test -p pmcp-team-servers team::server team::member` | task-created | ⬜ pending |
| 109-05-T3 | 109-05 | 2 | TEAM-05 | T-109-05-05 | Member LLM via SlotResolver (D-15); depth header→_meta edge | auto | `cargo build -p pmcp-team-servers --features "team-mcp http" --bin team-mcp` | task-created | ⬜ pending |
| 109-06-T1 | 109-06 | 3 | TEAM-01 | T-109-06-01 | In-memory wiring (no sockets); shared resolve_member_factory | auto | `cargo build -p pmcp-team-servers --all-features && cargo test -p pmcp-team-servers compose::wiring` | task-created | ⬜ pending |
| 109-06-T2 | 109-06 | 3 | TEAM-01 | T-109-06-02 | No leaked spawned tasks; FixedSource determinism | auto | `cargo test -p pmcp-team-servers --test small_team --all-features` | task-created | ⬜ pending |
| 109-07-T1 | 109-07 | 3 | TEAM-06 | T-109-07-* | Wire-level advertised==enforced via real Client | auto | `cargo build -p pmcp-team-servers --features conformance && cargo test -p pmcp-team-servers conformance` | task-created | ⬜ pending |
| 109-07-T2 | 109-07 | 3 | TEAM-06 | T-109-07-* | Every-tool/every-guard fixtures across four servers | auto | `cargo test -p pmcp-team-servers --test conformance --all-features` | task-created | ⬜ pending |
| 109-08-T1 | 109-08 | 4 | TEAM-06 | T-109-08-01 | Every equation bound to a concrete fn (no drift) | auto | `test -f contracts/team-servers/binding.yaml && grep -q "target_crate: pmcp-team-servers" contracts/team-servers/binding.yaml` | task-created | ⬜ pending |
| 109-08-T2 | 109-08 | 4 | TEAM-06 | T-109-08-02 | pmat comply in gate; guarded for pmat-absent | auto | `grep -q "^comply:" Makefile && make comply` | exists | ⬜ pending |
| 109-08-T3 | 109-08 | 4 | TEAM-06 | — | Full four-server E2E on FixedSource, offline | auto | `cargo run -p pmcp-team-servers --example doc_review_team --all-features` | task-created | ⬜ pending |
| 109-08-T4 | 109-08 | 4 | TEAM-01 | T-109-08-03 | Real subprocess tools/list smoke; bounded, no leak | auto | `cargo test -p pmcp-team-servers --test dev_binary_smoke --all-features` | task-created | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

The Roadmap "Wave 0" spikes were resolved during planning and folded into concrete plan tasks (see 109-RESEARCH.md `## Open Questions (RESOLVED)`):

- [x] `crates/pmcp-team-servers/` — new workspace crate with per-server feature flags → **109-01 Task 1/2**
- [x] Conformance-fixture test harness wired to Phase 107 (PKG-03) contract fixtures → **109-07 Task 1/2**
- [x] Spike: task-augmented `pmcp::Client` call for the team-mcp member hop (TEAM-05 `_meta[related_task]`) → **RESOLVED: `Client::call_tool_with_task` VERIFIED at `src/client/mod.rs:624` (109-PATTERNS.md); consumed in 109-05 Task 2**
- [x] Spike: `pmat comply check` invocation against existing `contracts/binding.yaml` (D-18) → **109-08 Task 1 probes the CLI before authoring, with a pmat-absent fallback**

No task carries an `<automated>MISSING` marker → no Wave-0 test-scaffold work is outstanding.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Real-LLM (Ollama/Anthropic) member run per configured slot | TEAM-05 / D-15 | Requires a live LLM endpoint; CI path uses FixedSource | Configure a member `AgentPackage` llm `ConfigSlot` (OLLAMA_BASE_URL / ANTHROPIC_API_KEY env), run `team-mcp --package <pkg>`, dispatch a `team_mcp__<member>` call, confirm a live completion |

> The prior "small team, one process local run" / "launch each dev binary; verify tools/list responds" manual row is now AUTOMATED: 109-06 Task 2 (`small_team` in-process) + 109-08 Task 4 (`dev_binary_smoke` real subprocess) cover the TEAM-01 runnable-binary surface.

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies (21/21 tasks carry `<automated>`)
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references (none — no `<automated>MISSING` markers exist)
- [x] No watch-mode flags
- [x] Feedback latency < 120s (quick run ~60s)
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved (plan-time validation contract complete; per-task Status flips to ✅ during execution)
