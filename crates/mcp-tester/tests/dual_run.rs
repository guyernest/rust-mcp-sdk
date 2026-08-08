//! Live-socket dual-run tests: era detection, `run_dual`, and the additivity
//! contract, all against a REAL in-process `pmcp` server.
//!
//! # The trade-off is INVERTED versus `transport_conformance_integration.rs`
//!
//! That file's header explains why it uses a hand-rolled `TcpListener` stub: it
//! needs to produce a response shape a correct-by-construction pmcp server
//! never emits (`200 + application/json + non-SSE body`).
//!
//! Here the requirement is the opposite one. What is under test is ERA
//! CLASSIFICATION, and the signatures it classifies on — a genuine
//! `server/discover` projection, the `-32601` a v1 server answers to it, the
//! `Mcp-Session-Id` a stateful v1 server mints, the `405` a v2 server returns to
//! a `GET` — are exactly what a stub would have to FAKE. A test that classified
//! hand-written bytes would be validating the harness against itself.
//! `crates/mcp-tester/Cargo.toml:20` already deps `pmcp` with `streamable-http`,
//! so a real server costs nothing.
//!
//! The stub is kept for the ONE case a real server cannot produce: counting the
//! sessions it minted and the `DELETE`s it received, which pmcp exposes no
//! accessor for.
//!
//! # Timeouts
//!
//! Every await is bounded by [`STEP_TIMEOUT`]. A hung server must FAIL this
//! test, not hang it — an unbounded await turns a server-side regression into a
//! CI timeout with no diagnosis attached.

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use mcp_tester::era_observations::{
    EraObservations, ObservationId, ObservedValue, METHOD_INITIALIZE, METHOD_SERVER_DISCOVER,
    PROBE_REGISTRY,
};
use mcp_tester::{
    compare_eras, load_default_baseline, ConformanceDomain, ConformanceRunner, DifferenceClass,
    EraSupport, OutputFormat, ServerTester, TestReport,
};
use pmcp::server::streamable_http_server::{StreamableHttpServer, StreamableHttpServerConfig};
use pmcp::server::Server;
use pmcp::types::protocol::{ProtocolVersion, PROTOCOL_VERSION_2026_07_28};
use pmcp::types::Content;
use pmcp::{RequestHandlerExtra, ToolHandler};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

/// Hard upper bound on any single awaited step.
///
/// A hung server must FAIL the test rather than hang it: an await with no
/// bound converts a server regression into an unattributed CI timeout.
const STEP_TIMEOUT: Duration = Duration::from_secs(20);

/// Per-request budget handed to every `ServerTester` built here.
const TESTER_TIMEOUT: Duration = Duration::from_secs(10);

/// The v1 protocol version an opted-in server keeps serving alongside v2.
const V1: &str = "2025-11-25";

// ===========================================================================
// A real in-process pmcp server, in three era configurations.
// ===========================================================================

struct EchoTool;

#[pmcp::async_trait]
impl ToolHandler for EchoTool {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        Ok(json!({ "content": [Content::Text { text: args.to_string() }] }))
    }
}

/// Build a server whose accept-list is exactly `versions`.
fn server_accepting(versions: Vec<&str>) -> Server {
    Server::builder()
        .name("era-fixture")
        .version("0.0.0")
        .tool("echo", EchoTool)
        .with_supported_protocol_versions(
            versions
                .into_iter()
                .map(|v| ProtocolVersion(v.to_string()))
                .collect::<Vec<_>>(),
        )
        .build()
        .expect("fixture server builds")
}

/// Spawn `server` on an ephemeral loopback port with the STATEFUL default
/// config.
///
/// The default config (not `::stateless()`) is deliberate: a build-time
/// stateless server removes the session machinery before a request is ever
/// seen, so it could never exercise the per-request era gate — nor the session
/// mint that the leak test is about.
async fn spawn(server: Server) -> (SocketAddr, JoinHandle<()>) {
    let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 0);
    let http = StreamableHttpServer::with_config(
        addr,
        Arc::new(tokio::sync::Mutex::new(server)),
        StreamableHttpServerConfig::default(),
    );
    tokio::time::timeout(STEP_TIMEOUT, http.start())
        .await
        .expect("server start must not hang")
        .expect("server starts")
}

/// Shut a spawned server down: drop sockets → abort → await.
///
/// The order matters. A bare `abort()` with no await leaves the aborted task
/// possibly unfinished when the test returns, which nextest reports as a LEAK.
async fn teardown(handle: JoinHandle<()>) {
    handle.abort();
    let _ = handle.await;
}

/// The MCP endpoint URL.
///
/// `StreamableHttpServer` routes POST/GET/DELETE at `/`
/// (`src/server/streamable_http_server.rs:325-327`), NOT at `/mcp`. Pointing a
/// probe at `/mcp` gets a `404`, which the era detector correctly reports as
/// `NoEraSpoken` — the endpoint answered, but not with an era.
fn mcp_url(addr: SocketAddr) -> String {
    format!("http://{addr}/")
}

fn tester(url: &str) -> ServerTester {
    ServerTester::new(url, TESTER_TIMEOUT, false, None, Some("http"), None)
        .expect("tester constructs")
}

fn v2_tester(url: &str) -> ServerTester {
    tester(url).with_protocol_version(ProtocolVersion(PROTOCOL_VERSION_2026_07_28.to_string()))
}

// ===========================================================================
// 1. Era detection against all three server configurations.
// ===========================================================================

#[tokio::test]
async fn detect_eras_reports_dual_for_an_opted_in_server() {
    let (addr, handle) = spawn(server_accepting(vec![V1, PROTOCOL_VERSION_2026_07_28])).await;
    let verdict = tokio::time::timeout(
        STEP_TIMEOUT,
        mcp_tester::detect_eras(&mcp_url(addr), TESTER_TIMEOUT),
    )
    .await
    .expect("detection must not hang");
    teardown(handle).await;

    assert_eq!(
        verdict,
        EraSupport::Dual,
        "a pmcp server that opted into 2026-07-28 STILL serves 2025-11-25 — \
         per-request era negotiation is the whole dual-version design, so DUAL \
         is the EXPECTED verdict here, not an exotic one"
    );
}

#[tokio::test]
async fn detect_eras_reports_v1_only_for_a_server_that_did_not_opt_in() {
    let (addr, handle) = spawn(server_accepting(vec![V1])).await;
    let verdict = tokio::time::timeout(
        STEP_TIMEOUT,
        mcp_tester::detect_eras(&mcp_url(addr), TESTER_TIMEOUT),
    )
    .await
    .expect("detection must not hang");
    teardown(handle).await;

    assert_eq!(
        verdict,
        EraSupport::V1Only,
        "a server with the default v1-only accept-list must not be reported as \
         serving v2"
    );
}

#[tokio::test]
async fn detect_eras_reports_unreachable_when_nothing_listens() {
    // Bind, read the port, then drop the listener so the port is free.
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    drop(listener);

    let verdict = tokio::time::timeout(
        STEP_TIMEOUT,
        mcp_tester::detect_eras(&mcp_url(addr), Duration::from_secs(2)),
    )
    .await
    .expect("detection must not hang");

    assert_eq!(
        verdict,
        EraSupport::Unreachable,
        "a port nothing listens on is an INFRASTRUCTURE fault, and must not be \
         reported as a server that speaks no era"
    );
    // The two "neither" outcomes must stay distinguishable.
    assert_ne!(verdict, EraSupport::NoEraSpoken);
}

// ===========================================================================
// 2. A real dual run.
// ===========================================================================

#[tokio::test]
async fn dual_run_against_a_dual_era_server_classifies_against_the_baseline() {
    let (addr, handle) = spawn(server_accepting(vec![V1, PROTOCOL_VERSION_2026_07_28])).await;
    let url = mcp_url(addr);

    let runner = ConformanceRunner::new(false, Some(vec![ConformanceDomain::Core]));
    let mut v1 = tester(&url);
    let mut v2 = v2_tester(&url);
    let report = tokio::time::timeout(STEP_TIMEOUT, runner.run_dual(&mut v1, &mut v2))
        .await
        .expect("a dual run must not hang");
    teardown(handle).await;

    assert_eq!(report.schema_version, 1);
    assert_eq!(report.era_support, "dual");
    assert!(
        report.v1_report.summary.total > 0,
        "the v1 suite must actually have run"
    );
    assert!(
        report.v2_report.summary.total > 0,
        "the v2 suite must actually have run"
    );

    // THE ANTI-VACUITY CHECK, live: the baseline records fourteen differences
    // that are correct by design, so two era runs against a dual-era server
    // MUST produce a non-empty classification.
    assert!(
        !report.differences.is_empty(),
        "an empty difference list against a dual-era server means the comparison \
         did not run; suspicion field said: {:?}",
        report.suspicion
    );
    assert!(
        report.suspicion.is_none(),
        "a non-empty comparison must not be flagged suspicious: {:?}",
        report.suspicion
    );

    // ERA-02 reproduces: a v1 server answers -32601 to `server/discover` and a
    // v2 server serves it. This is the strong positive assertion that the join
    // rule really does match a recorded delta against live wire behaviour.
    let discover = report
        .differences
        .iter()
        .find(|d| d.observation_id == METHOD_SERVER_DISCOVER.as_str())
        .expect("method.server_discover must be classified");
    assert_eq!(
        discover.class,
        DifferenceClass::Expected,
        "a v1 server answers -32601 to server/discover and a v2 server serves \
         it; that is ERA-02. Observed v1={:?} v2={:?}",
        discover.v1,
        discover.v2
    );

    // ERA-01 is classified MISSING — the server still answers `initialize` on
    // the v2 wire. See `the_server_still_answers_initialize_on_the_v2_wire`,
    // which pins the evidence.
    let initialize = report
        .differences
        .iter()
        .find(|d| d.observation_id == METHOD_INITIALIZE.as_str())
        .expect("every baseline entry must be classified, including ones that do not reproduce");
    assert_eq!(
        initialize.class,
        DifferenceClass::Missing,
        "ERA-01 does not currently reproduce server-side. Observed v1={:?} v2={:?}",
        initialize.v1,
        initialize.v2
    );

    // The join rule must FIRE on a broad set of live wire facts. A comparison in
    // which nothing matched is indistinguishable from "the eras agree" without
    // this floor, and a floor of 1 would not catch a rule that matched only by
    // accident.
    assert!(
        report.count(DifferenceClass::Expected) >= 6,
        "only {} of {} rows matched the baseline; the join rule may be \
         misfiring. Rows: {:#?}",
        report.count(DifferenceClass::Expected),
        report.differences.len(),
        report.differences
    );

    // Against a stock opted-in pmcp server nothing should differ in a way the
    // baseline does not document. If this fires, a real behaviour change
    // happened — investigate it before relaxing the assertion.
    let unexpected: Vec<&str> = report
        .differences
        .iter()
        .filter(|d| d.class == DifferenceClass::Unexpected)
        .map(|d| d.observation_id.as_str())
        .collect();
    assert!(
        unexpected.is_empty(),
        "UNDOCUMENTED era differences: {unexpected:?}. Full rows: {:#?}",
        report.findings()
    );

    // Rendering is byte-capturable and reports the three classes distinctly.
    let mut sink = Vec::<u8>::new();
    report.print_to_writer(&mut sink).expect("render");
    let text = String::from_utf8(sink).expect("utf8");
    assert!(text.contains("DUAL-RUN ERA COMPARISON"), "{text}");
    assert!(text.contains("EXPECTED ("), "{text}");
}

/// Every probe must be ATTEMPTED against a real server, and the coverage
/// contract with the baseline must hold both ways at runtime, not just in the
/// unit tests.
#[tokio::test]
async fn every_probe_runs_against_a_real_server() {
    let (addr, handle) = spawn(server_accepting(vec![V1, PROTOCOL_VERSION_2026_07_28])).await;
    let url = mcp_url(addr);

    let mut v2 = v2_tester(&url);
    // Establish the connection first — the capability probe reads the
    // projection the connection holds.
    let _ = tokio::time::timeout(STEP_TIMEOUT, v2.test_initialize())
        .await
        .expect("connect must not hang");
    let observations: EraObservations = tokio::time::timeout(
        STEP_TIMEOUT,
        mcp_tester::observe_era(&v2, pmcp::types::protocol::Era::V2),
    )
    .await
    .expect("observation must not hang");
    teardown(handle).await;

    let observed: Vec<ObservationId> = observations.ids();
    assert_eq!(
        observed.len(),
        PROBE_REGISTRY.len(),
        "every registered probe must record an observation; missing: {:?}",
        PROBE_REGISTRY
            .iter()
            .filter(|id| !observed.contains(id))
            .collect::<Vec<_>>()
    );

    // Most probes must have ESTABLISHED their fact against a live server. A
    // suite where everything came back `unavailable` would classify nothing and
    // is indistinguishable from success without this assertion.
    let established = observations
        .ids()
        .into_iter()
        .filter(|id| {
            observations
                .get(*id)
                .is_some_and(ObservedValue::is_established)
        })
        .count();
    assert!(
        established >= PROBE_REGISTRY.len() - 1,
        "only {established}/{} probes established a fact against a live server",
        PROBE_REGISTRY.len()
    );

    let baseline = load_default_baseline().expect("baseline loads");
    let (differences, _) = compare_eras(&observations, &observations, &baseline);
    assert_eq!(
        differences.len(),
        baseline.deltas.len(),
        "comparing a run against ITSELF must report every baseline delta as \
         MISSING — nothing differs from itself"
    );
    assert!(differences
        .iter()
        .all(|d| d.class == DifferenceClass::Missing));
}

// ===========================================================================
// 3. Degradation.
// ===========================================================================

#[tokio::test]
async fn a_v1_only_server_degrades_to_a_single_v1_run() {
    let (addr, handle) = spawn(server_accepting(vec![V1])).await;
    let url = mcp_url(addr);

    let verdict = tokio::time::timeout(STEP_TIMEOUT, mcp_tester::detect_eras(&url, TESTER_TIMEOUT))
        .await
        .expect("detection must not hang");
    assert_eq!(verdict, EraSupport::V1Only);

    // The degraded path is an ordinary single run, and it must still pass.
    let runner = ConformanceRunner::new(false, Some(vec![ConformanceDomain::Core]));
    let mut v1 = tester(&url);
    let report = tokio::time::timeout(STEP_TIMEOUT, runner.run(&mut v1))
        .await
        .expect("single run must not hang");
    teardown(handle).await;

    assert!(report.summary.total > 0);
    assert!(
        !report.has_failures(),
        "a v1-only server is NOT non-conformant — it simply has not opted into \
         2026-07-28: {:?}",
        report
            .tests
            .iter()
            .filter(|t| t.error.is_some())
            .collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn a_v2_run_establishes_without_initialize_and_c01_asserts_it() {
    let (addr, handle) = spawn(server_accepting(vec![V1, PROTOCOL_VERSION_2026_07_28])).await;
    let url = mcp_url(addr);

    let runner = ConformanceRunner::new(false, Some(vec![ConformanceDomain::Core]));
    let mut v2 = v2_tester(&url);
    let report = tokio::time::timeout(STEP_TIMEOUT, runner.run(&mut v2))
        .await
        .expect("v2 run must not hang");
    let has_initialize_result = v2.server_info().is_some();
    teardown(handle).await;

    assert!(
        !has_initialize_result,
        "a v2 connection must carry NO InitializeResult — synthesising one would \
         conceal ERA-01, the delta this tester exists to detect"
    );
    let c01 = report
        .tests
        .iter()
        .find(|t| t.name.starts_with("Core: initialize absent"))
        .expect("the v2 C-01 must run and be named for what it asserts");
    // C-01 currently FAILS, and that is the CORRECT result — see
    // `the_server_still_answers_initialize_on_the_v2_wire`. Asserting `Passed`
    // here would require weakening the probe until it stopped detecting the
    // very thing it was written to detect.
    assert_eq!(
        c01.status,
        mcp_tester::TestStatus::Failed,
        "C-01 must report that the server answers `initialize` on the v2 wire; \
         if this now passes, the server side has been severed and this \
         assertion (plus the ERA-01 finding) should be updated: {:?}",
        c01.details
    );
    assert!(
        c01.error
            .as_deref()
            .is_some_and(|e| e.contains("ANSWERED `initialize`")),
        "C-01's failure must name the wire fact it observed: {:?}",
        c01.error
    );
}

/// A FINDING, recorded as a test rather than as prose.
///
/// The baseline's ERA-01 records `initialize` as `served` on v1 and `absent` on
/// v2. MEASURED here against a real opted-in `pmcp` server: the SERVER still
/// answers a well-formed `initialize` on the `2026-07-28` wire, with HTTP 200
/// and a result — and that result is a MIXED envelope, carrying the v1
/// `protocolVersion: 2025-11-25` alongside the v2 `resultType` and
/// `_meta["io.modelcontextprotocol/serverInfo"]`.
///
/// ERA-01's own `source` column cites only CLIENT-side artifacts
/// (`REQUIREMENTS.md:911 (CLNT-01)`, `src/client/mod.rs:726-741`
/// `v2_synthetic_initialize_result`). The client's `initialize` is indeed local
/// and synthetic; the SERVER's `initialize` was never severed. So the delta is
/// real as written about the client and does not reproduce on the server.
///
/// The baseline is deliberately NOT edited to match. It is the phase's spec
/// artifact, and rewriting it so the comparison goes quiet is exactly the
/// re-recorded-golden anti-pattern `tests/report_compat.rs` warns about. The
/// tester reporting ERA-01 as MISSING is the tool working.
///
/// # The trap this test also closes
///
/// A probe whose `initialize` params omit `clientInfo`/`capabilities` is refused
/// `-32601` by the TYPED PARSE, before dispatch. Refusing a MALFORMED request is
/// not evidence that the METHOD is gone, so a probe built that way would report
/// `absent` against a server that serves `initialize` perfectly well. Both
/// shapes are asserted below so the distinction cannot be lost.
#[tokio::test]
async fn the_server_still_answers_initialize_on_the_v2_wire() {
    let (addr, handle) = spawn(server_accepting(vec![V1, PROTOCOL_VERSION_2026_07_28])).await;
    let url = mcp_url(addr);
    let mut v2 = v2_tester(&url);
    let connected = tokio::time::timeout(STEP_TIMEOUT, v2.test_initialize())
        .await
        .expect("connect must not hang");
    assert_eq!(connected.status, mcp_tester::TestStatus::Passed);

    let malformed = v2
        .raw_jsonrpc_probe(
            "initialize",
            "",
            json!({ "protocolVersion": PROTOCOL_VERSION_2026_07_28 }),
            pmcp::types::protocol::Era::V2,
            mcp_tester::tester::V2HeaderMode::Standard,
        )
        .await
        .expect("probe completes");
    let well_formed = v2
        .raw_jsonrpc_probe(
            "initialize",
            "",
            json!({
                "protocolVersion": PROTOCOL_VERSION_2026_07_28,
                "clientInfo": { "name": "mcp-tester", "version": "0.7.0" },
                "capabilities": {},
            }),
            pmcp::types::protocol::Era::V2,
            mcp_tester::tester::V2HeaderMode::Standard,
        )
        .await
        .expect("probe completes");
    teardown(handle).await;

    assert_eq!(
        malformed.error_code,
        Some(-32601),
        "an initialize whose params do not parse is refused before dispatch —          which is NOT evidence that the method is absent"
    );
    assert!(
        well_formed.result.is_some(),
        "FINDING RESOLVED? The server now refuses a well-formed `initialize` on          the 2026-07-28 wire, so ERA-01 reproduces server-side. Update this          test, C-01's expected status, and note it in the baseline. Observed:          {well_formed:?}"
    );
    let result = well_formed.result.expect("checked above");
    assert_eq!(
        result["protocolVersion"], "2025-11-25",
        "the served result is a MIXED envelope: a v1 protocolVersion alongside          v2 fields. Observed: {result}"
    );
    assert!(
        !result["resultType"].is_null(),
        "…and it carries the v2 `resultType`: {result}"
    );
}

// ===========================================================================
// 4. The additivity contract.
// ===========================================================================

/// Single-run output must be byte-identical to 0.7.0.
///
/// Captured through the SAME writer seam `tests/report_compat.rs` uses, and
/// asserted against that file's own criterion rather than a second, weaker
/// notion of "unchanged": a re-derived check could pass while the pinned
/// goldens failed.
#[tokio::test]
async fn single_run_output_is_unchanged_by_the_dual_run_work() {
    let (addr, handle) = spawn(server_accepting(vec![V1])).await;
    let url = mcp_url(addr);

    let runner = ConformanceRunner::new(false, Some(vec![ConformanceDomain::Core]));
    let mut v1 = tester(&url);
    let report: TestReport = tokio::time::timeout(STEP_TIMEOUT, runner.run(&mut v1))
        .await
        .expect("single run must not hang");
    teardown(handle).await;

    let mut sink = Vec::<u8>::new();
    report
        .print_to_writer(OutputFormat::Json, &mut sink)
        .expect("writing a report into a Vec<u8> cannot fail");
    let json = String::from_utf8(sink).expect("report output must be valid UTF-8");

    // The 0.7.0 JSON shape: exactly these top-level keys, nothing added.
    let parsed: Value = serde_json::from_str(&json).expect("valid JSON");
    let mut keys: Vec<&str> = parsed
        .as_object()
        .expect("object")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        vec!["duration", "summary", "tests", "timestamp"],
        "a new top-level key on TestReport is exactly what A-D11 forbids; the \
         dual-run data belongs in DualRunReport"
    );
    let test_keys: Vec<&str> = {
        let mut k: Vec<&str> = parsed["tests"][0]
            .as_object()
            .expect("object")
            .keys()
            .map(String::as_str)
            .collect();
        k.sort_unstable();
        k
    };
    // MEASURED against 0.7.0, not guessed: `TestResult` carries no
    // `skip_serializing_if`, so all SIX fields are always emitted. The point of
    // the assertion is that the set is exactly six and no seventh appears —
    // `cargo-pmcp` struct-literals this type positionally, so a new field is a
    // hard workspace compile break (A-D11).
    assert_eq!(
        test_keys,
        vec!["category", "details", "duration", "error", "name", "status"],
        "no field may be ADDED to TestResult"
    );
    assert!(
        !json.contains("dual"),
        "no dual-run data may appear on the single-run path: {json}"
    );
}

// ===========================================================================
// 5. The `_meta` reserved-key DRIFT TRIPWIRE.
// ===========================================================================

/// `era_observations` builds raw v2 requests using literal reserved `_meta`
/// keys, because pmcp's own constants are `pub(crate)` and their only public
/// re-export is behind the `testing` feature this crate does not enable.
///
/// This test closes the drift risk NON-CIRCULARLY: it captures the bytes a REAL,
/// SDK-built v2 `pmcp::Client` puts on the wire and asserts the literal appears
/// in them. If the SDK ever renames the key, this fails — it is comparing the
/// constant against the SDK's behaviour, not against itself.
#[tokio::test]
async fn the_sdk_emits_the_reserved_meta_key_this_crate_spells() {
    let captured: Arc<tokio::sync::Mutex<Vec<String>>> = Arc::new(tokio::sync::Mutex::new(vec![]));
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    let sink = captured.clone();
    let handle = tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            let sink = sink.clone();
            tokio::spawn(async move {
                let mut buf = vec![0u8; 8192];
                let Ok(n) = socket.read(&mut buf).await else {
                    return;
                };
                sink.lock()
                    .await
                    .push(String::from_utf8_lossy(&buf[..n]).into_owned());
                let body = r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32601,"message":"no"}}"#;
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(response.as_bytes()).await;
                let _ = socket.shutdown().await;
            });
        }
    });

    let mut v2 = v2_tester(&mcp_url(addr));
    let _ = tokio::time::timeout(STEP_TIMEOUT, v2.test_initialize()).await;
    teardown(handle).await;

    let requests = captured.lock().await.clone();
    assert!(
        !requests.is_empty(),
        "the SDK-built v2 client must have sent at least one request"
    );
    let all = requests.join("\n");
    assert!(
        all.contains(mcp_tester::tester::RESERVED_PROTOCOL_VERSION_KEY),
        "the SDK no longer emits `{}` — era_observations' literal has DRIFTED \
         from the crate. Captured:\n{all}",
        mcp_tester::tester::RESERVED_PROTOCOL_VERSION_KEY
    );
}

// ===========================================================================
// 6. The session-leak assertion (Pitfall 5).
// ===========================================================================

/// N detections against a session-minting server must not grow its session
/// count by N.
///
/// This is the one case the stub exists for: pmcp exposes no session-count
/// accessor, so the only way to OBSERVE the mitigation is to count what the
/// server was asked to do. The stub mints a session id on every `initialize`
/// and counts the `DELETE`s it receives; a detector that leaked would mint
/// without ever deleting.
#[tokio::test]
async fn era_detection_does_not_leak_a_session_per_invocation() {
    let minted = Arc::new(AtomicUsize::new(0));
    let deleted = Arc::new(AtomicUsize::new(0));

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    let (m, d) = (minted.clone(), deleted.clone());
    let handle = tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            let (m, d) = (m.clone(), d.clone());
            tokio::spawn(async move {
                let mut buf = vec![0u8; 8192];
                let Ok(n) = socket.read(&mut buf).await else {
                    return;
                };
                let request = String::from_utf8_lossy(&buf[..n]).into_owned();
                let response = if request.starts_with("DELETE") {
                    d.fetch_add(1, Ordering::SeqCst);
                    "HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_string()
                } else if request.contains("\"initialize\"") {
                    m.fetch_add(1, Ordering::SeqCst);
                    let body = json!({
                        "jsonrpc": "2.0",
                        "id": 1,
                        "result": {
                            "protocolVersion": V1,
                            "capabilities": {},
                            "serverInfo": { "name": "stub", "version": "0.0.0" },
                        }
                    })
                    .to_string();
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\
                         mcp-session-id: stub-session\r\nContent-Length: {}\r\n\r\n{body}",
                        body.len()
                    )
                } else {
                    let body = r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32601,"message":"no"}}"#;
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
                        body.len()
                    )
                };
                let _ = socket.write_all(response.as_bytes()).await;
                let _ = socket.shutdown().await;
            });
        }
    });

    let url = mcp_url(addr);
    const DETECTIONS: usize = 3;
    for _ in 0..DETECTIONS {
        let verdict = tokio::time::timeout(
            STEP_TIMEOUT,
            mcp_tester::detect_eras(&url, Duration::from_secs(5)),
        )
        .await
        .expect("detection must not hang");
        assert_eq!(
            verdict,
            EraSupport::V1Only,
            "the stub answers initialize and refuses server/discover"
        );
    }
    // Give the last DELETE's connection task a bounded moment to be accepted.
    for _ in 0..40 {
        if deleted.load(Ordering::SeqCst) >= DETECTIONS {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    teardown(handle).await;

    let minted = minted.load(Ordering::SeqCst);
    let deleted = deleted.load(Ordering::SeqCst);
    assert_eq!(
        minted, DETECTIONS,
        "each detection makes exactly one v1 initialize attempt"
    );
    assert_eq!(
        deleted,
        minted,
        "every session the detector minted must be torn down with a DELETE; \
         {minted} minted but only {deleted} deleted means the net session count \
         grew by {} per {DETECTIONS} detections (Pitfall 5)",
        minted - deleted
    );
}

// ===========================================================================
// 7. The BINARY, run in both modes against a live dual-era server.
// ===========================================================================

/// Run the real `mcp-tester` binary in both modes and pin the contract between
/// them.
///
/// `CARGO_BIN_EXE_mcp-tester` is the binary cargo just built, so this is the
/// shipped CLI — not a library call dressed up as one. Running BOTH modes in one
/// test is the point: the additivity claim is a claim about the DIFFERENCE
/// between two invocations, and asserting it anywhere else would compare against
/// a re-derived notion of "unchanged".
#[tokio::test(flavor = "multi_thread")]
async fn the_binary_runs_in_both_modes_against_a_live_server() {
    let (addr, handle) = spawn(server_accepting(vec![V1, PROTOCOL_VERSION_2026_07_28])).await;
    let url = mcp_url(addr);
    let exe = env!("CARGO_BIN_EXE_mcp-tester");

    let run = |extra: Vec<&str>| {
        let mut cmd = std::process::Command::new(exe);
        cmd.arg("conformance")
            .arg(&url)
            .args(["--domain", "core"])
            .args(extra);
        cmd.output().expect("the binary runs")
    };

    let plain = tokio::task::block_in_place(|| run(vec![]));
    let dual = tokio::task::block_in_place(|| run(vec!["--dual-run"]));
    teardown(handle).await;

    let plain_out = String::from_utf8_lossy(&plain.stdout).into_owned();
    let dual_out = String::from_utf8_lossy(&dual.stdout).into_owned();

    // Single run: no comparison section anywhere.
    assert!(
        !plain_out.contains("ERA COMPARISON"),
        "the comparison must be printed ONLY when --dual-run is passed:\n{plain_out}"
    );
    assert!(
        plain_out.contains("Core: initialize handshake"),
        "the single run must still be a v1 run:\n{plain_out}"
    );

    // Dual run: the comparison is printed, with all three class labels
    // available and the era verdict named.
    assert!(
        dual_out.contains("DUAL-RUN ERA COMPARISON"),
        "--dual-run must print the comparison:\n{dual_out}"
    );
    assert!(
        dual_out.contains("Era support : dual"),
        "the detected era support must be reported:\n{dual_out}"
    );
    assert!(
        dual_out.contains("EXPECTED (") && dual_out.contains("MISSING ("),
        "expected and missing must be rendered as distinct sections:\n{dual_out}"
    );

    // The exit code keeps meaning "did the suite pass" in BOTH modes: --dual-run
    // returns the v1 report, so a passing v1 server still exits 0 even though
    // the comparison carries findings.
    assert_eq!(
        plain.status.code(),
        Some(0),
        "a conformant v1 run exits 0:\n{plain_out}"
    );
    assert_eq!(
        dual.status.code(),
        Some(0),
        "--dual-run must not change the exit-code contract:\n{dual_out}"
    );
}
