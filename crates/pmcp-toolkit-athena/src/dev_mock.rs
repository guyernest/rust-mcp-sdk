//! Authentic in-process Athena mock — used by `tests/integration.rs` AND by
//! downstream examples that activate the `dev_mock` cargo feature.
//!
//! REVIEWS H5: this file lives under `src/` (not `tests/`) so it's reachable
//! from publishable example targets via the `dev_mock` feature. No container
//! runtime, no networking — pure in-process per
//! `feedback_avoid_docker_pure_rust_lambda` memory + D-07. The seam is the
//! [`SqlConnector`] trait itself, NOT the Athena StartQueryExecution → poll →
//! GetQueryResults wire protocol: the mock implements the trait directly and
//! records what it was asked to run so tests can assert the `:name` → `?`
//! translation crossed the boundary intact.
//!
//! REVIEWS M5: the mock supports multi-page simulation via
//! [`AthenaMock::with_pages`] + [`PAGINATED_QUERY_MARKER`]. A SQL containing the
//! marker returns the FLATTENED concatenation of every installed page — the same
//! `Vec<Value>` the real [`paginated_get_query_results`] helper would produce
//! against a multi-page Athena response. This is the behavioural seam the
//! pagination integration test asserts on.

#![cfg(any(test, feature = "dev_mock"))]

use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Mutex;

use pmcp_server_toolkit::sql::{
    translate_placeholders, ConnectorError, Dialect, SqlConnector, TranslatedSql,
};

/// REVIEWS M5: SQL containing this marker triggers the mock's multi-page
/// simulation — [`AthenaMock::execute`] returns the concatenated rows from every
/// page installed via [`AthenaMock::with_pages`].
pub const PAGINATED_QUERY_MARKER: &str = "/* PAGINATED */";

/// In-process Athena mock implementing [`SqlConnector`] directly.
///
/// Records the translated SQL and the assembled positional bind list of the most
/// recent [`execute`](SqlConnector::execute) call. All fields are `pub` so
/// downstream phases (85/86) can inspect the wire-format SQL externally.
pub struct AthenaMock {
    /// Seeded fixture rows keyed by table name.
    pub tables: HashMap<String, Vec<Value>>,
    /// REVIEWS M5 — multi-page simulation source. Each inner `Vec` is one page.
    pub pages: Vec<Vec<Value>>,
    /// Translated SQL recorded by the last `execute` call (for test inspection).
    pub last_translated_sql: Mutex<Option<String>>,
    /// Positional bind args recorded by the last `execute` call.
    pub last_positional_args: Mutex<Option<Vec<Value>>>,
}

impl AthenaMock {
    /// Construct a mock seeded with a small `images` table.
    #[must_use]
    pub fn open_images_fixture() -> Self {
        let mut tables = HashMap::new();
        tables.insert(
            "images".into(),
            vec![
                json!({"image_id": "img1", "label": "cat"}),
                json!({"image_id": "img2", "label": "dog"}),
            ],
        );
        Self {
            tables,
            pages: Vec::new(),
            last_translated_sql: Mutex::new(None),
            last_positional_args: Mutex::new(None),
        }
    }

    /// REVIEWS M5: install multi-page test data. [`execute`](SqlConnector::execute)
    /// returns the flattened concatenation of all pages when the SQL contains
    /// [`PAGINATED_QUERY_MARKER`].
    #[must_use]
    pub fn with_pages(mut self, pages: Vec<Vec<Value>>) -> Self {
        self.pages = pages;
        self
    }
}

#[async_trait]
impl SqlConnector for AthenaMock {
    fn dialect(&self) -> Dialect {
        Dialect::Athena
    }

    async fn execute(
        &self,
        sql: &str,
        params: &[(String, Value)],
    ) -> Result<Vec<Value>, ConnectorError> {
        let TranslatedSql {
            sql: translated,
            ordered_params,
        } = translate_placeholders(sql, Dialect::Athena);
        let positional: Vec<Value> = ordered_params
            .iter()
            .map(|n| {
                params
                    .iter()
                    .find(|(k, _)| k == n)
                    .map_or(Value::Null, |(_, v)| v.clone())
            })
            .collect();

        if let Ok(mut g) = self.last_translated_sql.lock() {
            *g = Some(translated.clone());
        }
        if let Ok(mut g) = self.last_positional_args.lock() {
            *g = Some(positional.clone());
        }

        // REVIEWS M5: multi-page simulation — concatenate every installed page.
        if translated.contains(PAGINATED_QUERY_MARKER) {
            let mut all_rows = Vec::new();
            for page in &self.pages {
                all_rows.extend(page.iter().cloned());
            }
            return Ok(all_rows);
        }
        cheap_query_engine(&self.tables, &translated, &positional)
    }

    async fn schema_text(&self) -> Result<String, ConnectorError> {
        Ok(
            "CREATE EXTERNAL TABLE images (\n  image_id STRING,\n  label STRING\n)\n\
            PARTITIONED BY (\n  dt STRING\n)\n\
            LOCATION 's3://example-bucket/images/';\n"
                .to_string(),
        )
    }
}

/// Tiny query recognizer for the fixture queries — NOT a general SQL engine.
fn cheap_query_engine(
    tables: &HashMap<String, Vec<Value>>,
    sql: &str,
    args: &[Value],
) -> Result<Vec<Value>, ConnectorError> {
    if sql.contains("FROM images WHERE image_id = ?") {
        let id = args
            .first()
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        let rows = tables.get("images").cloned().unwrap_or_default();
        return Ok(rows
            .into_iter()
            .filter(|r| r["image_id"].as_str() == Some(id))
            .collect());
    }
    if sql.contains("SELECT * FROM images") {
        return Ok(tables.get("images").cloned().unwrap_or_default());
    }
    Ok(vec![])
}
