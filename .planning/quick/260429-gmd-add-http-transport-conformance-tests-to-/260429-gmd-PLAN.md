---
phase: 260429-gmd
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - crates/mcp-tester/src/report.rs
  - crates/mcp-tester/src/tester.rs
  - crates/mcp-tester/src/conformance/mod.rs
  - crates/mcp-tester/src/conformance/transport.rs
  - crates/mcp-tester/src/lib.rs
  - cargo-pmcp/src/commands/test/conformance.rs
  - crates/mcp-tester/tests/transport_conformance_integration.rs
autonomous: true
requirements:
  - QUICK-260429-GMD
must_haves:
  truths:
    - "GET /mcp returning 200 + non-SSE JSON body (the cost-coach prod regression) is detected as FAIL by `cargo pmcp test conformance`"
    - "The new Transport domain reuses the HttpMiddlewareChain produced by `cargo pmcp auth` — never re-prompts for credentials and never re-builds an auth provider"
    - "Transport domain SKIPs cleanly with a clear message when transport_type is Stdio or JsonRpcHttp (Streamable-HTTP-only tests)"
    - "FAIL details include status code, content-type, and the first ~200 bytes of the body so the operator can diagnose the misconfigured edge layer at a glance"
    - "Auth-rejected probes (401/403) report as WARNING with text directing the user to `cargo pmcp auth login`, not as FAIL"
    - "The CI summary line includes Transport ordered second: `Conformance: Core=PASS Transport=PASS Tools=... ...`"
  artifacts:
    - path: "crates/mcp-tester/src/conformance/transport.rs"
      provides: "Transport domain conformance scenarios (GET /mcp, OPTIONS /mcp, DELETE /mcp)"
      min_lines: 200
    - path: "crates/mcp-tester/tests/transport_conformance_integration.rs"
      provides: "End-to-end test: in-process pmcp streamable_http_server passes Transport domain"
      min_lines: 60
  key_links:
    - from: "crates/mcp-tester/src/conformance/transport.rs"
      to: "crates/mcp-tester/src/tester.rs::ServerTester accessors"
      via: "url(), timeout(), insecure(), http_middleware_chain(), transport_type()"
      pattern: "tester\\.http_middleware_chain\\(\\)"
    - from: "cargo-pmcp/src/commands/test/conformance.rs::print_domain_summary"
      to: "crates/mcp-tester/src/report.rs::TestCategory::Transport"
      via: "domains array entry (\"Transport\", TestCategory::Transport)"
      pattern: "Transport.*TestCategory::Transport"
---

<objective>
Add an HTTP `Transport` conformance domain to `cargo pmcp test conformance` that catches Streamable-HTTP misconfigurations the existing JSON-RPC-over-POST suite cannot see — specifically the cost-coach prod regression where `GET /mcp` returned `200 OK` + a plain JSON body instead of either an SSE stream or `405 Method Not Allowed`.

Purpose: A spec-compliant Streamable-HTTP client (ChatGPT, Claude Desktop, etc.) opens a `GET /mcp` SSE channel as part of session establishment. A reverse proxy or edge function that rewrites `GET /mcp` to a JSON health response will silently break every spec-compliant client while the existing POST-only conformance suite reports green. This plan closes that gap by probing the raw HTTP surface, classifying responses by (status × content-type × body), and surfacing failures with enough detail to immediately identify the offending layer.

Output: A new `Transport` `ConformanceDomain` variant exposed end-to-end (mcp-tester library → cargo-pmcp CLI → CI summary line), backed by unit tests on the response classifier and an integration test against an in-process pmcp streamable_http_server.
</objective>

<execution_context>
@/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.claude/get-shit-done/workflows/execute-plan.md
@/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@CLAUDE.md
@crates/mcp-tester/src/conformance/mod.rs
@crates/mcp-tester/src/conformance/core_domain.rs
@crates/mcp-tester/src/tester.rs
@crates/mcp-tester/src/report.rs
@cargo-pmcp/src/commands/test/conformance.rs

<interfaces>
<!-- Contracts the executor needs. Extracted from the codebase -- no exploration required. -->

From `crates/mcp-tester/src/report.rs`:
```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TestCategory {
    Core,
    Protocol,
    Tools,
    Resources,
    Prompts,
    Performance,
    Compatibility,
    Apps,
    Tasks,
    // <-- ADD: Transport,
}

impl TestResult {
    pub fn passed(name, category, duration, details) -> Self;
    pub fn failed(name, category, duration, error) -> Self;
    pub fn warning(name, category, duration, details) -> Self;
    pub fn skipped(name, category, details) -> Self;
}
```

From `crates/mcp-tester/src/tester.rs` (private fields needing accessors):
```rust
pub struct ServerTester {
    url: String,                                                    // -> pub fn url(&self) -> &str
    pub transport_type: TransportType,                              // already pub
    timeout: Duration,                                              // -> pub fn timeout(&self) -> Duration
    insecure: bool,                                                 // -> pub fn insecure(&self) -> bool
    http_middleware_chain:
        Option<Arc<pmcp::client::http_middleware::HttpMiddlewareChain>>,
        // -> pub fn http_middleware_chain(&self) -> Option<&Arc<HttpMiddlewareChain>>
    // ... other fields stay private
}

pub enum TransportType { Http, Stdio, JsonRpcHttp }
```

From `src/client/http_middleware.rs`:
```rust
pub struct HttpRequest {
    pub method: String,
    pub url: String,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}
impl HttpRequest {
    pub fn new(method: String, url: String, body: Vec<u8>) -> Self;
    pub fn add_header(&mut self, name: &str, value: &str);
}

pub struct HttpMiddlewareContext { /* opaque */ }
impl HttpMiddlewareContext {
    pub fn new(url: String, method: String) -> Self;
}

impl HttpMiddlewareChain {
    pub async fn process_request(
        &self,
        request: &mut HttpRequest,
        context: &HttpMiddlewareContext,
    ) -> pmcp::Result<()>;
}
```

Established usage pattern (from `tester.rs::send_json_rpc_request` lines 240–256):
```rust
let mut http_req = HttpRequest::new("GET".into(), url.into(), Vec::new());
http_req.add_header("Accept", "text/event-stream");
let context = HttpMiddlewareContext::new(url.into(), "GET".into());
if let Some(chain) = tester.http_middleware_chain() {
    chain.process_request(&mut http_req, &context).await?;
}
// http_req.headers + http_req.body now contain auth-injected request -- send via reqwest
```

From `crates/mcp-tester/src/conformance/mod.rs`:
```rust
pub enum ConformanceDomain { Core, Tools, Resources, Prompts, Tasks }
// ConformanceDomain::from_str_loose(s) parses case-insensitively
// ConformanceRunner::run iterates domains and calls run_<domain>_conformance(tester)
```

Server canonical responses (from `src/server/streamable_http_server.rs:1518–1524`):
- Stateless GET /mcp -> 405 + JSON-RPC error code -32601, message "SSE not supported in stateless mode"
- Stateful GET /mcp -> 200 + Content-Type: text/event-stream, SSE body stream

Cost-coach regression signature (the FAIL case to detect):
- 200 + Content-Type: application/json + body like `{"ok":true,"service":"...","version":"..."}`
</interfaces>
</context>

<tasks>

<task type="auto" tdd="true">
  <name>Task 1: Add Transport TestCategory and ServerTester accessors</name>
  <files>
    crates/mcp-tester/src/report.rs
    crates/mcp-tester/src/tester.rs
  </files>
  <behavior>
    - `TestCategory::Transport` exists, derives the same traits as sibling variants (Debug, Clone, PartialEq, Eq, Serialize, Deserialize), round-trips through serde JSON.
    - `ServerTester::url(&self) -> &str` returns the URL the tester was constructed with.
    - `ServerTester::timeout(&self) -> Duration` returns the configured timeout.
    - `ServerTester::insecure(&self) -> bool` returns the TLS-insecure flag.
    - `ServerTester::http_middleware_chain(&self) -> Option<&Arc<HttpMiddlewareChain>>` returns a borrowed reference to the chain produced by `cargo pmcp auth` (Some when middleware is wired, None otherwise).
    - None of the accessors expose `api_key` directly — auth must travel exclusively through the middleware chain.
    - All accessors compile with `#![deny(missing_docs)]`-grade rustdoc and a doctest where structurally possible (the chain accessor's doctest can use `# fn main() {}` to demonstrate the call site without constructing a full ServerTester).
  </behavior>
  <action>
    Add `Transport` variant to `TestCategory` in `crates/mcp-tester/src/report.rs` (place it immediately after `Core` so the recommendations match-arm ordering stays readable; no recommendation block is required for Transport in this task — keep `_ => {}` catch-all behavior). Then add the four narrow accessors on `ServerTester` (`url`, `timeout`, `insecure`, `http_middleware_chain`) in `crates/mcp-tester/src/tester.rs` directly below the existing `pub fn new(...)` signature, before `send_json_rpc_request`. Each accessor must have a single-line rustdoc summary plus a longer paragraph for `http_middleware_chain` documenting that callers MUST reuse this chain (do not re-prompt for credentials). Remove the `#[allow(dead_code)]` attributes on `timeout` and `insecure` fields since they now have public accessors.
  </action>
  <verify>
    <automated>cargo build -p mcp-tester &amp;&amp; cargo test -p mcp-tester --lib report:: tester::accessors -- --test-threads=1</automated>
  </verify>
  <done>
    `TestCategory::Transport` is part of the public API of mcp-tester. `cargo doc -p mcp-tester --no-deps` succeeds with zero warnings. The four new accessors compile and have rustdoc. Existing 5 conformance domains still build and pass. `make quality-gate` from this point still passes (no clippy / fmt regressions).
  </done>
</task>

<task type="auto" tdd="true">
  <name>Task 2: Implement Transport conformance domain</name>
  <files>
    crates/mcp-tester/src/conformance/transport.rs
    crates/mcp-tester/src/conformance/mod.rs
    crates/mcp-tester/src/lib.rs
  </files>
  <behavior>
    Pure-function classifier first (this is the unit-testable core; keep it free of I/O):
    ```
    fn classify_get_mcp(status: u16, content_type: &str, body_prefix: &str) -> TestStatus
    ```
    - (405, "application/json...", body parses as JSON-RPC with `error.code == -32601`) -> Passed
    - (200, "text/event-stream...", _)                                                  -> Passed
    - (401 | 403, _, _)                                                                 -> Warning (auth issue)
    - (200, "application/json...", _) where body does NOT look like JSON-RPC error      -> Failed (the cost-coach regression)
    - any other (status, ct) combination                                                -> Failed
    Property: classifier is total — every (u16, &str, &str) yields exactly one TestStatus. No panics.

    Domain entry point:
    ```
    pub async fn run_transport_conformance(tester: &ServerTester) -> Vec<TestResult>
    ```
    - When `tester.transport_type` is `Stdio` or `JsonRpcHttp`: returns a single Skipped result with text "Transport: Streamable-HTTP-only tests skipped (transport={Stdio|JsonRpcHttp}). Re-run against an HTTP server to validate the GET/OPTIONS/DELETE surface."
    - When `Http`: runs at minimum these scenarios in order, each producing one TestResult with category `TestCategory::Transport`:
      1. `Transport: GET /mcp returns SSE stream OR 405` — raw GET with `Accept: text/event-stream`, middleware applied, 5s receive timeout (use `tokio::time::timeout`, NOT the long-poll keepalive). Read response head + up to 256 bytes of body. Failure detail string MUST be of the form: `"unexpected response: status={code} content-type={ct} body_prefix={truncated_to_200_chars}"`.
      2. `Transport: OPTIONS /mcp returns CORS or 405` — raw OPTIONS with `Origin: https://example.invalid` and `Access-Control-Request-Method: POST`, middleware applied. PASS on (2xx + at least one `Access-Control-*` response header) OR 405. FAIL on 200 + no CORS headers + non-MCP body. Same failure-detail format as above.
      3. `Transport: DELETE /mcp returns 200/204/405` — raw DELETE with no body, middleware applied. Mark this scenario's failure mode as `TestStatus::Warning` (not Failed) per plan constraints — emit `TestResult::warning(...)` for unexpected statuses rather than `failed(...)`. Document the rationale inline: "session-termination is per-spec but not currently a known live-failure mode; treat as warning until we see it in the wild".

    All raw HTTP requests share a helper:
    ```
    async fn raw_probe(
        tester: &ServerTester,
        method: &str,
        extra_headers: &[(&str, &str)],
        receive_timeout: Duration,
    ) -> Result<(u16, String, String), String>  // (status, content_type, body_prefix) | err string
    ```
    The helper builds an `HttpRequest`, applies `tester.http_middleware_chain()` if present via `process_request`, builds a `reqwest::Client` honoring `tester.insecure()` and `tester.timeout().min(receive_timeout)`, executes, reads at most 256 bytes of body via `response.bytes()` truncated, and returns the triple. On reqwest error, return `Err(format!("transport error: {e}"))` and let the caller convert to `TestResult::failed`.
  </behavior>
  <action>
    Create `crates/mcp-tester/src/conformance/transport.rs` implementing `classify_get_mcp` (pure), `raw_probe` (helper), and the three scenario functions (`test_get_mcp_returns_sse_or_405`, `test_options_mcp_returns_cors_or_405`, `test_delete_mcp_returns_session_termination_or_405`), plus the public entry point `run_transport_conformance`. Follow the structure of `core_domain.rs` exactly: `Vec<TestResult>` accumulator, per-test `let start = Instant::now()`, name strings prefixed with `"Transport: "`. Then in `crates/mcp-tester/src/conformance/mod.rs`: add `pub(crate) mod transport;`, add `Transport` variant to `ConformanceDomain` (after `Tasks` is fine — order in the enum does not affect CLI output), extend `from_str_loose` with `"transport" => Some(Self::Transport)`, and add a `should_run(ConformanceDomain::Transport)` block in `ConformanceRunner::run` placed AFTER the Core block but BEFORE the Tools block (so Transport runs second, matching the CI summary order constraint). Transport runs even when core has soft warnings, but must skip if `report.has_failures()` after Core (same gating as Tools/Resources/Prompts/Tasks). No re-export change is strictly required in `lib.rs` because `ConformanceDomain` is already re-exported, but verify the new variant is reachable via `mcp_tester::ConformanceDomain::Transport` from the cargo-pmcp side.

    Unit tests live at the bottom of `transport.rs` under `#[cfg(test)] mod tests`:
    - `classify_get_mcp_405_jsonrpc_passes()`
    - `classify_get_mcp_200_sse_passes()`
    - `classify_get_mcp_200_json_non_sse_fails()` — the cost-coach regression
    - `classify_get_mcp_401_warns()`
    - `classify_get_mcp_other_fails()` — covers 500, 502, weird statuses
    - Property test: `proptest!` over `(status: 0u16..1000, ct: any::<String>(), body: any::<String>())` asserts the classifier returns one of {Passed, Failed, Warning, Skipped} and never panics. Use the `proptest` dependency already in the workspace if available; otherwise gate the property test behind `#[cfg(feature = "proptest")]` and document the gate. Do NOT add `proptest` to mcp-tester Cargo.toml in this task — if it isn't already a dev-dep, write a hand-rolled exhaustive loop over a small status set instead and document the trade-off.

    No new runtime dependencies. Reuse `reqwest` (already in mcp-tester deps) and `tokio` (already a dep).
  </action>
  <verify>
    <automated>cargo test -p mcp-tester --lib conformance::transport:: -- --test-threads=1 &amp;&amp; cargo build -p mcp-tester</automated>
  </verify>
  <done>
    `cargo test -p mcp-tester` passes including all new transport unit tests (classifier truth-table + property/exhaustive). `cargo clippy -p mcp-tester --all-features -- -D warnings` is clean. Cognitive complexity of every new function is ≤ 25 (verify with `pmat analyze complexity --format json --max-cognitive 25 | jq '.violations[] | select(.path == "crates/mcp-tester/src/conformance/transport.rs")'` returning empty). Zero SATD comments (`grep -nE 'TODO|FIXME|XXX|HACK' crates/mcp-tester/src/conformance/transport.rs` returns nothing). Public functions have rustdoc.
  </done>
</task>

<task type="auto" tdd="true">
  <name>Task 3: Wire Transport into cargo-pmcp CLI summary + integration test</name>
  <files>
    cargo-pmcp/src/commands/test/conformance.rs
    crates/mcp-tester/tests/transport_conformance_integration.rs
  </files>
  <behavior>
    - The CI single-line summary printed by `print_domain_summary` becomes:
      `Conformance: Core=PASS Transport=PASS Tools=PASS Resources=PASS Prompts=PASS Tasks=SKIP`
      with Transport ordered second (immediately after Core).
    - `cargo pmcp test conformance --domain transport <url>` runs only the Transport domain (proves `from_str_loose("transport")` round-trip works through the CLI).
    - Integration test spins up an in-process pmcp `streamable_http_server` in stateless mode on `127.0.0.1:0` (ephemeral port), constructs a `ServerTester` against it with `force_transport = Some("http")` and no middleware, runs `ConformanceRunner::new(false, Some(vec![ConformanceDomain::Transport]))`, and asserts:
      * `report.summary.failed == 0`
      * At least one TestResult has category `TestCategory::Transport` and status `Passed`
      * The GET /mcp scenario specifically passes (search by name prefix `"Transport: GET /mcp"`)
    - The integration test cleanly shuts down the server (drop the JoinHandle / use a oneshot shutdown channel — follow whichever pattern other mcp-tester integration tests already use; if none exists, a `tokio::spawn` whose handle is aborted at end-of-test is acceptable).
  </behavior>
  <action>
    In `cargo-pmcp/src/commands/test/conformance.rs::print_domain_summary`, insert `("Transport", TestCategory::Transport)` as the second entry in the `domains` array (between Core and Tools). Verify the function signature and rest of `execute(...)` need no other changes — the runner already iterates whichever domains are registered.

    Create `crates/mcp-tester/tests/transport_conformance_integration.rs`. Look for an existing in-process server fixture under `crates/mcp-tester/tests/` or `tests/`; if one exists for streamable HTTP, import its helper. If none, build a minimal one inline:
    ```rust
    // Pseudo-outline:
    // 1. Build a pmcp::Server with no tools/resources (empty server is fine).
    // 2. Bind a stateless StreamableHttpServer to 127.0.0.1:0; capture the actual SocketAddr.
    // 3. tokio::spawn the server future.
    // 4. Build ServerTester::new(&format!("http://{}/mcp", addr), 5s timeout, false, None, Some("http"), None).
    // 5. Initialize the tester (call test_initialize via running the Core domain first OR call directly).
    // 6. Run ConformanceRunner with [Transport] only.
    // 7. assert!(!report.has_failures()).
    // 8. abort the server task.
    ```
    Reference: search the existing test corpus for `StreamableHttpServer` or `streamable_http_server` usage in `crates/mcp-tester/tests/` and `tests/` first; reuse before re-inventing. If the only existing fixture is in `examples/`, copy the minimal pattern (don't depend on `examples`).

    Do NOT add the integration test as a strict gate yet if the in-process fixture is non-trivial — if after 30 minutes of trying you cannot get the in-process server to bind cleanly, implement the integration test against a hand-rolled `tokio::net::TcpListener` that returns canned 405+JSON-RPC responses (the same shape `streamable_http_server.rs` returns). Document the choice in a comment at the top of the test file. The objective is to PROVE the wiring end-to-end against SOME real HTTP server, not specifically the pmcp one.
  </action>
  <verify>
    <automated>cargo test -p mcp-tester --test transport_conformance_integration -- --test-threads=1 &amp;&amp; cargo build -p cargo-pmcp &amp;&amp; cargo run -p cargo-pmcp --quiet -- test conformance --help | grep -q transport || true</automated>
  </verify>
  <done>
    Integration test passes locally. `make quality-gate` is green from a clean state (`cargo fmt --all -- --check`, full clippy with `--features full`, build, test, audit all pass — this is the canonical CI-equivalent gate per CLAUDE.md release workflow). The CI summary line, when run against a healthy stateless pmcp server, prints `Conformance: Core=PASS Transport=PASS ...`. When pointed at a deliberately-broken endpoint that returns 200+JSON for GET /mcp (the cost-coach regression simulator inside the integration test), the summary prints `Transport=FAIL` and the failure detail contains the literal substring `"status=200 content-type=application/json"`.
  </done>
</task>

</tasks>

<verification>
End-to-end phase verification (run all from repo root):

1. `make quality-gate` — full Toyota Way gate (fmt, clippy with pedantic+nursery, build, test, audit). MUST pass.
2. `cargo test -p mcp-tester` — all unit + integration tests pass, including new transport classifier truth-table tests and the in-process server integration test.
3. `pmat analyze complexity --format json --max-cognitive 25 | jq '.violations[] | select(.path | startswith("crates/mcp-tester/src/conformance/transport.rs") or startswith("crates/mcp-tester/src/tester.rs") or startswith("cargo-pmcp/src/commands/test/conformance.rs"))'` — empty array (zero new complexity violations).
4. `grep -nrE 'TODO|FIXME|XXX|HACK|allow\(dead_code\)' crates/mcp-tester/src/conformance/transport.rs cargo-pmcp/src/commands/test/conformance.rs` — empty (zero new SATD).
5. Manual smoke: `cargo run -p cargo-pmcp -- test conformance --domain transport http://localhost:PORT/mcp` against a known-good local pmcp server prints `Transport=PASS` in the summary line. Then deliberately route GET /mcp to a `200 + {"ok":true}` health endpoint (e.g. via a one-line nginx stub or a test fixture) and confirm the summary prints `Transport=FAIL` with the regression-shaped detail.
6. `cargo doc -p mcp-tester --no-deps` succeeds with zero warnings — confirms rustdoc on new accessors and public items is well-formed.
</verification>

<success_criteria>
- ✅ `TestCategory::Transport` is a stable public variant of mcp-tester's API.
- ✅ `ConformanceDomain::Transport` is selectable via `--domain transport` on the CLI.
- ✅ Running `cargo pmcp test conformance` against a stateless pmcp server produces `Transport=PASS` in the CI summary line, ordered second (Core, Transport, Tools, ...).
- ✅ Running `cargo pmcp test conformance` against a server that returns `200 OK + application/json + {"ok":true,...}` for `GET /mcp` (the cost-coach regression) produces `Transport=FAIL` with detail text containing `status=200`, `content-type=application/json`, and a body prefix — sufficient signal for an operator to identify the misconfigured edge layer in under 60 seconds.
- ✅ Running against a stdio target prints a clean SKIP with the explanatory message; no false failures.
- ✅ Running with no auth against a server requiring auth yields a WARNING (not FAIL) on the GET probe, with text directing the user to `cargo pmcp auth login`.
- ✅ Auth path: when invoked with valid `cargo pmcp auth` state, the new transport tests reuse the OAuth Bearer via the existing `HttpMiddlewareChain` — no second auth prompt, no duplicate token negotiation. Verifiable by inspecting RUST_LOG=debug output: only one `"OAuth"` middleware dispatch per probe.
- ✅ All Toyota Way gates from CLAUDE.md hold: `make quality-gate` green, zero clippy warnings (full feature set), zero SATD, all new functions ≤ cog 25, every new public item has rustdoc + doctest where applicable, ALWAYS testing satisfied (unit + property/exhaustive + integration; example not required for a test-suite-extension feature).
- ✅ DELETE /mcp scenario emits Warning-class results only (per scope constraint) — no strict-failure regressions on servers that don't implement the optional session-termination endpoint.
- ✅ `crates/mcp-tester/Cargo.toml` and `cargo-pmcp/Cargo.toml` have ZERO new runtime dependencies. Test-only additions (if any) live behind `[dev-dependencies]`.
</success_criteria>

<threat_model>
## Trust Boundaries

| Boundary | Description |
|----------|-------------|
| tester process → target HTTP server | The conformance suite issues raw HTTP probes to a user-supplied URL. The URL is untrusted input from the operator; the response is untrusted input from the target server. |
| auth middleware chain → outbound request | OAuth Bearer / API-Key material lives inside the chain and is injected into request headers; it MUST NOT leak into log output, failure-detail strings, or the test report JSON. |

## STRIDE Threat Register

| Threat ID | Category | Component | Disposition | Mitigation Plan |
|-----------|----------|-----------|-------------|-----------------|
| T-260429-01 | Information Disclosure | `transport.rs::raw_probe` failure-detail string | mitigate | Truncate body to 200 chars max before embedding in TestResult error/details. Never include request headers (which contain Authorization) in failure strings — only response status, response content-type, and response body prefix. Add a unit test asserting that a probe with `Authorization: Bearer SEKRET` in the request never produces a TestResult whose `error` or `details` string contains the substring `SEKRET`. |
| T-260429-02 | Information Disclosure | rustdoc / public API surface for new accessors | mitigate | `ServerTester::http_middleware_chain` returns `&Arc<HttpMiddlewareChain>` (opaque type) — does NOT expose the underlying token. Do NOT add an `api_key()` accessor; the existing private `api_key` field stays private. Doctest must demonstrate borrowing the chain, never reading credentials from it. |
| T-260429-03 | Tampering | Response classifier on adversarial bodies | mitigate | Classifier `classify_get_mcp` operates on `&str` body_prefix bounded to 200 chars BEFORE classification. JSON-RPC error-shape detection uses `serde_json::from_str` on the truncated prefix and treats parse failure as "not JSON-RPC error" (FAIL path), never panics. Property test asserts no panic across arbitrary `(u16, String, String)` inputs. |
| T-260429-04 | Denial of Service | `raw_probe` against malicious or slow servers | mitigate | Hard 5s receive timeout via `tokio::time::timeout`; body read capped at 256 bytes via `response.bytes()` followed by truncation (do NOT use streaming + accumulator without a cap). reqwest client built with `tester.timeout()` as overall timeout. The Transport domain must complete within ~30s even against a black-hole endpoint. |
| T-260429-05 | Spoofing | TLS-insecure flag passthrough | accept | The `--insecure` flag already exists on the parent `cargo pmcp test` command and is the operator's deliberate choice for self-signed dev environments. The new `tester.insecure()` accessor surfaces the same flag to the new domain; we don't change its policy. Documented constraint: production CI runs MUST NOT pass `--insecure`. |
| T-260429-06 | Repudiation | Logging of auth-rejected probes | accept | A 401/403 produces a WARNING TestResult; the operator-facing detail text says "authenticate first via `cargo pmcp auth login`". No identifying material is logged beyond status code. Acceptable because the only signal is "auth failed", which is already non-secret. |
| T-260429-07 | Elevation of Privilege | Re-using middleware chain across raw probes | mitigate | The new transport scenarios borrow the existing `Arc<HttpMiddlewareChain>` from `ServerTester` — they MUST NOT construct a new chain or a new auth provider. This is enforced by the API: only `http_middleware_chain()` is exposed (returns `Option<&Arc<...>>`). Code review checkpoint in Task 2: confirm no `OAuthHelper::new`, `AuthProvider::new`, or chain-builder calls appear in `transport.rs`. |
</threat_model>

<output>
After completion, create `.planning/quick/260429-gmd-add-http-transport-conformance-tests-to-/260429-gmd-SUMMARY.md` describing:
- The new TestCategory and ConformanceDomain variants and where they're exposed.
- The exact CI summary-line format (showing Transport in second position).
- The classifier truth-table (status × content-type → TestStatus) as the canonical reference.
- The auth-reuse property: every transport probe goes through the same HttpMiddlewareChain that `cargo pmcp auth` constructs.
- A worked example of the cost-coach-regression failure detail string an operator will see.
- Pointers to the unit, property/exhaustive, and integration tests so the next maintainer can extend them (e.g. add tests for spec-2026 SSE event-stream framing).
</output>
