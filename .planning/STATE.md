---
gsd_state_version: 1.0
milestone: v2.4
milestone_name: Agents & Teams — SDK Extraction
status: completed
stopped_at: Phase 108 context gathered
last_updated: "2026-07-18T18:12:47.830Z"
last_activity: 2026-07-18 -- Phase 108 marked complete
progress:
  total_phases: 63
  completed_phases: 3
  total_plans: 12
  completed_plans: 12
  percent: 5
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-07-17) · .planning/ROADMAP.md (v2.4 milestone, Phases 106-111) · docs/design/agents-teams-sdk-extraction-plan.md (approved)

**Core value:** The PMCP SDK is the reference implementation for agents-as-MCP-clients and agent teams — one open agent loop and one portable package format that run identically on a laptop, any deploy target, and pmcp.run (contracts + reference implementations in the SDK; operation + scale on the platform).
**Current focus:** Phase 108 — pmcp-agent-loop-crate

## Current Position

Phase: 108 — COMPLETE
Plan: 1 of 6
Status: Phase 108 complete
Last activity: 2026-07-18 -- Phase 108 marked complete

## v2.4 Phase Plan (6 phases, 31 requirements)

| Phase | Name | Goal | Reqs | Depends on |
|-------|------|------|------|------------|
| 106 | Client Host Surface | Client hosts server→client sampling/elicitation/roots + HITL hook; legacy inverted sampling documented as "LLM-server pattern" (design Phase A) | HOST-01..06 (6) | none (parallel with 107) |
| 107 | Contracts & Package Format | `pmcp-package` adopted into repo + published 0.1.0 (wire-frozen); team tool contracts as provable-contracts YAML (design Phase B) | PKG-01..03 (3) | none (parallel with 106) |
| 108 | `pmcp-agent` Loop Crate | Pure agent loop between effect seams, 3 CompletionSources, agent-as-server adapter, tasks-aware ToolInvoker, AgentPackage-configured (design Phase C) | AGNT-01..09 (9) | 106 + 107 |
| 109 | Team Reference Servers | `pmcp-team-servers` (feature-flagged) with dev-grade team-fs/approval-mcp/mem-mcp/team-mcp + conformance vs PKG-03 (design Phase D) | TEAM-01..06 (6) | 108 (+107 fixtures) |
| 110 | cargo-pmcp Agent & Team Verbs | `agent new`/`agent dev`, `team dev`, `package capture\|show` with version-pin tripwires (design Phase E) | CLI-01..04 (4) | 107, 108, 109 |
| 111 | Docs in Three Shapes + Examples | pmcp-book chapters + runnable examples + README/course, cargo-pmcp-first (design Phase F) | DOCS-01..03 (3) | 106-110 |

**Execution order:** 106 ∥ 107 → 108 → 109 → 110 → 111. Phases 106 and 107 are independent and may run in parallel. Contract-first (house rule): Phase 107 contracts precede the Phase 108/109 implementations. Phase 106 is small, independently shippable (pmcp minor bump), and unblocks Phase 108's `SamplingSource`.

**Publish-order impact (design §5):** new entries `pmcp-package` (leaf, before cargo-pmcp), `pmcp-agent` (after `pmcp`), `pmcp-team-servers` (after `pmcp-agent`); cargo-pmcp moves after all three. New version-pin tripwires: cargo-pmcp ↔ `pmcp-package`, agent scaffold ↔ `pmcp-agent`. All new crates 0.x/experimental; `pmcp` core changes (Phase 106) are additive minor bumps.

## Accumulated Context

### Roadmap Evolution

- v2.4 milestone roadmap created (2026-07-17): 6 phases (106-111) map 1:1 to the approved design doc's §4 phases A-F along the compliance→contracts→agent→teams→CLI→docs spine; all 31 v1 requirements mapped (100% coverage, no orphans).

### Decisions

Decisions are logged in PROJECT.md Key Decisions table. Decisions framing this milestone (from design §6 recommendations, approved):

- Boundary razor: contracts + reference implementations in the open SDK; operation + scale stay on pmcp.run.
- Crate name `pmcp-agent` (not `pmcp-agents`); one `pmcp-team-servers` crate with per-server feature flags (not four crates).
- `pmcp-package` adopted into this repo first, published 0.1.0 from here (source: `~/Development/mcp/sdk/pmcp-run/crates/pmcp-package` — import + publish-hygiene, not a rewrite); caret `"0.1"` dep, not `=0.1.0`.
- Legacy inverted sampling kept and documented as the "LLM-server pattern" (no breaking change / no deprecation).
- Sampling-first, not sampling-only: `SamplingSource` (zero-dep) first-class; `OpenAiCompatSource` + `AnthropicSource` feature-gated; three sources maximum, the trait is the extension point.
- The trait seams double as durability seams — the loop stays pure/replay-safe (mirrors the 2.13.0 `poll_decision` non-determinism-inside-the-step design).
- Team-tool contracts as provable-contracts YAML (house convention), namespaced provisional PMCP extensions.

### Pending Todos

None yet.

### Blockers/Concerns

None yet. (Research flags per phase to be surfaced during `/gsd:plan-phase`.)

## Deferred Items

Items deferred by design for this milestone (design §7 / REQUIREMENTS v2):

| Category | Item | Status | Deferred At |
|----------|------|--------|-------------|
| Deploy | AgentCore deploy adapter (`cargo pmcp deploy` target) | Deferred (DEFER-01) | v2.4 scope |
| Sources | Additional `CompletionSource` impls beyond the three shipped | Deferred (DEFER-02) | v2.4 scope |
| Memory | Scaled team-memory backends (embeddings/vector stores) in the open SDK | Deferred (DEFER-03) | v2.4 scope |
| Platform | pmcp.run adopting the loop/traits (companion §8 note) | Deferred (DEFER-04) | not SDK work |

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
| v2.3 | Excel-as-Configuration MCP Servers + Tasks DX arc | 91-96, 101-105 | 2026-07-05 |

## Session Continuity

Last session: 2026-07-18T04:26:30.367Z
Stopped at: Phase 108 context gathered
Resume file: .planning/phases/108-pmcp-agent-loop-crate/108-CONTEXT.md

## Performance Metrics

| Phase | Plan | Duration | Notes |
|-------|------|----------|-------|
| (v2.4 phases not yet planned) | — | — | — |
