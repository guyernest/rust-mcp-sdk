# Phase 4: Load Shaping and Tool Metrics - Context

**Gathered:** 2026-02-26
**Status:** Ready for planning

<domain>
## Phase Boundary

Developers can shape load with ramp-up/ramp-down phases, detect server breaking points automatically, and see per-tool performance breakdowns. This builds on Phase 3's CLI and reporting foundation. Scope is limited to load shaping, breaking point detection, and per-tool metrics -- not new transport modes, auth, or distributed load generation.

</domain>

<decisions>
## Implementation Decisions

### Ramp Phase Configuration
- Composable stages via `[[stage]]` array blocks in TOML config
- Each stage defines target VU count and duration
- Linear ramp curves only (no stepped or custom curves)
- If no `[[stage]]` blocks defined, flat load (all VUs start immediately) -- backwards compatible with Phase 2 behavior
- VU teardown on ramp-down: VU finishes its current scenario iteration before exiting (no mid-request cancellation)

### Breaking Point Detection
- Always on by default -- no flag needed to enable
- Report and continue: mark breaking point in output/report but keep running the full test
- Self-calibrating via rolling window: compare recent metrics against a rolling window of earlier measurements (no prior run or first-stage baseline needed)
- Sensible built-in defaults for thresholds (e.g. error rate spike, latency degradation) -- not user-configurable in this phase
- Live terminal warning when breaking point is detected (e.g. "Breaking point detected at 35 VUs (error rate >10%)")

### Per-Tool Metrics Display
- Terminal: grouped table section after overall summary, one row per tool
- Show all tools (no truncation/limit) -- MCP servers typically have bounded tool sets
- Per-tool metrics: P50/P95/P99 latency, requests/sec, total requests, error count, error rate (same depth as overall)
- JSON report: extended detail beyond terminal -- includes full histogram, min/max/mean, error breakdown by type per tool
- Terminal summary shows overall metrics only (not per-stage breakdown); per-stage data available in JSON

### Ramp-Phase Progress Display
- Live progress line includes current phase label: `[ramp-up 2/3] VUs: XX | req/s: 120 | errors: 0`
- Breaking point detection shows live warning line when triggered
- End summary is aggregate across all stages (overall only in terminal)

### Claude's Discretion
- VU count display format during ramp (current/target vs current only)
- Rolling window size and exact threshold defaults for breaking point detection
- Per-stage metrics depth in JSON report
- Exact terminal formatting of the per-tool table

</decisions>

<specifics>
## Specific Ideas

- k6-style composable stages pattern -- users familiar with k6 will recognize the `[[stage]]` approach
- Breaking point annotation should be prominent but not interrupt the test flow
- Per-tool table should be scannable -- same visual style as the existing k6-style terminal summary from Phase 3

</specifics>

<deferred>
## Deferred Ideas

None -- discussion stayed within phase scope

</deferred>

---

*Phase: 04-load-shaping-and-tool-metrics*
*Context gathered: 2026-02-26*
