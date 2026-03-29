# Phase 1: Foundation - Context

**Gathered:** 2026-02-26
**Status:** Ready for planning

<domain>
## Phase Boundary

Config types, MCP-aware HTTP client with session lifecycle, and HdrHistogram metrics primitives with coordinated omission correction. This phase delivers the building blocks — no concurrent execution, no CLI integration, no reports. Those are Phases 2-3.

</domain>

<decisions>
## Implementation Decisions

### Scenario Config Shape
- Tool calls defined with explicit tool name and JSON params (no auto-generation from schema)
- Weighted mix of multiple operations: tools, resources, AND prompts — not just tool calls
- Each MCP operation type (tools/call, resources/read, prompts/get) is a first-class scenario step with its own weight
- Target server URL specified via CLI flag only (--url), NOT in config file
- Config structure: Claude's discretion on TOML section layout

### MCP Client Behavior
- On session failure mid-test: retry initialize once, then mark VU as dead. Count failure in metrics.
- Tool/resource/prompt discovery: discover once (first VU calls tools/list, resources/list, prompts/list), cache result for all other VUs
- Client identity: send clientInfo with name='cargo-pmcp-loadtest' and version during initialize
- Server notifications: Claude's discretion on handling during streamable HTTP

### Metrics Pipeline
- Every MCP request counts toward throughput (initialize, tools/call, resources/read, prompts/get) — all require server effort
- The weighted mix and summary report break down metrics per MCP operation type
- Latency measured as full round-trip (HTTP request sent → full response body received and parsed)
- Success latency and error latency tracked in separate buckets for cleaner signal
- Time resolution: milliseconds (matches how users think about latency)
- Coordinated omission correction via HdrHistogram's record_corrected_value()

### File Conventions
- Loadtest config lives in a dedicated loadtest folder (similar to scenario test folder convention)
- JSON report output location: Claude's discretion based on existing conventions

### Claude's Discretion
- TOML section structure and field naming
- Server notification handling during streamable HTTP
- JSON report output directory convention
- HdrHistogram configuration details (bucket count, value range)

</decisions>

<specifics>
## Specific Ideas

- MCP operations include tools, resources, AND prompts — the load tester must support all three operation types, not just tool calls
- The scenario config should support weighted mix across operation types (e.g., 60% tools/call, 30% resources/read, 10% prompts/get)
- Follow the existing cargo-pmcp folder convention where scenario tests have their own dedicated directory

</specifics>

<deferred>
## Deferred Ideas

- **Tasks extension support** — The PMCP SDK recently introduced experimental Tasks functionality allowing shared variables between MCP client/server and across calls with the same task ID. The load testing architecture should leave room for stateful multi-call sequences with task IDs, but this is NOT v1 scope. Capture as a future phase or v2 requirement.
- **Auto-generated params from schema** — Could auto-generate tool/resource/prompt params from server schema for quick testing without manual param definition. Defer to v2.

</deferred>

---

*Phase: 01-foundation*
*Context gathered: 2026-02-26*
