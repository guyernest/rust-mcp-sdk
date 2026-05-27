//! First-class public SQLite connector (CONN-08) behind the `sqlite` feature.
//!
//! Promotes spike 005's `sqlite_backend` reference impl into a public
//! [`SqliteConnector`] type backed by `rusqlite` (with the `bundled` feature —
//! pure-Rust, no system SQLite, no Docker per the project's no-Docker rule). It
//! implements the full 3-method [`SqlConnector`] trait surface (CONN-01):
//! [`SqlConnector::dialect`], [`SqlConnector::execute`], and
//! [`SqlConnector::schema_text`].
//!
//! # Internal architecture (RESEARCH §1.4)
//!
//! The connection is held as `Arc<Mutex<rusqlite::Connection>>` where `Mutex`
//! is [`std::sync::Mutex`] (NOT [`tokio::sync::Mutex`]). Every sync `rusqlite`
//! call runs inside [`tokio::task::spawn_blocking`], and the mutex is locked
//! ONLY inside that blocking closure — so the async runtime is never blocked.
//!
//! The spike's `schema_blob` cache is dropped: [`SqlConnector::schema_text`]
//! fetches DDL fresh from `sqlite_master` on each call. The overhead is
//! negligible and it avoids stale-schema bugs after `CREATE TABLE` via
//! [`SqlConnector::execute`].
//!
//! # Placeholder convention
//!
//! `execute()` routes the SQL through [`translate_placeholders`] for
//! `Dialect::Sqlite`, which is identity on the SQL text but yields the
//! `ordered_params` binding order. SQLite recognises `:name` named
//! placeholders; bind values are looked up by name and bound via
//! `raw_bind_parameter`, never concatenated into the statement (T-84-04-01).
//!
//! # Example
//!
//! ```
//! # use pmcp_server_toolkit::sql::{SqliteConnector, SqlConnector, Dialect};
//! # #[tokio::main(flavor = "current_thread")]
//! # async fn main() {
//! let conn = SqliteConnector::open_in_memory().unwrap();
//! assert_eq!(conn.dialect(), Dialect::Sqlite);
//! let rows = conn.execute("SELECT 1 AS x", &[]).await.unwrap();
//! assert_eq!(rows[0]["x"], serde_json::json!(1));
//! # }
//! ```

use std::path::Path;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use rusqlite::types::{Value as SqlValue, ValueRef};
use rusqlite::Connection;
use serde_json::{Map, Value};

use crate::sql::{translate_placeholders, ConnectorError, Dialect, SqlConnector, TranslatedSql};

/// First-class SQLite connector backed by a bundled `rusqlite` connection.
///
/// Construct with [`SqliteConnector::open`] (file-backed) or
/// [`SqliteConnector::open_in_memory`] (`:memory:`, ideal for tests). Implements
/// the full [`SqlConnector`] trait so it drops straight into the toolkit's
/// `synthesize_from_config_with_connector` plumbing as an `Arc<dyn SqlConnector>`.
pub struct SqliteConnector {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteConnector {
    /// Open a file-backed SQLite database at `path`.
    ///
    /// # Errors
    ///
    /// Returns [`ConnectorError::Connection`] if `rusqlite` cannot open the
    /// database file. The error message comes from `rusqlite` and is bounded
    /// (e.g. "unable to open database file") — it does not echo arbitrary
    /// caller data (T-84-04-02).
    pub fn open(path: &Path) -> Result<Self, ConnectorError> {
        let conn = Connection::open(path).map_err(|e| ConnectorError::Connection(e.to_string()))?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Open an in-memory SQLite database (`:memory:`).
    ///
    /// Reusable across tests and ideal for ephemeral, schema-on-demand work:
    /// seed a schema via [`SqlConnector::execute`] after construction.
    ///
    /// # Errors
    ///
    /// Returns [`ConnectorError::Connection`] if `rusqlite` cannot create the
    /// in-memory database.
    pub fn open_in_memory() -> Result<Self, ConnectorError> {
        let conn =
            Connection::open_in_memory().map_err(|e| ConnectorError::Connection(e.to_string()))?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Run a multi-statement SQL batch (`;`-separated DDL + INSERTs) in ONE call.
    ///
    /// [`SqlConnector::execute`] is single-statement (it `prepare`s exactly one
    /// statement); a `schema.sql` bootstrap that issues several `CREATE TABLE`
    /// and `INSERT` statements cannot run through it. This inherent helper wraps
    /// [`rusqlite::Connection::execute_batch`], which executes every statement in
    /// the batch sequentially, so a demo database can be seeded with a single
    /// call inside the toolkit's ≤15-line wiring.
    ///
    /// This is an **inherent method on the concrete [`SqliteConnector`]** — it is
    /// deliberately NOT part of the locked 3-method [`SqlConnector`] trait. Call
    /// it on the concrete connector BEFORE wrapping it in
    /// `Arc<dyn SqlConnector>`; an `Arc<dyn SqlConnector>` exposes only the trait
    /// surface and would not resolve `execute_batch`.
    ///
    /// The batch is intended for IDEMPOTENT bootstrap SQL — use
    /// `CREATE TABLE IF NOT EXISTS` and `INSERT OR IGNORE` — so a second run
    /// against an already-seeded persisted database is safe and leaves exactly
    /// the seeded rows.
    ///
    /// # Errors
    ///
    /// Returns [`ConnectorError::Query`] if any statement in the batch fails
    /// (e.g. a syntax error); the message is the bounded `rusqlite` error string
    /// and does not echo the batch text. Returns [`ConnectorError::Driver`] if
    /// the connection mutex is poisoned or the blocking task fails to join.
    ///
    /// # Example
    ///
    /// ```
    /// # use pmcp_server_toolkit::sql::{SqliteConnector, SqlConnector};
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() {
    /// let conn = SqliteConnector::open_in_memory().unwrap();
    /// conn.execute_batch(
    ///     "CREATE TABLE t(id INTEGER); INSERT INTO t VALUES (1); INSERT INTO t VALUES (2);",
    /// )
    /// .await
    /// .unwrap();
    /// let rows = conn.execute("SELECT COUNT(*) AS c FROM t", &[]).await.unwrap();
    /// assert_eq!(rows[0]["c"], serde_json::json!(2));
    /// # }
    /// ```
    pub async fn execute_batch(&self, sql: &str) -> Result<(), ConnectorError> {
        let conn = Arc::clone(&self.conn);
        let sql = sql.to_string();
        tokio::task::spawn_blocking(move || -> Result<(), ConnectorError> {
            let guard = conn
                .lock()
                .map_err(|_| ConnectorError::Driver("mutex poisoned".into()))?;
            guard
                .execute_batch(&sql)
                .map_err(|e| ConnectorError::Query(e.to_string()))?;
            Ok(())
        })
        .await
        .map_err(|e| ConnectorError::Driver(format!("join error: {e}")))?
    }
}

/// Convert a [`serde_json::Value`] into a `rusqlite` bind value.
///
/// Booleans map to `Integer(0|1)`; integers to `Integer`; other numbers to
/// `Real`; strings to `Text`; arrays/objects are JSON-encoded into `Text` so
/// they round-trip losslessly (lifted from spike `main.rs:249-263`).
fn json_to_sql(v: &Value) -> SqlValue {
    match v {
        Value::Null => SqlValue::Null,
        Value::Bool(b) => SqlValue::Integer(i64::from(*b)),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                SqlValue::Integer(i)
            } else if let Some(f) = n.as_f64() {
                SqlValue::Real(f)
            } else {
                SqlValue::Null
            }
        },
        Value::String(s) => SqlValue::Text(s.clone()),
        _ => SqlValue::Text(v.to_string()),
    }
}

/// Convert a `rusqlite` [`ValueRef`] into a [`serde_json::Value`].
///
/// Blobs are rendered as the placeholder `"<blob>"` string — the toolkit's
/// JSON-over-MCP transport has no binary column type (lifted from spike
/// `main.rs:265-277`).
fn sql_to_json(v: ValueRef<'_>) -> Value {
    match v {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(i) => Value::Number(i.into()),
        ValueRef::Real(f) => serde_json::Number::from_f64(f)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        ValueRef::Text(t) => Value::String(String::from_utf8_lossy(t).into_owned()),
        ValueRef::Blob(_) => Value::String("<blob>".into()),
    }
}

/// Bind each named parameter onto the prepared statement in `ordered_params`
/// order.
///
/// Values are bound via `raw_bind_parameter` — NEVER concatenated into the SQL
/// text (T-84-04-01). A name present in `ordered_params` but absent from
/// `named_params` is skipped; SQLite reports the unbound `NULL` at query time.
fn bind_params(
    stmt: &mut rusqlite::Statement<'_>,
    ordered_params: &[String],
    named_params: &[(String, Value)],
) -> Result<(), ConnectorError> {
    for name in ordered_params {
        let Some((_, val)) = named_params.iter().find(|(n, _)| n == name) else {
            continue;
        };
        let bind_name = format!(":{name}");
        let idx = stmt
            .parameter_index(&bind_name)
            .map_err(|e| ConnectorError::ParameterBind {
                name: name.clone(),
                reason: e.to_string(),
            })?;
        if let Some(idx) = idx {
            stmt.raw_bind_parameter(idx, json_to_sql(val))
                .map_err(|e| ConnectorError::ParameterBind {
                    name: name.clone(),
                    reason: e.to_string(),
                })?;
        }
    }
    Ok(())
}

/// Drain a raw-queried statement into one JSON object per row, keyed by column
/// name (lifted from spike `main.rs:302-315`).
fn collect_rows(stmt: &mut rusqlite::Statement<'_>) -> Result<Vec<Value>, ConnectorError> {
    let cols: Vec<String> = stmt
        .column_names()
        .iter()
        .map(|c| (*c).to_string())
        .collect();
    let mut rows = stmt.raw_query();
    let mut out = Vec::new();
    while let Some(row) = rows
        .next()
        .map_err(|e| ConnectorError::Query(e.to_string()))?
    {
        let mut obj = Map::new();
        for (i, col) in cols.iter().enumerate() {
            let vr = row
                .get_ref(i)
                .map_err(|e| ConnectorError::Query(e.to_string()))?;
            obj.insert(col.clone(), sql_to_json(vr));
        }
        out.push(Value::Object(obj));
    }
    Ok(out)
}

#[async_trait]
impl SqlConnector for SqliteConnector {
    fn dialect(&self) -> Dialect {
        Dialect::Sqlite
    }

    async fn execute(
        &self,
        sql: &str,
        params: &[(String, Value)],
    ) -> Result<Vec<Value>, ConnectorError> {
        let conn = Arc::clone(&self.conn);
        let sql = sql.to_string();
        let params = params.to_vec();
        tokio::task::spawn_blocking(move || -> Result<Vec<Value>, ConnectorError> {
            let TranslatedSql {
                sql: translated,
                ordered_params,
            } = translate_placeholders(&sql, Dialect::Sqlite);
            // SQLite is identity — translated == sql — but ordered_params drives
            // the bind iteration order.
            let guard = conn
                .lock()
                .map_err(|_| ConnectorError::Driver("mutex poisoned".into()))?;
            let mut stmt = guard
                .prepare(&translated)
                .map_err(|e| ConnectorError::Query(e.to_string()))?;
            bind_params(&mut stmt, &ordered_params, &params)?;
            collect_rows(&mut stmt)
        })
        .await
        .map_err(|e| ConnectorError::Driver(format!("join error: {e}")))?
    }

    async fn schema_text(&self) -> Result<String, ConnectorError> {
        let conn = Arc::clone(&self.conn);
        tokio::task::spawn_blocking(move || -> Result<String, ConnectorError> {
            let guard = conn
                .lock()
                .map_err(|_| ConnectorError::Driver("mutex poisoned".into()))?;
            let mut stmt = guard
                .prepare(
                    "SELECT name, sql FROM sqlite_master \
                     WHERE type IN ('table', 'view') AND sql IS NOT NULL \
                     ORDER BY name",
                )
                .map_err(|e| ConnectorError::Schema(e.to_string()))?;
            let mut rows = stmt
                .query([])
                .map_err(|e| ConnectorError::Schema(e.to_string()))?;
            let mut out = String::new();
            while let Some(row) = rows
                .next()
                .map_err(|e| ConnectorError::Schema(e.to_string()))?
            {
                let ddl: String = row
                    .get(1)
                    .map_err(|e| ConnectorError::Schema(e.to_string()))?;
                out.push_str(&ddl);
                out.push_str(";\n");
            }
            Ok(out)
        })
        .await
        .map_err(|e| ConnectorError::Driver(format!("join error: {e}")))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_open_in_memory_succeeds() {
        let conn = SqliteConnector::open_in_memory();
        assert!(conn.is_ok(), "open_in_memory must succeed");
    }

    #[tokio::test]
    async fn test_dialect_returns_sqlite() {
        let conn = SqliteConnector::open_in_memory().unwrap();
        assert_eq!(conn.dialect(), Dialect::Sqlite);
    }

    #[tokio::test]
    async fn test_execute_no_params() {
        let conn = SqliteConnector::open_in_memory().unwrap();
        let rows = conn.execute("SELECT 1 AS x", &[]).await.unwrap();
        assert_eq!(rows, vec![json!({ "x": 1 })]);
    }

    #[tokio::test]
    async fn test_execute_with_named_param() {
        let conn = SqliteConnector::open_in_memory().unwrap();
        let rows = conn
            .execute("SELECT :v AS x", &[("v".into(), json!(42))])
            .await
            .unwrap();
        assert_eq!(rows, vec![json!({ "x": 42 })]);
    }

    #[tokio::test]
    async fn test_schema_text_returns_ddl() {
        let conn = SqliteConnector::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT)",
            &[],
        )
        .await
        .unwrap();
        let schema = conn.schema_text().await.unwrap();
        assert!(
            schema.contains("CREATE TABLE users"),
            "schema_text must echo sqlite_master DDL verbatim; got: {schema:?}"
        );
    }

    #[tokio::test]
    async fn test_execute_after_insert_returns_rows() {
        let conn = SqliteConnector::open_in_memory().unwrap();
        conn.execute("CREATE TABLE users (id INTEGER, name TEXT)", &[])
            .await
            .unwrap();
        conn.execute("INSERT INTO users VALUES (1, 'Ada')", &[])
            .await
            .unwrap();
        let rows = conn.execute("SELECT name FROM users", &[]).await.unwrap();
        assert_eq!(rows, vec![json!({ "name": "Ada" })]);
    }

    #[tokio::test]
    async fn test_execute_batch_seeds_multiple_tables() {
        let conn = SqliteConnector::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE artists (id INTEGER, name TEXT);
             CREATE TABLE albums (id INTEGER, title TEXT);
             INSERT INTO artists VALUES (1, 'AC-DC');
             INSERT INTO albums VALUES (1, 'For Those About To Rock');
             INSERT INTO albums VALUES (2, 'Let There Be Rock');",
        )
        .await
        .unwrap();

        let artists = conn.execute("SELECT name FROM artists", &[]).await.unwrap();
        assert_eq!(artists, vec![json!({ "name": "AC-DC" })]);

        let albums = conn
            .execute("SELECT COUNT(*) AS c FROM albums", &[])
            .await
            .unwrap();
        assert_eq!(albums, vec![json!({ "c": 2 })]);
    }

    #[tokio::test]
    async fn test_execute_batch_invalid_statement_returns_query_error() {
        let conn = SqliteConnector::open_in_memory().unwrap();
        let err = conn
            .execute_batch("CREATE TABLE ok (id INTEGER); NOT VALID SQL;")
            .await
            .expect_err("a syntactically-invalid batch statement must return Err, not panic");
        assert!(
            matches!(err, ConnectorError::Query(_)),
            "expected ConnectorError::Query, got: {err:?}"
        );
    }

    #[tokio::test]
    async fn test_execute_batch_idempotent_second_run_leaves_seeded_rows() {
        let conn = SqliteConnector::open_in_memory().unwrap();
        let bootstrap = "CREATE TABLE IF NOT EXISTS t (id INTEGER PRIMARY KEY);
                         INSERT OR IGNORE INTO t VALUES (1);
                         INSERT OR IGNORE INTO t VALUES (2);";

        conn.execute_batch(bootstrap)
            .await
            .expect("first bootstrap run succeeds");
        conn.execute_batch(bootstrap)
            .await
            .expect("second bootstrap run against persisted DB succeeds (idempotent)");

        let rows = conn
            .execute("SELECT COUNT(*) AS c FROM t", &[])
            .await
            .unwrap();
        assert_eq!(
            rows,
            vec![json!({ "c": 2 })],
            "idempotent batch must leave exactly the seeded rows after a second run"
        );
    }

    #[tokio::test]
    async fn test_concurrent_executes_serialize_via_mutex() {
        let conn = Arc::new(SqliteConnector::open_in_memory().unwrap());
        let a = {
            let conn = Arc::clone(&conn);
            tokio::spawn(async move { conn.execute("SELECT 1 AS x", &[]).await })
        };
        let b = {
            let conn = Arc::clone(&conn);
            tokio::spawn(async move { conn.execute("SELECT 2 AS x", &[]).await })
        };
        let (ra, rb) = tokio::join!(a, b);
        assert_eq!(ra.unwrap().unwrap(), vec![json!({ "x": 1 })]);
        assert_eq!(rb.unwrap().unwrap(), vec![json!({ "x": 2 })]);
    }
}
