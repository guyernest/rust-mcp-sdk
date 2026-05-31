//! Authentic in-process MySQL mock — used by `tests/integration.rs` AND by
//! downstream examples that activate the `dev_mock` cargo feature.
//!
//! REVIEWS H5: this file lives under `src/` (not `tests/`) so it's reachable
//! from publishable example targets via the `dev_mock` feature. No container
//! runtime, no networking — pure in-process per
//! `feedback_avoid_docker_pure_rust_lambda` memory + D-07. The seam is the
//! [`SqlConnector`] trait itself, not the MySQL wire protocol: the mock
//! implements the trait directly and records what it was asked to run so tests
//! can assert the `:name` → `?` translation crossed the boundary intact.

#![cfg(any(test, feature = "dev_mock"))]

use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Mutex;

use pmcp_server_toolkit::sql::{
    translate_placeholders, ConnectorError, Dialect, SqlConnector, TranslatedSql,
};

/// In-process MySQL mock implementing [`SqlConnector`] directly.
///
/// Records the translated SQL and the assembled positional bind list of the
/// most recent [`execute`](SqlConnector::execute) call. Both fields are `pub`
/// so downstream phases (85/86) can inspect the wire-format SQL externally and
/// verify `:name` → `?` translation crossed the boundary.
pub struct MysqlMock {
    /// Seeded fixture rows keyed by table name.
    pub tables: HashMap<String, Vec<Value>>,
    /// Translated SQL recorded by the last `execute` call (for test inspection).
    pub last_translated_sql: Mutex<Option<String>>,
    /// Positional bind args recorded by the last `execute` call.
    pub last_positional_args: Mutex<Option<Vec<Value>>>,
}

impl MysqlMock {
    /// Construct a mock seeded with a small `employees` table.
    #[must_use]
    pub fn employee_directory() -> Self {
        let mut tables = HashMap::new();
        tables.insert(
            "employees".into(),
            vec![
                json!({"id": 1, "name": "Ada Lovelace"}),
                json!({"id": 2, "name": "Alan Turing"}),
            ],
        );
        Self {
            tables,
            last_translated_sql: Mutex::new(None),
            last_positional_args: Mutex::new(None),
        }
    }
}

#[async_trait]
impl SqlConnector for MysqlMock {
    fn dialect(&self) -> Dialect {
        Dialect::MySql
    }

    async fn execute(
        &self,
        sql: &str,
        params: &[(String, Value)],
    ) -> Result<Vec<Value>, ConnectorError> {
        let TranslatedSql {
            sql: translated,
            ordered_params,
        } = translate_placeholders(sql, Dialect::MySql);
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
        cheap_query_engine(&self.tables, &translated, &positional)
    }

    async fn schema_text(&self) -> Result<String, ConnectorError> {
        // Backtick identifier quoting + ENGINE=InnoDB markers are the MySQL
        // dialect tells the integration test asserts on.
        Ok("-- SHOW CREATE TABLE (MySQL):\n\
            CREATE TABLE `employees` (\n  \
            `id` INT NOT NULL,\n  \
            `name` VARCHAR(255) NOT NULL\n\
            ) ENGINE=InnoDB;\n\
            CREATE TABLE `departments` (\n  \
            `id` INT NOT NULL,\n  \
            `name` VARCHAR(255) NOT NULL\n\
            ) ENGINE=InnoDB;\n"
            .into())
    }
}

/// Tiny query recognizer for the fixture queries — NOT a general SQL engine.
/// Recognizes `?` placeholders (MySQL form), unlike the Postgres mock's `$1`.
fn cheap_query_engine(
    tables: &HashMap<String, Vec<Value>>,
    sql: &str,
    args: &[Value],
) -> Result<Vec<Value>, ConnectorError> {
    if sql.contains("FROM employees WHERE id = ?") {
        let id = args
            .first()
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(-1);
        let rows = tables.get("employees").cloned().unwrap_or_default();
        return Ok(rows
            .into_iter()
            .filter(|r| r["id"].as_i64() == Some(id))
            .collect());
    }
    if sql.contains("SELECT * FROM employees") {
        return Ok(tables.get("employees").cloned().unwrap_or_default());
    }
    Ok(vec![])
}
