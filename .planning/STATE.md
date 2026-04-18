---
gsd_state_version: 1.0
milestone: v2.0
milestone_name: Protocol Modernization
status: Executing Phase 70
stopped_at: Phase 69 complete — follow-on proposals ready for ROADMAP slotting
last_updated: "2026-04-17T18:06:46.194Z"
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
**Current focus:** Phase 70 — Add Extensions typemap and peer back-channel to RequestHandlerExtra (PARITY-HANDLER-01)

## Current Position

Phase: 70 (Add Extensions typemap and peer back-channel to RequestHandlerExtra (PARITY-HANDLER-01)) — EXECUTING
Plan: 1 of 4

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

### Roadmap Evolution

- Phases 65-68 added: v2.1 rmcp Upgrades milestone (examples cleanup, macros rewrite, docs.rs pipeline, documentation polish)
- Phase 67.1 inserted after Phase 67: Code Mode Support (URGENT) — external developer support for code mode pattern (validation + execution) based on pmcp-run/built-in/shared/pmcp-code-mode SDK_DESIGN_SPEC.md
- Phase 67.2 inserted after Phase 67.1: Code Mode Derive Hardening (URGENT) — fix 3 critical derive macro issues from pmcp.run team review: policy_evaluator not called, static ValidationContext, hardcoded "graphql" code type
- Phase 69 added (initially added as duplicate Phase 68, renumbered 2026-04-16 to avoid collision with existing "Phase 68: General Documentation Polish"): rmcp parity research — scope narrowed to ergonomics-only + follow-on phase proposals; transports/examples/docs-coverage intentionally excluded to eliminate overlap with Phase 68. Deliverables: 69-RESEARCH.md (gap matrix) + 69-PROPOSALS.md (2–5 phase proposals seeded from High-severity gaps).
- Phase 70 added: Add Extensions typemap and peer back-channel to RequestHandlerExtra (PARITY-HANDLER-01) — bundles HANDLER-02 (Extensions typemap) + HANDLER-05 (peer handle) on the shared RequestHandlerExtra edit site per 69-PROPOSALS.md.
- Phase 71 added: Rustdoc fallback for #[mcp_tool] tool descriptions (PARITY-MACRO-01) — rustdoc-harvest fallback in pmcp-macros when `description = "..."` attribute is omitted, per 69-PROPOSALS.md Proposal 3 (MACRO-02).
- Phase 71 planned + replanned (2026-04-17): initial 3 plans → replanned to **4 plans / 12 tasks / 4 waves** after Codex cross-AI review surfaced 2 HIGH findings. HIGH-1 resolved via new `crates/pmcp-macros-support/` sibling crate (proc-macro crates cannot export public items; Option A adopted). HIGH-2 resolved via explicit `^pmcp = ` ripple audit + concurrent `cargo-pmcp 0.6.0→0.6.1` + `mcp-tester 0.5.0→0.5.1` patch bumps per CLAUDE.md §"Version Bump Rules". Semver posture revised: **pmcp 2.3.0→2.4.0 (minor, not patch)** — rustdoc-only macro source form is additive feature. Final VERIFICATION PASSED after 2 revision iterations.

### Pending Todos

None.

### Blockers/Concerns

None.

## Session Continuity

Last session: 2026-04-17T04:48:35Z
Stopped at: Phase 69 complete — follow-on proposals ready for ROADMAP slotting
Resume: Run /gsd:add-phase to slot a 69-PROPOSALS.md entry into the roadmap, or /gsd-plan-phase for a scheduled v2.1 phase.
