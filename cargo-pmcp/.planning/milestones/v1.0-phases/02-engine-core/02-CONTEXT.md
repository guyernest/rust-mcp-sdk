# Phase 2: Engine Core - Context

**Gathered:** 2026-02-26
**Status:** Ready for planning

<domain>
## Phase Boundary

Concurrent load test execution engine — N virtual users running against a deployed MCP server with live terminal progress. This phase delivers the VU scheduler, run controller (duration/iteration stopping), and live display. It does NOT deliver CLI integration (Phase 3), JSON reports (Phase 3), or scenario scripting beyond weighted random step selection.

</domain>

<decisions>
## Implementation Decisions

### VU Lifecycle
- On session failure mid-test: respawn with exponential backoff, max 3 attempts before permanent death
- Step selection: weighted random based on configured weights (e.g., 60% tools/call, 30% resources/read, 10% prompts/get)
- Discovery: implicit via initialize — the initialize handshake returns server capabilities (no separate tools/list, resources/list, prompts/list calls needed)
- Dead VUs counted in metrics; active VU count drops as VUs die

### Run Termination
- Support both duration-based (--duration 30s) and iteration-based (--iterations 1000) stopping
- If both specified: first limit hit wins (whichever triggers first stops the test)
- Graceful drain on stop: stop sending NEW requests, wait for in-flight to complete (up to timeout), then report
- Ctrl+C handling: first Ctrl+C triggers graceful drain + partial report, second Ctrl+C hard aborts

### Live Terminal Output
- k6-style compact display: single updating block that refreshes in-place
- Metrics shown: requests/sec, error count + error rate, active VU count, P95 latency, elapsed time
- Colored output: red for errors, green for healthy metrics (disable with --no-color or when piped)
- Refresh rate: every 2 seconds

### Ramp-up Behavior
- Default: all VUs start at once
- Optional --ramp-up flag for linear stagger (e.g., --ramp-up 30s spreads VU spawning over 30 seconds)
- Duration timer starts when first VU spawns (ramp-up is included in total test time)
- No ramp-down period — all VUs stop together, graceful drain handles in-flight requests
- Ramp-up metrics excluded from final report (warm-up data)

### Claude's Discretion
- Channel architecture for metrics aggregation (mpsc, broadcast, etc.)
- Tokio task spawning strategy for VUs
- Exponential backoff timing parameters
- indicatif vs custom terminal rendering
- How ramp-up exclusion window is tracked internally

</decisions>

<specifics>
## Specific Ideas

- Live display should feel like k6 output — compact, information-dense, updates in-place without scrolling
- The colored crate is already a dependency in the project, use it for terminal colors
- indicatif is already a dependency, leverage it for the progress display

</specifics>

<deferred>
## Deferred Ideas

- Explicit discovery calls (tools/list, resources/list, prompts/list) — initialize returns capabilities, separate discovery deferred
- Ramp-down period (gradual VU removal before stop)
- Configurable retry count (fixed at 3 for v1)
- Configurable refresh rate (fixed at 2s for v1)

</deferred>

---

*Phase: 02-engine-core*
*Context gathered: 2026-02-26*
