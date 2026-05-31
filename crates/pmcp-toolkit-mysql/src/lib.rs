//! MySQL connector for pmcp-server-toolkit.
//!
//! Pure-Rust + Lambda-deployable: sqlx 0.8.6 with `tls-rustls-aws-lc-rs` (no
//! OpenSSL) per `feedback_avoid_docker_pure_rust_lambda` memory.
//!
//! [`MysqlConnector`] implements the toolkit's 3-method
//! [`SqlConnector`](pmcp_server_toolkit::sql::SqlConnector) trait:
//! [`dialect`](pmcp_server_toolkit::sql::SqlConnector::dialect),
//! [`execute`](pmcp_server_toolkit::sql::SqlConnector::execute) (canonical
//! `:name` placeholders translated to `?` via
//! [`translate_placeholders`](pmcp_server_toolkit::sql::translate_placeholders)),
//! and [`schema_text`](pmcp_server_toolkit::sql::SqlConnector::schema_text)
//! (driven by `information_schema.columns` filtered by the MySQL database name).
//!
//! REVIEWS M3: [`MysqlConnector::connect`] uses
//! [`MySqlPool::connect_lazy`](sqlx::mysql::MySqlPool::connect_lazy) to defer
//! TCP I/O to first use. `connect_lazy` parses the URL synchronously, so a
//! malformed URL returns [`ConnectorError::Connection`] immediately, while a
//! real connection failure surfaces on the first
//! [`execute`](SqlConnector::execute) / [`schema_text`](SqlConnector::schema_text)
//! call. The `pub async fn` signature is retained for API symmetry with
//! `PostgresConnector::connect` and to leave room for a future `connect_eager`
//! variant that DOES open the connection.
//!
//! REVIEWS H5: the `dev_mock` feature exposes
//! `pmcp_toolkit_mysql::dev_mock::MysqlMock` for examples + downstream
//! integration tests. It is NOT enabled by default.
//!
//! # Security
//!
//! [`ConnectorError::Connection`] error text NEVER contains the raw URL or its
//! password — the password segment is redacted via [`sanitize_url`] before the
//! error is constructed (T-84-06-02).
//!
//! # Example
//!
//! ```no_run
//! # use pmcp_toolkit_mysql::MysqlConnector;
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let conn = MysqlConnector::connect("mysql://localhost/mydb").await?;
//! # Ok(())
//! # }
//! ```

#![allow(clippy::doc_markdown)]

use async_trait::async_trait;
use serde_json::{json, Map, Value};
use sqlx::mysql::{MySqlArguments, MySqlPool, MySqlRow};
use sqlx::query::Query;
use sqlx::{Column, MySql, Row, TypeInfo};

use pmcp_server_toolkit::sql::{
    translate_placeholders, ConnectorError, Dialect, SqlConnector, TranslatedSql,
};

pub mod dev_mock;

/// MySQL connector backed by a `sqlx` [`MySqlPool`].
///
/// Construct with [`MysqlConnector::connect`]. The pool is built lazily via
/// [`MySqlPool::connect_lazy`](sqlx::mysql::MySqlPool::connect_lazy) (REVIEWS
/// M3) — no TCP connection opens until the first
/// [`execute`](SqlConnector::execute) / [`schema_text`](SqlConnector::schema_text)
/// call. `database` is the schema name parsed from the URL, used to filter
/// `information_schema.columns` in [`schema_text`](SqlConnector::schema_text).
pub struct MysqlConnector {
    pool: MySqlPool,
    database: String,
}

/// Redact the password segment of a MySQL connection URL.
///
/// Returns the URL with any `:password@` segment rewritten to `:***@`. Used on
/// the [`MysqlConnector::connect`] error path so a malformed-URL error never
/// echoes the caller's secret (T-84-06-02). Plain best-effort string surgery —
/// it never parses, so it cannot itself fail on malformed input.
fn sanitize_url(url: &str) -> String {
    let Some(scheme_end) = url.find("://") else {
        return url.to_string();
    };
    let authority_start = scheme_end + 3;
    let rest = &url[authority_start..];
    let authority_end = rest.find('/').map_or(rest.len(), |i| i);
    let authority = &rest[..authority_end];

    // Only the userinfo segment (before '@') can carry a password.
    let Some(at) = authority.find('@') else {
        return url.to_string();
    };
    let userinfo = &authority[..at];
    let Some(colon) = userinfo.find(':') else {
        return url.to_string();
    };

    let user = &userinfo[..colon];
    let after_at = &authority[at..];
    format!(
        "{}{}:***{}{}",
        &url[..authority_start],
        user,
        after_at,
        &rest[authority_end..]
    )
}

/// Extract the MySQL database (schema) name from a connection URL.
///
/// Captures the path component after the host authority — e.g.
/// `mysql://host/mydb` → `Some("mydb")`. Any query string after `?` is
/// stripped. Returns `None` when no database segment is present. Used to filter
/// `information_schema.columns` in [`schema_text`](SqlConnector::schema_text)
/// (MySQL has no `'public'` schema like Postgres).
fn parse_database_from_url(url: &str) -> Option<String> {
    let scheme_end = url.find("://")?;
    let rest = &url[scheme_end + 3..];
    let slash = rest.find('/')?;
    let after = &rest[slash + 1..];
    let db = after.split(['?', '/']).next().unwrap_or("");
    if db.is_empty() {
        None
    } else {
        Some(db.to_string())
    }
}

impl MysqlConnector {
    /// Connect to a MySQL backend by URL, returning a pooled connector.
    ///
    /// REVIEWS M3: builds the pool via
    /// [`MySqlPool::connect_lazy`](sqlx::mysql::MySqlPool::connect_lazy), which
    /// parses the URL synchronously and defers TCP I/O to first use. The
    /// constructor therefore returns immediately and is offline-safe — real
    /// connection failures surface on the first
    /// [`execute`](SqlConnector::execute) / [`schema_text`](SqlConnector::schema_text)
    /// call.
    ///
    /// # Errors
    ///
    /// Returns [`ConnectorError::Connection`] if the URL cannot be parsed. The
    /// error message redacts the password via [`sanitize_url`] — it never echoes
    /// the raw secret (T-84-06-02).
    pub async fn connect(url: &str) -> Result<Self, ConnectorError> {
        let database = parse_database_from_url(url).unwrap_or_default();
        // REVIEWS M3: connect_lazy parses the URL synchronously, defers TCP I/O
        // to first use. Matches Plan 06's "returns Ok(Self) on URL parse success".
        let pool = MySqlPool::connect_lazy(url).map_err(|e| {
            ConnectorError::Connection(format!("mysql url ({}): {e}", sanitize_url(url)))
        })?;
        Ok(Self { pool, database })
    }
}

/// Bind one [`Value`] onto a `sqlx` query, dispatching on the JSON variant.
///
/// `serde_json::Value` does not impl `Encode<MySql>` directly, so each variant
/// is bound through a concrete Rust type. `Value::Null` binds an explicit typed
/// `None`; object/array shapes are serialized to a JSON text fallback.
fn bind_one<'q>(
    q: Query<'q, MySql, MySqlArguments>,
    v: &Value,
) -> Query<'q, MySql, MySqlArguments> {
    match v {
        Value::Null => q.bind(None::<&str>),
        Value::Bool(b) => q.bind(*b),
        Value::Number(n) if n.is_i64() => q.bind(n.as_i64().unwrap_or(0)),
        Value::Number(n) => q.bind(n.as_f64().unwrap_or(0.0)),
        Value::String(s) => q.bind(s.clone()),
        arr_or_obj => q.bind(serde_json::to_string(arr_or_obj).unwrap_or_default()),
    }
}

/// Convert one column of a row into a [`Value`], dispatching on the MySQL type
/// name (RESEARCH §1.2). Unknown column types fall through to a text read.
fn column_to_value(row: &MySqlRow, idx: usize, type_name: &str) -> Value {
    match type_name {
        "BIGINT" | "INT" | "MEDIUMINT" | "SMALLINT" | "TINYINT" => row
            .try_get::<Option<i64>, _>(idx)
            .ok()
            .flatten()
            .map_or(Value::Null, |i| json!(i)),
        "DOUBLE" | "FLOAT" => row
            .try_get::<Option<f64>, _>(idx)
            .ok()
            .flatten()
            .map_or(Value::Null, |f| json!(f)),
        "BOOLEAN" | "BOOL" => row
            .try_get::<Option<bool>, _>(idx)
            .ok()
            .flatten()
            .map_or(Value::Null, |b| json!(b)),
        // "VARCHAR" | "TEXT" | "CHAR" | "DECIMAL" and everything else: text read.
        _ => row
            .try_get::<Option<String>, _>(idx)
            .ok()
            .flatten()
            .map_or(Value::Null, |s| json!(s)),
    }
}

/// Convert a driver row into a JSON object keyed by column name (D-01 shape).
fn row_to_value(row: &MySqlRow) -> Value {
    let mut obj = Map::new();
    for (idx, col) in row.columns().iter().enumerate() {
        obj.insert(
            col.name().to_string(),
            column_to_value(row, idx, col.type_info().name()),
        );
    }
    Value::Object(obj)
}

/// Read a string column from an `information_schema` row by name, defaulting to
/// empty on any decode error so DDL rendering never panics mid-pass.
fn schema_col(row: &MySqlRow, name: &str) -> String {
    row.try_get::<String, _>(name).unwrap_or_default()
}

/// Group `information_schema.columns` rows into `CREATE TABLE` blocks with
/// MySQL backtick identifier quoting and an `ENGINE=InnoDB` footer.
///
/// Each row carries `table_name`, `column_name`, `data_type`, and `is_nullable`
/// (`"YES"`/`"NO"`). Rows arrive ordered by `table_name`, `ordinal_position`, so
/// a single pass emits one block per table.
fn format_information_schema_as_ddl(rows: &[MySqlRow]) -> String {
    let mut out = String::new();
    let mut current_table: Option<String> = None;

    for row in rows {
        let table = schema_col(row, "table_name");
        let column = schema_col(row, "column_name");
        let data_type = schema_col(row, "data_type");
        let is_nullable = schema_col(row, "is_nullable");

        if current_table.as_deref() != Some(table.as_str()) {
            if current_table.is_some() {
                out.push_str(") ENGINE=InnoDB;\n");
            }
            out.push_str(&format!("CREATE TABLE `{table}` (\n"));
            current_table = Some(table);
        }
        let not_null = if is_nullable == "NO" { " NOT NULL" } else { "" };
        out.push_str(&format!("  `{column}` {data_type}{not_null}\n"));
    }
    if current_table.is_some() {
        out.push_str(") ENGINE=InnoDB;\n");
    }
    out
}

#[async_trait]
impl SqlConnector for MysqlConnector {
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
        let mut q = sqlx::query(&translated);
        for name in &ordered_params {
            let v = params
                .iter()
                .find(|(k, _)| k == name)
                .map_or(Value::Null, |(_, v)| v.clone());
            q = bind_one(q, &v);
        }
        let rows = q
            .fetch_all(&self.pool)
            .await
            .map_err(|e| ConnectorError::Query(e.to_string()))?;
        Ok(rows.iter().map(row_to_value).collect())
    }

    async fn schema_text(&self) -> Result<String, ConnectorError> {
        let rows = sqlx::query(
            "SELECT table_name, column_name, data_type, is_nullable \
             FROM information_schema.columns WHERE table_schema = ? \
             ORDER BY table_name, ordinal_position",
        )
        .bind(&self.database)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ConnectorError::Schema(e.to_string()))?;
        Ok(format_information_schema_as_ddl(&rows))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_url_redacts_password() {
        assert_eq!(
            sanitize_url("mysql://user:secret@host/db"),
            "mysql://user:***@host/db"
        );
    }

    #[test]
    fn test_sanitize_url_without_password_unchanged() {
        assert_eq!(
            sanitize_url("mysql://host/db"),
            "mysql://host/db",
            "no userinfo → unchanged"
        );
        assert_eq!(
            sanitize_url("mysql://user@host/db"),
            "mysql://user@host/db",
            "user without password → unchanged"
        );
    }

    #[test]
    fn test_parse_database_from_url() {
        assert_eq!(
            parse_database_from_url("mysql://localhost/mydb"),
            Some("mydb".to_string())
        );
        assert_eq!(
            parse_database_from_url("mysql://user:pw@host:3306/shop?ssl=true"),
            Some("shop".to_string()),
            "query string and port are stripped"
        );
        assert_eq!(
            parse_database_from_url("mysql://localhost/"),
            None,
            "empty database segment → None"
        );
        assert_eq!(
            parse_database_from_url("not a url"),
            None,
            "no scheme → None"
        );
    }

    #[test]
    fn test_bind_one_dispatch() {
        // Each variant must build a bindable query without panicking. We can't
        // inspect the bound value off a `Query` (sqlx hides its arg buffer), so
        // the assertion is that dispatch is total across every Value shape.
        for v in [
            Value::Null,
            json!(true),
            json!(42_i64),
            json!(2.5_f64),
            json!("hello"),
            json!([1, 2, 3]),
            json!({"k": "v"}),
        ] {
            let _ = bind_one(sqlx::query("SELECT ?"), &v);
        }
    }

    // REVIEWS M3: connect_lazy returns Ok without opening a TCP connection.
    #[tokio::test]
    async fn test_connect_lazy_returns_ok_without_network() {
        // No MySQL server runs locally; connect_lazy must still return Ok(_)
        // because it only parses the URL and defers I/O to first use.
        let result = MysqlConnector::connect("mysql://localhost/db").await;
        assert!(
            result.is_ok(),
            "connect_lazy must return Ok without a reachable server"
        );
    }

    // REVIEWS M3: a structurally invalid URL fails synchronously, redacted.
    #[tokio::test]
    async fn test_connect_invalid_url_returns_err() {
        match MysqlConnector::connect("not a url").await {
            Err(ConnectorError::Connection(msg)) => {
                assert!(
                    !msg.contains("password"),
                    "error text must not contain the literal 'password'; got: {msg:?}"
                );
            },
            Err(other) => panic!("expected ConnectorError::Connection, got {other:?}"),
            Ok(_) => panic!("malformed URL must error"),
        }
    }

    #[tokio::test]
    async fn test_connect_invalid_url_does_not_echo_password() {
        match MysqlConnector::connect("mysql://u:hunter2@@@bad url/db").await {
            Err(ConnectorError::Connection(msg)) => {
                assert!(
                    !msg.contains("hunter2"),
                    "error text must not echo the password; got: {msg:?}"
                );
            },
            Err(other) => panic!("expected ConnectorError::Connection, got {other:?}"),
            // connect_lazy may accept this — if so there is no error path to check.
            Ok(_) => {},
        }
    }
}
