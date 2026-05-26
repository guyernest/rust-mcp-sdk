//! D-06 integration anchor — synthesizer emits structuredContent for widget tools.
//!
//! Per RESEARCH §4 Option A: when `ToolDecl.ui_resource_uri.is_some()`, the
//! synthesizer flips `widget_meta` on the synthesized `ToolInfo` so pmcp
//! core's `CallToolResult::with_widget_enrichment` populates `structured_content`
//! with the `Vec<Value>` rows returned by the connector. This test asserts the
//! contract end-to-end against an in-memory SQLite connector.
//!
//! REVIEWS H1: This file is owned by Plan 04 (not Plan 03) because it consumes
//! `SqliteConnector`, which Plan 04 ships. REVIEWS H4: seed SQL uses `:name`
//! style placeholders only — a bare `?` does not bind through
//! `translate_placeholders` against `Dialect::Sqlite` (the translator only
//! recognises `:name`).

#![cfg(all(feature = "code-mode", feature = "sqlite"))]

use std::sync::Arc;

use serde_json::{json, Value};

use pmcp::types::{CallToolResult, ToolInfo};
use pmcp::RequestHandlerExtra;
use pmcp_server_toolkit::config::ServerConfig;
use pmcp_server_toolkit::sql::{SqlConnector, SqliteConnector};
use pmcp_server_toolkit::tools::synthesize_from_config_with_connector;

/// A config whose single `[[tools]]` entry declares a `ui_resource_uri` so the
/// synthesizer must flip widget metadata on (D-06).
const CONFIG_WIDGET: &str = r#"
[server]
name = "widget-demo"
version = "0.1.0"

[[tools]]
name = "list_things"
description = "Returns a constant row"
sql = "SELECT 1 AS x"
ui_resource_uri = "ui://test"
"#;

/// Same shape but WITHOUT `ui_resource_uri` — widget metadata must stay off.
const CONFIG_NO_WIDGET: &str = r#"
[server]
name = "plain-demo"
version = "0.1.0"

[[tools]]
name = "list_things"
description = "Returns a constant row"
sql = "SELECT 1 AS x"
"#;

#[tokio::test]
async fn widget_tool_gets_widget_meta() {
    let cfg = ServerConfig::from_toml_strict_validated(CONFIG_WIDGET).expect("config parses");
    let conn: Arc<dyn SqlConnector> = Arc::new(SqliteConnector::open_in_memory().unwrap());
    let tools = synthesize_from_config_with_connector(&cfg, conn).expect("synthesize");

    assert_eq!(tools.len(), 1, "one tool from config");
    assert!(
        tools[0].1.widget_meta().is_some(),
        "ui_resource_uri set ⇒ ToolInfo.widget_meta() must be Some so D-06 fires"
    );
}

#[tokio::test]
async fn non_widget_tool_has_no_widget_meta() {
    let cfg = ServerConfig::from_toml_strict_validated(CONFIG_NO_WIDGET).expect("config parses");
    let conn: Arc<dyn SqlConnector> = Arc::new(SqliteConnector::open_in_memory().unwrap());
    let tools = synthesize_from_config_with_connector(&cfg, conn).expect("synthesize");

    assert_eq!(tools.len(), 1, "one tool from config");
    assert!(
        tools[0].1.widget_meta().is_none(),
        "no ui_resource_uri ⇒ widget_meta() must be None (text-content fallback)"
    );
}

#[tokio::test]
async fn widget_handler_produces_value_array() {
    // REVIEWS H4: seed using `:name` placeholders ONLY — bare `?` would not bind
    // through translate_placeholders against Dialect::Sqlite.
    let conn: Arc<dyn SqlConnector> = Arc::new(SqliteConnector::open_in_memory().unwrap());
    conn.execute("CREATE TABLE t (x INTEGER)", &[]).await.unwrap();
    conn.execute(
        "INSERT INTO t VALUES (:p1), (:p2)",
        &[("p1".into(), json!(1)), ("p2".into(), json!(2))],
    )
    .await
    .unwrap();
    // Sanity check the seed actually landed (bare `?` would silently fail here).
    let seeded = conn
        .execute("SELECT x FROM t ORDER BY x", &[])
        .await
        .unwrap();
    assert_eq!(seeded.len(), 2);
    assert_eq!(seeded[0]["x"], json!(1));
    assert_eq!(seeded[1]["x"], json!(2));

    // Synthesize a tool whose SQL returns both rows, then invoke its handler.
    let cfg_toml = r#"
[server]
name = "rows-demo"
version = "0.1.0"

[[tools]]
name = "all_rows"
description = "Returns all rows"
sql = "SELECT x FROM t ORDER BY x"
ui_resource_uri = "ui://rows"
"#;
    let cfg = ServerConfig::from_toml_strict_validated(cfg_toml).expect("config parses");
    let tools =
        synthesize_from_config_with_connector(&cfg, Arc::clone(&conn)).expect("synthesize");
    let handler = &tools[0].2;

    let out = handler
        .handle(json!({}), RequestHandlerExtra::default())
        .await
        .expect("handler runs");

    let rows = out.as_array().expect("handler returns a Value::Array of rows");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["x"], json!(1));
    assert_eq!(rows[1]["x"], json!(2));
}

#[tokio::test]
async fn end_to_end_structured_content_via_pmcp_core() {
    // Build a ToolInfo carrying widget metadata in the same `ui.resourceUri`
    // shape the synthesizer emits (apply_widget_meta in tools.rs), then exercise
    // pmcp core's widget-enrichment gate directly. This is the binding test that
    // proves D-06 populates structured_content.
    let rows: Value = json!([{ "x": 1 }, { "x": 2 }]);
    let info = ToolInfo::new("all_rows", Some("rows".into()), json!({ "type": "object" }))
        .with_meta_entry("ui", json!({ "resourceUri": "ui://rows" }));
    assert!(
        info.widget_meta().is_some(),
        "ToolInfo built with ui.resourceUri must report widget_meta"
    );

    let result = CallToolResult::new(vec![]).with_widget_enrichment(&info, rows.clone());
    assert_eq!(
        result.structured_content,
        Some(rows),
        "with_widget_enrichment must populate structured_content for a widget tool"
    );

    // Negative path: a non-widget ToolInfo leaves structured_content None.
    let plain = ToolInfo::new("plain", Some("plain".into()), json!({ "type": "object" }));
    let plain_result =
        CallToolResult::new(vec![]).with_widget_enrichment(&plain, json!([{ "x": 1 }]));
    assert_eq!(
        plain_result.structured_content, None,
        "non-widget tool must NOT populate structured_content (text-content fallback)"
    );
}
