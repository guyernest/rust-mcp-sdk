---
phase: 84
slug: sql-connectors-postgres-mysql-athena-sqlite
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-05-19
---

# Phase 84 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Derived from 84-RESEARCH.md §6 (Validation Architecture).

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo test` (Rust 2021, workspace) + `cargo fuzz` + `cargo run --example` |
| **Config file** | Per-crate `Cargo.toml` `[dev-dependencies]` + workspace root `Cargo.toml` |
| **Quick run command** | `cargo test -p pmcp-server-toolkit --features sqlite` (single crate, fast) |
| **Full suite command** | `make quality-gate` (matches CI exactly — `cargo fmt --all --check`, clippy pedantic+nursery with `--features full`, build, test workspace, audit) |
| **Estimated runtime** | Quick ~15s · Full ~3–5min (matches CI wall clock) |

---

## Sampling Rate

- **After every task commit:** Run `cargo check -p {touched_crate}` (≤5s incremental)
- **After every task that adds tests:** Run `cargo test -p {touched_crate}` (≤30s)
- **After every plan wave:** Run `cargo test --workspace --features sqlite` plus `cargo clippy --workspace --all-targets -- -D warnings` (≤90s)
- **Before `/gsd:verify-work`:** Full `make quality-gate` must be green
- **Max feedback latency:** 30 seconds at task granularity, 90 seconds at wave granularity

---

## Per-Task Verification Map

> Filled by the planner during PLAN.md generation. One row per task in the
> task XML blocks across all `84-NN-PLAN.md` files. The matrix below seeds the
> structure with one row per phase-level coverage axis from RESEARCH.md §6 so
> the planner can expand it row-by-row as it writes plans.

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 84-XX-YY | (planner) | (planner) | CONN-03 | — | placeholder translation never panics on malformed input | property | `cargo test -p pmcp-server-toolkit --features sqlite translate_placeholders_props` | ❌ W0 | ⬜ pending |
| 84-XX-YY | (planner) | (planner) | CONN-01 | — | `execute()` returns ordered, JSON-serializable rows | unit + integration | `cargo test -p pmcp-server-toolkit --features sqlite sql::` | ❌ W0 | ⬜ pending |
| 84-XX-YY | (planner) | (planner) | CONN-05 | — | Postgres connector binds `$1` correctly against authentic in-process mock | integration | `cargo test -p pmcp-toolkit-postgres --tests` | ❌ W0 | ⬜ pending |
| 84-XX-YY | (planner) | (planner) | CONN-06 | — | MySQL connector binds `?` correctly against authentic in-process mock | integration | `cargo test -p pmcp-toolkit-mysql --tests` | ❌ W0 | ⬜ pending |
| 84-XX-YY | (planner) | (planner) | CONN-07 | — | Athena connector binds `?` + GetTableMetadata for schema_text against in-process mock | integration | `cargo test -p pmcp-toolkit-athena --tests` | ❌ W0 | ⬜ pending |
| 84-XX-YY | (planner) | (planner) | CONN-08 | — | SQLite connector executes against real in-memory `rusqlite` DB | integration | `cargo test -p pmcp-server-toolkit --features sqlite sqlite::` | ❌ W0 | ⬜ pending |
| 84-XX-YY | (planner) | (planner) | CONN-04 | — | `build_code_mode_prompt(connector)` produces dialect-aware body for all 4 dialects | unit | `cargo test -p pmcp-server-toolkit --features sqlite code_mode::build_code_mode_prompt` | ❌ W0 | ⬜ pending |
| 84-XX-YY | (planner) | (planner) | CONN-02 | — | `schema_text()` is dialect-styled (not normalized) and folds `[[database.tables]]` descriptions | unit | `cargo test -p pmcp-server-toolkit --features sqlite schema_text` | ❌ W0 | ⬜ pending |
| 84-XX-YY | (planner) | (planner) | TEST-01 | — | All four per-backend integration suites exit 0 (no Docker / no testcontainers anywhere) | workspace | `cargo test --workspace --features sqlite` + `! grep -rn testcontainers crates/pmcp-toolkit-*` | ❌ W0 | ⬜ pending |
| 84-XX-YY | (planner) | (planner) | TEST-07 | — | Toolkit config-toml fuzz target survives 60s without panic | fuzz | `cd crates/pmcp-server-toolkit/fuzz && cargo +nightly fuzz run pmcp_config_toml_parser -- -max_total_time=60` | ❌ W0 | ⬜ pending |
| 84-XX-YY | (planner) | (planner) | CONN-01/03/04 | — | All new public types/fns have working doctests | doctest | `cargo test --doc -p pmcp-server-toolkit --features sqlite` + `cargo test --doc -p pmcp-toolkit-{postgres,mysql,athena}` | ❌ W0 | ⬜ pending |
| 84-XX-YY | (planner) | (planner) | CONN-05/06/07/08 | — | One `cargo run --example` per backend demonstrating Shape-C-shaped use | example | `cargo run -p pmcp-toolkit-{postgres,mysql,athena} --example {name}` and `cargo run -p pmcp-server-toolkit --features sqlite --example {name}` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

The planner SHALL allocate Wave 0 tasks to create the following before any
production code lands:

- [ ] `crates/pmcp-server-toolkit/src/sql/translate.rs` — module stub for the `SqlWalker` state machine + `TranslatedSql` struct (skeleton + `#[cfg(test)] mod tests` placeholder for property tests)
- [ ] `crates/pmcp-server-toolkit/tests/translate_placeholders_props.rs` — property-test scaffold using `proptest` (per CLAUDE.md "ALWAYS property testing"), invariants: idempotence-without-placeholders, bind-order preservation, no-panic on malformed input
- [ ] `crates/pmcp-toolkit-postgres/` — new workspace member skeleton (`Cargo.toml`, `src/lib.rs` stub, `tests/mock_postgres.rs` shell, `tests/integration.rs` shell, `examples/shape_c.rs` shell)
- [ ] `crates/pmcp-toolkit-mysql/` — same skeleton shape
- [ ] `crates/pmcp-toolkit-athena/` — same skeleton shape
- [ ] `crates/pmcp-server-toolkit/fuzz/fuzz_targets/pmcp_config_toml_parser.rs` — confirmed present (extend, do not duplicate); add new corpus entries seeding per-backend `[database]` keys
- [ ] Cargo.toml root `[workspace.members]` — three new entries inserted at line ~541 per RESEARCH.md §7

*If existing infrastructure covers a row, the planner SHALL strike it; otherwise it becomes a Wave 0 task.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Real-AWS Athena query against a live workgroup | CONN-07 | Requires AWS credentials + an Athena-backed S3 dataset; CI has neither | One-time smoke run by operator: `AWS_PROFILE=… cargo run -p pmcp-toolkit-athena --example shape_c` against the `open-images` config |
| Real-Postgres / MySQL container query | CONN-05 / CONN-06 | We deliberately ship NO Docker / testcontainers — authentic in-process mocks cover automated CI | One-time smoke run by operator: `DATABASE_URL=postgres://… cargo run -p pmcp-toolkit-postgres --example shape_c`, same for MySQL with their own URL |
| `make quality-gate` matches CI exactly | TEST-01 | Reproducing CI's `--features full` pedantic+nursery clippy locally is the contract per CLAUDE.md | Operator runs `make quality-gate` before pushing the phase merge |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references (skeleton crates + property-test scaffold + workspace-members insertion)
- [ ] No watch-mode flags (no `cargo watch`; CI uses one-shot `cargo test`)
- [ ] Feedback latency < 90s at wave granularity
- [ ] `nyquist_compliant: true` set in frontmatter after planner fills the per-task matrix

**Approval:** pending
