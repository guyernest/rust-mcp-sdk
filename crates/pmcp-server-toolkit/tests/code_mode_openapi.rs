//! OAPI-10 / D-02 — the generalized `code_mode_tools_from_executor` serves BOTH
//! SQL and OpenAPI flavors, and the OpenAPI `validate_code` path REALLY runs JS
//! validation (Codex HIGH: valid passes, invalid fails, disallowed op rejected).
//!
//! Run with: `cargo test -p pmcp-server-toolkit --features openapi-code-mode \
//! --test code_mode_openapi -- --test-threads=1`. Test fns are `code_mode_`- /
//! `openapi_`-prefixed so the positional `code_mode` verify filter resolves.

#![cfg(feature = "openapi-code-mode")]

use std::sync::Arc;

use pmcp::Server;
use pmcp_server_toolkit::code_mode::{
    code_mode_tools_from_executor, CodeExecutor, ExecutionConfig, HttpCodeExecutor, JsCodeExecutor,
    ValidationFlavor,
};
use pmcp_server_toolkit::config::ServerConfig;
use pmcp_server_toolkit::http::auth::{create_auth_provider, AuthConfig};

/// A read-only `[code_mode]` config with an inline dev secret (these tests
/// exercise wiring + JS validation, not secret resolution).
fn code_mode_config() -> ServerConfig {
    let toml = r#"
[server]
name = "code-mode-openapi-test"
version = "0.1.0"

[code_mode]
enabled = true
server_id = "code-mode-openapi-test"
allow_writes = false
allow_deletes = false
allow_ddl = false
token_secret = "code-mode-openapi-secret-16-or-more"
allow_inline_token_secret_for_dev = true
"#;
    ServerConfig::from_toml_strict_validated(toml).expect("config parses + validates")
}

#[cfg(feature = "sqlite")]
fn sql_executor(cfg: &ServerConfig) -> Arc<dyn CodeExecutor> {
    use pmcp_server_toolkit::code_mode::SqlCodeExecutor;
    use pmcp_server_toolkit::sql::{SqlConnector, SqliteConnector};
    let connector: Arc<dyn SqlConnector> =
        Arc::new(SqliteConnector::open_in_memory().expect("open in-memory sqlite"));
    Arc::new(SqlCodeExecutor::new(connector, cfg.clone()).expect("build sql executor"))
}

fn http_js_executor() -> Arc<dyn CodeExecutor> {
    let auth = create_auth_provider(&AuthConfig::None).expect("noauth");
    let http = HttpCodeExecutor::new(
        reqwest::Client::new(),
        "https://api.example.com".to_string(),
        auth,
    );
    Arc::new(JsCodeExecutor::new(http, ExecutionConfig::default()))
}

#[cfg(feature = "sqlite")]
#[test]
fn code_mode_generalized_sql_still_works() {
    // SQL flavor: wire with a SqlCodeExecutor + ValidationFlavor::Sql; both
    // tools register (the existing SQL anchor behavior is preserved through the
    // generalized signature).
    let cfg = code_mode_config();
    let builder = Server::builder().name("t").version("0.1.0");
    let server =
        code_mode_tools_from_executor(builder, &cfg, sql_executor(&cfg), ValidationFlavor::Sql)
            .expect("SQL flavor wiring succeeds")
            .build()
            .expect("server builds");
    assert!(server.get_tool("validate_code").is_some());
    assert!(server.get_tool("execute_code").is_some());
}

#[test]
fn code_mode_openapi_flavor_registers() {
    // OpenApi flavor: wire a JsCodeExecutor<HttpCodeExecutor> +
    // ValidationFlavor::OpenApi; the two tools register with the "openapi"
    // builder flavor (the format enum on the tool schema).
    let cfg = code_mode_config();
    let builder = Server::builder().name("t").version("0.1.0");
    let server =
        code_mode_tools_from_executor(builder, &cfg, http_js_executor(), ValidationFlavor::OpenApi)
            .expect("OpenApi flavor wiring succeeds")
            .build()
            .expect("server builds");

    let validate = server
        .get_tool("validate_code")
        .expect("validate_code registered");
    let info = validate.metadata().expect("validate_code has metadata");
    let format_enum = &info.input_schema["properties"]["format"]["enum"];
    assert_eq!(
        format_enum,
        &serde_json::json!(["openapi"]),
        "OpenApi flavor must advertise the 'openapi' code format, got {format_enum}"
    );
    assert!(server.get_tool("execute_code").is_some());
}

// =============================================================================
// OpenAPI validate_code REAL-BEHAVIOR (Codex HIGH) — drive the registered
// validate_code tool and assert valid passes / invalid fails / disallowed
// op rejected. The full byte-equality engine-parity proof is Plan 05.
// =============================================================================

/// Build a server with the OpenApi-flavored code-mode tools and return the
/// `validate_code` handler.
fn openapi_validate_handler() -> Arc<dyn pmcp::ToolHandler> {
    let cfg = code_mode_config();
    let builder = Server::builder().name("t").version("0.1.0");
    let server =
        code_mode_tools_from_executor(builder, &cfg, http_js_executor(), ValidationFlavor::OpenApi)
            .expect("OpenApi flavor wiring succeeds")
            .build()
            .expect("server builds");
    Arc::clone(
        server
            .get_tool("validate_code")
            .expect("validate_code registered"),
    )
}

async fn run_validate(code: &str) -> pmcp::Result<serde_json::Value> {
    let handler = openapi_validate_handler();
    let extra = pmcp::RequestHandlerExtra::default();
    handler
        .handle(serde_json::json!({ "code": code }), extra)
        .await
}

#[tokio::test]
async fn openapi_validate_code_real_behavior_valid_passes() {
    // (1) A VALID read-only JS script passes validation and issues a token.
    let out = run_validate("const r = await api.get(\"/users\"); return r;")
        .await
        .expect("valid JS must pass validation (Ok, not a tool rejection)");
    // The validation response is a JSON object; a valid script is not an error.
    assert!(
        out.is_object(),
        "valid validation must return a JSON object: {out}"
    );
}

#[tokio::test]
async fn openapi_validate_code_real_behavior_invalid_fails() {
    // (2) An INVALID JS script (uses eval — a security violation) fails: real
    // SWC-backed JS validation runs, so this is rejected (Err / tool_rejected),
    // NOT silently accepted.
    let result = run_validate("const x = eval(\"api.get('/users')\");").await;
    assert!(
        result.is_err(),
        "invalid JS (eval) must be rejected by real JS validation, got {result:?}"
    );
}

#[tokio::test]
async fn openapi_validate_code_real_behavior_disallowed_op_rejected() {
    // (3) A script using a disallowed construct (an unbounded `while` loop — a
    // DoS policy violation) is rejected by the validation pipeline.
    let code = "let i = 0; while (i < 10) { await api.get(\"/data\"); i++; }";
    let result = run_validate(code).await;
    assert!(
        result.is_err(),
        "an unbounded while-loop must be rejected by the JS validation pipeline, got {result:?}"
    );
}
