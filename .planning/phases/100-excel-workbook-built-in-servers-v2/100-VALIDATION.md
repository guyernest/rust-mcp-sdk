---
phase: 100
slug: excel-workbook-built-in-servers-v2
status: planned
nyquist_compliant: true
wave_0_complete: false
created: 2026-06-20
updated: 2026-06-20
revision: reviews (Codex cross-AI review feedback applied)
---

# Phase 100 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.
> Detailed dimension-by-dimension validation architecture lives in `100-RESEARCH.md`
> ("Validation Architecture" section). This file is the executable sampling contract;
> the planner/Nyquist auditor fills the Per-Task Verification Map from the PLAN.md tasks.

---

## Locked Requirement Breakdown (WBV2-01..08)

Adopted from 100-RESEARCH.md §"Proposed Requirement Breakdown", mapped 1:1 to the spec's 7
phasing steps (§11) + a cross-cutting quality/purity gate. Every requirement is covered by ≥1 plan.

| Req ID | Step | Description | Plan(s) | Success Criterion |
|--------|------|-------------|---------|-------------------|
| **WBV2-01** | 1 | Ship a provenance-valid `template.xlsx` (rust_xlsxwriter, RAW ExcelTrusted, NO override sidecar): Inputs Table (`name\|value\|description\|tier` + tier dropdown + sample enum dropdown), calc + reference regions, ≥1 named output Table with a caption. Two committed copies byte-identical (test-enforced). Doubles as the honest reference fixture. | 100-01 | SC4 |
| **WBV2-02** | 2 | Ingest harvests Excel Tables: `TableRecord{name,area,columns}` on `SheetRecord` via `get_tables()/get_name()/get_columns()`; per-row type/unit/enum/tier harvest; malformed table XML contained as a clean `CompileError` via a `catch_unwind` seam (fuzz-proven); real-template end-to-end harvest integration test. | 100-02 | SC1 |
| **WBV2-03** | 3 | Manifest model lift `CellMap{inputs,outputs}` → `{inputs[], tools[]}`; `Dag::upstream_input_leaves` reachability; per-tool `input_keys` derived from the DAG; §4.2 edge cases; a TRANSITIONAL deprecated `outputs()` accessor keeps the whole workspace compiling green at end of wave 3 (removed in Plan 04). | 100-03 | SC1, SC2 |
| **WBV2-04** | 4 | One named MCP tool per output Table: sanitized name (`^[a-zA-Z0-9_-]{1,64}$`, LOCKED 5-rule semantics + post-sanitize collision detection), caption description, per-tool DAG-derived `inputSchema` + non-empty `outputSchema` (structuredContent); N-handler registration. F2 retained. | 100-04 | SC2, SC5 |
| **WBV2-05** | 5 | Per-tool reconciliation against each tool's own oracle via an aggregated `ToolReconcileReport` (any mismatch → non-zero); reshape F1 into cell-precise ROW lints; retire F1/F3-input + `strip_governance_prefix` + `name_named_inputs` + `CalculateHandler` + the Plan 03 `outputs()` shim. | 100-04 | SC3, SC5 |
| **WBV2-06** | 6 | `cargo pmcp workbook explain <file>`: read-only ingest→synth→render the emitted tool surface (text first; `--format json` add-on) before deploy. | 100-05 | SC3 |
| **WBV2-07** | 7 | Docs/training: pmcp-book + pmcp-course chapters seeded from the spec + the template (table model only; retired MECHANISM identifiers absent, retirement prose allowed). | 100-05 | SC4 (training arm) |
| **WBV2-08** | cross | `make quality-gate` + PMAT (cog ≤25) + `make purity-check` all green; no umya/calamine/quick-xml in any served tree; rust_xlsxwriter confined to compiler/author; retired-symbol sweep proves the named-range model + Plan 03 shim fully gone. | 100-06 | SC5 |

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo test` (unit + integration), `proptest` (property), `cargo fuzz` (fuzz), `cargo run --example` (example) |
| **Config file** | workspace `Cargo.toml` (no extra config) |
| **Quick run command** | `cargo test -p <workbook-crate> --lib` |
| **Full suite command** | `make quality-gate` (fmt --all + clippy pedantic/nursery + build + test + audit) + PMAT complexity gate + `make purity-check` + retired-symbol sweep |
| **Estimated runtime** | ~minutes (workspace build dominates) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p <workbook-crate> --lib`
- **After every plan wave:** Run `make quality-gate` (+ PMAT + purity for any touched compiler code)
- **At end of Plan 03 (cross-wave gate):** `cargo build --workspace` + `cargo test --workspace --no-run` must both exit 0 (no red workspace into wave 4)
- **Before `/gsd:verify-work`:** Full suite must be green (Success Criterion 5: quality-gate + PMAT + purity + retired-symbol sweep all green)
- **Max feedback latency:** keep `--lib` quick run under the dev loop; full gate at wave boundaries

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 01-T1 | 100-01 | 1 | WBV2-01 | T-100-01 | Table author surface stays `#[cfg(test)]` (no rust_xlsxwriter in prod build) | unit | `cargo test -p pmcp-workbook-compiler --lib fixture_author` | ✅ fixture_author.rs | ⬜ pending |
| 01-T2 | 100-01 | 1 | WBV2-01 | T-100-01, T-100-02 | Shipped template classifies RAW ExcelTrusted (NO override sidecar); two copies byte-identical | unit | `cargo test -p pmcp-workbook-compiler --test template_provenance` | ❌ template.xlsx, template_provenance.rs (Wave 0) | ⬜ pending |
| 02-T1 | 100-02 | 2 | WBV2-02 | T-100-03, T-100-05 | TableRecord holds only owned String/RangeRef (no umya leak); catch_unwind seam → corrupted tableN.xml = CompileError, not panic | unit | `cargo test -p pmcp-workbook-compiler --lib ingest -- table_record` | ✅ ingest/cell_map.rs, ingest/mod.rs | ⬜ pending |
| 02-T2 | 100-02 | 2 | WBV2-02 | — | Per-row type/unit/enum/tier harvest; ineligible DV → WARNING not error | unit | `cargo test -p pmcp-workbook-compiler --lib synth -- harvest` | ✅ manifest/synth.rs | ⬜ pending |
| 02-T3 | 100-02 | 2 | WBV2-02 | T-100-03, T-100-04 | Malformed table XML → clean CompileError via the catch_unwind seam, never panic | fuzz | `cargo +nightly fuzz run workbook_table_ingest -- -runs=20000 -max_total_time=60` | ❌ fuzz/fuzz_targets/workbook_table_ingest.rs (Wave 0) | ⬜ pending |
| 02-T4 | 100-02 | 2 | WBV2-02 | T-100-05 | Harvest projection total + stable (type/unit/enum/tier); unit∈{USD,rate,date,None}, tier∈{strict,variable} | property | `cargo test -p pmcp-workbook-compiler --test harvest_roundtrip_prop` | ❌ tests/harvest_roundtrip_prop.rs (Wave 0) | ⬜ pending |
| 02-T5 | 100-02 | 2 | WBV2-02 | T-100-05 | REAL template.xlsx end-to-end harvest: row-level key/dtype/unit/enum/tier/description + table/caption→tool linkage | integration | `cargo test -p pmcp-workbook-compiler --test template_harvest_e2e` | ❌ tests/template_harvest_e2e.rs (Wave 0) | ⬜ pending |
| 03-T1 | 100-03 | 3 | WBV2-03 | T-100-07, T-100-16 | Tool type lives in reader-free model (serde-only derive); transitional deprecated outputs() accessor flattens tools[] | unit | `cargo test -p pmcp-workbook-runtime --lib artifact_model` | ✅ artifact_model.rs | ⬜ pending |
| 03-T2 | 100-03 | 3 | WBV2-03 | T-100-06 | Derived leaves ⊆ inputs (no computed/constant cell becomes an input) | property | `cargo test -p pmcp-workbook-runtime --lib dag -- upstream_input_leaves` | ✅ dag.rs | ⬜ pending |
| 03-T3 | 100-03 | 3 | WBV2-03 | T-100-06 | Per-tool input_keys minimal + DAG-derived; feeds-no-tool lint | unit | `cargo test -p pmcp-workbook-compiler --lib -- build_tools` | ✅ artifact/cell_map.rs | ⬜ pending |
| 03-T4 | 100-03 | 3 | WBV2-03 | T-100-06 | upstream_input_leaves total over arbitrary (incl. cyclic) DAGs; result ⊆ inputs | fuzz | `cargo +nightly fuzz run dag_upstream_leaves -- -runs=20000 -max_total_time=60` | ❌ fuzz/fuzz_targets/dag_upstream_leaves.rs (Wave 0) | ⬜ pending |
| 03-T5 | 100-03 | 3 | WBV2-03 | T-100-16 | Whole workspace compiles green at end of wave 3 via the transitional outputs() shim (no cross-wave red) | gate | `cargo build --workspace && cargo test --workspace --no-run` | n/a (call sites in place) | ⬜ pending |
| 04-T1 | 100-04 | 4 | WBV2-04 | T-100-10, T-100-11 | Per-tool schema keeps additionalProperties:false; sanitize LOCKED 5-rule semantics + reject empty; F2 retained; Checkpoint A tree builds green (shim present) | unit + build | `cargo test -p pmcp-server-toolkit --lib workbook && cargo build --workspace` | ✅ schema.rs, handler.rs, mod.rs | ⬜ pending |
| 04-T2 | 100-04 | 4 | WBV2-05 | T-100-08, T-100-09, T-100-16, T-100-17 | Per-tool reconcile via ToolReconcileReport (any mismatch→non-zero); cell-precise row lints; post-sanitize collision lint; strict/computed never advertised; Plan 03 shim removed (Checkpoint C) | unit + build | `cargo test -p pmcp-workbook-compiler --lib -- row_lint reconcile json_key collision && cargo build --workspace` | ✅ lib.rs, manifest_model.rs, artifact_model.rs, fixture_author.rs | ⬜ pending |
| 04-T3 | 100-04 | 4 | WBV2-04 | T-100-10 | tools/list returns N tools w/ disjoint DAG-derived I/O schemas | integration + example | `cargo test -p pmcp-server-toolkit --test workbook_multi_tool && cargo run --example workbook_table_authoring` | ❌ workbook_multi_tool.rs, examples/workbook_table_authoring.rs (Wave 0) | ⬜ pending |
| 04-T4 | 100-04 | 4 | WBV2-04, WBV2-05 | T-100-10, T-100-11, T-100-17 | sanitize_tool_name Ok⊆^[a-zA-Z0-9_-]{1,64}$ (else Err); post-sanitize collisions flagged; every per-tool inputSchema additionalProperties=false | property | `cargo test -p pmcp-server-toolkit --test workbook_tool_name_prop` | ❌ tests/workbook_tool_name_prop.rs (Wave 0) | ⬜ pending |
| 05-T1 | 100-05 | 5 | WBV2-06 | T-100-12, T-100-13 | explain inherits ingest catch_unwind umya-isolation; preview = runtime schema (no divergence) | integration + CLI + example | `cargo test -p cargo-pmcp --test workbook_explain && cargo run -p cargo-pmcp --example workbook_explain && cargo run -p cargo-pmcp -- workbook explain crates/pmcp-workbook-compiler/tests/fixtures/template.xlsx` | ❌ explain.rs, tests/workbook_explain.rs, examples/workbook_explain.rs (Wave 0) | ⬜ pending |
| 05-T2 | 100-05 | 5 | WBV2-07 | — | Chapters teach only the table model; retired in_*/out_*/define_name MECHANISM identifiers proven ABSENT (scoped negative grep); retirement prose allowed | doc-build + scoped-negative-grep | `cd pmcp-book && mdbook build && cd ../pmcp-course && mdbook build && cd .. && ! grep -rnE '(^|[^a-z])(in_[a-z]|out_[a-z])|define_name' pmcp-book/src/workbook-table-authoring.md pmcp-course/src/workbook-table-authoring.md` | ❌ workbook-table-authoring.md ×2 (Wave 0) | ⬜ pending |
| 06-T1 | 100-06 | 6 | WBV2-08 | T-100-15 | No cog>25 / SATD / clippy regression | gate | `make quality-gate` | n/a | ⬜ pending |
| 06-T2 | 100-06 | 6 | WBV2-08 | T-100-14 | No umya/calamine/quick-xml/rust_xlsxwriter in any served tree | gate | `make purity-check` | n/a | ⬜ pending |
| 06-T3 | 100-06 | 6 | WBV2-08 | T-100-18 | Retired-symbol sweep: strip_governance_prefix/name_named_inputs/promote_named_outputs/CalculateHandler/CellMap::outputs() shim/in_*-out_* sites all absent | gate | `! grep -rnE 'strip_governance_prefix\|name_named_inputs\|promote_named_outputs\|CalculateHandler' crates/ cargo-pmcp/ examples/ --include=*.rs && ! grep -rn 'fn outputs' crates/pmcp-workbook-runtime/src/artifact_model.rs` | n/a | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Provenance-valid `template.xlsx` reference fixture (RAW ExcelTrusted, NO override; replaces misleading hand-authored fixtures) — gates Success Criterion 4 (Plan 01)
- [ ] `crates/pmcp-workbook-compiler/tests/template_provenance.rs` — RAW ExcelTrusted assertion + byte-equality of the two committed copies (Plan 01)
- [ ] `crates/pmcp-workbook-compiler/fuzz/fuzz_targets/workbook_table_ingest.rs` — malformed table XML → clean error via the catch_unwind seam (Plan 02, Pitfall 2)
- [ ] `crates/pmcp-workbook-compiler/tests/harvest_roundtrip_prop.rs` — proptest: harvest projection total + stable (Plan 02, CLAUDE.md ALWAYS PROPERTY)
- [ ] `crates/pmcp-workbook-compiler/tests/template_harvest_e2e.rs` — integration: REAL template.xlsx end-to-end row-level harvest + table/caption→tool linkage (Plan 02, review finding #7)
- [ ] Property test harness for `Dag::upstream_input_leaves` (random DAG generator) (Plan 03)
- [ ] `crates/pmcp-workbook-compiler/fuzz/fuzz_targets/dag_upstream_leaves.rs` — fuzz: upstream_input_leaves total over arbitrary/cyclic DAGs (Plan 03, CLAUDE.md ALWAYS FUZZ)
- [ ] Cross-wave compile gate: transitional `CellMap::outputs()` accessor keeps `cargo build --workspace` green at end of Plan 03 (Plan 03 Task 5, review finding #1)
- [ ] `crates/pmcp-server-toolkit/tests/workbook_multi_tool.rs` — `tools/list` returns one tool per output Table (Plan 04)
- [ ] `crates/pmcp-server-toolkit/tests/workbook_tool_name_prop.rs` — proptest: sanitize_tool_name charset + post-sanitize collision + per-tool additionalProperties:false (Plan 04, CLAUDE.md ALWAYS PROPERTY, T-100-10/11/17)
- [ ] `examples/workbook_table_authoring.rs` — author template → compile → list tools (Plan 04)
- [ ] `cargo-pmcp/tests/workbook_explain.rs` — snapshot fixture for `workbook explain` text output (Plan 05)
- [ ] `cargo-pmcp/examples/workbook_explain.rs` — `cargo run --example` tool-surface demonstration (Plan 05, CLAUDE.md ALWAYS EXAMPLE)
- [ ] Table-harvest unit fixtures (input/output Excel Tables with type/unit/enum/tier witnesses) (Plan 02)
- [ ] Fail-helpful lint negative fixtures (blank name, duplicate key, value-less row, no-caption output, unmappable tool name, post-sanitize tool-name collision, input-feeds-no-tool) (Plan 04)

*Existing `cargo test`/`proptest`/`cargo fuzz` infrastructure covers the rest.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| `cargo pmcp workbook explain <file>` preview reads as a coherent tool surface to a human | Success Criterion 3 | "reads well for LLM selection" is a human-judged property | Run on the shipped template; confirm tool names/descriptions/IO schemas render |

*All other phase behaviors have automated verification.*

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references (incl. the new e2e + collision + cross-wave-compile gates)
- [x] No watch-mode flags
- [x] Feedback latency acceptable for Rust build loop
- [x] `nyquist_compliant: true` set in frontmatter
- [x] Cross-AI review findings (Codex #1–#10) folded into the Per-Task Verification Map

**Approval:** planned (auditor to confirm at execution)
</content>
</invoke>
