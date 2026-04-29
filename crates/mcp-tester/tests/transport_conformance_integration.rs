//! Integration tests for the `Transport` conformance domain.
//!
//! These tests prove the wiring end-to-end: build a `ServerTester`, run the
//! `Transport` domain via `ConformanceRunner`, and assert the probes hit a
//! real HTTP server and classify its responses correctly.
//!
//! We use a hand-rolled `tokio::net::TcpListener` stub instead of an
//! in-process `pmcp` `streamable_http_server` because:
//! 1. The plan permits this fallback ("PROVE the wiring end-to-end against
//!    SOME real HTTP server, not specifically the pmcp one").
//! 2. We need to deliberately produce the regression response shape
//!    (`200 + application/json + non-SSE body`) — easier with canned bytes
//!    than with the pmcp server (which is correct by construction).

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use mcp_tester::{ConformanceDomain, ConformanceRunner, ServerTester, TestCategory, TestStatus};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

type Handler = Arc<dyn Fn(&str) -> Vec<u8> + Send + Sync + 'static>;

/// Spawn a one-shot HTTP/1.1 stub. Each accepted connection reads one request,
/// runs `handler(request_text)` to produce the response bytes, writes them,
/// and closes. Returns the bound `SocketAddr` and the accept-loop `JoinHandle`
/// so the test can `abort()` it on teardown.
async fn spawn_stub_server(handler: Handler) -> (SocketAddr, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind 127.0.0.1:0");
    let addr = listener.local_addr().expect("local_addr");

    let join = tokio::spawn(async move {
        loop {
            let (mut socket, _peer) = match listener.accept().await {
                Ok(x) => x,
                Err(_) => break,
            };
            let handler = handler.clone();
            tokio::spawn(async move {
                let mut buf = vec![0u8; 4096];
                let n = match socket.read(&mut buf).await {
                    Ok(n) => n,
                    Err(_) => return,
                };
                let request_text = String::from_utf8_lossy(&buf[..n]).into_owned();
                let response = handler(&request_text);
                let _ = socket.write_all(&response).await;
                let _ = socket.shutdown().await;
            });
        }
    });

    (addr, join)
}

fn http_response(status_line: &str, content_type: Option<&str>, body: &str) -> Vec<u8> {
    let mut header = format!("{status_line}\r\nContent-Length: {}\r\n", body.len());
    if let Some(ct) = content_type {
        header.push_str(&format!("Content-Type: {ct}\r\n"));
    }
    header.push_str("Connection: close\r\n\r\n");
    let mut out = header.into_bytes();
    out.extend_from_slice(body.as_bytes());
    out
}

/// Spec-compliant stateless responses. Mirrors the canonical pmcp
/// `streamable_http_server::handle_get` 405-with-JSON-RPC branch.
fn good_handler(req: &str) -> Vec<u8> {
    let method = req.split_whitespace().next().unwrap_or("");
    match method {
        "GET" => http_response(
            "HTTP/1.1 405 Method Not Allowed",
            Some("application/json"),
            r#"{"jsonrpc":"2.0","error":{"code":-32601,"message":"SSE not supported in stateless mode"},"id":null}"#,
        ),
        "OPTIONS" => {
            let body = "";
            let mut header = format!(
                "HTTP/1.1 204 No Content\r\nContent-Length: {}\r\n",
                body.len()
            );
            header.push_str("Access-Control-Allow-Origin: *\r\n");
            header.push_str("Access-Control-Allow-Methods: POST, GET, OPTIONS, DELETE\r\n");
            header.push_str("Connection: close\r\n\r\n");
            header.into_bytes()
        },
        "DELETE" => http_response("HTTP/1.1 405 Method Not Allowed", None, ""),
        _ => http_response("HTTP/1.1 404 Not Found", None, ""),
    }
}

/// Regression simulator: a misconfigured edge layer that rewrites
/// `GET /mcp` into a JSON health endpoint. This is the failure mode
/// the Transport domain was built to catch.
fn regression_handler(req: &str) -> Vec<u8> {
    let method = req.split_whitespace().next().unwrap_or("");
    match method {
        "GET" => http_response(
            "HTTP/1.1 200 OK",
            Some("application/json"),
            r#"{"ok":true,"message":"MCP Server. POST JSON-RPC to '/' for MCP requests."}"#,
        ),
        "OPTIONS" => {
            let body = "";
            let mut header = format!(
                "HTTP/1.1 204 No Content\r\nContent-Length: {}\r\n",
                body.len()
            );
            header.push_str("Access-Control-Allow-Origin: *\r\n");
            header.push_str("Connection: close\r\n\r\n");
            header.into_bytes()
        },
        _ => http_response(
            "HTTP/1.1 404 Not Found",
            Some("application/json"),
            r#"{"message":"Not Found"}"#,
        ),
    }
}

fn build_tester(addr: SocketAddr) -> ServerTester {
    let url = format!("http://{addr}/mcp");
    ServerTester::new(
        &url,
        Duration::from_secs(5),
        false,
        None,
        Some("http"),
        None,
    )
    .expect("ServerTester::new")
}

async fn run_transport_only(tester: &mut ServerTester) -> mcp_tester::TestReport {
    let runner = ConformanceRunner::new(false, Some(vec![ConformanceDomain::Transport]));
    runner.run(tester).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn transport_domain_passes_against_spec_compliant_stub() {
    let (addr, handle) = spawn_stub_server(Arc::new(good_handler)).await;
    let mut tester = build_tester(addr);

    let report = run_transport_only(&mut tester).await;
    handle.abort();

    assert!(
        !report.has_failures(),
        "spec-compliant stub must not fail Transport: {:?}",
        report
            .tests
            .iter()
            .map(|t| (t.name.as_str(), &t.status, t.error.as_deref()))
            .collect::<Vec<_>>()
    );

    let get = report
        .tests
        .iter()
        .find(|t| t.name.starts_with("Transport: GET /mcp"))
        .expect("GET /mcp test exists");
    assert_eq!(get.category, TestCategory::Transport);
    assert_eq!(
        get.status,
        TestStatus::Passed,
        "GET /mcp must pass on a 405+JSON-RPC server; details={:?} error={:?}",
        get.details,
        get.error
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn transport_domain_catches_get_mcp_regression() {
    let (addr, handle) = spawn_stub_server(Arc::new(regression_handler)).await;
    let mut tester = build_tester(addr);

    let report = run_transport_only(&mut tester).await;
    handle.abort();

    let get = report
        .tests
        .iter()
        .find(|t| t.name.starts_with("Transport: GET /mcp"))
        .expect("GET /mcp test exists");
    assert_eq!(get.category, TestCategory::Transport);
    assert_eq!(
        get.status,
        TestStatus::Failed,
        "GET /mcp must fail on the 200+JSON-non-SSE regression; details={:?} error={:?}",
        get.details,
        get.error
    );

    let err = get.error.as_deref().unwrap_or("");
    assert!(
        err.contains("status=200"),
        "failure detail must include status=200; got: {err}"
    );
    assert!(
        err.contains("content-type=application/json"),
        "failure detail must include content-type=application/json; got: {err}"
    );
    // The body prefix is what makes the regression diagnosable in <60s.
    assert!(
        err.contains("body_prefix="),
        "failure detail must include body_prefix=; got: {err}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn transport_domain_options_passes_with_cors() {
    let (addr, handle) = spawn_stub_server(Arc::new(good_handler)).await;
    let mut tester = build_tester(addr);

    let report = run_transport_only(&mut tester).await;
    handle.abort();

    let opts = report
        .tests
        .iter()
        .find(|t| t.name.starts_with("Transport: OPTIONS /mcp"))
        .expect("OPTIONS /mcp test exists");
    assert_eq!(opts.status, TestStatus::Passed);
}
