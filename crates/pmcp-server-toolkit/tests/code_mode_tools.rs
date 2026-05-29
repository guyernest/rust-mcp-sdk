//! SHAP-A-01 / SC-3 anchor — the LOCKED connector-aware code-mode API.
//!
//! Verifies that `try_code_mode_from_config_with_connector` registers BOTH
//! `validate_code` and `execute_code` on the built `pmcp::Server`, that the
//! connectorless `try_code_mode_from_config` registers NEITHER (documented
//! validation-only / no-tool path), that the no-op is preserved when
//! `[code_mode]` is absent, and that static `[code_mode]` policy
//! (allow_writes / allow_deletes / allow_ddl) is enforced through the
//! validate_code tool (DELETE/DDL under a read-only config issue no token).
//!
//! Run with: `cargo test -p pmcp-server-toolkit --features "code-mode sqlite"
//! --test code_mode_tools -- --test-threads=1`.

#![cfg(all(feature = "code-mode", feature = "sqlite"))]

use std::sync::Arc;

use pmcp::Server;
use pmcp_server_toolkit::config::ServerConfig;
use pmcp_server_toolkit::sql::{SqlConnector, SqliteConnector};
use pmcp_server_toolkit::ServerBuilderExt;

/// A read-only `[code_mode]` config (allow_writes/deletes/ddl all false).
///
/// Uses a dev inline `token_secret` rather than an `env:` reference: these
/// tests exercise static-policy ENFORCEMENT (DELETE/DDL rejection), not secret
/// resolution, so an inline secret avoids mutating the shared process
/// environment and the multi-threaded `std::env` race that comes with it.
/// (Env-resolution itself is covered by `tests/env_expansion.rs` and the
/// `resolve_token_secret_*` unit tests.)
fn read_only_config() -> ServerConfig {
    let toml = r#"
[server]
name = "code-mode-tools-test"
version = "0.1.0"

[code_mode]
enabled = true
server_id = "code-mode-tools-test"
allow_writes = false
allow_deletes = false
allow_ddl = false
token_secret = "code-mode-tools-secret-16-or-more"
allow_inline_token_secret_for_dev = true
"#;
    ServerConfig::from_toml_strict_validated(toml).expect("config parses + validates")
}

/// A bare config with NO `[code_mode]` block.
fn no_code_mode_config() -> ServerConfig {
    let toml = r#"
[server]
name = "no-code-mode"
version = "0.1.0"
"#;
    ServerConfig::from_toml_strict_validated(toml).expect("config parses + validates")
}

fn in_memory_connector() -> Arc<dyn SqlConnector> {
    Arc::new(SqliteConnector::open_in_memory().expect("open in-memory sqlite"))
}

#[test]
fn with_connector_registers_both_tools() {
    let cfg = read_only_config();
    let server = Server::builder()
        .name("t")
        .version("0.1.0")
        .try_code_mode_from_config_with_connector(&cfg, in_memory_connector())
        .expect("connector-aware code-mode wiring succeeds")
        .build()
        .expect("server builds");

    assert!(
        server.get_tool("validate_code").is_some(),
        "try_code_mode_from_config_with_connector must register validate_code"
    );
    assert!(
        server.get_tool("execute_code").is_some(),
        "try_code_mode_from_config_with_connector must register execute_code"
    );
}

#[test]
fn with_connector_no_op_when_section_absent() {
    let cfg = no_code_mode_config();
    let server = Server::builder()
        .name("t")
        .version("0.1.0")
        .try_code_mode_from_config_with_connector(&cfg, in_memory_connector())
        .expect("no-op path succeeds")
        .build()
        .expect("server builds");

    assert!(server.get_tool("validate_code").is_none());
    assert!(
        server.get_tool("execute_code").is_none(),
        "no [code_mode] block must register neither tool"
    );
}

#[test]
fn connectorless_path_registers_no_tools_but_validates_pipeline() {
    // The connectorless try_code_mode_from_config builds + validates the
    // pipeline (R9 / secret errors would fire) but registers NO tools — it is
    // the documented validation-only path.
    let cfg = read_only_config();
    let server = Server::builder()
        .name("t")
        .version("0.1.0")
        .try_code_mode_from_config(&cfg)
        .expect("connectorless path validates pipeline")
        .build()
        .expect("server builds");

    assert!(
        server.get_tool("execute_code").is_none(),
        "connectorless try_code_mode_from_config must register NO execute_code tool"
    );
    assert!(
        server.get_tool("validate_code").is_none(),
        "connectorless try_code_mode_from_config must register NO validate_code tool"
    );
}

#[tokio::test]
async fn validate_code_rejects_delete_under_read_only_policy() {
    // SC-3: validate_code on a DELETE under allow_deletes=false reports a
    // policy failure and issues NO approval token.
    let cfg = read_only_config();
    let server = Server::builder()
        .name("t")
        .version("0.1.0")
        .try_code_mode_from_config_with_connector(&cfg, in_memory_connector())
        .expect("wiring succeeds")
        .build()
        .expect("server builds");

    let handler = server
        .get_tool("validate_code")
        .expect("validate_code present");
    let args = serde_json::json!({ "code": "DELETE FROM Artist WHERE ArtistId = 1" });
    // A policy rejection surfaces as a tool ERROR so the MCP `tools/call` result
    // has `isError: true` (the reference observable the generated.yaml `failure`
    // assertions verify — SC-3). The rejection JSON (`valid: false` + violation)
    // is carried in the error message.
    let err = handler
        .handle(args, pmcp::RequestHandlerExtra::default())
        .await
        .expect_err("DELETE under allow_deletes=false must REJECT as a tool error");
    let out = rejection_json(&err);

    assert_eq!(
        out["valid"], false,
        "DELETE under allow_deletes=false must be invalid: {out}"
    );
    assert!(
        out["approval_token"].is_null(),
        "an invalid DELETE must NOT receive an approval token: {out}"
    );
}

/// Parse the rejection JSON carried in a `validate_code` tool rejection
/// (`pmcp::Error::ToolRejected { details, .. }`).
///
/// A policy rejection surfaces as `Error::ToolRejected` (which the server maps
/// to a `CallToolResult { isError: true }`); the structured `valid: false` +
/// `violations` payload rides in `details`.
fn rejection_json(err: &pmcp::Error) -> serde_json::Value {
    match err {
        pmcp::Error::ToolRejected { details, .. } => details
            .clone()
            .expect("validate_code rejection carries the violation JSON in `details`"),
        other => panic!("expected a ToolRejected error, got {other:?}"),
    }
}

#[tokio::test]
async fn validate_code_rejects_ddl_under_read_only_policy() {
    let cfg = read_only_config();
    let server = Server::builder()
        .name("t")
        .version("0.1.0")
        .try_code_mode_from_config_with_connector(&cfg, in_memory_connector())
        .expect("wiring succeeds")
        .build()
        .expect("server builds");

    let handler = server
        .get_tool("validate_code")
        .expect("validate_code present");
    let args = serde_json::json!({ "code": "DROP TABLE Artist" });
    // DDL rejection also surfaces as a tool error (SC-3).
    let err = handler
        .handle(args, pmcp::RequestHandlerExtra::default())
        .await
        .expect_err("DROP under allow_ddl=false must REJECT as a tool error");
    let out = rejection_json(&err);

    assert_eq!(
        out["valid"], false,
        "DROP under allow_ddl=false must be invalid"
    );
    assert!(out["approval_token"].is_null());
}

#[tokio::test]
async fn validate_code_permits_select_under_read_only_policy() {
    let cfg = read_only_config();
    let server = Server::builder()
        .name("t")
        .version("0.1.0")
        .try_code_mode_from_config_with_connector(&cfg, in_memory_connector())
        .expect("wiring succeeds")
        .build()
        .expect("server builds");

    let handler = server
        .get_tool("validate_code")
        .expect("validate_code present");
    let args = serde_json::json!({ "code": "SELECT ArtistId, Name FROM Artist LIMIT 10" });
    let out = handler
        .handle(args, pmcp::RequestHandlerExtra::default())
        .await
        .expect("validate_code returns a JSON response");

    assert_eq!(
        out["valid"], true,
        "a read-only SELECT must validate under the read-only policy: {out}"
    );
    assert!(
        out["approval_token"].is_string(),
        "a valid SELECT must receive an approval token: {out}"
    );
}
