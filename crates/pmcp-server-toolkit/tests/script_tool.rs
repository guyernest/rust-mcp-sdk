//! OAPI-02b / D-01 / D-02 — `ScriptToolHandler` integration tests.
//!
//! Drives the synthesized script-tool handler (admin-authored embedded JS run
//! over the SAME `pmcp_code_mode` engine Code Mode uses) against a wiremock
//! backend, proving:
//! - (a) a single-step script (`return await api.get(...)`) returns the mock JSON,
//! - (b) the london-tube multi-call chain (`filter` → per-line `api.get`) returns
//!   the chained result,
//! - (c) `args.maxLines` from `[[tools.parameters]]` is bound into the script,
//! - (d) an `ExecutionConfig` `max_api_calls` cap aborts an over-budget script
//!   (the T-90-05-02 DoS bound) — no infinite loop.
//!
//! Run with: `cargo test -p pmcp-server-toolkit --features openapi-code-mode \
//! --test script_tool -- --test-threads=1`. Offline (wiremock only — D-04).

#![cfg(feature = "openapi-code-mode")]

use std::sync::Arc;

use pmcp::RequestHandlerExtra;
use pmcp_code_mode::ExecutionConfig;
use pmcp_server_toolkit::code_mode::HttpCodeExecutor;
use pmcp_server_toolkit::config::{ParamDecl, ServerConfig, ServerSection, ToolDecl};
use pmcp_server_toolkit::http::auth::{create_auth_provider, AuthConfig};
use pmcp_server_toolkit::http::{HttpClient, HttpConnector};
use pmcp_server_toolkit::synthesize_from_config_with_http_connector_and_scripts;
use serde_json::{json, Value};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Build a minimal `ServerConfig` carrying the given script tools.
fn cfg_with_tools(tools: Vec<ToolDecl>) -> ServerConfig {
    ServerConfig {
        server: ServerSection {
            name: "tfl-demo".to_string(),
            version: "0.1.0".to_string(),
            ..Default::default()
        },
        tools,
        ..Default::default()
    }
}

/// Build the shared backend pieces: an `HttpConnector` (for single-call tools)
/// and an `HttpCodeExecutor` (the SAME engine seam script tools + Code Mode use),
/// both pointed at `base_url` with `NoAuth`.
fn backend(base_url: &str) -> (Arc<dyn HttpConnector>, HttpCodeExecutor) {
    let auth = create_auth_provider(&AuthConfig::None).expect("noauth");
    let connector: Arc<dyn HttpConnector> = Arc::new(
        HttpClient::new(reqwest::Client::new(), base_url.to_string(), auth.clone())
            .expect("build http client"),
    );
    let http_exec = HttpCodeExecutor::new(reqwest::Client::new(), base_url.to_string(), auth);
    (connector, http_exec)
}

/// Synthesize the single script tool in `cfg` and return its handler.
fn one_script_handler(
    cfg: &ServerConfig,
    connector: Arc<dyn HttpConnector>,
    http_exec: HttpCodeExecutor,
    exec_config: ExecutionConfig,
) -> Arc<dyn pmcp::server::ToolHandler> {
    let mut out = synthesize_from_config_with_http_connector_and_scripts(
        cfg,
        connector,
        http_exec,
        exec_config,
    )
    .expect("synthesize script tool");
    assert_eq!(out.len(), 1, "exactly one tool expected");
    out.remove(0).2
}

/// (a) A single-step script returns the mocked status JSON unchanged.
#[tokio::test]
async fn script_tool_single_call_returns_mock_json() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/Line/Mode/tube/Status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            { "id": "victoria", "name": "Victoria" }
        ])))
        .mount(&server)
        .await;

    let cfg = cfg_with_tools(vec![ToolDecl {
        name: "tube_status".to_string(),
        description: Some("Raw tube status".to_string()),
        // The engine routes a top-level ApiCall through the executor only when
        // it is bound first (a bare `return await api.get(...)` reaches the
        // evaluator); bind then return — the reference scripts do the same.
        script: Some(
            "const status = await api.get('/Line/Mode/tube/Status');\nreturn status;".to_string(),
        ),
        ..Default::default()
    }]);
    let (connector, http_exec) = backend(&server.uri());
    let handler = one_script_handler(&cfg, connector, http_exec, ExecutionConfig::default());

    let out = handler
        .handle(json!({}), RequestHandlerExtra::default())
        .await
        .expect("single-call script must succeed");
    assert_eq!(out, json!([{ "id": "victoria", "name": "Victoria" }]));
}

/// Mount the london-tube-shaped responses: a `/Line/Mode/tube/Status` list with
/// one disrupted line + one good line, and per-line `/Line/{id}/Disruption`.
async fn mount_tube(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/Line/Mode/tube/Status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {
                "id": "victoria",
                "name": "Victoria",
                "lineStatuses": [ { "statusSeverity": 6 } ]
            },
            {
                "id": "central",
                "name": "Central",
                "lineStatuses": [ { "statusSeverity": 10 } ]
            }
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

/// The disrupted-lines chain from RESEARCH "Code Examples" (D-01) — filter +
/// slice(0, args.maxLines) + per-line api.get + push.
const TUBE_CHAIN: &str = r#"
const statuses = await api.get('/Line/Mode/tube/Status');
const disrupted = statuses.filter(line => line.lineStatuses.some(s => s.statusSeverity < 10));
const out = [];
for (const line of disrupted.slice(0, args.maxLines)) {
  const detail = await api.get(`/Line/${line.id}/Disruption`);
  out.push({ line: line.name, detail });
}
return { count: out.length, lines: out };
"#;

/// (b) The multi-call chain (filter/slice/map across steps) returns the chained
/// result, using `args.maxLines`.
#[tokio::test]
async fn script_tool_multi_call_chain_returns_chained_result() {
    let server = MockServer::start().await;
    mount_tube(&server).await;

    let cfg = cfg_with_tools(vec![ToolDecl {
        name: "disrupted_lines".to_string(),
        description: Some("Disrupted tube lines with detail".to_string()),
        script: Some(TUBE_CHAIN.to_string()),
        parameters: vec![ParamDecl {
            name: "maxLines".to_string(),
            param_type: Some("integer".to_string()),
            required: false,
            ..Default::default()
        }],
        ..Default::default()
    }]);
    let (connector, http_exec) = backend(&server.uri());
    let handler = one_script_handler(&cfg, connector, http_exec, ExecutionConfig::default());

    let out = handler
        .handle(json!({ "maxLines": 5 }), RequestHandlerExtra::default())
        .await
        .expect("multi-call chain must succeed");

    // Only `victoria` is disrupted (statusSeverity 6 < 10); `central` (10) is not.
    assert_eq!(out["count"], json!(1));
    let lines = out["lines"].as_array().expect("lines array");
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0]["line"], json!("Victoria"));
    assert_eq!(lines[0]["detail"]["description"], json!("Severe delays"));
}

/// (c) The client args from `[[tools.parameters]]` are bound to `args` inside
/// the script (T-90-05-03): a script that reads `args.maxLines` after a backend
/// call observes the exact value the caller supplied.
#[tokio::test]
async fn script_tool_args_max_lines_binding_is_honored() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/Line/Mode/tube/Status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            { "id": "victoria", "name": "Victoria" }
        ])))
        .mount(&server)
        .await;

    let cfg = cfg_with_tools(vec![ToolDecl {
        name: "echo_max".to_string(),
        description: Some("Echo the bound args.maxLines alongside a backend call".to_string()),
        // Make a backend call (so this exercises the executor path), then return
        // the bound arg — proving `args` carries the client-supplied value.
        script: Some(
            "const lines = await api.get('/Line/Mode/tube/Status');\n\
             return { received: args.maxLines, lineCount: lines.length };"
                .to_string(),
        ),
        parameters: vec![ParamDecl {
            name: "maxLines".to_string(),
            param_type: Some("integer".to_string()),
            required: false,
            ..Default::default()
        }],
        ..Default::default()
    }]);
    let (connector, http_exec) = backend(&server.uri());
    let handler = one_script_handler(&cfg, connector, http_exec, ExecutionConfig::default());

    let out = handler
        .handle(json!({ "maxLines": 7 }), RequestHandlerExtra::default())
        .await
        .expect("script reading args.maxLines must succeed");
    assert_eq!(
        out["received"],
        json!(7),
        "the client's args.maxLines must be bound into the script's `args`"
    );
    assert_eq!(out["lineCount"], json!(1));
}

/// (d) A `max_api_calls` cap aborts an over-budget script with a bounded error
/// (no infinite loop) — the T-90-05-02 DoS bound.
#[tokio::test]
async fn script_tool_exceeding_max_api_calls_is_bounded() {
    let server = MockServer::start().await;
    // Many disrupted lines, each requiring a per-line detail call.
    let statuses: Vec<Value> = (0..10)
        .map(|i| {
            json!({
                "id": format!("line{i}"),
                "name": format!("Line {i}"),
                "lineStatuses": [ { "statusSeverity": 6 } ]
            })
        })
        .collect();
    Mock::given(method("GET"))
        .and(path("/Line/Mode/tube/Status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!(statuses)))
        .mount(&server)
        .await;
    // Any per-line disruption call resolves (so the only bound is max_api_calls).
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "description": "x" })))
        .mount(&server)
        .await;

    let cfg = cfg_with_tools(vec![ToolDecl {
        name: "disrupted_lines".to_string(),
        description: Some("Disrupted tube lines".to_string()),
        script: Some(TUBE_CHAIN.to_string()),
        parameters: vec![ParamDecl {
            name: "maxLines".to_string(),
            param_type: Some("integer".to_string()),
            required: false,
            ..Default::default()
        }],
        ..Default::default()
    }]);
    let (connector, http_exec) = backend(&server.uri());
    // Cap at 2 API calls: the status call + one detail call, then the next
    // detail call must abort. maxLines large enough to exceed the budget.
    let bounded = ExecutionConfig {
        max_api_calls: 2,
        ..ExecutionConfig::default()
    };
    let handler = one_script_handler(&cfg, connector, http_exec, bounded);

    let err = handler
        .handle(json!({ "maxLines": 10 }), RequestHandlerExtra::default())
        .await
        .expect_err("an over-budget script must abort, not run unbounded");
    let rendered = err.to_string();
    assert!(
        rendered.contains("script execution failed"),
        "expected a bounded execution error, got: {rendered}"
    );
}
