# Phase 3: CLI and Reports - Research

**Researched:** 2026-02-27
**Domain:** CLI integration, terminal rendering, JSON report serialization
**Confidence:** HIGH

## Summary

Phase 3 wires the existing load test engine (Phase 2) into the `cargo pmcp loadtest` CLI subcommand and adds two output modes: a colorized k6-style terminal summary and a machine-readable JSON report file. The existing codebase provides all the infrastructure needed -- the `LoadTestEngine`, `MetricsSnapshot`, `McpError` classification, and the `LiveDisplay` system are complete. This phase adds a clap subcommand tree, a summary renderer, a JSON report struct with `Serialize`, a config auto-discovery walker, an init command with optional server schema discovery, and file I/O for reports.

The crate already has all required dependencies: `clap` (4, derive), `colored` (3), `serde` (1, derive), `serde_json` (1), `chrono` (0.4), `toml` (1.0), and `reqwest` (0.12, json). No new dependencies are needed. The `MetricsSnapshot` struct needs `Serialize` added, a new `LoadTestReport` struct wraps the snapshot with config, timestamp, and schema version, and the terminal summary is a pure function that formats a `LoadTestResult` into a string.

**Primary recommendation:** Implement as three submodules under `src/commands/loadtest/`: `mod.rs` (clap subcommand enum), `run.rs` (CLI-to-engine bridge + summary renderer), `init.rs` (config file generator with optional discovery). The JSON report struct lives in `src/loadtest/report.rs` as it is a library-level type reusable by external tools.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Target URL is a **positional argument**: `cargo pmcp loadtest run http://localhost:3000/mcp`
- Config file auto-discovered from `.pmcp/loadtest.toml` (walks parent directories like `.git` discovery), with `--config path/to/file.toml` override
- Common overrides via CLI flags: `--vus`, `--duration`, `--iterations` override loadtest.toml values
- Less common settings (timeout, scenario steps) are config-only -- no CLI flags
- JSON report written automatically to `.pmcp/reports/` after every run; `--no-report` flag to suppress
- `--no-color` flag to disable colors; auto-detect TTY for piped output
- **k6-style summary** with dotted-line metric rows (metric_name.........: value details)
- ASCII art header branded for cargo-pmcp, showing tool name, VU count, duration, scenario info
- Errors **grouped by classification type** (JSON-RPC, HTTP, Timeout, Connection) with counts
- Color: green for passing metrics, red for errors, yellow for warnings
- Auto-detect TTY; `--no-color` flag for CI/piped output
- **Top-level `schema_version` field**: `{ "schema_version": "1.0", ... }`
- File naming: **timestamped** in `.pmcp/reports/loadtest-YYYY-MM-DDTHH-MM-SS.json`
- Report directory auto-created on first run
- Report depth: **summary + error breakdown** -- aggregate metrics (percentiles, throughput, error_rate, total_requests), error counts by type, no per-request data
- **Embed full resolved config** in the report (VUs, duration, scenario, timeout) for reproducibility
- Fields: schema_version, timestamp, duration, config, metrics (latency percentiles, throughput, error_rate, total_requests), errors (by classification), target_url
- `cargo pmcp loadtest init` generates `.pmcp/loadtest.toml` with sensible defaults and inline comments explaining each field
- **Schema discovery from running server**: `cargo pmcp loadtest init http://localhost:3000/mcp` -- connects to server, discovers available tools/resources/prompts, auto-populates scenario with real tool names and parameters
- Without URL: generates template with example scenario steps (commented out)
- **Error on existing file** -- refuses to overwrite `.pmcp/loadtest.toml` if it exists; `--force` flag to explicitly replace

### Claude's Discretion
- Exact ASCII art design for the summary header
- Specific metric names and dot-padding formatting
- JSON field naming conventions (camelCase vs snake_case)
- Config auto-discovery implementation details (how far up to walk)
- How discovery populates scenario weights and parameters

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| CONF-02 | User can run load tests via `cargo pmcp loadtest` CLI command | Clap subcommand pattern from existing `Test` and `Deploy` commands; `LoadTestEngine` API from Phase 2; tokio runtime bridge pattern used by `Landing` and `Preview` commands |
| CONF-03 | User can generate starter loadtest config via `cargo pmcp loadtest init` | Init pattern from `deploy/init.rs`; schema discovery pattern from `mcp-tester/scenario_generator.rs`; `McpClient` already supports `tools/list`, `resources/list`, `prompts/list` via OperationType enum |
| METR-04 | Load test produces colorized terminal summary report at completion | `colored` crate (v3, already dep); `LiveDisplay::format_status` pattern for color coding; `MetricsSnapshot` has all data (p50/p95/p99, error counts, operation counts) |
| METR-05 | Load test outputs JSON report file for CI/CD pipelines | `serde::Serialize` derive (already available); `serde_json::to_string_pretty`; `chrono` (0.4, already dep) for timestamps; `LoadTestResult` + `LoadTestConfig` provide all required data |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| clap | 4 (derive) | CLI argument parsing and subcommand tree | Already used throughout cargo-pmcp; derive mode matches existing pattern |
| serde | 1 (derive) | JSON report serialization | Already a dependency; `Serialize` derive is the standard Rust approach |
| serde_json | 1 | JSON report writing | Already a dependency; `to_writer_pretty` for human-inspectable JSON |
| colored | 3 | Terminal color output | Already used by `LiveDisplay` and other commands |
| chrono | 0.4 | ISO-8601 timestamp for report filenames and report body | Already a dependency in Cargo.toml |
| toml | 1.0 | Config file parsing and template generation | Already used for `LoadTestConfig::from_toml` |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| reqwest | 0.12 | Server discovery during `loadtest init` with URL | Already used by `McpClient`; reuse for discovery handshake |
| indicatif | 0.18 | Progress spinner during discovery | Already used by `LiveDisplay` |
| atty | 0.2 | TTY detection for auto-disabling color | Already a dependency; note: `std::io::IsTerminal` is also available in existing code |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `colored` | `console` (0.16, also a dep) | `colored` is what `LiveDisplay` already uses; `console` is used in schema commands. Stick with `colored` for consistency within loadtest module |
| `chrono` | `time` crate | `chrono` is already in Cargo.toml; no reason to add another time crate |
| `atty` | `std::io::IsTerminal` | `IsTerminal` is already used in `display.rs`; prefer the stdlib approach since it's already the pattern. `atty` is a legacy dep |

**Installation:**
No new dependencies required. All libraries are already in `Cargo.toml`.

## Architecture Patterns

### Recommended Project Structure
```
src/
├── commands/
│   ├── loadtest/
│   │   ├── mod.rs          # LoadtestCommand enum (Run/Init subcommands)
│   │   ├── run.rs          # Run command: config resolution, engine exec, summary, report
│   │   └── init.rs         # Init command: template generation, optional server discovery
│   └── mod.rs              # Add `pub mod loadtest;`
├── loadtest/
│   ├── report.rs           # LoadTestReport struct (Serialize) + write_report()
│   ├── summary.rs          # Terminal summary renderer (pure functions)
│   └── ...                 # (existing modules unchanged)
└── main.rs                 # Add Loadtest variant to Commands enum
```

### Pattern 1: Clap Subcommand Enum (from existing codebase)
**What:** Each major command is a clap `Subcommand` enum with an `execute()` method.
**When to use:** All CLI entry points follow this pattern.
**Example:**
```rust
// Source: existing src/commands/test/mod.rs pattern
#[derive(Debug, Subcommand)]
pub enum LoadtestCommand {
    /// Run a load test against an MCP server
    Run {
        /// Target MCP server URL
        url: String,

        /// Path to config file (default: auto-discover .pmcp/loadtest.toml)
        #[arg(long)]
        config: Option<PathBuf>,

        /// Number of virtual users (overrides config)
        #[arg(long)]
        vus: Option<u32>,

        /// Test duration in seconds (overrides config)
        #[arg(long)]
        duration: Option<u64>,

        /// Iteration limit (overrides config)
        #[arg(long)]
        iterations: Option<u64>,

        /// Disable JSON report output
        #[arg(long)]
        no_report: bool,

        /// Disable colored output
        #[arg(long)]
        no_color: bool,
    },

    /// Generate a starter loadtest config file
    Init {
        /// Optional server URL for schema discovery
        url: Option<String>,

        /// Overwrite existing config file
        #[arg(long)]
        force: bool,
    },
}
```

### Pattern 2: Tokio Runtime Bridge (from existing codebase)
**What:** CLI `main()` is synchronous. Async commands create a tokio runtime.
**When to use:** When calling async engine code from the sync CLI.
**Example:**
```rust
// Source: existing Commands::Landing pattern in main.rs
Commands::Loadtest { command } => {
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(command.execute())?;
}
```

### Pattern 3: Config Auto-Discovery (`.git`-style parent walk)
**What:** Walk parent directories from CWD looking for `.pmcp/loadtest.toml`.
**When to use:** When no `--config` flag is provided.
**Example:**
```rust
fn discover_config() -> Option<PathBuf> {
    let mut dir = std::env::current_dir().ok()?;
    loop {
        let candidate = dir.join(".pmcp").join("loadtest.toml");
        if candidate.exists() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}
```
**Termination:** Stop at filesystem root (when `dir.pop()` returns false). This matches `.git` discovery semantics. No need for a depth limit -- the worst case is a handful of directory lookups.

### Pattern 4: Config Override Merge
**What:** CLI flags override config file values.
**When to use:** For `--vus`, `--duration`, `--iterations` flags.
**Example:**
```rust
fn apply_overrides(config: &mut LoadTestConfig, vus: Option<u32>, duration: Option<u64>, iterations: Option<u64>) {
    if let Some(v) = vus {
        config.settings.virtual_users = v;
    }
    if let Some(d) = duration {
        config.settings.duration_secs = d;
    }
    // iterations is an engine parameter, not a config parameter
}
```
**Note:** The `LoadTestConfig.settings` fields are already public and mutable. Override is a direct field assignment.

### Pattern 5: Report as Serializable Struct
**What:** A dedicated struct for the JSON report that derives `Serialize`.
**When to use:** For the JSON report file output.
**Example:**
```rust
#[derive(Debug, Serialize)]
pub struct LoadTestReport {
    pub schema_version: String,
    pub timestamp: String,
    pub target_url: String,
    pub duration_secs: f64,
    pub config: ResolvedConfig,
    pub metrics: ReportMetrics,
    pub errors: HashMap<String, u64>,
}
```

### Pattern 6: Pure-Function Summary Renderer
**What:** Terminal summary as a pure function `fn render_summary(result: &LoadTestResult, config: &LoadTestConfig, url: &str) -> String`.
**When to use:** For testable, deterministic terminal output.
**Why:** `LiveDisplay::format_status` is already a static method that takes data and returns a string. The summary renderer follows the same principle -- no side effects, easy unit testing.

### Anti-Patterns to Avoid
- **Mixing I/O with formatting:** Keep the summary renderer pure. Write to stdout separately. This makes testing trivial.
- **Coupling report struct to MetricsSnapshot:** The report struct should be its own type with `Serialize`, not a direct mirror of `MetricsSnapshot`. The snapshot has `HashMap<OperationType, u64>` which doesn't serialize cleanly (non-string keys). The report struct should use `HashMap<String, u64>` with the `Display` impl of `OperationType`.
- **Blocking on file I/O in async context:** Report writing is a small file write. Use `std::fs::write` (not `tokio::fs`). It happens after the test is done, so blocking is fine.
- **Hardcoding report path:** Use a configurable base directory with a default of `.pmcp/reports/`.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Terminal color detection | Custom isatty check | `std::io::IsTerminal` (already used in display.rs) | Cross-platform, stdlib, no dependency |
| ISO-8601 timestamps | Manual formatting | `chrono::Utc::now().format(...)` | Already a dependency; handles timezone correctly |
| Config file walking | Custom directory traversal | Simple `dir.pop()` loop (see Pattern 3) | Trivial enough to not need a library, but don't overcomplicate with depth limits or symlink handling |
| JSON pretty-printing | Manual indentation | `serde_json::to_writer_pretty` | Standard, tested, correct |
| Server discovery for init | Custom HTTP + JSON-RPC | Reuse `McpClient` from `loadtest/client.rs` | Already handles initialize handshake, session ID, and JSON-RPC parsing |
| Dot-padding for metric names | Manual string padding | `format!("{:.<width$}", name, width = pad_width)` | Rust's `format!` fill character syntax handles this natively |

**Key insight:** The existing codebase provides 90% of what Phase 3 needs. The `McpClient` can do discovery (just send `tools/list`, `resources/list`, `prompts/list` after initialize). The `MetricsSnapshot` has all the data. The `colored` crate handles terminal styling. The main work is wiring, formatting, and serialization -- not new infrastructure.

## Common Pitfalls

### Pitfall 1: Non-String HashMap Keys in Serde JSON
**What goes wrong:** `MetricsSnapshot` uses `HashMap<OperationType, u64>` for `operation_counts` and `per_operation_errors`. Serde JSON cannot serialize non-string map keys by default.
**Why it happens:** `OperationType` is an enum, not a `String`. `serde_json::to_string` will fail with "key must be a string".
**How to avoid:** In the report struct, convert to `HashMap<String, u64>` using `OperationType::to_string()` (the `Display` impl already produces correct wire-format strings like `"tools/call"`).
**Warning signs:** Test failure when serializing a `LoadTestReport` with non-empty operation counts.

### Pitfall 2: Config File Not Found vs Invalid Config
**What goes wrong:** The error message is the same for "no config file found" and "config file exists but is invalid TOML".
**Why it happens:** Both are surfaced as a single error type.
**How to avoid:** Distinguish three states: (1) no config found (print helpful message about `cargo pmcp loadtest init`), (2) config found but parse error (show the parse error with file path), (3) config found and valid.
**Warning signs:** Users running `cargo pmcp loadtest run` for the first time get a confusing error.

### Pitfall 3: Report Timestamp Format in Filenames
**What goes wrong:** Using `:` in filenames (from ISO-8601 `HH:MM:SS` format) fails on Windows.
**Why it happens:** Windows forbids `:` in filenames.
**How to avoid:** The CONTEXT.md already specifies `loadtest-YYYY-MM-DDTHH-MM-SS.json` (hyphens, not colons). Use `chrono::Utc::now().format("%Y-%m-%dT%H-%M-%S")`.
**Warning signs:** File creation failure on Windows CI.

### Pitfall 4: Overwriting Existing Init Config Without --force
**What goes wrong:** `loadtest init` silently overwrites an existing config file.
**Why it happens:** Forgot to check file existence before writing.
**How to avoid:** CONTEXT.md explicitly requires error-on-existing with `--force` flag. Check `path.exists()` before writing.
**Warning signs:** User loses their customized config after running init again.

### Pitfall 5: Discovery Timeout During Init
**What goes wrong:** `cargo pmcp loadtest init http://...` hangs indefinitely if the server is down or slow.
**Why it happens:** Default reqwest timeout may be very long or infinite.
**How to avoid:** Use a reasonable timeout (10 seconds) for the discovery connection. Print a clear error message if the server is unreachable.
**Warning signs:** The init command appears to hang with no output.

### Pitfall 6: Missing `Serialize` on Nested Types
**What goes wrong:** `LoadTestReport` derives `Serialize` but references `LoadTestConfig` which only derives `Deserialize`.
**Why it happens:** Phase 1 only needed to *read* configs, not write them.
**How to avoid:** Add `Serialize` derive to `LoadTestConfig`, `Settings`, and `ScenarioStep`. These types are in `src/loadtest/config.rs`. The `serde_json::Value` type already implements `Serialize`.
**Warning signs:** Compile error when building the report serialization.

## Code Examples

### Example 1: k6-Style Dotted Metric Row
```rust
// Pure function for testability
fn format_metric_row(name: &str, value: &str, pad_width: usize) -> String {
    // Uses Rust's fill-character format: {:<.pad_width$} fills with dots
    format!("  {name:.<pad_width$}: {value}")
}

// Usage:
// format_metric_row("http_req_duration", "avg=142ms  min=10ms  med=45ms  max=500ms  p(95)=200ms  p(99)=450ms", 40)
// Produces: "  http_req_duration.......................: avg=142ms  min=10ms  ..."
```

### Example 2: ASCII Header
```rust
fn render_header(url: &str, vus: u32, duration_secs: u64, scenario_count: usize) -> String {
    format!(
        r#"
          /\      |  cargo-pmcp loadtest
         /  \     |
    /\  /    \    |  target:    {url}
   /  \/      \   |  vus:       {vus}
  /    \       \  |  duration:  {duration_secs}s
 /      \       \ |  scenarios: {scenario_count} steps
"#
    )
}
```

### Example 3: JSON Report Struct
```rust
use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Serialize)]
pub struct LoadTestReport {
    pub schema_version: String,
    pub timestamp: String,
    pub target_url: String,
    pub duration_secs: f64,
    pub config: ResolvedConfig,
    pub metrics: ReportMetrics,
    pub errors: HashMap<String, u64>,
}

#[derive(Debug, Serialize)]
pub struct ResolvedConfig {
    pub virtual_users: u32,
    pub duration_secs: u64,
    pub timeout_ms: u64,
    pub scenario: Vec<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct ReportMetrics {
    pub total_requests: u64,
    pub success_count: u64,
    pub error_count: u64,
    pub error_rate: f64,
    pub throughput_rps: f64,
    pub latency: LatencyMetrics,
}

#[derive(Debug, Serialize)]
pub struct LatencyMetrics {
    pub p50_ms: u64,
    pub p95_ms: u64,
    pub p99_ms: u64,
}
```

### Example 4: Config Discovery Walk
```rust
use std::path::{Path, PathBuf};

/// Discover `.pmcp/loadtest.toml` by walking parent directories.
///
/// Starts from `start_dir` and walks up until either the file is found
/// or the filesystem root is reached.
fn discover_config(start_dir: &Path) -> Option<PathBuf> {
    let mut dir = start_dir.to_path_buf();
    loop {
        let candidate = dir.join(".pmcp").join("loadtest.toml");
        if candidate.exists() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}
```

### Example 5: Init Template Generation (Without Discovery)
```rust
fn generate_default_template() -> String {
    r#"# Load test configuration for cargo-pmcp
# See: https://github.com/paiml/rust-mcp-sdk/tree/main/cargo-pmcp#load-testing

[settings]
# Number of concurrent virtual users
virtual_users = 10

# Test duration in seconds
duration_secs = 60

# Per-request timeout in milliseconds
timeout_ms = 5000

# Expected interval between requests (ms) for coordinated omission correction
# expected_interval_ms = 100

# Define your scenario steps below. Each step has a type, weight, and parameters.
# Weights determine the relative frequency of each operation.

[[scenario]]
type = "tools/call"
weight = 70
tool = "your-tool-name"
# arguments = { key = "value" }

# [[scenario]]
# type = "resources/read"
# weight = 20
# uri = "file:///your/resource/uri"

# [[scenario]]
# type = "prompts/get"
# weight = 10
# prompt = "your-prompt-name"
# arguments = { key = "value" }
"#.to_string()
}
```

### Example 6: Discovery-Based Init (With Server URL)
```rust
// Reuse McpClient for discovery
async fn discover_server_schema(url: &str) -> Result<DiscoveredSchema, anyhow::Error> {
    let http = reqwest::Client::new();
    let timeout = Duration::from_secs(10);
    let mut client = McpClient::new(http, url.to_owned(), timeout);

    // Initialize session
    client.initialize().await.map_err(|e| anyhow::anyhow!("Failed to connect: {e}"))?;

    // Discover tools via tools/list
    let tools_body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": client.next_id(),
        "method": "tools/list",
        "params": {}
    });
    // ... send and parse response

    Ok(DiscoveredSchema { tools, resources, prompts })
}
```
**Note:** `McpClient` currently has `call_tool`, `read_resource`, `get_prompt` but NOT `list_tools`, `list_resources`, `list_prompts`. These methods need to be added for discovery, or the discovery can construct raw JSON-RPC requests using the existing `send_request` infrastructure. The latter approach avoids modifying the existing client API and keeps discovery self-contained in the init command.

However, since `send_request` is private on `McpClient`, the cleanest approach is to either: (a) make `send_request` pub(crate), or (b) add discovery methods to `McpClient`. Option (b) is more idiomatic.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Custom ANSI escape codes | `colored` crate (v3) | Standard practice | Cross-platform color support including Windows |
| `atty` crate for TTY detection | `std::io::IsTerminal` (stable since Rust 1.70) | Rust 1.70 (2023) | No dependency needed; already used in `display.rs` |
| `chrono` for all time needs | `std::time` for durations + `chrono` for wall-clock | Current | Use `Duration` for elapsed, `chrono::Utc::now()` only for timestamps |
| Manual JSON construction | `serde_json::to_writer_pretty` with derived `Serialize` | Standard practice | Type-safe, never produces invalid JSON |

**Deprecated/outdated:**
- `atty` crate: Superseded by `std::io::IsTerminal`. Still in Cargo.toml but the codebase already uses the stdlib approach in `display.rs`.

## Open Questions

1. **JSON field naming convention: camelCase vs snake_case**
   - What we know: CONTEXT.md lists this as Claude's discretion. The MCP protocol itself uses `camelCase`. The Rust ecosystem conventionally uses `snake_case` in JSON serialization.
   - Recommendation: Use `snake_case` for the JSON report. Rationale: (a) `serde` defaults to snake_case, requiring no extra attributes; (b) the report is a cargo-pmcp artifact, not an MCP protocol message; (c) CI/CD pipelines parsing JSON with `jq` work equally well with either convention. If needed later, `#[serde(rename_all = "camelCase")]` can be added.

2. **McpClient method addition vs raw JSON-RPC for discovery**
   - What we know: `McpClient` has methods for `tools/call`, `resources/read`, `prompts/get` but not for `tools/list`, `resources/list`, `prompts/list`. The `send_request` method is private.
   - Recommendation: Add `list_tools()`, `list_resources()`, `list_prompts()` methods to `McpClient`. These are simple JSON-RPC calls that follow the exact same pattern as existing methods. This keeps the client API complete and the init command clean.

3. **Error count tracking in report by category**
   - What we know: `MetricsSnapshot` has `per_operation_errors: HashMap<OperationType, u64>` (errors by operation type) but NOT errors by error category (jsonrpc/http/timeout/connection). The error category is available on individual `McpError` instances but not aggregated by the `MetricsRecorder`.
   - Recommendation: The CONTEXT.md specifies "errors grouped by classification type (JSON-RPC, HTTP, Timeout, Connection) with counts". This requires extending `MetricsRecorder` to track error counts by category. Add a `HashMap<String, u64>` field `error_category_counts` to `MetricsRecorder` and `MetricsSnapshot`, incrementing in `record()` when the sample is an error using `McpError::error_category()`.

## Sources

### Primary (HIGH confidence)
- Existing codebase: `src/loadtest/` modules (config.rs, engine.rs, metrics.rs, display.rs, client.rs, vu.rs, error.rs)
- Existing codebase: `src/commands/test/mod.rs` (clap subcommand pattern)
- Existing codebase: `src/commands/test/generate.rs` (schema discovery via mcp-tester)
- Existing codebase: `src/main.rs` (command routing, tokio runtime bridge)
- Existing codebase: `crates/mcp-tester/src/scenario_generator.rs` (discovery handshake pattern)

### Secondary (MEDIUM confidence)
- [k6 end-of-test summary documentation](https://grafana.com/docs/k6/latest/results-output/end-of-test/) - Terminal summary format reference
- [k6 custom summary documentation](https://grafana.com/docs/k6/latest/results-output/end-of-test/custom-summary/) - JSON export format reference
- [k6 terminal output discussion](https://github.com/loadimpact/k6/issues/1319) - Dotted-line format details

### Tertiary (LOW confidence)
None.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - All libraries already in Cargo.toml; patterns verified in existing codebase
- Architecture: HIGH - Follows established patterns (clap subcommands, tokio bridge, pure formatters) already used throughout cargo-pmcp
- Pitfalls: HIGH - Identified from direct code inspection (non-string HashMap keys, missing Serialize derives, private send_request)

**Research date:** 2026-02-27
**Valid until:** 2026-03-27 (stable domain -- Rust CLI patterns change slowly)
