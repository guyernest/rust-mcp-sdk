# Project Retrospective

*A living document updated after each milestone. Lessons feed forward into future planning.*

## Milestone: v1.0 — MCP Load Testing

**Shipped:** 2026-02-27
**Phases:** 4 | **Plans:** 14 | **Execution time:** 1.28 hours

### What Was Built
- Complete MCP load testing CLI (`cargo pmcp loadtest run/init`)
- Concurrent VU engine with tokio task tracking and graceful shutdown
- HdrHistogram metrics with coordinated omission correction
- k6-style live terminal display and colorized summary reports
- Schema-versioned JSON reports for CI/CD pipelines (v1.1)
- Stage-driven load shaping (ramp-up/hold/ramp-down)
- Breaking point auto-detection with rolling window algorithm
- Per-tool metrics breakdown in terminal and JSON output

### What Worked
- **Research-first approach:** Pre-phase research prevented dead ends (CO correction, channel-based metrics, k6 display style)
- **Dependency-ordered phases:** Foundation → Engine → CLI → Shaping followed natural dependency graph perfectly
- **Pure function pattern for rendering:** `render_summary()` and `format_status()` as pure functions enabled deterministic unit testing without terminal
- **Dual-recorder metrics pattern:** Live display vs report recording solved the ramp-up exclusion problem cleanly
- **Gap closure plans:** Phase 4 plan 04-04 caught a wiring gap (breaking point VU counts) before shipping
- **Fast execution:** 14 plans in 1.28 hours (avg 5.6 min/plan) — tight scope per plan kept velocity high

### What Was Inefficient
- **ROADMAP.md checkbox sync:** Phase 2-4 plan checkboxes in ROADMAP.md fell out of sync (showed `[ ]` despite complete SUMMARY files) — cosmetic but confusing
- **Audit scope mismatch:** Milestone audit ran after Phase 3 but Phase 4 added 3 more requirements — had to note "out of scope" requirements that were actually in-scope
- **Plan count mismatch in Phase 4:** Roadmap originally listed 3 plans but 4 were needed (gap closure plan 04-04 added mid-execution)

### Patterns Established
- **Serde tagged enum** for TOML scenario config (`type="tools/call"`)
- **Channel-based async metrics:** mpsc for samples, watch for snapshots — never shared mutable state
- **Dual lib+bin crate layout** for fuzz/test/example imports
- **Per-VU child_token cancellation** for selective LIFO ramp-down
- **Rolling window self-calibrating detection** for breaking points (no upfront thresholds)
- **Schema versioning** with additive-only field changes (v1.0 → v1.1)

### Key Lessons
1. **Bake correctness primitives in early:** Coordinated omission correction in Phase 1 was critical — retrofitting into an existing metrics pipeline would have been painful
2. **Pure functions for testability:** Every rendering/formatting function should be data-in/string-out for deterministic testing
3. **Watch channel > polling for display:** tokio::sync::watch gives zero-contention display updates without locking metrics
4. **Property tests suppress CO correction:** Using high expected_interval_ms in property tests prevents coordinated omission noise in deterministic assertions
5. **Gap closure is cheaper than refactoring:** A focused 2-minute gap closure plan (04-04) is far cheaper than discovering the gap post-release

### Cost Observations
- Model mix: ~70% sonnet (execution), ~20% opus (planning/verification), ~10% haiku (research)
- Sessions: ~6 sessions across 1 day
- Notable: Average 5.6 min/plan execution — Phase 2 was fastest (4.3 min avg) due to well-defined Phase 1 foundation

---

## Cross-Milestone Trends

### Process Evolution

| Milestone | Execution Time | Phases | Key Change |
|-----------|---------------|--------|------------|
| v1.0 | 1.28 hours | 4 | Established research-first, dependency-ordered phases, gap closure pattern |

### Cumulative Quality

| Milestone | Plans | Requirements | Tech Debt |
|-----------|-------|-------------|-----------|
| v1.0 | 14 | 16/16 satisfied | 3 info-level items |

### Top Lessons (Verified Across Milestones)

1. Research-first prevents dead ends — confirmed by zero phase-blocking discoveries during execution
2. Pure function rendering enables deterministic testing — confirmed across display, summary, and report modules
