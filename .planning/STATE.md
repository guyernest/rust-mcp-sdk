---
gsd_state_version: 1.0
milestone: v2.3
milestone_name: Excel-as-Configuration MCP Servers
status: verifying
stopped_at: Completed 98-01-PLAN.md (DSTK-02 config contract + RED stack.ts-guard regression)
last_updated: "2026-06-17T00:25:31.660Z"
last_activity: 2026-06-17
progress:
  total_phases: 53
  completed_phases: 47
  total_plans: 221
  completed_plans: 221
  percent: 89
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-06-09) · .planning/ROADMAP.md (v2.3 milestone, Phases 91-96)

**Core value:** Compile, never interpret — any project can compile a governed Excel workbook into a tested, versioned, deterministic MCP server where the workbook is simultaneously the specification (formula DAG), the test oracle (cached cell values = assertions), and the output template.
**Current focus:** Phase 98 — deploy-stack-ts-regeneration-guard-config-driven-metadata

## Current Position

Phase: 98 (deploy-stack-ts-regeneration-guard-config-driven-metadata) — EXECUTING
Plan: 4 of 4 (98-01 complete)
Status: Phase complete — ready for verification
Last activity: 2026-06-17

Progress: [████████████████████] 286/290 plans (99%) · v2.3 phases 91–96 all Complete

## v2.3 Phase Plan (6 phases, 38 requirements)

| Phase | Goal | Reqs | Critical-path |
|-------|------|------|---------------|
| 91 | Workbook Runtime + Purity Gate + Dialect Spec — reader-free leaf, `cargo tree`/`cargo-deny` purity gate on day one, dialect spec + linter | 6 | yes — proves the boundary, blocks all |
| 92 | BundleSource + Served-Tool Toolkit Module — freeze the bundle contract from the consumer side, 5 tools fully manifest-driven, fail-closed | 9 | yes — locks contract before compiler re-cut |
| 93 | Workbook Compiler + §5 Fixes + Promote Gate — umya-isolated pipeline, manifest-driven emit, CR-01/CR-02/WR-01, umya provenance, change-class + golden gate | 14 | yes — heaviest lift |
| 94 | CLI Subcommands + `pmcp.toml` — compile/lint/emit thin shells, `--accept` flow, project config kills single-workbook assumptions | 4 | over stable compiler |
| 95 | Shape A Binary `pmcp-workbook-server` — pure-config binary from a bundle alone, mirrors `pmcp-sql-server` | 1 | over toolkit module + `pmcp.toml` |
| 96 | Shape B Scaffold + Dialect-Version + Generalization Validation — `--kind workbook-server`, second-workbook gate, Excel-quirk corpus | 4 | proves generalization |

**Execution order:** 91 → 92 → 93 → 94 → 95 → 96 (strictly sequential — each phase's output is the next phase's dependency)

## Accumulated Context

### Decisions

Decisions are logged in PROJECT.md Key Decisions table. Decisions framing this milestone:

- v2.3: Runtime-first build order (RFC §7) — port `pmcp-workbook-runtime` before any `umya` code so the purity gate defends the boundary from day one.
- v2.3: Freeze the bundle contract from the consumer side (Phase 92) BEFORE re-cutting the compiler (Phase 93).
- v2.3: §5 generalization fixes (manifest-driven emit, CR-01/CR-02/WR-01, umya provenance) land in the compiler-owning phase (93), not deferred.
- v2.3: Mirror the v2.2 toolkit pattern (toolkit feature module + Shape A binary + Shape B scaffold); explicitly does NOT touch `pmcp-code-mode`.
- [Phase ?]: Phase 91-01: pmcp-workbook-runtime lifted reader-free from lighthouse; D-08 adds Deserialize to finding types; no pmcp dep (D-09 permits but runtime is pmcp-free); writer-only rust_xlsxwriter — zip enters only via the writer
- [Phase ?]: Phase 91-02: pmcp-workbook-dialect reader-free slot 2b; flat 13-fn WHITELIST (D-05); doc-const binding test enforces WBDL-01; re-exports runtime findings (D-03); linter/WorkbookMap deferred to Phase 93 (D-02)
- [Phase ?]: Phase 91-03: WBRT-04 fail-closed three-layer purity gate (cargo-tree per-crate/per-feature negative+positive + crate-local cargo-deny bans + crate split); merge-blocking CI gate; just+make entrypoints (D-09); WBDL-03 re-mapped to Phase 93 (D-02)
- Phase 96-02 (WBCL-05): `cargo pmcp new --kind workbook-server` Shape B scaffold; scaffold assets EMBEDDED under the cargo-pmcp package root via include_dir!/include_bytes! (publish-safe, NOT copied from crates/* at generate-time); narrow #[path] lib seam (templates_workbook_server) exposes the generator to the example+integration test; emitted Cargo.toml is purity-safe (default-features=false, workbook-embedded+http); reuses the tax-calc@1.1.0 golden (D-07, no new .xlsx)
- Phase 96-03 (WBEX-01/02 de-risk): reusable #[cfg(test)] rust_xlsxwriter fixture author (fixture_author.rs) retires the .xlsx authoring landmine -- genuine Excel identity (rust_xlsxwriter 0.95 defaults: Application=Microsoft Excel + AppVersion=12.0000 + non-sentinel calcId 124519 => ExcelTrusted, no DocProperties needed) asserted DIRECTLY via classify_authored(), cached-<v> reconcile oracle via Formula::set_result, env-gated #[ignore] regenerate_fixtures generator (normal tests use TempDir), production-refusal guard (T-96-07). Per-fixture *.provenance-override.json + *.gen.json sidecars
- Phase 96-03: 1900-leap disposition (A) DAG-expressible -- IF(serial>59, serial+1, serial) over f64 with whitelisted ops only; NO DATE/DATEVALUE added (WBDL-01 doc-const binding + deferred-functions boundary held; dialect crate byte-clean); committed leap1900-probe.xlsx reconcile fixture compiles+reconciles; SPIKE-1900-leap.md carries ## WBEX-02 Traceability for Plan 96-05
- Phase 96-04 (WBEX-01 generalization gate PROVEN): a second synthetic loan rate-tier workbook compiles via the GENERIC compile_workbook driver and serves its OWN get_manifest/tools/list schema (loan input/output keys present, tax-calc keys absent, the two key sets DISJOINT) behind the SAME five generic tool names -- the disjointness read off the generic toolkit fns (input_schema_for_manifest/output_schema_for_manifest/GetManifestHandler) IS the proof (T-96-11), zero per-workbook served Rust. Whitelist-legal (VLOOKUP + INDEX-MATCH cross-check, IFERROR, nested-IF tiering, ROUND/CEILING; NO PMT/POWER/exponentiation, D-02). Added name_named_inputs() -- the in_* input named-range convention mirroring out_* -- so the served input schema carries semantic keys (loan_amount) not the cell's numeric value (Rule 2 deviation). Custom-unit acceptance item NOT achievable via the compile path (synth sets role.unit=None; cell_map reads role.unit) -> asserted the generic { value, unit } projection RUNS per output instead. reemit_loan.rs (9 #[cfg(test)] assertions incl. production-refusal T-96-10) + committed synthetic loan-calc.xlsx (zero customer/TowelRads material, T-96-12)
- Phase 96-05 (WBEX-02 Excel-quirk corpus COMPLETE): 8 quirks in BOTH D-08 layers -- scalar_eval unit tests (runtime crate, 8 #[test] each with a {formula+context, oracle, expected} tuple; half-rounding asserts excel_round source of truth; 1900-leap asserts the >59 boundary + +1 offset components per SPIKE, no DATE) + penny-reconcile mini fixtures (quirks_reconcile.rs harness: compile via override -> load bundle -> seed inputs -> run_executor -> RETRIEVE recomputed value + cached oracle -> within_tol, cannot pass on compile-success alone T-96-14b; a wrong-oracle negative test proves the value is graded; production-refusal spot check T-96-13). 3 of 4 NAMED quirks have a real reconcile fixture (1900-leap reuses leap1900-probe.xlsx; empty-cell coercion via A2+(A1=IF(A2>9999,1)->Empty) since an absent range member is a hard #REF! not Empty; half-rounding). Error propagation (named) is the plan-sanctioned scalar_eval-only stand-in: the runtime Div clamps zero-divisor NaN->0 (WR-02/IN-03) and errors short-circuit at preflight_error, so a numeric reconcile fixture is not expressible. Quirk->WBEX-02 traceability map in the quirks_reconcile.rs module doc. Reverted incidental regenerate_fixtures rewrites of existing leap/loan fixtures (no edits to existing fixtures). make quality-gate DEFERRED to the phase verifier.
- [Phase ?]: Phase 98 DSTK-01: shared exists-guard + --regenerate-stack/--force preserve curated stack.ts on both deploy targets (IAM validation kept outside the guard)

### Pending Todos

None yet.

### Blockers/Concerns

- [Phase 93 research flag] Confirm whether the SWC/`pmcp-code-mode` JS oracle is still load-bearing for offline penny-reconcile parity, or whether pure-Rust `scalar_eval` fully covers it (LOW-MEDIUM confidence; verify against the lighthouse Phase-10 reconcile path during Phase 93 planning).
- [Phase 91] Re-derive the `quick-xml` / `zip` transitive pins via `cargo tree -p umya-spreadsheet -i quick-xml` against the actual resolved workspace (do not fork a second copy).

## Deferred Items

Items deferred by design for this milestone:

| Category | Item | Status | Deferred At |
|----------|------|--------|-------------|
| BundleSource | S3 / registry bundle store (documented seam on the trait) | Deferred | v2.3 scope |
| Dialect | Named-range-backed validation lists (inline-literal DV enums only ship) | Deferred | v2.3 scope |
| Compiler | Capability cells (Rust/remote/MCP escape hatches) | Deferred | v2.3 scope |
| Compiler | Row-block iteration / arbitrary-N loops | Deferred | v2.3 scope |
| Served | Wire deferred error triggers (`stale_oracle`, `unapproved_assumption`) | Deferred | v2.x |

## Shipped Milestones

| Version | Name | Phases | Date |
|---------|------|--------|------|
| v1.0 | MCP Tasks Foundation | 1-3 | 2026-02-22 |
| v1.1 | Task-Prompt Bridge | 4-8 | 2026-02-23 |
| v1.2 | Pluggable Storage Backends | 9-13 | 2026-02-24 |
| v1.3 | MCP Apps Developer Experience | 14-19 | 2026-02-26 |
| v1.4 | Book & Course Update | 20-24 | 2026-02-28 |
| v2.0 | Protocol Modernization | 54-59 | — |
| v2.2 | Configuration-Only MCP Servers (SQL + OpenAPI) | 82-90.2 | substantially shipped |

## Session Continuity

Last session: 2026-06-17T00:11:29.496Z
Stopped at: Completed 98-01-PLAN.md (DSTK-02 config contract + RED stack.ts-guard regression)
Resume file: None

## Performance Metrics

| Phase | Plan | Duration | Notes |
|-------|------|----------|-------|
| Phase 91 P01 | 14 | 3 tasks | 21 files |
| Phase 91 P02 | 9min | 3 tasks | 4 files |
| Phase 91 P03 | 22min | 3 tasks | 9 files |
| Phase 96 P02 | 38min | 3 tasks | 15 files |
| Phase 96 P03 | 35min | 2 tasks | 6 files |
| Phase 96 P04 | ~40min | 2 tasks | 6 files |
| Phase 96 P05 | ~40min | 2 tasks | 9 files |
| Phase 98 P01 | ~12min | 2 tasks | 2 files |
| Phase 98 P98-02 | 25min | 2 tasks | 6 files |
| Phase 98 P03 | 40min | 2 tasks | 5 files |
