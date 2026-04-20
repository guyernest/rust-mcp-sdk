---
gsd_state_version: 1.0
milestone: v2.0
milestone_name: Protocol Modernization
status: Phase complete — ready for verification
stopped_at: Phase 73 context gathered
last_updated: "2026-04-20T18:09:06.193Z"
progress:
  total_phases: 40
  completed_phases: 35
  total_plans: 84
  completed_plans: 84
  percent: 100
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-04-10)

**Core value:** Close credibility and DX gaps where rmcp outshines PMCP -- documentation accuracy, feature gate presentation, macro documentation, example index, repo hygiene.
**Current focus:** Phase 72 — investigate-rmcp-as-foundations-for-pmcp-evaluate-using-rmcp

## Current Position

Phase: 72 (investigate-rmcp-as-foundations-for-pmcp-evaluate-using-rmcp) — EXECUTING
Plan: 3 of 3

## Shipped Milestones

| Version | Name | Phases | Date |
|---------|------|--------|------|
| v1.0 | MCP Tasks Foundation | 1-3 | 2026-02-22 |
| v1.1 | Task-Prompt Bridge | 4-8 | 2026-02-23 |
| v1.2 | Pluggable Storage Backends | 9-13 | 2026-02-24 |
| v1.3 | MCP Apps Developer Experience | 14-19 | 2026-02-26 |
| v1.4 | Book & Course Update | 20-24 | 2026-02-28 |
| v1.5 | Cloud Load Testing Upload | 25-26 | 2026-03-01 |

## Performance Metrics

**Velocity:**

- Total plans completed: 93 (v1.0: 9, v1.1: 10, v1.2: 9, v1.3: 12, v1.4: 10, v1.5: 6, v1.6: 5, v1.7: 4, v2.0: 11)
- Total phases completed: 29

## Accumulated Context

### Decisions

See PROJECT.md Key Decisions table for full history.

v2.1 decisions:

- 4 phases derived from 5 requirement categories following research-recommended dependency order: examples+protocol -> macros -> docs.rs pipeline -> polish
- EXMP and PROT combined into Phase 65 (both are credibility fixes, no dependency between them, co-deliverable)
- Phase ordering follows the docs.rs build pipeline dependency: content accuracy first, then rendering pipeline, then polish
- No new runtime dependencies for this milestone -- all fixes are config, content, and attribute changes
- [Phase 65]: All 17 orphan examples compile successfully -- registered all with import-derived feature flags (no deletions needed)
- [Phase 65]: examples/README.md replaced with PMCP example index — 63 examples categorized by Role/Capability/Complexity + migration reference
- [Phase 69.1]: Pinned rmcp comparison baseline at 1.5.0 (latest stable on crates.io as of 2026-04-16, tag rmcp-v1.5.0); pmcp baseline is v2.3.0 + feat/sql-code-mode at commit dbaee6cc
- [Phase 69.1]: 4 High-severity ergonomics gaps identified — MACRO-02 (rustdoc fallback for tool description), HANDLER-02 (Extensions typemap on RequestHandlerExtra), HANDLER-05 (peer handle in RequestHandlerExtra), CLIENT-02 (typed call_tool + list_all_* pagination helpers). Plan 02 will derive one follow-on phase proposal per High row.
- [Phase 69.2]: 3 follow-on phase proposals drafted in 69-PROPOSALS.md — PARITY-HANDLER-01 (bundling HANDLER-02 + HANDLER-05 on shared RequestHandlerExtra edit site, target v2.2, 4 plans), PARITY-CLIENT-01 (CLIENT-02, target late v2.1, 3 plans), PARITY-MACRO-01 (MACRO-02, target late v2.1, 3 plans). Row-ID bijection verified: all 4 High Row IDs cited in Derived-from + Rationale subsections. Flagged regex bug in Task 2 verify block for Plan 03 correction (pipe-table trailing `|` not matched).
- [Phase 69]: rmcp parity research complete — 69-RESEARCH.md (gap matrix, 32 rows total, 4 High-severity) + 69-PROPOSALS.md (3 proposals). 3 PARITY-* requirement IDs landed in REQUIREMENTS.md (one per proposal); follow-on phases not yet scheduled.
- [Phase 72]: [Phase 72 Plan 02]: PoC Slice 1 spike EXECUTED on throwaway branch — T4_compile_errors=0, T4_loc_delta=537, ~15 min wall-clock under 4-hour hard time-box; serde-shape divergence found (rmcp requires params: {}; rejects null/missing) downgrades INVENTORY row 1 EXACT→compatible-via-adapter
- [Phase 72]: [Phase 72 Plan 02]: Rubric shipped 9 thresholds T1..T9; T8 historical churn + T9 enterprise-feature-preservation added; T2 expanded with PR merge latency; gh fallback URL codified verbatim; default-to-B logic removed per HIGH-1

### Roadmap Evolution

- Phases 65-68 added: v2.1 rmcp Upgrades milestone (examples cleanup, macros rewrite, docs.rs pipeline, documentation polish)
- Phase 67.1 inserted after Phase 67: Code Mode Support (URGENT) — external developer support for code mode pattern (validation + execution) based on pmcp-run/built-in/shared/pmcp-code-mode SDK_DESIGN_SPEC.md
- Phase 67.2 inserted after Phase 67.1: Code Mode Derive Hardening (URGENT) — fix 3 critical derive macro issues from pmcp.run team review: policy_evaluator not called, static ValidationContext, hardcoded "graphql" code type
- Phase 69 added (initially added as duplicate Phase 68, renumbered 2026-04-16 to avoid collision with existing "Phase 68: General Documentation Polish"): rmcp parity research — scope narrowed to ergonomics-only + follow-on phase proposals; transports/examples/docs-coverage intentionally excluded to eliminate overlap with Phase 68. Deliverables: 69-RESEARCH.md (gap matrix) + 69-PROPOSALS.md (2–5 phase proposals seeded from High-severity gaps).
- Phase 70 added: Add Extensions typemap and peer back-channel to RequestHandlerExtra (PARITY-HANDLER-01) — bundles HANDLER-02 (Extensions typemap) + HANDLER-05 (peer handle) on the shared RequestHandlerExtra edit site per 69-PROPOSALS.md.
- Phase 71 added: Rustdoc fallback for #[mcp_tool] tool descriptions (PARITY-MACRO-01) — rustdoc-harvest fallback in pmcp-macros when `description = "..."` attribute is omitted, per 69-PROPOSALS.md Proposal 3 (MACRO-02).
- Phase 71 planned + replanned (2026-04-17): initial 3 plans → replanned to **4 plans / 12 tasks / 4 waves** after Codex cross-AI review surfaced 2 HIGH findings. HIGH-1 resolved via new `crates/pmcp-macros-support/` sibling crate (proc-macro crates cannot export public items; Option A adopted). HIGH-2 resolved via explicit `^pmcp = ` ripple audit + concurrent `cargo-pmcp 0.6.0→0.6.1` + `mcp-tester 0.5.0→0.5.1` patch bumps per CLAUDE.md §"Version Bump Rules". Semver posture revised: **pmcp 2.3.0→2.4.0 (minor, not patch)** — rustdoc-only macro source form is additive feature. Final VERIFICATION PASSED after 2 revision iterations.
- Phase 72 added (2026-04-19): Investigate rmcp as foundations for pmcp — evaluate using rmcp for the protocol layer while repositioning pmcp + tooling as the pragmatic, batteries-included SDK for enterprise use cases. Goal is a research/decision phase to reduce protocol-spec maintenance burden and focus pmcp on higher-level DX.
- Phase 72 planned + replanned (2026-04-19): initial 3 plans → replanned to **3 plans / 12 tasks / 3 waves** after Gemini + Codex cross-AI review (72-REVIEWS.md) surfaced 3 HIGH findings. HIGH-1 (consensus, default-to-B bias) resolved by removing the default-to-B rule and replacing it with an explicit decision tree (N<3→DEFER, N=3..4→D, N≥5→highest-scoring); valid recommendation set tightened to {A, B, C1, C2, D, DEFER} — E is prohibited as an outcome (contingency-only footnote). HIGH-2 (consensus, PoC/threshold resolution gap) resolved by adding Plan 02 Task 1b which EXECUTES PoC Slice 1 on a throwaway branch `spike/72-poc-slice-1` with a 4-hour hard time-box, producing 72-POC-RESULTS.md with real T4_compile_errors + T4_loc_delta, then deletes the branch + scratch dir. HIGH-3 (Codex-only, weak inventory evidence) resolved by upgrading 72-01 Task 2 row schema from 5 to 9 columns (adding exact symbols, public API surface, owned impls/macros, serde compat risk, feature flags, downstream crates). Also: strategy matrix rows changed {A,B,C,D,E}→{A,B,C1,C2,D} + E as footnote; rubric expanded T1..T9 including T8 (historical churn, 180d git log on src/types/+src/shared/) and T9 (enterprise-feature-preservation checklist for TypedTool/workflow/mcp_apps/auth/middleware/mcp-preview/cargo-pmcp); T2 expanded to include PR merge latency + codified gh fallback URL; Plan 01 Task 0 creates 72-CONTEXT.md locking T6/T7; Plan 03 Task 1b runs an awk semantic audit that auto-downgrades to DEFER if any `### Criterion` subsection fails to cite a T-ID + inventory/matrix row. 7 deliverables total (was 5). Final VERIFICATION PASSED on first revision iteration.
- Phase 72 executed + verified (2026-04-20): 3 waves shipped; final **Recommendation: D** (Maintain pmcp as authoritative Rust MCP SDK). N=7/9 resolved thresholds (T6/T7 remain UNKNOWN). Slice 1 spike executed on throwaway branch — T4_compile_errors=0, T4_loc_delta=537, serde `params: null` round-trip FAILS against rmcp 1.5.0, downgrading inventory row 1 from EXACT to compatible-via-adapter (strongest counterargument to A/B). 72-REVIEWS.md HIGH findings all resolved in final artifacts. Verification PASSED 15/15 after 1 gap-closure iteration (C3/C5 matrix-citation regex fixes + REQUIREMENTS.md -01/-02 ledger sync). Phase 69's parity phases remain the forward path.
- Phase 73 added (2026-04-20): Typed client helpers + list_all pagination (PARITY-CLIENT-01) — implements 69-PROPOSALS.md Proposal 2 (CLIENT-02). Adds `call_tool_typed<T>` / `call_prompt_typed<T>` (typed-arg serialization) and `list_all_tools` / `list_all_prompts` / `list_all_resources` (auto-paginating on next_cursor with max-iteration safety cap) to `Client`. Additive, non-breaking; 3 plans expected; minor semver bump.

### Pending Todos

None.

### Blockers/Concerns

None.

## Session Continuity

Last session: 2026-04-20T18:09:06.181Z
Stopped at: Phase 73 context gathered
Resume: Run /gsd:add-phase to slot a 69-PROPOSALS.md entry into the roadmap, or /gsd-plan-phase for a scheduled v2.1 phase.
