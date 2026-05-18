// Net-new code for Phase 83 TKIT-10 (trait stub) + Phase 84 (full trait + per-backend impls).
// Per review R2 (BOTH reviewers HIGH severity), the Phase 83 trait surface is
// INTENTIONALLY MINIMIZED to just dialect() + schema_text() — execute() +
// placeholder translation are deferred to Phase 84 where they're validated
// against real backends (Postgres / MySQL / Athena / SQLite).

//! SQL connector trait stub (2-method MVP) + dialect enum.
//!
//! Phase 83 ships a minimized trait surface to AVOID OSSIFYING the public API
//! before any real connector validates the contract. Per review R2:
//! - `execute(sql, params) -> Vec<Value>` would commit to row shape, error
//!   shape, placeholder ownership, async behavior, and parameter model before
//!   real connector impls exist (Codex HIGH).
//! - `translate_placeholders(&str) -> String` loses binding-order information
//!   needed by positional dialects (Codex HIGH).
//! - Streaming and transactions are common enterprise SQL requirements that
//!   `Vec<Value>` precludes (Gemini HIGH).
//!
//! Phase 84 lands the full trait surface (`execute`, optional streaming,
//! placeholder translation returning `TranslatedSql { sql, ordered_params }`)
//! once the first real connector validates the shape.
//!
//! Phase 83 still ships TKIT-10 prompt assembly per CONTEXT.md D-12 — the
//! assembler calls [`SqlConnector::schema_text`] only, never `execute()`, so the
//! minimization does not block this phase's deliverable.

// Why: dialect display names ("PostgreSQL", "MySQL") are proper nouns that
// clippy::doc_markdown otherwise flags as needing back-ticks.
#![allow(clippy::doc_markdown)]

use async_trait::async_trait;
use thiserror::Error;

/// Minimized 2-method connector trait — Phase 83 MVP.
///
/// Phase 83 ships ONLY `dialect()` + `schema_text()`. The rest of the SQL
/// connector contract — query execution, streaming, transactions, and
/// dialect-aware placeholder translation — lands in Phase 84 once the first
/// real connector validates the shape.
///
/// # Semver-evolution plan (per review R2)
///
/// This trait WILL grow in `pmcp-server-toolkit 0.2.0` with:
/// - `execute(sql, params) -> impl futures::Stream<Item = Result<Row>>`
///   (streaming rather than `Vec<Value>` — Gemini HIGH severity in R2).
/// - `translate_placeholders(&str) -> TranslatedSql { sql, ordered_params }`
///   (preserves bind ordering — Codex HIGH severity in R2).
/// - Transaction support (begin / commit / rollback or a `transaction()`
///   continuation).
///
/// Downstream impl-authors targeting Phase 84 should plan against this growth.
/// Adding trait methods with defaults in a minor release is semver-compatible
/// for `Send + Sync + 'static` traits in Rust; the variants on [`Dialect`] and
/// [`ConnectorError`] are `#[non_exhaustive]` so they can also be extended
/// additively without semver break.
///
/// Phase 84's per-backend crates (`pmcp-toolkit-postgres`,
/// `pmcp-toolkit-mysql`, `pmcp-toolkit-athena`, plus the `sqlite` feature) are
/// the canonical impls.
#[async_trait]
pub trait SqlConnector: Send + Sync + 'static {
    /// Identify the dialect for prompt assembly + (future) placeholder translation.
    fn dialect(&self) -> Dialect;

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

/// Errors a [`SqlConnector`] impl may surface from [`SqlConnector::schema_text`].
///
/// Phase 84 may extend this enum (via the `#[non_exhaustive]` escape hatch)
/// once `execute()` lands and surfaces more failure modes (query errors,
/// transaction conflicts, etc.).
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
}

/// Crate-internal mock connector for testing TKIT-10 prompt assembly without
/// requiring a real driver. Phase 84's real impls subsume this for production.
#[cfg(any(test, feature = "sqlite"))]
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
