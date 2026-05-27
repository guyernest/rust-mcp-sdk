//! Task 1 (Plan 85-05) integration tests for [`pmcp_sql_server::build_server`].
//!
//! Builds a [`pmcp::Server`] from the vendored Chinook reference config + the
//! data-bearing `chinook.db` fixture (Plan 03) through the real `dispatch` →
//! `build_server` path the binary uses, then asserts the assembled surface:
//!
//! - curated `[[tools]]` (`search_tracks`) AND code-mode `validate_code` /
//!   `execute_code` are registered (REVIEW FIX #4 — the LOCKED connector-aware
//!   code-mode API);
//! - ALL THREE configured resources survive, with the schema resource's content
//!   replaced by the `--schema` DDL while examples + learnings keep their
//!   configured bodies (REVIEW FIX #2);
//! - the configured `start_code_mode` prompt is present with a body resolved
//!   from its `include_resources` against the MERGED resources (REVIEW FIX #3).
//!
//! Run with:
//! ```sh
//! cargo test -p pmcp-sql-server --no-default-features --features sqlite \
//!     --test assemble -- --test-threads=1
//! ```

#![cfg(feature = "sqlite")]

use std::collections::HashMap;

use pmcp::RequestHandlerExtra;
use pmcp_server_toolkit::resources::StaticResourceHandler;
use pmcp_server_toolkit::ServerConfig;
use pmcp_sql_server::{build_server, dispatch, merge_schema_resource};

const REFERENCE_CONFIG: &str = include_str!("fixtures/reference-config.toml");
const CHINOOK_DDL: &str = include_str!("fixtures/chinook.ddl");

/// Absolute path to the data-bearing Chinook fixture committed in Plan 03.
fn chinook_db_path() -> String {
    format!("{}/tests/fixtures/chinook.db", env!("CARGO_MANIFEST_DIR"))
}

/// Parse the reference config and re-point its SQLite `file_path` from the
/// Lambda-bundled `/var/task/assets/chinook.db` to the vendored test fixture,
/// and set `CODE_MODE_SECRET` so the `${CODE_MODE_SECRET}` token_secret resolves.
fn reference_cfg() -> ServerConfig {
    // The reference config's `[code_mode] token_secret = "${CODE_MODE_SECRET}"`
    // (Plan 01 ${VAR} expansion) reads this env var at code-mode wiring time.
    std::env::set_var("CODE_MODE_SECRET", "assemble-test-secret-min-16-bytes");

    let mut cfg = ServerConfig::from_toml_strict_validated(REFERENCE_CONFIG)
        .expect("reference config must parse + validate");
    cfg.database.file_path = Some(chinook_db_path());
    cfg
}

async fn build_reference_server() -> pmcp::Server {
    let cfg = reference_cfg();
    let connector = dispatch(&cfg).await.expect("sqlite dispatch must succeed");
    build_server(&cfg, connector, CHINOOK_DDL.to_string()).expect("build_server must succeed")
}

#[tokio::test]
async fn curated_and_code_mode_tools_present() {
    let server = build_reference_server().await;

    assert!(
        server.get_tool("search_tracks").is_some(),
        "curated [[tools]] entry search_tracks must be registered"
    );
    assert!(
        server.get_tool("validate_code").is_some(),
        "code-mode validate_code must be registered via the LOCKED connector-aware API"
    );
    assert!(
        server.get_tool("execute_code").is_some(),
        "code-mode execute_code must be registered via the LOCKED connector-aware API"
    );
}

#[tokio::test]
async fn all_three_resources_present_with_schema_merged() {
    let cfg = reference_cfg();

    // Build the merged resource handler the same way build_server does and
    // assert all three configured resources resolve with the right content.
    let merged = merge_schema_resource(&cfg, CHINOOK_DDL);
    let handler = StaticResourceHandler::from_configs(&merged).expect("merged handler builds");

    let schema = handler
        .get("docs://chinook/schema")
        .expect("schema resource present");
    assert!(
        schema.content.contains("# Database Schema"),
        "schema resource carries the # Database Schema header"
    );
    assert!(
        schema.content.contains("CREATE TABLE"),
        "schema resource content is the --schema DDL"
    );

    let examples = handler
        .get("docs://chinook/examples")
        .expect("examples resource preserved (REVIEW FIX #2)");
    assert!(
        !examples.content.is_empty(),
        "examples resource keeps its configured content"
    );

    let learnings = handler
        .get("code-mode://learnings")
        .expect("learnings resource preserved (REVIEW FIX #2)");
    assert!(
        !learnings.content.is_empty(),
        "learnings resource keeps its configured content"
    );
}

#[tokio::test]
async fn start_code_mode_prompt_present_with_resolved_body() {
    let server = build_reference_server().await;

    let prompt = server
        .get_prompt("start_code_mode")
        .expect("configured start_code_mode prompt must be registered (REVIEW FIX #3)");

    // The prompt body is resolved from include_resources against the MERGED
    // resources, so it is non-empty and contains the merged schema content.
    let result = prompt
        .handle(HashMap::new(), RequestHandlerExtra::default())
        .await
        .expect("start_code_mode prompt handle must succeed");
    assert_eq!(result.messages.len(), 1, "prompt yields one message");

    let body = match &result.messages[0].content {
        pmcp::types::Content::Text { text } => text.clone(),
        other => panic!("expected text content, got {other:?}"),
    };
    assert!(!body.is_empty(), "prompt body must be non-empty");
    assert!(
        body.contains("# Database Schema") || body.contains("CREATE TABLE"),
        "prompt body resolves the merged schema resource content"
    );
}

#[tokio::test]
async fn empty_code_mode_builds_curated_tools_only() {
    // A config with curated tools but NO [code_mode] block builds a server with
    // the curated tools and no code-mode tools (graceful no-op).
    let toml = r#"
[server]
name = "no-code-mode"
version = "0.1.0"

[database]
type = "sqlite"

[[tools]]
name = "ping"
description = "synthetic test tool"
"#;
    let mut cfg = ServerConfig::from_toml_strict_validated(toml).expect("parse");
    cfg.database.file_path = Some(":memory:".to_string());

    let connector = dispatch(&cfg).await.expect("sqlite :memory: dispatch");
    let server = build_server(&cfg, connector, "CREATE TABLE x (id INT);".to_string())
        .expect("build_server with no [code_mode]");

    assert!(server.get_tool("ping").is_some(), "curated tool present");
    assert!(
        server.get_tool("validate_code").is_none(),
        "no code-mode tools when [code_mode] absent"
    );
    assert!(
        server.get_tool("execute_code").is_none(),
        "no code-mode tools when [code_mode] absent"
    );
}
