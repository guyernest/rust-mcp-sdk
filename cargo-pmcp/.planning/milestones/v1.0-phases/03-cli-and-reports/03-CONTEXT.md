# Phase 3: CLI and Reports - Context

**Gathered:** 2026-02-27
**Status:** Ready for planning

<domain>
## Phase Boundary

Wire the load test engine into the `cargo pmcp loadtest` CLI subcommand and produce both a colorized terminal summary and a machine-readable JSON report. The engine, VU loop, metrics pipeline, and live display already exist from Phase 2 — this phase adds the CLI entry point, end-of-test summary rendering, and JSON report serialization.

</domain>

<decisions>
## Implementation Decisions

### CLI flag design
- Target URL is a **positional argument**: `cargo pmcp loadtest run http://localhost:3000/mcp`
- Config file auto-discovered from `.pmcp/loadtest.toml` (walks parent directories like `.git` discovery), with `--config path/to/file.toml` override
- Common overrides via CLI flags: `--vus`, `--duration`, `--iterations` override loadtest.toml values
- Less common settings (timeout, scenario steps) are config-only — no CLI flags
- JSON report written automatically to `.pmcp/reports/` after every run; `--no-report` flag to suppress
- `--no-color` flag to disable colors; auto-detect TTY for piped output

### Terminal summary layout
- **k6-style summary** with dotted-line metric rows (metric_name.........: value details)
- ASCII art header branded for cargo-pmcp, showing tool name, VU count, duration, scenario info
- Errors **grouped by classification type** (JSON-RPC, HTTP, Timeout, Connection) with counts
- Color: green for passing metrics, red for errors, yellow for warnings
- Auto-detect TTY; `--no-color` flag for CI/piped output

### JSON report schema
- **Top-level `schema_version` field**: `{ "schema_version": "1.0", ... }`
- File naming: **timestamped** in `.pmcp/reports/loadtest-YYYY-MM-DDTHH-MM-SS.json`
- Report directory auto-created on first run
- Report depth: **summary + error breakdown** — aggregate metrics (percentiles, throughput, error rate), error counts by type, no per-request data
- **Embed full resolved config** in the report (VUs, duration, scenario, timeout) for reproducibility
- Fields: schema_version, timestamp, duration, config, metrics (latency percentiles, throughput, error_rate, total_requests), errors (by classification), target_url

### Init command behavior
- `cargo pmcp loadtest init` generates `.pmcp/loadtest.toml` with sensible defaults and inline comments explaining each field
- **Schema discovery from running server**: `cargo pmcp loadtest init http://localhost:3000/mcp` — connects to server, discovers available tools/resources/prompts, auto-populates scenario with real tool names and parameters (similar to `cargo pmcp test` discovery)
- Without URL: generates template with example scenario steps (commented out)
- **Error on existing file** — refuses to overwrite `.pmcp/loadtest.toml` if it exists; `--force` flag to explicitly replace

### Claude's Discretion
- Exact ASCII art design for the summary header
- Specific metric names and dot-padding formatting
- JSON field naming conventions (camelCase vs snake_case)
- Config auto-discovery implementation details (how far up to walk)
- How discovery populates scenario weights and parameters

</decisions>

<specifics>
## Specific Ideas

- "We have in the `cargo pmcp test` an option to discover the MCP server schema and populate the scenarios file. We can have a similar option here." — The init command should mirror `cargo pmcp test`'s discovery pattern for consistency across the cargo-pmcp tooling.
- k6's summary style is the reference for the terminal output — compact, professional, familiar to load testing users.
- Reports should be self-contained: anyone reading just the JSON file should understand what test was run and what the results were.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 03-cli-and-reports*
*Context gathered: 2026-02-27*
