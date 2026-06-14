---
gsd_state_version: 1.0
milestone: v2.3
milestone_name: Excel-as-Configuration MCP Servers
status: executing
stopped_at: Phase 95 context gathered
last_updated: "2026-06-14T13:47:53.401Z"
last_activity: 2026-06-14
progress:
  total_phases: 53
  completed_phases: 45
  total_plans: 214
  completed_plans: 214
  percent: 85
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-06-09) · .planning/ROADMAP.md (v2.3 milestone, Phases 91-96)

**Core value:** Compile, never interpret — any project can compile a governed Excel workbook into a tested, versioned, deterministic MCP server where the workbook is simultaneously the specification (formula DAG), the test oracle (cached cell values = assertions), and the output template.
**Current focus:** Phase 94 — cli-subcommands-pmcp-toml

## Current Position

Phase: 999.1
Plan: Not started
Status: Executing Phase 94
Last activity: 2026-06-14

Progress: [███░░░░░░░] 33% (v2.3 phases: 91, 92 done; 93–96 remain)

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

Last session: 2026-06-14T13:47:53.395Z
Stopped at: Phase 95 context gathered
Resume file: .planning/phases/95-shape-a-binary-pmcp-workbook-server/95-CONTEXT.md

## Performance Metrics

| Phase | Plan | Duration | Notes |
|-------|------|----------|-------|
| Phase 91 P01 | 14 | 3 tasks | 21 files |
| Phase 91 P02 | 9min | 3 tasks | 4 files |
| Phase 91 P03 | 22min | 3 tasks | 9 files |
