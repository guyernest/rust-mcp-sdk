//! D-02 / OAPI-10 — engine-parity proof: a script tool and `execute_code` run
//! the SAME script through the SAME engine and produce IDENTICAL output.
//!
//! This is the headline decision made provable: ONE engine, two surfaces. A
//! script promoted from Code Mode (`execute_code`, via `JsCodeExecutor`) to a
//! curated script tool (`ScriptToolHandler`) behaves identically — both compile
//! the JS with `PlanCompiler`, run it with `PlanExecutor` over the SAME
//! `HttpCodeExecutor`, and bind the client args to `args`.
//!
//! The `engine_parity` test asserts:
//! - the two surfaces return byte-equal `serde_json::Value` output, and
//! - both issue the SAME backend request sequence against the wiremock server
//!   (so parity is engine behavior, not just a coincidental final value).
//!
//! Offline (wiremock only — D-04). Run with:
//! `cargo test -p pmcp-server-toolkit --features openapi-code-mode \
//! --test script_tool_engine_parity -- --test-threads=1`.

#![cfg(feature = "openapi-code-mode")]

use std::sync::Arc;

use pmcp::RequestHandlerExtra;
use pmcp_code_mode::{CodeExecutor, ExecutionConfig, JsCodeExecutor};
use pmcp_server_toolkit::code_mode::HttpCodeExecutor;
use pmcp_server_toolkit::config::{ParamDecl, ServerConfig, ServerSection, ToolDecl};
use pmcp_server_toolkit::http::auth::{create_auth_provider, AuthConfig};
use pmcp_server_toolkit::http::{HttpClient, HttpConnector};
use pmcp_server_toolkit::synthesize_from_config_with_http_connector_and_scripts;
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// The ONE script `S` exercised by BOTH surfaces — the disrupted-lines chain
/// (filter + per-line `api.get`, reading `args.maxLines`).
const SCRIPT: &str = r#"
const statuses = await api.get('/Line/Mode/tube/Status');
const disrupted = statuses.filter(line => line.lineStatuses.some(s => s.statusSeverity < 10));
const out = [];
for (const line of disrupted) {
  const detail = await api.get(`/Line/${line.id}/Disruption`);
  out.push({ line: line.name, detail, max: args.maxLines });
}
return { count: out.length, lines: out };
"#;

/// Mount the london-tube-shaped responses (one disrupted line + one good line +
/// the per-line disruption detail).
async fn mount_tube(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/Line/Mode/tube/Status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            { "id": "victoria", "name": "Victoria", "lineStatuses": [ { "statusSeverity": 6 } ] },
            { "id": "central", "name": "Central", "lineStatuses": [ { "statusSeverity": 10 } ] }
        ])))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/Line/victoria/Disruption"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "description": "Severe delays" })),
        )
        .mount(server)
        .await;
}

/// Build the shared `HttpCodeExecutor` (the SAME engine seam both surfaces use)
/// over `base_url` with `NoAuth`, plus a connector for the synthesizer signature.
fn shared_executor(base_url: &str) -> (Arc<dyn HttpConnector>, HttpCodeExecutor) {
    let auth = create_auth_provider(&AuthConfig::None).expect("noauth");
    let connector: Arc<dyn HttpConnector> = Arc::new(
        HttpClient::new(reqwest::Client::new(), base_url.to_string(), auth.clone())
            .expect("build http client"),
    );
    let http_exec = HttpCodeExecutor::new(reqwest::Client::new(), base_url.to_string(), auth);
    (connector, http_exec)
}

/// Collect the (method, path) request sequence the mock observed, sorted for a
/// set-equality comparison that is robust to harmless ordering differences while
/// still proving both paths hit the SAME backend endpoints the SAME number of
/// times.
async fn observed_requests(server: &MockServer) -> Vec<(String, String)> {
    let mut seq: Vec<(String, String)> = server
        .received_requests()
        .await
        .expect("wiremock records requests")
        .iter()
        .map(|r| (r.method.to_string(), r.url.path().to_string()))
        .collect();
    seq.sort();
    seq
}

#[tokio::test]
async fn engine_parity_script_tool_equals_execute_code() {
    let args = json!({ "maxLines": 3 });

    // ---- Path A: ScriptToolHandler (the curated script-tool surface) ----
    let server_a = MockServer::start().await;
    mount_tube(&server_a).await;
    let (connector_a, http_exec_a) = shared_executor(&server_a.uri());
    let cfg = ServerConfig {
        server: ServerSection {
            name: "parity".to_string(),
            version: "0.1.0".to_string(),
            ..Default::default()
        },
        tools: vec![ToolDecl {
            name: "disrupted".to_string(),
            description: Some("disrupted lines".to_string()),
            script: Some(SCRIPT.to_string()),
            parameters: vec![ParamDecl {
                name: "maxLines".to_string(),
                param_type: Some("integer".to_string()),
                required: false,
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    let mut tools = synthesize_from_config_with_http_connector_and_scripts(
        &cfg,
        connector_a,
        http_exec_a,
        ExecutionConfig::default(),
    )
    .expect("synthesize script tool");
    let handler = tools.remove(0).2;
    let path_a_output = handler
        .handle(args.clone(), RequestHandlerExtra::default())
        .await
        .expect("script tool (Path A) must succeed");
    let path_a_requests = observed_requests(&server_a).await;

    // ---- Path B: JsCodeExecutor over the SAME HttpCodeExecutor (Code Mode) ----
    let server_b = MockServer::start().await;
    mount_tube(&server_b).await;
    let (_connector_b, http_exec_b) = shared_executor(&server_b.uri());
    let js_executor: Arc<dyn CodeExecutor> =
        Arc::new(JsCodeExecutor::new(http_exec_b, ExecutionConfig::default()));
    let path_b_output = js_executor
        .execute(SCRIPT, Some(&args))
        .await
        .expect("execute_code (Path B) must succeed");
    let path_b_requests = observed_requests(&server_b).await;

    // ---- D-02: byte-equal output across both surfaces ----
    assert_eq!(
        path_a_output, path_b_output,
        "D-02: the SAME script must produce IDENTICAL output via a script tool and via execute_code"
    );

    // ---- D-02: identical backend request sequence (engine behavior, not just value) ----
    assert_eq!(
        path_a_requests, path_b_requests,
        "D-02: both surfaces must issue the SAME backend request sequence"
    );

    // Sanity: the script actually ran the chain (not an empty no-op equality).
    assert_eq!(path_a_output["count"], json!(1));
    assert!(
        path_a_requests.len() >= 2,
        "expected the status call + at least one per-line detail call, got {path_a_requests:?}"
    );
}
