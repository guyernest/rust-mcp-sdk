---
gsd_state_version: 1.0
milestone: v2.2
milestone_name: Configuration-Only MCP Servers
status: executing
stopped_at: Phase 82 context gathered
last_updated: "2026-05-18T01:25:55.232Z"
last_activity: 2026-05-18 -- Phase 82 planning complete
progress:
  total_phases: 44
  completed_phases: 33
  total_plans: 137
  completed_plans: 134
  percent: 75
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-05-17)

**Core value:** Enterprise developers build production-grade SQL MCP servers from configuration + schema files alone — no Rust required — while preserving PMCP's security, tools/resources/prompts/tasks/skills standards and pmcp.run hosting integration.
**Current focus:** Milestone v2.2 — Phase 82 (Builder DX Prerequisites) next

## Current Position

Phase: Not started — next is Phase 82 (Builder DX Prerequisites)
Plan: —
Status: Ready to execute
Last activity: 2026-05-18 -- Phase 82 planning complete

**Carryover from v2.1:** Phase 81 (update-pmcp-book-and-pmcp-course-with-v2-advanced-topics-cod) was executing at v2.1 close; will be tracked separately and folded into v2.1 completion. Operator follow-ups deferred from Phase 75 Wave 5 still pending: (a) merge Phase 75 Wave 5 + 75.5 to paiml/rust-mcp-sdk:main; (b) post-merge run `gh workflow run quality-badges.yml -R paiml/rust-mcp-sdk` and append observation to `.planning/phases/75-fix-pmat-issues/75-05-GATE-VERIFICATION.md` "## Badge flip observation" section.

## v2.2 Phase Plan (8 phases, 49 requirements)

| Phase | Goal | Reqs | Critical-path |
|-------|------|------|---------------|
| 82 | Builder DX Prerequisites — `tool_arc` / `prompt_arc` on public builder + in-process driver | 3 | yes — blocks 83+ |
| 83 | Toolkit Core Lift (`pmcp-server-toolkit`) — public crates.io crate, ToolInfo synthesis, code-mode wiring | 12 | yes — anchor, blocks 84/87/88 |
| 84 | SQL Connectors — Postgres/MySQL/Athena crates + SQLite feature flag, pure-Rust drivers | 10 | yes — anchor, blocks 85/86/88 |
| 85 | Shape A Pure-Config Binary + Reference Parity — `pmcp-sql-server`, open-images reproduction | 3 | yes — proves the lift |
| 86 | Shapes B/C/D — Scaffold + ≤15-line Example + Deploy | 5 | branching from 85 |
| 87 | Type 2 Authoring Skills Server (`pmcp-config-helper`) | 8 | branching from 83 |
| 88 | Dogfood — `crates/pmcp-server` on toolkit | 2 | branching from 83+84 |
| 89 | Documentation, Migration Guide & Examples Index | 6 | finalizes milestone |

**Execution order:** 82 → 83 → 84 → 85 → (86 ‖ 87) → 88 → 89

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

- Total plans completed: 114 (v1.0: 9, v1.1: 10, v1.2: 9, v1.3: 12, v1.4: 10, v1.5: 6, v1.6: 5, v1.7: 4, v2.0: 11)
- Total phases completed: 29

## Accumulated Context

### Decisions

See PROJECT.md Key Decisions table for full history.

v2.2 decisions (this session):

- **Phase numbering:** Continue from v2.1's last phase (81). v2.2 starts at Phase 82, no `--reset-phase-numbers` flag.
- **8 phases derived** from 49 requirements respecting spike-validated dependency order: BLDR → TKIT → CONN → SHAP-A → (SHAP-B/C/D ‖ SKLL) → DOGF → DOCS.
- **Anchor phases:** Phase 83 (TKIT, 12 reqs) and Phase 84 (CONN, 10 reqs) are the two intentionally-large "lift the proto-SDK" phases. All other phases sit at 2–8 reqs per the user's medium-sized-phase guidance.
- **TEST-* requirements distributed**, not collected — TEST-01 + TEST-07 → 84 (connector tests + fuzz), TEST-02 + TEST-03 → 83 (toolkit unit/property/doctest), TEST-04 → 87 (dual-surface + §9), TEST-05 + TEST-06 → 86 (scaffold-to-run + deploy integration). Per CLAUDE.md ALWAYS requirements each phase carries its own test types.
- **Phase 85 = Shape A + REF parity together** — reproducing open-images via Shape A IS the parity check for REF-02; separating them would force a synthetic intermediate.
- **Phase 87 (SKLL) is parallelisable with Phase 86 (B/C/D)** after Phase 83 lands — neither depends on the other.
- **Phase 89 absorbs REF-03** (migration recipe) into DOCS-01 (book chapter) rather than as a standalone phase — same audience, same surface, same artifact.
- **Hard-encoded invariants in success criteria:** REF-01 superset (no renames) called out in P83 SC-2 + P85 SC-2; pure-Rust Lambda + no-Docker in P84 SC-4; dual-mode intentional in P83 SC-2/SC-3; dual-surface byte-equality in P87 SC-2; SEP-2640 §9 list-exclusion in P87 SC-3.

Inherited from v2.1 (see PROJECT.md + prior Decisions log):

- 4 phases derived from 5 requirement categories following research-recommended dependency order: examples+protocol -> macros -> docs.rs pipeline -> polish
- EXMP and PROT combined into Phase 65 (both are credibility fixes, no dependency between them, co-deliverable)
- Phase ordering follows the docs.rs build pipeline dependency: content accuracy first, then rendering pipeline, then polish
- No new runtime dependencies for this milestone -- all fixes are config, content, and attribute changes
- [Phase 65]: All 17 orphan examples compile successfully -- registered all with import-derived feature flags (no deletions needed)
- [Phase 65]: examples/README.md replaced with PMCP example index — 63 examples categorized by Role/Capability/Complexity + migration reference

### Roadmap Evolution

- **2026-05-17 — v2.2 ROADMAP block written.** 8 phases (82–89) covering BLDR (P82) → TKIT (P83) → CONN (P84) → SHAP-A + REF parity (P85) → SHAP-B/C/D (P86) ‖ SKLL (P87) → DOGF (P88) → DOCS+REF-03 (P89). 49/49 v2.2 requirements mapped 1:1 with no orphans. REQUIREMENTS.md traceability table updated to replace TBD (v2.2) entries with concrete phase numbers.
- See PROJECT.md / prior STATE.md `Roadmap Evolution` history for v1.0–v2.1 evolution log (long; not duplicated here).

### Pending Todos

- **OPERATOR DECISION REQUIRED before Wave 1**: D-10-B scope-expansion. Pick one of (1) split Phase 75 into 75 + 75.5 — recommended; (2) accept additional refactor effort in single phase; (3) raise cog threshold (rejected per CONTEXT.md). See `.planning/phases/75-fix-pmat-issues/75-00-SUMMARY.md` "SCOPE EXPANSION DETECTED" section.

### Blockers/Concerns

- Wave 5 must patch `quality-badges.yml` per D-11-B — without that, no amount of complexity reduction flips the badge.

### Quick Tasks Completed

| # | Description | Date | Commit | Status | Directory |
|---|-------------|------|--------|--------|-----------|
| 260516-b2p | AuthProvider::on_unauthorized + transport retry-once + MSRV 1.91 + pmcp 2.8.0 ripple | 2026-05-16 | aba393aa | Shipped (PR [#256](https://github.com/paiml/rust-mcp-sdk/pull/256)) | [260516-b2p-add-authprovider-on-unauthorized-hook-tr](./quick/260516-b2p-add-authprovider-on-unauthorized-hook-tr/) |
| 260517-hi5 | Extract `x-pmcp-claim-custom-*` headers in `extract_auth_from_proxy_headers` (Cognito `custom:*` attribute forwarding) | 2026-05-17 | bbc019ba | Done | [260517-hi5-extract-x-pmcp-claim-custom-headers-in-e](./quick/260517-hi5-extract-x-pmcp-claim-custom-headers-in-e/) |

### Last Activity

**2026-05-17** — v2.2 ROADMAP defined. Eight phases (82–89) cover 49 requirements (BLDR ×3 + TKIT ×10 + CONN ×8 + SHAP ×4 + SKLL ×7 + REF ×3 + DOGF ×2 + DOCS ×5 + TEST ×7). Coverage 49/49 with no orphans and no duplicates. Critical path runs Phase 82 (Builder DX) → Phase 83 (TKIT anchor, 12 reqs) → Phase 84 (CONN anchor, 10 reqs) → Phase 85 (Shape A + REF parity) → branching to Phase 86 (Shapes B/C/D) ‖ Phase 87 (Type 2 authoring skills) → Phase 88 (dogfood) → Phase 89 (docs + migration). All five critical invariants encoded as named success-criteria items: REF-01 superset, pure-Rust Lambda + no-Docker, dual-mode intentional, dual-surface byte-equality, SEP-2640 §9 list-exclusion. Next: `/gsd-plan-phase 82`.

**2026-05-17** — Completed quick task 260517-hi5: extract `x-pmcp-claim-custom-*` headers in `extract_auth_from_proxy_headers` so Cognito `custom:*` attributes forwarded by pmcp.run mcp-proxy surface via `AuthContext.claims["custom:<snake>"]`. Additive change (no public-API break); 4 unit tests verbatim from spec; `docs/proxy-contract.md` created; CHANGELOG `[2.8.1]` entry. `make quality-gate` green end-to-end on clean worktree. Bumped to pmcp 2.8.1; PR #257 opened to upstream/main.

**2026-05-16** — Shipped v2.8.0 bundle release via PR [#256](https://github.com/paiml/rust-mcp-sdk/pull/256):

- Quick task 260516-b2p (AuthProvider::on_unauthorized + transport retry-once)
- Phase 80 (SEP-2640 Agent Skills)
- Phase 81 (pmcp-book + pmcp-course v2 topic updates)
- MSRV bump 1.83 → 1.91
- Workspace dep ripple to pmcp 2.8.0 (publishes 7 crates on `v2.8.0` tag)

## Session Continuity

Last session: 2026-05-18T00:02:18.553Z
Stopped at: Phase 82 context gathered
Resume: Next is `/gsd-plan-phase 82` to break Phase 82 (Builder DX Prerequisites) into plans. Phase 82 is the unblocker for every subsequent v2.2 phase that uses `tool_arc` / `prompt_arc` — without it, every config-driven toolkit author writes a 20-line delegating wrapper shim (the same DX paper-cut spike 004 hit). After 82, the critical path is 83 (TKIT anchor) → 84 (CONN anchor) → 85 (Shape A + REF parity). Phases 86 and 87 can run in parallel once 83 lands.
