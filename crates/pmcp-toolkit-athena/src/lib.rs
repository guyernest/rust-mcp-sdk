//! Amazon Athena connector for pmcp-server-toolkit.
//!
//! Implements `pmcp_server_toolkit::sql::SqlConnector` over `aws-sdk-athena`
//! (no `aws-sdk-glue` — `GetTableMetadata` covers schema introspection,
//! Landmine #4). Pure-Rust + Lambda-deployable per
//! `feedback_avoid_docker_pure_rust_lambda` memory.
//!
//! [`AthenaConnector`] implements the toolkit's 3-method
//! [`SqlConnector`](pmcp_server_toolkit::sql::SqlConnector) trait:
//! [`dialect`](pmcp_server_toolkit::sql::SqlConnector::dialect),
//! [`execute`](pmcp_server_toolkit::sql::SqlConnector::execute) (canonical
//! `:name` placeholders translated to `?` via
//! [`translate_placeholders`](pmcp_server_toolkit::sql::translate_placeholders),
//! values threaded into `StartQueryExecution.execution_parameters`), and
//! [`schema_text`](pmcp_server_toolkit::sql::SqlConnector::schema_text) (driven
//! by `GetTableMetadata`, NOT the Glue catalog).
//!
//! Athena is a query-then-poll backend: `execute` issues
//! `StartQueryExecution`, polls `GetQueryExecution` with capped exponential
//! backoff until the query reaches a terminal state, then paginates
//! `GetQueryResults` via `next_token` (REVIEWS M5) and converts the
//! `VarCharValue` cells into JSON objects keyed by column name (D-01).
//!
//! REVIEWS M4: the public constructor honours CONTEXT.md D-08 (LOCKED) —
//! [`AthenaConnector::from_config`] takes EXACTLY two positional args
//! (`region`, `workgroup`). The remaining knobs (`database`, `output_location`,
//! `query_timeout_ms`, `tables`) are applied via builder methods or by
//! constructing an [`AthenaConfig`] and feeding it to
//! [`AthenaConnector::from_athena_config`]. `output_location` is REQUIRED before
//! the first [`execute`](SqlConnector::execute) — a runtime gate rejects an
//! empty value.
//!
//! REVIEWS H5: the `dev_mock` feature exposes
//! `pmcp_toolkit_athena::dev_mock::AthenaMock` for examples + downstream
//! integration tests. It is NOT enabled by default.
//!
//! # Security
//!
//! [`ConnectorError`] error text NEVER contains raw AWS credentials — AKIA-shaped
//! access-key IDs, long secret-key runs, and `Bearer` tokens are redacted via
//! [`strip_aws_credentials`] before the error is constructed (T-84-07-02).
//!
//! # Example
//!
//! ```no_run
//! use pmcp_toolkit_athena::AthenaConnector;
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! // REVIEWS M4: 2-arg constructor per CONTEXT.md D-08, builders fill the rest.
//! let conn = AthenaConnector::from_config("us-east-1", "primary")
//!     .await?
//!     .with_database("analytics")
//!     .with_output_location("s3://my-bucket/athena-results/")
//!     .with_tables(vec!["images".to_string()]);
//! # let _ = conn;
//! # Ok(()) }
//! ```

#![allow(clippy::doc_markdown)]

use async_trait::async_trait;
use aws_sdk_athena::types::{
    QueryExecutionContext, QueryExecutionState, ResultConfiguration, ResultSet, TableMetadata,
};
use aws_sdk_athena::Client;
use serde_json::{json, Map, Value};
use std::time::Duration;
use tokio::time::Instant;

use pmcp_server_toolkit::sql::{
    translate_placeholders, ConnectorError, Dialect, SqlConnector, TranslatedSql,
};

pub mod dev_mock;

/// Initial poll delay (ms) for the [`poll_until_done`] backoff loop.
const INITIAL_BACKOFF_MS: u64 = 500;
/// Backoff cap (ms): no single poll waits longer than this.
const BACKOFF_CAP_MS: u64 = 5_000;
/// Athena catalog name used for `GetTableMetadata` lookups.
const DATA_CATALOG: &str = "AwsDataCatalog";

/// Configuration for an [`AthenaConnector`].
///
/// REVIEWS M4: the public constructor [`AthenaConnector::from_config`] takes only
/// `region` + `workgroup` (CONTEXT.md D-08, LOCKED). This struct carries the full
/// set of knobs so consumers that need everything at once can build it directly
/// and call [`AthenaConnector::from_athena_config`]. All fields are `pub` for
/// ergonomic read/write.
#[derive(Debug, Clone)]
pub struct AthenaConfig {
    /// AWS region the Athena workgroup lives in (e.g. `"us-east-1"`).
    pub region: String,
    /// Athena workgroup that scopes query execution + result configuration.
    pub workgroup: String,
    /// Glue/Athena database (schema) name; defaults to `"default"`.
    pub database: String,
    /// `s3://...` URI Athena writes query results to. REQUIRED before the first
    /// [`execute`](SqlConnector::execute); an empty value is rejected at runtime.
    pub output_location: String,
    /// Total query budget in milliseconds; the poll loop bails when exceeded.
    pub query_timeout_ms: u64,
    /// Table names [`schema_text`](SqlConnector::schema_text) introspects via
    /// `GetTableMetadata`.
    pub tables: Vec<String>,
}

impl AthenaConfig {
    /// Construct a config with `region` + `workgroup` and defaulted knobs:
    /// `database = "default"`, `output_location = ""`, `query_timeout_ms =
    /// 60_000`, `tables = []`.
    #[must_use]
    pub fn new(region: impl Into<String>, workgroup: impl Into<String>) -> Self {
        Self {
            region: region.into(),
            workgroup: workgroup.into(),
            database: "default".into(),
            output_location: String::new(),
            query_timeout_ms: 60_000,
            tables: Vec::new(),
        }
    }
}

/// Amazon Athena connector backed by `aws-sdk-athena`.
///
/// Construct with [`AthenaConnector::from_config`] (REVIEWS M4: 2-arg, per
/// CONTEXT.md D-08) then chain builder methods, or build an [`AthenaConfig`] and
/// pass it to [`AthenaConnector::from_athena_config`]. The connector is a query-
/// then-poll surface: [`execute`](SqlConnector::execute) starts a query, polls to
/// completion, and paginates the result set.
pub struct AthenaConnector {
    client: Client,
    config: AthenaConfig,
}

impl AthenaConnector {
    /// Construct an Athena connector from a region + workgroup.
    ///
    /// REVIEWS M4: EXACTLY 2 positional args per CONTEXT.md D-08 (LOCKED). The
    /// connector starts with `database = "default"`, `output_location = ""`,
    /// `query_timeout_ms = 60_000`, `tables = []`. Apply the rest via the
    /// `with_*` builder methods (`output_location` is required before
    /// [`execute`](SqlConnector::execute)).
    ///
    /// # Errors
    ///
    /// Returns [`ConnectorError::Connection`] if AWS configuration cannot load.
    pub async fn from_config(region: &str, workgroup: &str) -> Result<Self, ConnectorError> {
        Self::from_athena_config(AthenaConfig::new(region, workgroup)).await
    }

    /// Construct an Athena connector from a fully-populated [`AthenaConfig`]
    /// (REVIEWS M4 secondary constructor).
    ///
    /// # Errors
    ///
    /// Returns [`ConnectorError::Connection`] if AWS configuration cannot load.
    pub async fn from_athena_config(cfg: AthenaConfig) -> Result<Self, ConnectorError> {
        let aws_cfg = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .region(aws_config::Region::new(cfg.region.clone()))
            .load()
            .await;
        let client = Client::new(&aws_cfg);
        Ok(Self {
            client,
            config: cfg,
        })
    }

    /// Set the Glue/Athena database (schema) name. Builder method (REVIEWS M4).
    #[must_use]
    pub fn with_database(mut self, db: impl Into<String>) -> Self {
        self.config.database = db.into();
        self
    }

    /// Set the `s3://...` result output location. Builder method (REVIEWS M4).
    #[must_use]
    pub fn with_output_location(mut self, loc: impl Into<String>) -> Self {
        self.config.output_location = loc.into();
        self
    }

    /// Set the total query timeout budget in milliseconds. Builder method
    /// (REVIEWS M4).
    #[must_use]
    pub fn with_query_timeout(mut self, ms: u64) -> Self {
        self.config.query_timeout_ms = ms;
        self
    }

    /// Set the table names [`schema_text`](SqlConnector::schema_text)
    /// introspects. Builder method (REVIEWS M4).
    #[must_use]
    pub fn with_tables(mut self, tables: Vec<String>) -> Self {
        self.config.tables = tables;
        self
    }
}

/// Stringify a JSON [`Value`] for Athena's `execution_parameters` wire format.
///
/// Athena (Presto/Trino) binds positional parameters as strings; type info is
/// lost on the wire but injection is prevented because Presto binds the strings
/// parametrically. `Null` → empty string, scalars → their natural rendering,
/// arrays/objects → compact JSON text.
fn stringify_value(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        arr_or_obj => serde_json::to_string(arr_or_obj).unwrap_or_default(),
    }
}

/// Next poll delay: double the previous wait, capped at `cap`.
///
/// Used by [`poll_until_done`] so a long-running Athena query is polled with
/// exponential backoff but never waits more than `cap` ms between checks. The
/// sequence from the [`INITIAL_BACKOFF_MS`] seed with `cap == 5000` is
/// `500, 1000, 2000, 4000, 5000, 5000, ...`.
fn next_backoff_ms(prev: u64, cap: u64) -> u64 {
    prev.saturating_mul(2).min(cap)
}

/// Build the ordered positional bind list for a translated statement.
///
/// Each entry in `ordered_params` is matched against the caller's named pairs;
/// a missing name binds the empty string (Athena treats it as a NULL-ish empty
/// parameter). Stringified per [`stringify_value`].
fn build_execution_parameters(
    ordered_params: &[String],
    params: &[(String, Value)],
) -> Vec<String> {
    ordered_params
        .iter()
        .map(|n| {
            params
                .iter()
                .find(|(k, _)| k == n)
                .map_or_else(String::new, |(_, v)| stringify_value(v))
        })
        .collect()
}

/// Redact AWS credentials from an arbitrary error string before it reaches an
/// MCP client (T-84-07-02 / Landmine #10).
///
/// Replaces every `AKIA`/`ASIA`-prefixed 20-char access-key ID and long
/// secret-key-shaped base64 runs (≥40 chars) with `***`. Best-effort token scan
/// — it never parses, so it cannot fail on malformed input.
fn strip_aws_credentials(s: &str) -> String {
    s.split_whitespace()
        .map(redact_token)
        .collect::<Vec<_>>()
        .join(" ")
}

/// Redact a single whitespace-delimited token if it looks like a credential.
///
/// Splits the helper out of [`strip_aws_credentials`] so each stays well under
/// PMAT cog 25.
fn redact_token(token: &str) -> String {
    // Strip surrounding punctuation so "AKIA...," still matches the shape.
    let trimmed = token.trim_matches(|c: char| !c.is_ascii_alphanumeric());
    if !trimmed.is_empty() && looks_like_credential(trimmed) {
        token.replace(trimmed, "***")
    } else {
        token.to_string()
    }
}

/// True if `t` matches a known AWS credential shape: an `AKIA`/`ASIA` access-key
/// ID (20 uppercase-alphanumeric chars) or a long secret-key-shaped run.
fn looks_like_credential(t: &str) -> bool {
    let is_access_key = t.len() == 20
        && (t.starts_with("AKIA") || t.starts_with("ASIA"))
        && t.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit());
    let is_secret_run = t.len() >= 40
        && t.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '/' || c == '+' || c == '=');
    is_access_key || is_secret_run
}

/// Poll `GetQueryExecution` with capped exponential backoff until the query
/// reaches a terminal state.
///
/// Returns `Ok(())` on `Succeeded`; [`ConnectorError::Query`] on
/// `Failed`/`Cancelled` (with the state-change reason, credential-stripped); and
/// [`ConnectorError::Query`] if `query_timeout_ms` elapses before a terminal
/// state. Backoff starts at [`INITIAL_BACKOFF_MS`] and doubles to
/// [`BACKOFF_CAP_MS`] so the timeout check fires within bounded latency
/// (T-84-07-04).
///
/// # Errors
///
/// See above — failed/cancelled query or elapsed timeout.
async fn poll_until_done(
    client: &Client,
    exec_id: &str,
    query_timeout_ms: u64,
) -> Result<(), ConnectorError> {
    let deadline = Instant::now() + Duration::from_millis(query_timeout_ms);
    let mut backoff = INITIAL_BACKOFF_MS;
    loop {
        let resp = client
            .get_query_execution()
            .query_execution_id(exec_id)
            .send()
            .await
            .map_err(|e| ConnectorError::Query(strip_aws_credentials(&e.to_string())))?;
        let status = resp.query_execution().and_then(|q| q.status());
        match status.and_then(|s| s.state()) {
            Some(QueryExecutionState::Succeeded) => return Ok(()),
            Some(state @ (QueryExecutionState::Failed | QueryExecutionState::Cancelled)) => {
                let reason = status
                    .and_then(|s| s.state_change_reason())
                    .unwrap_or("no reason provided");
                return Err(ConnectorError::Query(strip_aws_credentials(&format!(
                    "query {state:?}: {reason}"
                ))));
            },
            _ => {},
        }
        if Instant::now() >= deadline {
            return Err(ConnectorError::Query(format!(
                "query timed out after {query_timeout_ms}ms"
            )));
        }
        tokio::time::sleep(Duration::from_millis(backoff)).await;
        backoff = next_backoff_ms(backoff, BACKOFF_CAP_MS);
    }
}

/// Extract column headers from the first row of a [`ResultSet`].
///
/// Athena's `GetQueryResults` returns the column names as the literal first row
/// of page 1. Returns `None` when the result set has no rows.
fn extract_headers(rs: &ResultSet) -> Option<Vec<String>> {
    let first = rs.rows().first()?;
    Some(
        first
            .data()
            .iter()
            .map(|d| d.var_char_value().unwrap_or_default().to_string())
            .collect(),
    )
}

/// Accumulate data rows from a [`ResultSet`] into `out` as JSON objects keyed by
/// the cached `headers`.
///
/// `skip_first` drops row 0 on the first page (it carries the headers, not
/// data). Subsequent pages pass `skip_first = false`. Cells beyond the header
/// count are ignored; missing cells bind `Value::Null`.
fn accumulate_rows(out: &mut Vec<Value>, rs: &ResultSet, skip_first: bool, headers: &[String]) {
    let data_rows = rs.rows().iter().skip(usize::from(skip_first));
    for row in data_rows {
        let cells = row.data();
        let mut obj = Map::new();
        for (idx, name) in headers.iter().enumerate() {
            let value = cells
                .get(idx)
                .and_then(|d| d.var_char_value())
                .map_or(Value::Null, |s| json!(s));
            obj.insert(name.clone(), value);
        }
        out.push(Value::Object(obj));
    }
}

/// Paginate `GetQueryResults` for `exec_id`, accumulating rows from EVERY page.
///
/// REVIEWS M5: loops on `next_token` until exhausted so a multi-page result set
/// is never silently truncated (T-84-07-06). The header row (page 1, row 0) is
/// extracted once and reused for every subsequent page.
///
/// # Errors
///
/// Returns [`ConnectorError::Query`] (credential-stripped) on any
/// `GetQueryResults` failure.
async fn paginated_get_query_results(
    client: &Client,
    exec_id: &str,
) -> Result<Vec<Value>, ConnectorError> {
    let mut next_token: Option<String> = None;
    let mut all_rows: Vec<Value> = Vec::new();
    let mut headers: Option<Vec<String>> = None;
    loop {
        let mut req = client.get_query_results().query_execution_id(exec_id);
        if let Some(ref t) = next_token {
            req = req.next_token(t.clone());
        }
        let resp = req
            .send()
            .await
            .map_err(|e| ConnectorError::Query(strip_aws_credentials(&e.to_string())))?;
        if let Some(rs) = resp.result_set() {
            let skip_first = headers.is_none();
            if headers.is_none() {
                headers = extract_headers(rs);
            }
            if let Some(ref h) = headers {
                accumulate_rows(&mut all_rows, rs, skip_first, h);
            }
        }
        next_token = resp.next_token().map(ToString::to_string);
        if next_token.is_none() {
            break;
        }
    }
    Ok(all_rows)
}

/// Render a column slice as comma-separated `  name type` DDL lines.
fn format_columns(columns: &[aws_sdk_athena::types::Column]) -> String {
    columns
        .iter()
        .map(|c| format!("  {} {}", c.name(), c.r#type().unwrap_or("string")))
        .collect::<Vec<_>>()
        .join(",\n")
}

/// Render a [`TableMetadata`] as `CREATE EXTERNAL TABLE` DDL.
///
/// Emits the column list, an optional `PARTITIONED BY` block from the table's
/// partition keys, and a `LOCATION 's3://...'` line drawn from the table
/// parameters' `"location"` entry (empty when absent). Schema is sourced from
/// `GetTableMetadata`, NOT the Glue catalog (Landmine #4).
fn format_table_metadata(meta: &TableMetadata) -> String {
    let mut out = String::new();
    out.push_str(&format!("CREATE EXTERNAL TABLE {} (\n", meta.name()));
    out.push_str(&format_columns(meta.columns()));
    out.push_str("\n)\n");
    let partitions = meta.partition_keys();
    if !partitions.is_empty() {
        out.push_str(&format!(
            "PARTITIONED BY (\n{}\n)\n",
            format_columns(partitions)
        ));
    }
    let location = meta
        .parameters()
        .and_then(|p| p.get("location"))
        .map_or("", String::as_str);
    out.push_str(&format!("LOCATION '{location}';\n"));
    out
}

#[async_trait]
impl SqlConnector for AthenaConnector {
    fn dialect(&self) -> Dialect {
        Dialect::Athena
    }

    async fn execute(
        &self,
        sql: &str,
        params: &[(String, Value)],
    ) -> Result<Vec<Value>, ConnectorError> {
        // REVIEWS M4 runtime gate: output_location is required before execute.
        if self.config.output_location.is_empty() {
            return Err(ConnectorError::Connection(
                "output_location not configured — call .with_output_location(...) before execute"
                    .into(),
            ));
        }
        let TranslatedSql {
            sql: translated,
            ordered_params,
        } = translate_placeholders(sql, Dialect::Athena);
        let stringified = build_execution_parameters(&ordered_params, params);
        let exec = self
            .client
            .start_query_execution()
            .query_string(translated)
            .query_execution_context(
                QueryExecutionContext::builder()
                    .database(&self.config.database)
                    .build(),
            )
            .work_group(&self.config.workgroup)
            .result_configuration(
                ResultConfiguration::builder()
                    .output_location(&self.config.output_location)
                    .build(),
            )
            .set_execution_parameters(if stringified.is_empty() {
                None
            } else {
                Some(stringified)
            })
            .send()
            .await
            .map_err(|e| ConnectorError::Query(strip_aws_credentials(&e.to_string())))?;
        let exec_id = exec
            .query_execution_id()
            .ok_or_else(|| ConnectorError::Query("missing query execution id".into()))?;
        poll_until_done(&self.client, exec_id, self.config.query_timeout_ms).await?;
        // REVIEWS M5: paginate GetQueryResults across all pages.
        paginated_get_query_results(&self.client, exec_id).await
    }

    async fn schema_text(&self) -> Result<String, ConnectorError> {
        let mut out = String::new();
        for table in &self.config.tables {
            let meta = self
                .client
                .get_table_metadata()
                .catalog_name(DATA_CATALOG)
                .database_name(&self.config.database)
                .table_name(table)
                .send()
                .await
                .map_err(|e| ConnectorError::Schema(strip_aws_credentials(&e.to_string())))?;
            if let Some(m) = meta.table_metadata() {
                out.push_str(&format_table_metadata(m));
                out.push('\n');
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aws_sdk_athena::types::{Column, TableMetadata};
    use std::collections::HashMap;

    #[test]
    fn test_dialect_is_athena() {
        // dialect() is a pure constant; assert via the Dialect enum so the test
        // needs no AWS client construction.
        assert_eq!(Dialect::Athena.name(), "Amazon Athena (Presto/Trino)");
    }

    #[test]
    fn test_stringify_value_dispatch() {
        assert_eq!(stringify_value(&Value::Null), "");
        assert_eq!(stringify_value(&json!(true)), "true");
        assert_eq!(stringify_value(&json!(42)), "42");
        assert_eq!(stringify_value(&json!(2.5)), "2.5");
        assert_eq!(stringify_value(&json!("hello")), "hello");
        assert_eq!(stringify_value(&json!([1, 2])), "[1,2]");
        assert_eq!(stringify_value(&json!({"k": "v"})), "{\"k\":\"v\"}");
    }

    #[test]
    fn test_next_backoff_ms_doubles_to_cap_5000() {
        let mut seq = Vec::new();
        let mut b = INITIAL_BACKOFF_MS;
        for _ in 0..6 {
            seq.push(b);
            b = next_backoff_ms(b, BACKOFF_CAP_MS);
        }
        assert_eq!(seq, vec![500, 1000, 2000, 4000, 5000, 5000]);
        assert_eq!(next_backoff_ms(500, 5000), 1000);
        assert_eq!(next_backoff_ms(4000, 5000), 5000);
        assert_eq!(next_backoff_ms(5000, 5000), 5000);
    }

    #[test]
    fn test_strip_aws_credentials_redacts_akia_prefix() {
        let redacted = strip_aws_credentials("err: AKIAIOSFODNN7EXAMPLE oops");
        assert!(redacted.contains("***"), "expected redaction, got: {redacted}");
        assert!(
            !redacted.contains("AKIAIOSFODNN7EXAMPLE"),
            "access key must be stripped, got: {redacted}"
        );
        assert!(redacted.contains("err:"), "safe text preserved: {redacted}");
        assert!(redacted.contains("oops"), "safe text preserved: {redacted}");
    }

    #[test]
    fn test_strip_aws_credentials_redacts_secret_run() {
        // 40-char secret-key-shaped run.
        let secret = "wJalrXUtnFEMIK7MDENGbPxRfiCYEXAMPLEKEYabc";
        let redacted = strip_aws_credentials(&format!("boom {secret} end"));
        assert!(redacted.contains("***"));
        assert!(!redacted.contains(secret), "secret run stripped: {redacted}");
    }

    #[test]
    fn test_strip_aws_credentials_does_not_destroy_safe_text() {
        let safe = "query error: table not found near line 3";
        assert_eq!(strip_aws_credentials(safe), safe);
    }

    #[test]
    fn test_build_execution_parameters_orders_and_stringifies() {
        let ordered = vec!["a".to_string(), "b".to_string(), "a".to_string()];
        let params = vec![("a".to_string(), json!(1)), ("b".to_string(), json!("x"))];
        let out = build_execution_parameters(&ordered, &params);
        assert_eq!(out, vec!["1", "x", "1"]);
    }

    #[test]
    fn test_build_execution_parameters_missing_name_binds_empty() {
        let ordered = vec!["missing".to_string()];
        let out = build_execution_parameters(&ordered, &[]);
        assert_eq!(out, vec![String::new()]);
    }

    #[test]
    fn test_format_table_metadata_shape() {
        let mut parameters = HashMap::new();
        parameters.insert("location".to_string(), "s3://example/images/".to_string());
        let col = |name: &str| {
            Column::builder()
                .name(name)
                .r#type("string")
                .build()
                .expect("column builds")
        };
        let meta = TableMetadata::builder()
            .name("images")
            .columns(col("image_id"))
            .columns(col("label"))
            .partition_keys(col("dt"))
            .set_parameters(Some(parameters))
            .build()
            .expect("table metadata builds");
        let text = format_table_metadata(&meta);
        assert!(text.contains("CREATE EXTERNAL TABLE images"), "{text}");
        assert!(text.contains("PARTITIONED BY"), "{text}");
        assert!(text.contains("LOCATION 's3://example/images/'"), "{text}");
        assert!(text.contains("image_id string"), "{text}");
    }

    // REVIEWS M4: from_config takes EXACTLY 2 positional args per D-08 (LOCKED).
    // AthenaConfig::new is the synchronous proof the default knobs match D-08;
    // from_config(region, workgroup) calls it verbatim.
    #[test]
    fn test_from_config_two_args_matches_d08_signature() {
        let cfg = AthenaConfig::new("us-east-1", "primary");
        assert_eq!(cfg.region, "us-east-1");
        assert_eq!(cfg.workgroup, "primary");
        assert_eq!(cfg.database, "default");
        assert_eq!(cfg.output_location, "");
        assert_eq!(cfg.query_timeout_ms, 60_000);
        assert!(cfg.tables.is_empty());
    }

    // REVIEWS M4: execute() rejects an empty output_location BEFORE any AWS call.
    #[tokio::test]
    async fn test_execute_without_output_location_returns_connection_error() {
        let conn = AthenaConnector::from_config("us-east-1", "primary")
            .await
            .expect("from_config builds offline");
        match conn.execute("SELECT 1", &[]).await {
            Err(ConnectorError::Connection(msg)) => {
                assert!(
                    msg.contains("output_location not configured"),
                    "runtime gate message: {msg}"
                );
            },
            other => panic!("expected Connection error, got {other:?}"),
        }
    }

    // REVIEWS M4: builder methods populate the AthenaConfig.
    #[tokio::test]
    async fn test_with_builders_populate_athena_config() {
        let conn = AthenaConnector::from_config("eu-west-1", "wg")
            .await
            .expect("from_config builds offline")
            .with_database("analytics")
            .with_output_location("s3://bucket/out/")
            .with_query_timeout(12_345)
            .with_tables(vec!["t1".to_string(), "t2".to_string()]);
        assert_eq!(conn.config.region, "eu-west-1");
        assert_eq!(conn.config.workgroup, "wg");
        assert_eq!(conn.config.database, "analytics");
        assert_eq!(conn.config.output_location, "s3://bucket/out/");
        assert_eq!(conn.config.query_timeout_ms, 12_345);
        assert_eq!(conn.config.tables, vec!["t1".to_string(), "t2".to_string()]);
    }
}
