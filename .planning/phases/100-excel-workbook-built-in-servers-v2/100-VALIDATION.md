---
phase: 100
slug: excel-workbook-built-in-servers-v2
status: planned
nyquist_compliant: true
wave_0_complete: false
created: 2026-06-20
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
| **WBV2-01** | 1 | Ship a provenance-valid `template.xlsx` (rust_xlsxwriter, ExcelTrusted): Inputs Table (`name\|value\|description\|tier` + tier dropdown + sample enum dropdown), calc + reference regions, ≥1 named output Table with a caption. Doubles as the honest reference fixture. | 100-01 | SC4 |
| **WBV2-02** | 2 | Ingest harvests Excel Tables: `TableRecord{name,area,columns}` on `SheetRecord` via `get_tables()/get_name()/get_columns()`; per-row type/unit/enum/tier harvest; malformed table XML contained as a clean `CompileError` (fuzz-proven). | 100-02 | SC1 |
| **WBV2-03** | 3 | Manifest model lift `CellMap{inputs,outputs}` → `{inputs[], tools[]}`; `Dag::upstream_input_leaves` reachability; per-tool `input_keys` derived from the DAG; §4.2 edge cases. | 100-03 | SC1, SC2 |
| **WBV2-04** | 4 | One named MCP tool per output Table: sanitized name (`^[a-zA-Z0-9_-]{1,64}$`), caption description, per-tool DAG-derived `inputSchema` + non-empty `outputSchema` (structuredContent); N-handler registration. F2 retained. | 100-04 | SC2, SC5 |
| **WBV2-05** | 5 | Per-tool reconciliation against each tool's own oracle; reshape F1 into cell-precise ROW lints; retire F1/F3-input + `strip_governance_prefix` + `name_named_inputs`. | 100-04 | SC3, SC5 |
| **WBV2-06** | 6 | `cargo pmcp workbook explain <file>`: read-only ingest→synth→render the emitted tool surface (text first; `--format json` add-on) before deploy. | 100-05 | SC3 |
| **WBV2-07** | 7 | Docs/training: pmcp-book + pmcp-course chapters seeded from the spec + the template. | 100-05 | SC4 (training arm) |
| **WBV2-08** | cross | `make quality-gate` + PMAT (cog ≤25) + `make purity-check` all green; no umya/calamine/quick-xml in any served tree; rust_xlsxwriter confined to compiler/author. | 100-06 | SC5 |

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | `cargo test` (unit + integration), `proptest` (property), `cargo fuzz` (fuzz), `cargo run --example` (example) |
| **Config file** | workspace `Cargo.toml` (no extra config) |
| **Quick run command** | `cargo test -p <workbook-crate> --lib` |
| **Full suite command** | `make quality-gate` (fmt --all + clippy pedantic/nursery + build + test + audit) + PMAT complexity gate + `make purity-check` |
| **Estimated runtime** | ~minutes (workspace build dominates) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p <workbook-crate> --lib`
- **After every plan wave:** Run `make quality-gate` (+ PMAT + purity for any touched compiler code)
- **Before `/gsd:verify-work`:** Full suite must be green (Success Criterion 5: quality-gate + PMAT + purity all green)
- **Max feedback latency:** keep `--lib` quick run under the dev loop; full gate at wave boundaries

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 01-T1 | 100-01 | 1 | WBV2-01 | T-100-01 | Table author surface stays `#[cfg(test)]` (no rust_xlsxwriter in prod build) | unit | `cargo test -p pmcp-workbook-compiler --lib fixture_author` | ✅ fixture_author.rs | ⬜ pending |
| 01-T2 | 100-01 | 1 | WBV2-01 | T-100-01, T-100-02 | Shipped template classifies ExcelTrusted (not UmyaFabricated) | unit | `cargo test -p pmcp-workbook-compiler --test template_provenance` | ❌ template.xlsx, template_provenance.rs (Wave 0) | ⬜ pending |
| 02-T1 | 100-02 | 2 | WBV2-02 | T-100-05 | TableRecord holds only owned String/RangeRef (no umya leak) | unit | `cargo test -p pmcp-workbook-compiler --lib ingest -- table_record` | ✅ ingest/cell_map.rs, ingest/mod.rs | ⬜ pending |
| 02-T2 | 100-02 | 2 | WBV2-02 | — | Per-row type/unit/enum/tier harvest; ineligible DV → WARNING not error | unit | `cargo test -p pmcp-workbook-compiler --lib synth -- harvest` | ✅ manifest/synth.rs | ⬜ pending |
| 02-T3 | 100-02 | 2 | WBV2-02 | T-100-03, T-100-04 | Malformed table XML → clean CompileError, never panic | fuzz | `cargo +nightly fuzz run workbook_table_ingest -- -runs=20000 -max_total_time=60` | ❌ fuzz/fuzz_targets/workbook_table_ingest.rs (Wave 0) | ⬜ pending |
| 03-T1 | 100-03 | 3 | WBV2-03 | T-100-07 | Tool type lives in reader-free model (serde-only derive) | unit | `cargo test -p pmcp-workbook-runtime --lib artifact_model` | ✅ artifact_model.rs | ⬜ pending |
| 03-T2 | 100-03 | 3 | WBV2-03 | T-100-06 | Derived leaves ⊆ inputs (no computed/constant cell becomes an input) | property | `cargo test -p pmcp-workbook-runtime --lib dag -- upstream_input_leaves` | ✅ dag.rs | ⬜ pending |
| 03-T3 | 100-03 | 3 | WBV2-03 | T-100-06 | Per-tool input_keys minimal + DAG-derived; feeds-no-tool lint | unit | `cargo test -p pmcp-workbook-compiler --lib -- build_tools` | ✅ artifact/cell_map.rs | ⬜ pending |
| 04-T1 | 100-04 | 4 | WBV2-04 | T-100-10, T-100-11 | Per-tool schema keeps additionalProperties:false; sanitize rejects empty name; F2 retained | unit | `cargo test -p pmcp-server-toolkit --lib workbook` | ✅ schema.rs, handler.rs, mod.rs | ⬜ pending |
| 04-T2 | 100-04 | 4 | WBV2-05 | T-100-08, T-100-09 | Strict/computed cells never advertised as inputs; cell-precise row lints; per-tool reconcile | unit | `cargo test -p pmcp-workbook-compiler --lib -- row_lint reconcile json_key` | ✅ lib.rs, manifest_model.rs, fixture_author.rs | ⬜ pending |
| 04-T3 | 100-04 | 4 | WBV2-04 | T-100-10 | tools/list returns N tools w/ disjoint DAG-derived I/O schemas | integration + example | `cargo test -p pmcp-server-toolkit --test workbook_multi_tool && cargo run --example workbook_table_authoring` | ❌ workbook_multi_tool.rs, examples/workbook_table_authoring.rs (Wave 0) | ⬜ pending |
| 05-T1 | 100-05 | 5 | WBV2-06 | T-100-12, T-100-13 | explain inherits ingest umya-isolation; preview = runtime schema (no divergence) | integration + CLI | `cargo test -p cargo-pmcp --test workbook_explain && cargo run -p cargo-pmcp -- workbook explain crates/pmcp-workbook-compiler/tests/fixtures/template.xlsx` | ❌ explain.rs, workbook_explain.rs (Wave 0) | ⬜ pending |
| 05-T2 | 100-05 | 5 | WBV2-07 | — | Chapters teach only the table model (no retired in_*/out_*) | doc-build | `cd pmcp-book && mdbook build && cd ../pmcp-course && mdbook build` | ❌ workbook-table-authoring.md ×2 (Wave 0) | ⬜ pending |
| 06-T1 | 100-06 | 5 | WBV2-08 | T-100-15 | No cog>25 / SATD / clippy regression | gate | `make quality-gate` | n/a | ⬜ pending |
| 06-T2 | 100-06 | 5 | WBV2-08 | T-100-14 | No umya/calamine/quick-xml/rust_xlsxwriter in any served tree | gate | `make purity-check` | n/a | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Provenance-valid `template.xlsx` reference fixture (replaces misleading hand-authored fixtures) — gates Success Criterion 4 (Plan 01)
- [ ] `crates/pmcp-workbook-compiler/tests/template_provenance.rs` — ExcelTrusted assertion (Plan 01)
- [ ] `crates/pmcp-workbook-compiler/fuzz/fuzz_targets/workbook_table_ingest.rs` — malformed table XML → clean error (Plan 02, Pitfall 2)
- [ ] Property test harness for `Dag::upstream_input_leaves` (random DAG generator) (Plan 03)
- [ ] `crates/pmcp-server-toolkit/tests/workbook_multi_tool.rs` — `tools/list` returns one tool per output Table (Plan 04)
- [ ] `examples/workbook_table_authoring.rs` — author template → compile → list tools (Plan 04)
- [ ] `cargo-pmcp/tests/workbook_explain.rs` — snapshot fixture for `workbook explain` text output (Plan 05)
- [ ] Table-harvest unit fixtures (input/output Excel Tables with type/unit/enum/tier witnesses) (Plan 02)
- [ ] Fail-helpful lint negative fixtures (blank name, duplicate key, value-less row, no-caption output, unmappable tool name, input-feeds-no-tool) (Plan 04)

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
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency acceptable for Rust build loop
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** planned (auditor to confirm at execution)
