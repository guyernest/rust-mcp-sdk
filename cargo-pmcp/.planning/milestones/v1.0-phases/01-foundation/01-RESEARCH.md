# Phase 1: Foundation - Research

**Researched:** 2026-02-26
**Domain:** TOML config parsing, MCP HTTP client with session lifecycle, HdrHistogram metrics with coordinated omission correction
**Confidence:** HIGH

## Summary

Phase 1 delivers three independent building blocks: typed TOML config structs for load test scenarios, a stateful MCP HTTP client that performs the initialize handshake and manages sessions, and an HdrHistogram-based metrics pipeline with coordinated omission correction. These components have no dependencies on each other and can be built in parallel.

The load test client should NOT reuse the parent SDK's `Client<StreamableHttpTransport>` directly. That client is designed for long-lived session management with SSE streaming, middleware chains, and protocol abstractions that add latency overhead inappropriate for a load testing hot path. Instead, the load test client should be a purpose-built thin wrapper around `reqwest::Client` that constructs JSON-RPC requests directly using the SDK's existing type definitions (`JSONRPCRequest`, `InitializeRequest`, `CallToolRequest`, etc.) and sends them as HTTP POSTs. This gives full control over timing, per-request timeouts, and error classification without fighting the transport abstraction.

**Primary recommendation:** Build a thin reqwest-based MCP client using the SDK's protocol types for serialization, paired with an HdrHistogram metrics recorder that uses `record_correct()` from the start, and TOML config structs that support weighted mixes of tools/call, resources/read, and prompts/get operations.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Tool calls defined with explicit tool name and JSON params (no auto-generation from schema)
- Weighted mix of multiple operations: tools, resources, AND prompts -- not just tool calls
- Each MCP operation type (tools/call, resources/read, prompts/get) is a first-class scenario step with its own weight
- Target server URL specified via CLI flag only (--url), NOT in config file
- On session failure mid-test: retry initialize once, then mark VU as dead. Count failure in metrics.
- Tool/resource/prompt discovery: discover once (first VU calls tools/list, resources/list, prompts/list), cache result for all other VUs
- Client identity: send clientInfo with name='cargo-pmcp-loadtest' and version during initialize
- Every MCP request counts toward throughput (initialize, tools/call, resources/read, prompts/get)
- The weighted mix and summary report break down metrics per MCP operation type
- Latency measured as full round-trip (HTTP request sent -> full response body received and parsed)
- Success latency and error latency tracked in separate buckets for cleaner signal
- Time resolution: milliseconds (matches how users think about latency)
- Coordinated omission correction via HdrHistogram's record_correct() (at-recording correction)
- Loadtest config lives in a dedicated loadtest folder (similar to scenario test folder convention)

### Claude's Discretion
- TOML section structure and field naming
- Server notification handling during streamable HTTP
- JSON report output directory convention
- HdrHistogram configuration details (bucket count, value range)
- Config structure: TOML section layout

### Deferred Ideas (OUT OF SCOPE)
- Tasks extension support (shared variables, task IDs across calls) -- future phase or v2
- Auto-generated params from schema -- v2
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| CONF-01 | User can define load test scenarios in TOML config file | TOML config structs with serde derive; supports weighted scenario steps for tools/call, resources/read, prompts/get. Uses `toml = "0.9"` already in Cargo.toml. |
| LOAD-03 | User can configure per-request timeout | reqwest `RequestBuilder::timeout()` enables per-request timeout that overrides client-level default. Stored as `timeout_ms` in TOML config, converted to `Duration` at parse time. |
| MCP-01 | Each virtual user performs its own MCP initialize handshake and maintains its session | Thin reqwest client sends `InitializeRequest` as JSON-RPC POST, reads `mcp-session-id` from response header, stores it, and attaches it on subsequent requests. Uses SDK types for serialization. |
| MCP-03 | JSON-RPC errors are classified separately from HTTP errors | Error enum with variants: `JsonRpc { code, message }` (from response payload), `Http { status }` (4xx/5xx), `Timeout`, `Connection`. JSON-RPC codes map to SDK's `ErrorCode` constants (-32601 method not found, -32602 invalid params, etc.). |
| METR-01 | Load test reports latency percentiles (P50/P95/P99) using HdrHistogram | `hdrhistogram = "7.5"` crate. Use `Histogram::<u64>::new(3)` for auto-resizing with 3 significant figures. `record_correct(value_ms, expected_interval_ms)` for coordinated omission correction at recording time. `value_at_quantile(0.50/0.95/0.99)` for percentile extraction. |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `reqwest` | `0.12` (existing in Cargo.toml) | HTTP client for MCP requests | Already a project dependency. Provides per-request timeout via `RequestBuilder::timeout()`, connection pooling via `pool_max_idle_per_host()`, and `tcp_nodelay(true)` (on by default). |
| `hdrhistogram` | `7.5.4` (new) | Latency percentile computation with coordinated omission correction | The Rust port of Gil Tene's HdrHistogram. Used by linkerd-proxy, vector.dev, and every serious Rust latency measurement tool. Provides `record_correct()` for at-recording coordinated omission correction. |
| `serde` + `serde_json` | `1.x` (existing) | JSON-RPC serialization/deserialization | Already in project. Used to serialize `JSONRPCRequest` and deserialize `JSONRPCResponse` types from the parent SDK. |
| `toml` | `0.9` (existing) | TOML config file parsing | Already in Cargo.toml. Derives `Deserialize` on config structs for zero-boilerplate parsing. |
| `tokio` | `1.x` (existing) | Async runtime, channels | Already in project with `full` feature. Provides `mpsc` channel for metrics pipeline (VU -> aggregator), `Instant` for timing. |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `thiserror` | `2.x` (existing) | Error type definitions | Define `LoadTestError` enum with variants for JSON-RPC, HTTP, timeout, and connection errors. |
| `url` | `2.5` (in parent SDK) | URL parsing and validation | Parse and validate the `--url` CLI flag. May need to add to cargo-pmcp Cargo.toml if not already transitive. |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| reqwest (thin client) | Parent SDK `Client<StreamableHttpTransport>` | SDK client adds SSE streaming, middleware chains, protocol negotiation overhead. Too heavy for a load testing hot path where microsecond overhead matters. |
| reqwest (thin client) | hyper directly | Lower level but reqwest's connection pool management and per-request timeout are exactly what we need. No benefit to going lower. |
| hdrhistogram | tdigest crate | t-digest is approximate; HdrHistogram is exact within configured precision. User decision locked HdrHistogram. |
| At-recording correction (`record_correct`) | Post-recording (`clone_correct`) | Post-recording requires keeping raw histogram and correcting at report time. At-recording is simpler and matches the "bake it in from day one" principle. User decision locked at-recording correction. |

**Installation:**
```toml
# Add to cargo-pmcp/Cargo.toml [dependencies]
hdrhistogram = "7.5"
```

## Architecture Patterns

### Recommended Project Structure
```
cargo-pmcp/src/
├── loadtest/              # Load test engine (library code, no CLI knowledge)
│   ├── mod.rs             # Public API: re-exports
│   ├── config.rs          # TOML config types (Deserialize)
│   ├── client.rs          # MCP-aware HTTP client (reqwest wrapper)
│   ├── error.rs           # LoadTestError enum with classification
│   └── metrics.rs         # HdrHistogram wrapper, RequestSample, MetricsRecorder
├── commands/
│   └── loadtest/          # CLI integration (Phase 3, not this phase)
│       └── mod.rs
└── main.rs                # Add `mod loadtest;` declaration
```

### Pattern 1: Thin MCP Client Over Reqwest
**What:** A purpose-built HTTP client that serializes MCP requests using the parent SDK's type definitions and sends them as JSON-RPC over HTTP POST. It manages session state (mcp-session-id header) and classifies errors into JSON-RPC vs HTTP vs timeout categories.
**When to use:** Every MCP request in the load test.
**Example:**
```rust
// Source: Derived from parent SDK's streamable_http.rs and client/mod.rs patterns
use reqwest::Client;
use serde_json::json;
use std::time::{Duration, Instant};

/// MCP-aware HTTP client for load testing.
/// Each virtual user owns one instance with its own session.
pub struct McpClient {
    http: Client,
    base_url: String,
    session_id: Option<String>,
    request_timeout: Duration,
    next_request_id: u64,
}

impl McpClient {
    pub fn new(base_url: String, timeout: Duration) -> Self {
        let http = Client::builder()
            .pool_max_idle_per_host(10)
            .tcp_nodelay(true)
            .connect_timeout(Duration::from_secs(5))
            .build()
            .expect("Failed to build HTTP client");

        Self {
            http,
            base_url,
            session_id: None,
            request_timeout: timeout,
            next_request_id: 1,
        }
    }

    pub async fn initialize(&mut self) -> Result<InitializeResult, McpError> {
        let body = json!({
            "jsonrpc": "2.0",
            "id": self.next_id(),
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": {
                    "name": "cargo-pmcp-loadtest",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }
        });

        let start = Instant::now();
        let response = self.http.post(&self.base_url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream")
            .timeout(self.request_timeout)
            .json(&body)
            .send()
            .await;
        let elapsed = start.elapsed();

        // Extract mcp-session-id from response headers
        // Classify errors: HTTP vs timeout vs connection
        // Parse JSON-RPC response, classify JSON-RPC errors
        // ...
    }

    fn next_id(&mut self) -> u64 {
        let id = self.next_request_id;
        self.next_request_id += 1;
        id
    }
}
```

### Pattern 2: Error Classification Enum
**What:** A single error type that classifies failures into distinct categories that the metrics pipeline can count separately.
**When to use:** Every error path in the MCP client.
**Example:**
```rust
// Source: Derived from parent SDK's error/mod.rs ErrorCode constants
use thiserror::Error;

#[derive(Debug, Error, Clone)]
pub enum McpError {
    /// JSON-RPC protocol error from the server
    #[error("JSON-RPC error {code}: {message}")]
    JsonRpc {
        code: i32,
        message: String,
    },

    /// HTTP transport error (4xx, 5xx)
    #[error("HTTP {status}: {body}")]
    Http {
        status: u16,
        body: String,
    },

    /// Request timed out
    #[error("Request timed out after {duration:?}")]
    Timeout {
        duration: Duration,
    },

    /// Connection failed (DNS, TCP, TLS)
    #[error("Connection error: {message}")]
    Connection {
        message: String,
    },
}

impl McpError {
    /// Classify a reqwest error into the appropriate variant
    pub fn from_reqwest(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            Self::Timeout { duration: Duration::default() }
        } else if err.is_connect() {
            Self::Connection { message: err.to_string() }
        } else if let Some(status) = err.status() {
            Self::Http { status: status.as_u16(), body: err.to_string() }
        } else {
            Self::Connection { message: err.to_string() }
        }
    }

    /// Classify from a JSON-RPC error in the response body
    pub fn from_jsonrpc(code: i32, message: String) -> Self {
        Self::JsonRpc { code, message }
    }

    /// Standard JSON-RPC error codes for classification
    pub fn is_method_not_found(&self) -> bool {
        matches!(self, Self::JsonRpc { code: -32601, .. })
    }

    pub fn is_invalid_params(&self) -> bool {
        matches!(self, Self::JsonRpc { code: -32602, .. })
    }
}
```

### Pattern 3: HdrHistogram Metrics Recorder
**What:** A metrics type that wraps HdrHistogram with separate success/error buckets and coordinated omission correction baked in.
**When to use:** Recording every request sample.
**Example:**
```rust
// Source: hdrhistogram docs (https://docs.rs/hdrhistogram/7.5.4/hdrhistogram/struct.Histogram.html)
use hdrhistogram::Histogram;
use std::time::Duration;

/// A single request measurement sample
pub struct RequestSample {
    pub operation: OperationType,
    pub duration: Duration,
    pub result: Result<(), McpError>,
    pub timestamp: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OperationType {
    Initialize,
    ToolsCall,
    ResourcesRead,
    PromptsGet,
    ToolsList,
    ResourcesList,
    PromptsList,
}

/// Metrics recorder with coordinated omission correction
pub struct MetricsRecorder {
    success_histogram: Histogram<u64>,
    error_histogram: Histogram<u64>,
    expected_interval_ms: u64,
}

impl MetricsRecorder {
    pub fn new(expected_interval_ms: u64) -> Self {
        // 3 significant figures: 0.1% precision
        // Auto-resizing: no need to specify max value upfront
        Self {
            success_histogram: Histogram::<u64>::new(3)
                .expect("Failed to create histogram"),
            error_histogram: Histogram::<u64>::new(3)
                .expect("Failed to create histogram"),
            expected_interval_ms,
        }
    }

    pub fn record(&mut self, sample: &RequestSample) {
        let ms = sample.duration.as_millis() as u64;
        match &sample.result {
            Ok(()) => {
                // At-recording coordinated omission correction
                self.success_histogram
                    .record_correct(ms, self.expected_interval_ms)
                    .ok();
            }
            Err(_) => {
                self.error_histogram
                    .record_correct(ms, self.expected_interval_ms)
                    .ok();
            }
        }
    }

    pub fn p50(&self) -> u64 {
        self.success_histogram.value_at_quantile(0.50)
    }

    pub fn p95(&self) -> u64 {
        self.success_histogram.value_at_quantile(0.95)
    }

    pub fn p99(&self) -> u64 {
        self.success_histogram.value_at_quantile(0.99)
    }
}
```

### Pattern 4: TOML Config with Weighted Operations
**What:** Typed TOML config structs that support weighted mixes of all three MCP operation types.
**When to use:** Loading the loadtest scenario configuration.
**Example:**
```rust
// Source: Existing cargo-pmcp TOML patterns (utils/config.rs, landing/config.rs)
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct LoadTestConfig {
    /// General load test settings
    pub settings: Settings,
    /// Scenario steps with weighted mix
    pub scenario: Vec<ScenarioStep>,
}

#[derive(Debug, Deserialize)]
pub struct Settings {
    /// Number of virtual users
    pub virtual_users: u32,
    /// Test duration in seconds
    pub duration_secs: u64,
    /// Per-request timeout in milliseconds
    pub timeout_ms: u64,
    /// Expected request interval for coordinated omission correction (ms)
    #[serde(default = "default_expected_interval")]
    pub expected_interval_ms: u64,
}

fn default_expected_interval() -> u64 {
    100 // 100ms default expected interval
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum ScenarioStep {
    #[serde(rename = "tools/call")]
    ToolCall {
        weight: u32,
        tool: String,
        #[serde(default)]
        arguments: serde_json::Value,
    },
    #[serde(rename = "resources/read")]
    ResourceRead {
        weight: u32,
        uri: String,
    },
    #[serde(rename = "prompts/get")]
    PromptGet {
        weight: u32,
        prompt: String,
        #[serde(default)]
        arguments: HashMap<String, String>,
    },
}
```

**Corresponding TOML file:**
```toml
[settings]
virtual_users = 10
duration_secs = 60
timeout_ms = 5000

[[scenario]]
type = "tools/call"
weight = 60
tool = "calculate"
arguments = { expression = "2 + 2" }

[[scenario]]
type = "resources/read"
weight = 30
uri = "file:///data/config.json"

[[scenario]]
type = "prompts/get"
weight = 10
prompt = "summarize"
arguments = { text = "Hello world" }
```

### Anti-Patterns to Avoid
- **Reusing the parent SDK's Client<StreamableHttpTransport> for load testing:** The SDK client has SSE stream management, middleware chains, protocol negotiation, and Arc<RwLock<...>> layers that add overhead and hide timing. A load testing client needs raw control over the HTTP request/response cycle for accurate measurement.
- **Shared mutable histograms across VU tasks:** Never use `Arc<Mutex<Histogram>>`. Each VU should send `RequestSample` structs through an mpsc channel to a single aggregator task that owns the histogram. This pattern is implemented in Phase 2 but the `MetricsRecorder` API must be designed for single-owner usage from the start.
- **Using `Histogram::new_with_max()` with a guessed maximum:** Use `Histogram::new(sigfig)` which auto-resizes. Load testing latency can spike to seconds; a hardcoded max clips real data.
- **Recording timing around JSON parse + histogram record:** Measure `Instant::now()` immediately before the HTTP send and immediately after receiving the full response body. Do not include JSON parsing or histogram recording in the timed section.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Percentile computation | Custom percentile calculation | `hdrhistogram::Histogram::value_at_quantile()` | HdrHistogram uses a compressed logarithmic structure with configurable precision. Hand-rolled percentiles over Vec<f64> require sorting (O(n log n)), use unbounded memory, and cannot correct for coordinated omission. |
| Coordinated omission correction | Custom backfill logic | `hdrhistogram::Histogram::record_correct(value, interval)` | Gil Tene's algorithm generates synthetic samples for the expected-but-missing requests during slow periods. The math is subtle and the hdrhistogram implementation is battle-tested. |
| HTTP connection pooling | Custom connection management | `reqwest::Client` with `pool_max_idle_per_host()` | Connection pool management involves keep-alive negotiation, idle timeout, and per-host limits. reqwest handles this correctly out of the box. |
| JSON-RPC framing | Custom JSON-RPC serializer | Parent SDK's `JSONRPCRequest`, `JSONRPCResponse`, `JSONRPCError` types | The parent SDK already defines correct, tested serde types for the JSON-RPC protocol. Serialize with `serde_json::to_vec()` and deserialize with `serde_json::from_slice()`. |
| TOML parsing | Custom config parser | `toml::from_str()` with `#[derive(Deserialize)]` | TOML parsing has edge cases (inline tables, arrays of tables, string escaping). The `toml` crate handles all of these and is already in the project. |
| MCP session header management | Custom header injection | `reqwest::RequestBuilder::header("mcp-session-id", &self.session_id)` | The session ID is just an HTTP header (`mcp-session-id`). No custom abstraction needed; reqwest's builder handles it. |

**Key insight:** The parent SDK's type definitions (`InitializeRequest`, `CallToolRequest`, `ReadResourceRequest`, `GetPromptRequest`, `JSONRPCRequest`, `JSONRPCResponse`, `JSONRPCError`, `ErrorCode`) provide correct serialization for all MCP protocol messages. Use these types for constructing request bodies and parsing response bodies, but send them over a plain reqwest client rather than through the SDK's transport layer.

## Common Pitfalls

### Pitfall 1: Coordinated Omission Making Servers Look Faster Than They Are
**What goes wrong:** A closed-loop load generator (send request, wait for response, send next) throttles its own request rate when the server slows down. If the server takes 500ms to respond instead of 10ms, the generator sends 50x fewer requests during that period. The histogram records fewer samples during the slow period, making P99 look better than reality.
**Why it happens:** The generator and the server are coordination-coupled. When the server is slow, the generator is also slow, and both are slow together.
**How to avoid:** Use HdrHistogram's `record_correct(value, expected_interval)` on every sample. This generates synthetic samples for the "missing" requests that would have been sent in an open-loop model. The `expected_interval` parameter should be derived from the target request rate (e.g., 10 VUs at 100 req/s = one request every 100ms per VU).
**Warning signs:** P99 latency that is suspiciously close to P50 under heavy load. If P50 is 10ms and P99 is 15ms under 100 concurrent users, something is hiding tail latency.

### Pitfall 2: reqwest Connection Pool Starvation
**What goes wrong:** Default reqwest pool limits throttle concurrency, causing the load generator to queue requests internally. The measured latency includes both the server's actual response time AND the pool queue wait time.
**Why it happens:** reqwest defaults to `usize::MAX` idle connections per host, but the OS file descriptor limit (often 256 on macOS default) is the real bottleneck. When all connections are in use and the fd limit is reached, new connections fail or queue.
**How to avoid:** Configure `pool_max_idle_per_host(virtual_users * 2)`. At startup, check `ulimit -n` on macOS/Linux and warn if it is below `virtual_users * 4`. Set `tcp_nodelay(true)` (already the reqwest default, but make it explicit for documentation).
**Warning signs:** Latency that increases linearly with VU count even when the server CPU is idle. Connection errors under moderate load.

### Pitfall 3: Measuring Deserialization Time as Server Latency
**What goes wrong:** The timer is started before the HTTP send and stopped after JSON parsing. The measured latency includes the client's own JSON deserialization time, which can be 1-5ms for large responses.
**Why it happens:** It is natural to wrap the entire "send and parse" operation in a single timer.
**How to avoid:** Capture `Instant::now()` immediately before `reqwest::send()`. Capture a second `Instant::now()` immediately after the full response body is received (after `.bytes()` or `.text()`, but before `serde_json::from_slice()`). Record the difference. Parse the response body after recording the timing.
**Warning signs:** Baseline latency of 3-5ms against a server that should respond in < 1ms.

### Pitfall 4: MCP Session ID Not Being Propagated
**What goes wrong:** The client performs the initialize handshake successfully but does not extract and forward the `mcp-session-id` response header on subsequent requests. The server treats each subsequent request as a new session, either rejecting it (409 Conflict) or creating a new session each time.
**Why it happens:** The `mcp-session-id` header is set by the server in the initialize response, not in the JSON body. It is easy to parse the JSON-RPC body and forget the HTTP headers.
**How to avoid:** After every response (not just initialize), check for the `mcp-session-id` header and store it. Attach it on every subsequent request. The header name is defined in the parent SDK as `http_constants::MCP_SESSION_ID = "mcp-session-id"`.
**Warning signs:** Every request after initialize returns 409 or triggers a new session creation. Server logs show session count growing linearly with request count.

### Pitfall 5: Not Sending the `initialized` Notification After Initialize
**What goes wrong:** The MCP protocol requires the client to send an `initialized` notification (JSON-RPC notification, no `id` field) after receiving the initialize response and before sending any other requests. Skipping this causes well-behaved servers to reject subsequent tool calls.
**Why it happens:** The `initialized` notification is easy to overlook because it has no response and feels redundant.
**How to avoid:** After successfully parsing the initialize response, immediately send a JSON-RPC notification with method `"notifications/initialized"` and no params. This is a POST with no expected response (server returns 202 Accepted or 200 with empty body).
**Warning signs:** Tool calls fail with "not initialized" errors. Server logs show "received request before initialization complete".

## Code Examples

### Example 1: Complete Initialize Handshake Over HTTP

```rust
// Source: Derived from parent SDK's client/mod.rs initialize() and
// shared/streamable_http.rs send_with_options()

/// Perform the full MCP initialize handshake:
/// 1. POST initialize request
/// 2. Extract session ID from response header
/// 3. Send initialized notification
async fn initialize(&mut self) -> Result<(), McpError> {
    // Step 1: Send initialize request
    let init_body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": self.next_id(),
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": {
                "name": "cargo-pmcp-loadtest",
                "version": env!("CARGO_PKG_VERSION")
            }
        }
    });

    let response = self.http.post(&self.base_url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream")
        .timeout(self.request_timeout)
        .json(&init_body)
        .send()
        .await
        .map_err(McpError::from_reqwest)?;

    // Step 2: Extract session ID from headers (BEFORE consuming body)
    if let Some(session_id) = response.headers().get("mcp-session-id") {
        self.session_id = session_id.to_str().ok().map(String::from);
    }

    // Check HTTP status
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(McpError::Http { status: status.as_u16(), body });
    }

    // Parse JSON-RPC response
    let body: serde_json::Value = response.json().await
        .map_err(|e| McpError::Connection { message: e.to_string() })?;

    if let Some(error) = body.get("error") {
        let code = error.get("code").and_then(|c| c.as_i64()).unwrap_or(-32603) as i32;
        let message = error.get("message").and_then(|m| m.as_str()).unwrap_or("Unknown").to_string();
        return Err(McpError::JsonRpc { code, message });
    }

    // Step 3: Send initialized notification (no id = notification)
    let notif_body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    });

    let mut notif_req = self.http.post(&self.base_url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream")
        .timeout(self.request_timeout);

    // Attach session ID if we got one
    if let Some(ref sid) = self.session_id {
        notif_req = notif_req.header("mcp-session-id", sid);
    }

    // Server returns 202 Accepted or 200 for notifications
    let _ = notif_req.json(&notif_body).send().await;

    Ok(())
}
```

### Example 2: Recording a Timed Request with Coordinated Omission Correction

```rust
// Source: hdrhistogram docs (https://docs.rs/hdrhistogram/7.5.4)

use hdrhistogram::Histogram;
use std::time::{Duration, Instant};

let mut histogram = Histogram::<u64>::new(3).unwrap();
let expected_interval_ms: u64 = 100; // One request every 100ms expected

// Timing a request
let start = Instant::now();
let result = client.post(&url)
    .timeout(Duration::from_millis(5000))
    .json(&body)
    .send()
    .await;
let elapsed_ms = start.elapsed().as_millis() as u64;

// Record with coordinated omission correction
// If elapsed_ms is 500ms and expected_interval is 100ms,
// this also records synthetic samples at 400ms, 300ms, 200ms, 100ms
// to account for the 4 requests that "would have been sent"
histogram.record_correct(elapsed_ms, expected_interval_ms).ok();

// Extract percentiles
let p50 = histogram.value_at_quantile(0.50);
let p95 = histogram.value_at_quantile(0.95);
let p99 = histogram.value_at_quantile(0.99);
```

### Example 3: TOML Config Parsing

```rust
// Source: Existing cargo-pmcp pattern (utils/config.rs)

use std::fs;
use std::path::Path;

fn load_config(path: &Path) -> Result<LoadTestConfig, anyhow::Error> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read config: {}", path.display()))?;
    let config: LoadTestConfig = toml::from_str(&content)
        .with_context(|| format!("Failed to parse TOML config: {}", path.display()))?;

    // Validation: at least one scenario step required
    if config.scenario.is_empty() {
        anyhow::bail!("Config must contain at least one [[scenario]] step");
    }

    // Validation: weights must sum to > 0
    let total_weight: u32 = config.scenario.iter().map(|s| match s {
        ScenarioStep::ToolCall { weight, .. } => *weight,
        ScenarioStep::ResourceRead { weight, .. } => *weight,
        ScenarioStep::PromptGet { weight, .. } => *weight,
    }).sum();

    if total_weight == 0 {
        anyhow::bail!("Total scenario weights must be greater than 0");
    }

    Ok(config)
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| SSE-only MCP transport | Streamable HTTP (JSON or SSE response) | MCP spec 2025-03-26 | Load tester should request `Accept: application/json, text/event-stream` and handle both. JSON mode is simpler for request/response patterns. |
| `hdrsample` crate | `hdrhistogram` crate (renamed) | 2020 | The crate was renamed from `hdrsample` to `hdrhistogram`. Always use `hdrhistogram`. |
| Post-recording correction only | Both at-recording and post-recording | hdrhistogram 7.x | `record_correct()` enables at-recording correction. User decision locks this approach. |
| MCP protocol version "2024-11-05" | Protocol versions "2025-03-26" (default) and "2025-06-18" (latest) | 2025 | Parent SDK supports both. Load tester should send `"2025-06-18"` as protocol_version in initialize. |

**Deprecated/outdated:**
- `hdrsample`: Renamed to `hdrhistogram`. Do not use `hdrsample`.
- `reqwest` 0.11: Project uses 0.12 which has breaking changes in builder API. Ensure examples target 0.12.
- MCP "SSE transport": The MCP spec moved from dedicated SSE transport to "streamable HTTP" which can return either JSON or SSE. The load tester should handle both but prefer JSON for simplicity.

## Open Questions

1. **Should the load tester client use `Accept: application/json` (JSON only) or `Accept: application/json, text/event-stream` (both)?**
   - What we know: The MCP streamable HTTP spec allows clients to request either. JSON-only is simpler for the load tester (no SSE parsing needed). The parent SDK sends `application/json, text/event-stream` by default.
   - What's unclear: Whether some MCP servers reject clients that don't accept SSE.
   - Recommendation: Send `Accept: application/json, text/event-stream` (matching the SDK's `ACCEPT_STREAMABLE` constant) to maximize compatibility, but parse only the JSON case initially. If the response has `Content-Type: text/event-stream`, extract the JSON-RPC message from the first SSE event data field. This can be a simple string extraction, not a full SSE parser.

2. **What is the right `expected_interval_ms` default for coordinated omission correction?**
   - What we know: The value should approximate the expected time between consecutive requests from a single VU. If a VU is supposed to fire once every 100ms, use 100. Too low over-corrects; too high under-corrects.
   - What's unclear: Without load shaping (Phase 4), we do not control request rate. In Phase 1, VUs will fire as fast as possible (closed loop).
   - Recommendation: Default to 100ms (10 req/s per VU, a reasonable baseline). Make it configurable in the TOML settings as `expected_interval_ms`. Document clearly that this should match the expected per-VU request rate. Phase 4's open-loop scheduler will compute this automatically.

3. **Should the MCP client reuse a single `reqwest::Client` across all VUs or create one per VU?**
   - What we know: `reqwest::Client` uses an internal connection pool. Sharing a single client across VUs means shared pool management. One client per VU means independent pools.
   - What's unclear: Whether shared vs. per-VU pooling matters for accuracy at Phase 1's scale (not yet concurrent).
   - Recommendation: Design the `McpClient` struct to accept a `reqwest::Client` via constructor injection (clone of an Arc). This allows Phase 2 to decide sharing strategy. In Phase 1 unit tests, create one per test case.

## Sources

### Primary (HIGH confidence)
- Parent SDK `src/shared/streamable_http.rs` - Verified streamable HTTP transport implementation, session ID management via `mcp-session-id` header, `Accept: application/json, text/event-stream` header pattern
- Parent SDK `src/types/jsonrpc.rs` - Verified JSON-RPC request/response/notification types with serde derives
- Parent SDK `src/types/protocol.rs` - Verified `InitializeRequest`, `CallToolRequest`, `ReadResourceRequest`, `GetPromptRequest` type definitions
- Parent SDK `src/error/mod.rs` - Verified `ErrorCode` constants: -32700 parse, -32601 method not found, -32602 invalid params, -32603 internal, -32001 timeout
- Parent SDK `src/lib.rs` - Verified `LATEST_PROTOCOL_VERSION = "2025-06-18"`, `DEFAULT_PROTOCOL_VERSION = "2025-03-26"`
- Parent SDK `src/client/mod.rs` - Verified initialize handshake flow: send InitializeRequest, parse InitializeResult, send initialized notification
- [hdrhistogram docs.rs](https://docs.rs/hdrhistogram/latest/hdrhistogram/struct.Histogram.html) - Verified `record_correct()`, `value_at_quantile()`, `new(sigfig)` constructor API
- [crates.io API](https://crates.io/crates/hdrhistogram) - Verified latest version 7.5.4
- [reqwest RequestBuilder docs](https://docs.rs/reqwest/latest/reqwest/struct.RequestBuilder.html) - Verified per-request `timeout()` method that overrides client-level timeout
- [reqwest ClientBuilder docs](https://docs.rs/reqwest/latest/reqwest/struct.ClientBuilder.html) - Verified `pool_max_idle_per_host()`, `tcp_nodelay()`, `connect_timeout()` methods

### Secondary (MEDIUM confidence)
- Existing cargo-pmcp source code (`main.rs`, `commands/test/check.rs`, `utils/config.rs`) - Verified project patterns: clap subcommands, TOML config with serde derive, colored terminal output
- Project research summary (`.planning/research/SUMMARY.md`) - Architecture patterns, pitfalls, phase structure previously validated

### Tertiary (LOW confidence)
- None. All findings verified against primary sources.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - All libraries verified against crates.io API and docs.rs. Only new dependency (hdrhistogram 7.5.4) is well-established with verified API.
- Architecture: HIGH - Derived directly from reading parent SDK source code. MCP protocol flow verified against actual implementation in `client/mod.rs` and `streamable_http.rs`.
- Pitfalls: HIGH - Coordinated omission is Gil Tene's documented result. Connection pool and session pitfalls verified against reqwest docs and MCP SDK source.

**Research date:** 2026-02-26
**Valid until:** 2026-03-28 (stable domain, well-documented libraries)
