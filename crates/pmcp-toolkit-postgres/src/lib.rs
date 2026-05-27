//! Postgres connector for pmcp-server-toolkit.
//!
//! Pure-Rust + Lambda-deployable per `feedback_avoid_docker_pure_rust_lambda`
//! memory. Uses tokio-postgres 0.7.17 + deadpool-postgres 0.14.1.
//!
//! [`PostgresConnector`] implements the toolkit's 3-method
//! [`SqlConnector`](pmcp_server_toolkit::sql::SqlConnector) trait:
//! [`dialect`](pmcp_server_toolkit::sql::SqlConnector::dialect),
//! [`execute`](pmcp_server_toolkit::sql::SqlConnector::execute) (canonical
//! `:name` placeholders translated to `$1`/`$2` via
//! [`translate_placeholders`](pmcp_server_toolkit::sql::translate_placeholders)),
//! and [`schema_text`](pmcp_server_toolkit::sql::SqlConnector::schema_text)
//! (driven by `information_schema.columns`).
//!
//! REVIEWS M2: v0.2 `PgParam` supports 5 scalar variants (Null, Bool, I64, F64,
//! Str). Object/Array params are explicitly rejected with
//! [`ConnectorError::ParameterBind`]. JSON support is deferred to v0.3 via
//! `tokio_postgres::types::Json<T>` wrapper once the type-mapping contract is
//! fully designed.
//!
//! REVIEWS H5: the `dev_mock` feature exposes
//! `pmcp_toolkit_postgres::dev_mock::PostgresMock` for examples + downstream
//! integration tests. It is NOT enabled by default.
//!
//! # Security
//!
//! [`ConnectorError::Connection`] error text NEVER contains the raw URL or its
//! password — the password segment is redacted via [`sanitize_url`] before the
//! error is constructed (T-84-05-02).
//!
//! # Example
//!
//! ```no_run
//! # use pmcp_toolkit_postgres::PostgresConnector;
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let conn = PostgresConnector::connect("postgres://localhost/mydb").await?;
//! # Ok(())
//! # }
//! ```

#![allow(clippy::doc_markdown)]

use async_trait::async_trait;
use deadpool_postgres::{Manager, ManagerConfig, Pool};
use serde_json::{json, Map, Value};
use std::error::Error as StdError;
use tokio_postgres::types::{IsNull, ToSql, Type};
use tokio_postgres::NoTls;

use pmcp_server_toolkit::sql::{
    translate_placeholders, ConnectorError, Dialect, SqlConnector, TranslatedSql,
};

pub mod dev_mock;

/// Postgres connector backed by a `deadpool-postgres` [`Pool`].
///
/// Construct with [`PostgresConnector::connect`]. The pool internally spawns the
/// `tokio_postgres::Connection` future (Landmine #13) so callers never have to —
/// using bare `tokio_postgres::connect` would require a manual
/// `tokio::spawn(connection)` that this type sidesteps entirely.
pub struct PostgresConnector {
    pool: Pool,
}

/// Redact the password segment of a Postgres connection URL.
///
/// Returns the URL with any `:password@` segment rewritten to `:***@`. Used
/// on the [`PostgresConnector::connect`] error path so a malformed-URL error
/// never echoes the caller's secret (T-84-05-02). Plain best-effort string
/// surgery — it never parses, so it cannot itself fail on malformed input.
fn sanitize_url(url: &str) -> String {
    // Find the authority section between "://" and the first '/' after it.
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

impl PostgresConnector {
    /// Connect to a Postgres backend by URL, returning a pooled connector.
    ///
    /// The URL is parsed into a `tokio_postgres::Config`, wrapped in a
    /// `deadpool_postgres::Manager`, and built into a [`Pool`]. The pool is lazy
    /// — no TCP connection is opened until the first
    /// [`execute`](SqlConnector::execute) /
    /// [`schema_text`](SqlConnector::schema_text) call acquires a client.
    ///
    /// # Errors
    ///
    /// Returns [`ConnectorError::Connection`] if the URL cannot be parsed or the
    /// pool cannot be built. The error message redacts the password via
    /// [`sanitize_url`] — it never echoes the raw secret (T-84-05-02).
    pub async fn connect(url: &str) -> Result<Self, ConnectorError> {
        let pg_config: tokio_postgres::Config =
            url.parse().map_err(|e: tokio_postgres::Error| {
                ConnectorError::Connection(format!("invalid url ({}): {e}", sanitize_url(url)))
            })?;
        let mgr = Manager::from_config(pg_config, NoTls, ManagerConfig::default());
        let pool = Pool::builder(mgr)
            .max_size(16)
            .build()
            .map_err(|e| ConnectorError::Connection(format!("pool build: {e}")))?;
        Ok(Self { pool })
    }
}

/// A single bound parameter value, narrowed to the 5 scalar shapes a v0.2
/// Postgres connector accepts (REVIEWS M2).
///
/// `serde_json::Value` does NOT impl `ToSql` without tokio-postgres's
/// `with-serde_json-1` feature, so this enum is the bridge: it owns the value
/// and implements [`ToSql`] by dispatching to the inner Rust type. Object/array
/// params never reach this enum — [`value_to_pg_param`] rejects them upstream.
#[derive(Debug, Clone)]
enum PgParam {
    Null,
    Bool(bool),
    I64(i64),
    F64(f64),
    Str(String),
}

impl ToSql for PgParam {
    fn to_sql(
        &self,
        ty: &Type,
        out: &mut bytes::BytesMut,
    ) -> Result<IsNull, Box<dyn StdError + Sync + Send>> {
        match self {
            PgParam::Null => Ok(IsNull::Yes),
            PgParam::Bool(b) => b.to_sql(ty, out),
            PgParam::I64(i) => i.to_sql(ty, out),
            PgParam::F64(f) => f.to_sql(ty, out),
            PgParam::Str(s) => s.to_sql(ty, out),
        }
    }

    // Adaptive: defer the type check to the server. The scalar variants cover
    // the common int/float/bool/text/null columns; Postgres reports a mismatch
    // at query time if a value cannot coerce.
    fn accepts(_ty: &Type) -> bool {
        true
    }

    tokio_postgres::types::to_sql_checked!();
}

/// Convert a named [`Value`] into a [`PgParam`], rejecting object/array shapes.
///
/// REVIEWS M2: `Value::Object` / `Value::Array` return
/// [`ConnectorError::ParameterBind`] — JSON params require Postgres JSON support
/// deferred to v0.3. `name` is threaded in so the error names the offending
/// parameter.
///
/// # Errors
///
/// Returns [`ConnectorError::ParameterBind`] when `v` is a JSON object or array.
fn value_to_pg_param(name: &str, v: &Value) -> Result<PgParam, ConnectorError> {
    match v {
        Value::Null => Ok(PgParam::Null),
        Value::Bool(b) => Ok(PgParam::Bool(*b)),
        Value::Number(n) if n.is_i64() => Ok(PgParam::I64(n.as_i64().unwrap_or(0))),
        Value::Number(n) => Ok(PgParam::F64(n.as_f64().unwrap_or(0.0))),
        Value::String(s) => Ok(PgParam::Str(s.clone())),
        Value::Array(_) | Value::Object(_) => Err(ConnectorError::ParameterBind {
            name: name.to_string(),
            reason: "object/array params require Postgres JSON support (deferred to v0.3)"
                .to_string(),
        }),
    }
}

/// Convert one column of a row into a [`Value`], dispatching on the Postgres
/// type name (RESEARCH §1.1). Unknown / JSON column types fall through to a
/// text read (REVIEWS M2 — column-side JSON mapping is a v0.3 increment).
fn column_to_value(row: &tokio_postgres::Row, idx: usize, type_name: &str) -> Value {
    match type_name {
        "int8" | "int4" | "int2" => row
            .try_get::<_, Option<i64>>(idx)
            .ok()
            .flatten()
            .map_or(Value::Null, |i| json!(i)),
        "float8" | "float4" => row
            .try_get::<_, Option<f64>>(idx)
            .ok()
            .flatten()
            .map_or(Value::Null, |f| json!(f)),
        "bool" => row
            .try_get::<_, Option<bool>>(idx)
            .ok()
            .flatten()
            .map_or(Value::Null, |b| json!(b)),
        // "text" | "varchar" | "char" | "bpchar" and everything else: text read.
        _ => row
            .try_get::<_, Option<String>>(idx)
            .ok()
            .flatten()
            .map_or(Value::Null, |s| json!(s)),
    }
}

/// Convert a driver row into a JSON object keyed by column name (D-01 shape).
fn row_to_value(row: &tokio_postgres::Row) -> Value {
    let mut obj = Map::new();
    for (idx, col) in row.columns().iter().enumerate() {
        obj.insert(
            col.name().to_string(),
            column_to_value(row, idx, col.type_().name()),
        );
    }
    Value::Object(obj)
}

/// Build the ordered [`PgParam`] bind list from the translated bind order and
/// the caller's named params. A name absent from `params` binds `NULL`.
///
/// # Errors
///
/// Propagates [`ConnectorError::ParameterBind`] from [`value_to_pg_param`] when
/// any bound value is a JSON object or array (REVIEWS M2).
fn build_bind_list(
    ordered_params: &[String],
    params: &[(String, Value)],
) -> Result<Vec<PgParam>, ConnectorError> {
    ordered_params
        .iter()
        .map(|n| {
            let v = params
                .iter()
                .find(|(k, _)| k == n)
                .map_or(Value::Null, |(_, v)| v.clone());
            value_to_pg_param(n, &v)
        })
        .collect()
}

/// Group `information_schema.columns` rows into `CREATE TABLE` blocks.
///
/// Each row carries `table_name`, `column_name`, `data_type`, and
/// `is_nullable` (`"YES"`/`"NO"`). Rows arrive ordered by `table_name`,
/// `ordinal_position`, so a single pass emits one block per table.
fn format_information_schema_as_ddl(rows: &[tokio_postgres::Row]) -> String {
    let mut out = String::new();
    let mut current_table: Option<String> = None;

    for row in rows {
        let table: String = row.get("table_name");
        let column: String = row.get("column_name");
        let data_type: String = row.get("data_type");
        let is_nullable: String = row.get("is_nullable");

        if current_table.as_deref() != Some(table.as_str()) {
            if current_table.is_some() {
                out.push_str(");\n");
            }
            out.push_str(&format!("CREATE TABLE {table} (\n"));
            current_table = Some(table);
        }
        let not_null = if is_nullable == "NO" { " NOT NULL" } else { "" };
        out.push_str(&format!("  {column} {data_type}{not_null}\n"));
    }
    if current_table.is_some() {
        out.push_str(");\n");
    }
    out
}

#[async_trait]
impl SqlConnector for PostgresConnector {
    fn dialect(&self) -> Dialect {
        Dialect::Postgres
    }

    async fn execute(
        &self,
        sql: &str,
        params: &[(String, Value)],
    ) -> Result<Vec<Value>, ConnectorError> {
        let TranslatedSql {
            sql: translated,
            ordered_params,
        } = translate_placeholders(sql, Dialect::Postgres);
        // REVIEWS M2: propagate ParameterBind errors for object/array params.
        let owned = build_bind_list(&ordered_params, params)?;
        let refs: Vec<&(dyn ToSql + Sync)> =
            owned.iter().map(|p| p as &(dyn ToSql + Sync)).collect();
        let client = self
            .pool
            .get()
            .await
            .map_err(|e| ConnectorError::Connection(format!("pool acquire: {e}")))?;
        let rows = client
            .query(&translated, &refs)
            .await
            .map_err(|e| ConnectorError::Query(e.to_string()))?;
        Ok(rows.iter().map(row_to_value).collect())
    }

    async fn schema_text(&self) -> Result<String, ConnectorError> {
        let client = self
            .pool
            .get()
            .await
            .map_err(|e| ConnectorError::Connection(format!("pool acquire: {e}")))?;
        let rows = client
            .query(
                "SELECT table_name, column_name, data_type, is_nullable, character_maximum_length \
                 FROM information_schema.columns WHERE table_schema = 'public' \
                 ORDER BY table_name, ordinal_position",
                &[],
            )
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
            sanitize_url("postgres://user:secret@host/db"),
            "postgres://user:***@host/db"
        );
    }

    #[test]
    fn test_sanitize_url_without_password_unchanged() {
        assert_eq!(
            sanitize_url("postgres://host/db"),
            "postgres://host/db",
            "no userinfo → unchanged"
        );
        assert_eq!(
            sanitize_url("postgres://user@host/db"),
            "postgres://user@host/db",
            "user without password → unchanged"
        );
    }

    #[tokio::test]
    async fn test_connect_invalid_url_returns_connection_error() {
        // `PostgresConnector` is not `Debug` (it wraps a deadpool `Pool`), so we
        // match the Result directly rather than via `expect_err`.
        match PostgresConnector::connect("postgres://u:secret@:notaport/db").await {
            Err(ConnectorError::Connection(msg)) => {
                assert!(
                    !msg.contains("secret"),
                    "error text must not echo the password; got: {msg:?}"
                );
                assert!(
                    !msg.contains("password"),
                    "error text must not contain the literal 'password'; got: {msg:?}"
                );
            },
            Err(other) => panic!("expected ConnectorError::Connection, got {other:?}"),
            Ok(_) => panic!("malformed URL must error"),
        }
    }

    #[test]
    fn test_value_to_pg_param_scalar_dispatch() {
        assert!(matches!(
            value_to_pg_param("x", &Value::Null),
            Ok(PgParam::Null)
        ));
        assert!(matches!(
            value_to_pg_param("x", &json!(true)),
            Ok(PgParam::Bool(true))
        ));
        assert!(matches!(
            value_to_pg_param("x", &json!(42_i64)),
            Ok(PgParam::I64(42))
        ));
        assert!(matches!(
            value_to_pg_param("x", &json!(2.5_f64)),
            Ok(PgParam::F64(_))
        ));
        assert!(matches!(
            value_to_pg_param("x", &json!("hi")),
            Ok(PgParam::Str(_))
        ));
    }

    #[test]
    fn test_value_to_pg_param_rejects_object() {
        let err = value_to_pg_param("user_meta", &json!({"key": "value"}))
            .expect_err("object param must be rejected");
        match err {
            ConnectorError::ParameterBind { name, reason } => {
                assert_eq!(name, "user_meta");
                assert!(
                    reason.contains("object/array params require Postgres JSON support"),
                    "reason: {reason:?}"
                );
            },
            other => panic!("expected ParameterBind, got {other:?}"),
        }
    }

    #[test]
    fn test_value_to_pg_param_rejects_array() {
        let err =
            value_to_pg_param("tags", &json!([1, 2, 3])).expect_err("array param must be rejected");
        match err {
            ConnectorError::ParameterBind { name, reason } => {
                assert_eq!(name, "tags");
                assert!(
                    reason.contains("object/array params require Postgres JSON support"),
                    "reason: {reason:?}"
                );
            },
            other => panic!("expected ParameterBind, got {other:?}"),
        }
    }

    #[test]
    fn test_build_bind_list_propagates_object_rejection() {
        let err = build_bind_list(&["x".to_string()], &[("x".to_string(), json!({"k": "v"}))])
            .expect_err("object bind must propagate as error");
        assert!(matches!(
            err,
            ConnectorError::ParameterBind { ref name, .. } if name == "x"
        ));
    }

    #[test]
    fn test_build_bind_list_missing_name_binds_null() {
        let list = build_bind_list(&["x".to_string()], &[]).expect("missing name binds NULL");
        assert_eq!(list.len(), 1);
        assert!(matches!(list[0], PgParam::Null));
    }
}
