// Net-new code for Phase 83 TKIT-10 (trait stub) + Phase 84 (full trait + per-backend impls).
// Phase 83 INTENTIONALLY MINIMIZED the trait surface to dialect() + schema_text()
// (per review R2, BOTH reviewers HIGH severity) so execute() + placeholder
// translation could be validated against real backends first. Phase 84 (CONN-01)
// now ships the full 3-method trait surface validated against Postgres / MySQL /
// Athena / SQLite.

//! SQL connector trait (3-method surface) + dialect enum.
//!
//! Phase 83 shipped a minimized 2-method MVP (`dialect()` + `schema_text()`) to
//! AVOID OSSIFYING the public API before any real connector validated the
//! contract. Phase 84 (CONN-01) ships the full 3-method trait surface, adding
//! [`SqlConnector::execute`] now that the per-backend connectors validate the
//! shape:
//! - `execute(sql, &[(String, Value)]) -> Result<Vec<Value>, ConnectorError>`
//!   returns one JSON object per row — the exact shape MCP transport needs at
//!   the `tools/call` → `structuredContent` boundary (D-01).
//! - Parameters are a slice of named `(name, value)` pairs so the caller
//!   controls bind order and they round-trip cleanly through `serde_json` (D-03).
//!
//! Streaming and transactions remain deferred to a future semver-additive
//! release (D-02) — see [`SqlConnector`] for the evolution plan.
//!
//! Phase 83's TKIT-10 prompt assembly calls [`SqlConnector::schema_text`] only,
//! never `execute()`, so the additional method does not change that surface.

// Why: dialect display names ("PostgreSQL", "MySQL") are proper nouns that
// clippy::doc_markdown otherwise flags as needing back-ticks.
#![allow(clippy::doc_markdown)]

use async_trait::async_trait;
use thiserror::Error;

/// Dialect-aware placeholder translation (CONN-03).
///
/// Public surface lives at `pmcp_server_toolkit::sql::translate_placeholders`
/// per D-05 — a free helper, not a trait method.
pub mod translate;
pub use translate::{translate_placeholders, TranslatedSql};

/// First-class SQLite connector (CONN-08), gated behind the `sqlite` feature.
///
/// Ships `SqliteConnector` — a real `rusqlite`-backed [`SqlConnector`] impl —
/// alongside the test-only `pub(crate) MockSqlConnector` fixture (Open Question
/// #3): the two coexist, the mock is NOT removed.
#[cfg(feature = "sqlite")]
pub mod sqlite;
#[cfg(feature = "sqlite")]
pub use sqlite::SqliteConnector;

/// Three-method SQL connector trait — Phase 84 ships the full trait surface.
///
/// Phase 83 shipped a 2-method MVP (`dialect()` + `schema_text()`); Phase 84
/// (CONN-01) lands `execute()` between them now that the per-backend connectors
/// validate the row/error/parameter shape. The trait is the stable contract the
/// per-backend crates (`pmcp-toolkit-postgres`, `pmcp-toolkit-mysql`,
/// `pmcp-toolkit-athena`, plus the `sqlite` feature `SqliteConnector`) implement.
///
/// # Semver-evolution plan
///
/// This trait WILL grow additively in a future minor release with:
/// - `execute_stream(sql, params) -> impl Stream<Item = Result<Value>>`, shipped
///   with a default body backed by `execute(...).map(stream::iter)` so it is
///   semver-compatible on a `Send + Sync + 'static` trait — for the
///   large-result-scan case (e.g. an Athena warehousing tool). Deferred per D-02
///   because no v2.2 reference scenario needs it.
/// - Transaction support as a separate `SqlTransactional` trait extension, when
///   a real consumer needs it. Deferred per D-02 — the v2.2 reference scenarios
///   are read-only and Athena has no real transaction model.
///
/// The variants on [`Dialect`] and [`ConnectorError`] are `#[non_exhaustive]`
/// so they can be extended additively without a semver break.
///
/// # Example
///
/// A minimal connector implementing all three methods. The example defines a
/// LOCAL dummy struct — it deliberately does NOT reference any downstream
/// per-backend crate, because those depend on `pmcp-server-toolkit` and would
/// create a circular doctest dependency (REVIEWS H6).
///
/// ```no_run
/// use pmcp_server_toolkit::sql::{SqlConnector, Dialect, ConnectorError};
/// use async_trait::async_trait;
/// use serde_json::Value;
///
/// struct Dummy;
///
/// #[async_trait]
/// impl SqlConnector for Dummy {
///     fn dialect(&self) -> Dialect { Dialect::Sqlite }
///     async fn execute(&self, _sql: &str, _params: &[(String, Value)])
///         -> Result<Vec<Value>, ConnectorError> {
///         Ok(vec![])
///     }
///     async fn schema_text(&self) -> Result<String, ConnectorError> {
///         Ok(String::new())
///     }
/// }
/// ```
#[async_trait]
pub trait SqlConnector: Send + Sync + 'static {
    /// Identify the dialect for prompt assembly + placeholder translation.
    fn dialect(&self) -> Dialect;

    /// Execute a query and return one [`serde_json::Value`] per result row.
    ///
    /// `sql` is the canonical statement (placeholders in the toolkit's `:name`
    /// form); `params` is a slice of named `(name, value)` pairs the caller
    /// controls the order of (D-03). Per-backend impls translate placeholders
    /// to their dialect via [`translate_placeholders`] and bind from `params`,
    /// then convert driver-native rows into JSON objects (D-01).
    ///
    /// Each returned `Value` is typically a JSON object keyed by column name —
    /// the exact shape MCP transport needs to populate the `tools/call`
    /// response's `structuredContent` field (D-06).
    ///
    /// # Errors
    ///
    /// Returns a [`ConnectorError`] when the backend cannot connect
    /// ([`ConnectorError::Connection`]), the driver fails
    /// ([`ConnectorError::Driver`]), the query is rejected
    /// ([`ConnectorError::Query`]), or a parameter cannot be bound
    /// ([`ConnectorError::ParameterBind`]).
    async fn execute(
        &self,
        sql: &str,
        params: &[(String, serde_json::Value)],
    ) -> Result<Vec<serde_json::Value>, ConnectorError>;

    /// Render the backend's schema as DDL or equivalent text for inclusion in
    /// the code-mode prompt. Phase 84 impls drive this from `information_schema`,
    /// the Glue catalog, or `sqlite_master` per dialect.
    ///
    /// Implementations should keep output BOUNDED — token-budget the schema
    /// before returning. The toolkit does not truncate (T-83-07-03).
    ///
    /// # Errors
    ///
    /// Returns a [`ConnectorError`] when the backend cannot enumerate its
    /// schema (I/O failure, permission denied, missing catalog, etc.).
    async fn schema_text(&self) -> Result<String, ConnectorError>;
}

/// Supported SQL dialects (4-variant per spike 005).
///
/// `#[non_exhaustive]` permits additive evolution to `Oracle` / `SqlServer` /
/// `DuckDb` / `ClickHouse` in later phases without semver break.
///
/// # Example
///
/// ```
/// use pmcp_server_toolkit::sql::Dialect;
///
/// assert_eq!(Dialect::Postgres.name(), "PostgreSQL");
/// assert!(Dialect::Sqlite.placeholder_guidance().contains(":name"));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Dialect {
    /// PostgreSQL — positional `$1`, `$2`, ... placeholders.
    Postgres,
    /// MySQL — positional `?` placeholders.
    MySql,
    /// Amazon Athena (Presto/Trino) — positional `?` placeholders.
    Athena,
    /// SQLite — named `:name` or positional `?` placeholders.
    Sqlite,
}

impl Dialect {
    /// Stable, human-readable name for prompts and logs.
    ///
    /// # Example
    ///
    /// ```
    /// use pmcp_server_toolkit::sql::Dialect;
    /// assert_eq!(Dialect::MySql.name(), "MySQL");
    /// ```
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Postgres => "PostgreSQL",
            Self::MySql => "MySQL",
            Self::Athena => "Amazon Athena (Presto/Trino)",
            Self::Sqlite => "SQLite",
        }
    }

    /// One-line guidance string for the code-mode prompt body explaining the
    /// dialect's placeholder convention. Used by `assemble_code_mode_prompt`
    /// even though Phase 83 does not ship `translate_placeholders` — the LLM
    /// still benefits from knowing the eventual binding shape.
    ///
    /// # Example
    ///
    /// ```
    /// use pmcp_server_toolkit::sql::Dialect;
    /// assert!(Dialect::Postgres.placeholder_guidance().contains("$1"));
    /// ```
    #[must_use]
    pub const fn placeholder_guidance(self) -> &'static str {
        match self {
            Self::Postgres => "Use $1, $2, $3, ... for positional parameters.",
            Self::MySql => "Use ? for positional parameters in argument order.",
            Self::Athena => "Use ? for positional parameters in argument order.",
            Self::Sqlite => "Use :name for named parameters or ? for positional.",
        }
    }
}

/// Errors a [`SqlConnector`] impl may surface from [`SqlConnector::schema_text`]
/// or [`SqlConnector::execute`].
///
/// The enum is `#[non_exhaustive]`, so Phase 84 (CONN-01) adds the execute-time
/// variants (`Driver`, `Query`, `ParameterBind`, `Connection`) additively
/// without a semver break, and later phases can add more failure modes the same
/// way.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ConnectorError {
    /// Underlying transport / driver I/O error.
    #[error("connector I/O error: {0}")]
    Io(String),

    /// Failed to enumerate or render the schema text.
    #[error("schema fetch failed: {0}")]
    Schema(String),

    /// A connector was asked to handle work for the wrong dialect (e.g. a
    /// query labelled Postgres routed to a MySQL connector).
    #[error("dialect mismatch: query used {used:?} but connector is {actual:?}")]
    DialectMismatch {
        /// Dialect declared by the caller / query.
        used: Dialect,
        /// Dialect actually served by this connector.
        actual: Dialect,
    },

    /// The underlying driver reported a failure (e.g. a `tokio-postgres`,
    /// `sqlx`, `aws-sdk-athena`, or `rusqlite` error) that is not a query or
    /// connection problem on its own.
    #[error("driver error: {0}")]
    Driver(String),

    /// The backend rejected the query (syntax error, unknown table/column,
    /// permission denied on the statement, etc.).
    #[error("query error: {0}")]
    Query(String),

    /// A named parameter from the caller's `&[(String, Value)]` slice could not
    /// be bound to the translated statement (type mismatch, missing binding,
    /// unsupported value shape, etc.).
    #[error("parameter bind failed for '{name}': {reason}")]
    ParameterBind {
        /// Name of the parameter that failed to bind.
        name: String,
        /// Human-readable reason the bind failed.
        reason: String,
    },

    /// The connector could not establish or maintain a connection to the
    /// backend.
    ///
    /// # Security
    ///
    /// Implementors MUST redact credentials (passwords, AWS keys) before
    /// constructing this variant — the inner `String` reaches MCP clients via
    /// `Display`. NEVER pass a raw `DATABASE_URL` or `AWS_*` value here; strip
    /// or mask the secret first (T-84-01-01).
    #[error("connection error: {0}")]
    Connection(String),
}

/// Crate-internal mock connector for testing TKIT-10 prompt assembly without
/// requiring a real driver. Phase 84's real impls subsume this for production.
///
/// Gated `cfg(any(test, feature = "sqlite"))` so Plan 08's smoke test can
/// reach it under `--features sqlite`. Carries `#[allow(dead_code)]` because
/// under `--features sqlite` (without `cfg(test)`) there are no in-crate
/// callers — only Plan 08's smoke test references it from outside.
#[cfg(any(test, feature = "sqlite"))]
#[allow(dead_code)]
pub(crate) struct MockSqlConnector {
    /// Dialect the mock claims to serve.
    pub dialect: Dialect,
    /// Canned schema text returned by `schema_text()`.
    pub schema: String,
}

#[cfg(any(test, feature = "sqlite"))]
#[async_trait]
impl SqlConnector for MockSqlConnector {
    fn dialect(&self) -> Dialect {
        self.dialect
    }

    /// Fixture-only: `MockSqlConnector` exists for TKIT-10 prompt-assembly
    /// tests that exercise only `dialect()` + `schema_text()`. It is not a real
    /// driver — use `SqliteConnector` (Plan 84-04) for real query execution.
    async fn execute(
        &self,
        _sql: &str,
        _params: &[(String, serde_json::Value)],
    ) -> Result<Vec<serde_json::Value>, ConnectorError> {
        Err(ConnectorError::Driver(
            "MockSqlConnector::execute is fixture-only; use SqliteConnector for real execution"
                .into(),
        ))
    }

    async fn schema_text(&self) -> Result<String, ConnectorError> {
        Ok(self.schema.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn dialect_name_stable_for_all_variants() {
        for d in [
            Dialect::Postgres,
            Dialect::MySql,
            Dialect::Athena,
            Dialect::Sqlite,
        ] {
            assert!(
                !d.name().is_empty(),
                "Dialect::name must be non-empty for {d:?}"
            );
        }
    }

    #[test]
    fn dialect_placeholder_guidance_stable_for_all_variants() {
        for d in [
            Dialect::Postgres,
            Dialect::MySql,
            Dialect::Athena,
            Dialect::Sqlite,
        ] {
            assert!(
                !d.placeholder_guidance().is_empty(),
                "guidance must be non-empty for {d:?}"
            );
        }
    }

    proptest! {
        /// TEST-02: dialect guidance is total (every input dialect returns non-empty).
        /// Slim version of the Phase 83 dialect-aware property test; the full
        /// `translate_placeholders` property test lives in Phase 84 per review R2.
        #[test]
        fn every_dialect_has_guidance(idx in 0usize..4) {
            let d = match idx {
                0 => Dialect::Postgres,
                1 => Dialect::MySql,
                2 => Dialect::Athena,
                _ => Dialect::Sqlite,
            };
            prop_assert!(!d.placeholder_guidance().is_empty());
            prop_assert!(!d.name().is_empty());
        }
    }
}

/// Compile-only assertions that the now-3-method `SqlConnector` trait object
/// is still object-safe and `Send + Sync + 'static` — the bound per-backend
/// crates and the toolkit's `Arc<dyn SqlConnector>` plumbing rely on.
#[cfg(test)]
mod execute_signature_tests {
    use super::SqlConnector;

    fn assert_send_sync<T: Send + Sync + 'static>() {}

    #[test]
    fn connector_trait_object_is_send_sync_static() {
        assert_send_sync::<Box<dyn SqlConnector>>();
    }
}

/// Unit tests for the execute-time `ConnectorError` variants (CONN-01 / Task 2).
///
/// Verifies the `thiserror` `Display` format and confirms the `Connection`
/// variant is not designed as a credential-leak channel (T-84-01-01). Real
/// redaction lives in the per-backend connectors (Plans 05/06/07); this guard
/// proves the variant itself does not synthesize credential strings.
#[cfg(test)]
mod connector_error_tests {
    use super::ConnectorError;

    #[test]
    fn test_display_format_driver() {
        assert_eq!(
            format!("{}", ConnectorError::Driver("oops".into())),
            "driver error: oops"
        );
    }

    #[test]
    fn test_display_format_parameter_bind() {
        assert_eq!(
            format!(
                "{}",
                ConnectorError::ParameterBind {
                    name: "id".into(),
                    reason: "expected int, got string".into(),
                }
            ),
            "parameter bind failed for 'id': expected int, got string"
        );
    }

    #[test]
    fn test_connection_display_does_not_echo_password() {
        let err = ConnectorError::Connection("connection refused".into());
        let rendered = format!("{err}");
        for forbidden in ["password", "AWS_SECRET_ACCESS_KEY", "DATABASE_URL"] {
            assert!(
                !rendered.contains(forbidden),
                "Connection Display must not synthesize the credential token {forbidden:?}; got {rendered:?}"
            );
        }
    }
}
