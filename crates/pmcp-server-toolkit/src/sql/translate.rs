//! Dialect-aware placeholder translation (CONN-03).
//!
//! Translates canonical `:name` named placeholders in a SQL string into the
//! positional form each backend dialect expects, while preserving the binding
//! order so the per-backend `execute()` impl can build a positional argument
//! list from the caller's `&[(String, serde_json::Value)]`.
//!
//! Wave 0 ships only the [`TranslatedSql`] struct and a stub
//! [`translate_placeholders`] that returns its input verbatim. Plan 02 replaces
//! the stub body with the `SqlWalker` state machine (split-helper form per
//! PATTERNS Pattern G, keeping every helper under PMAT cog 25) and turns the
//! five property tests in `mod proptests` from RED to GREEN.
//!
//! Public surface lives at `pmcp_server_toolkit::sql::translate_placeholders`
//! (D-05): a free helper, NOT a trait method — every connector calls it the
//! same way, so putting it on the trait would invite per-backend drift.

// Why: dialect display names are proper nouns that clippy::doc_markdown
// otherwise flags as needing back-ticks.
#![allow(clippy::doc_markdown)]

use super::Dialect;

/// Result of translating canonical `:name` placeholders into a dialect's
/// positional form, plus the binding order needed to bind values positionally.
///
/// Per-backend `execute()` impls destructure this and iterate `ordered_params`
/// to bind driver-native positional parameters from the caller's
/// `&[(String, serde_json::Value)]` named pairs.
///
/// # Example
///
/// ```
/// use pmcp_server_toolkit::sql::{translate_placeholders, Dialect, TranslatedSql};
///
/// // Wave 0 stub returns input verbatim; Plan 02 ships the real translation.
/// let translated: TranslatedSql = translate_placeholders("SELECT 1", Dialect::Postgres);
/// assert_eq!(translated.sql, "SELECT 1");
/// assert!(translated.ordered_params.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslatedSql {
    /// The SQL string with placeholders rewritten into the target dialect's
    /// positional form (`$1`/`$2` for Postgres, `?` for MySQL/Athena, `:name`
    /// kept for SQLite).
    pub sql: String,
    /// Placeholder names in positional binding order. The Nth entry names the
    /// value that the Nth positional parameter should bind.
    pub ordered_params: Vec<String>,
}

/// Translate canonical `:name` placeholders in `sql` into `dialect`'s
/// positional form, returning the rewritten SQL plus the binding order.
///
/// Wave 0 STUB: returns the input verbatim with an empty bind list. Plan 02
/// replaces this body with the `SqlWalker` state machine.
///
/// # Example
///
/// ```
/// use pmcp_server_toolkit::sql::{translate_placeholders, Dialect};
///
/// assert_eq!(translate_placeholders("SELECT 1", Dialect::Postgres).sql, "SELECT 1");
/// ```
#[must_use]
pub fn translate_placeholders(sql: &str, _dialect: Dialect) -> TranslatedSql {
    TranslatedSql {
        sql: sql.to_string(),
        ordered_params: vec![],
    }
}

#[cfg(test)]
mod proptests {
    #![allow(clippy::no_effect_underscore_binding)]
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// Invariant 1: idempotence for `:name`-free SQL (any dialect) —
        /// `translate.sql == input`. RED until Plan 02.
        #[test]
        fn idempotence_no_placeholders(s in any::<String>()) {
            let _ = &s;
            panic!("RED — Plan 02 implements");
        }
    }

    proptest! {
        /// Invariant 2: bind-order preservation — `ordered_params` lists
        /// placeholder names in positional order. RED until Plan 02.
        #[test]
        fn bind_order_preserved(s in any::<String>()) {
            let _ = &s;
            panic!("RED — Plan 02 implements");
        }
    }

    proptest! {
        /// Invariant 3: Postgres positional indexing — `$1, $2, ..., $n`
        /// (repeats get fresh `$k`). RED until Plan 02.
        #[test]
        fn postgres_positional_indexing(s in any::<String>()) {
            let _ = &s;
            panic!("RED — Plan 02 implements");
        }
    }

    proptest! {
        /// Invariant 4: SQLite identity — `Dialect::Sqlite` ⇒
        /// `translate.sql == input`. RED until Plan 02.
        #[test]
        fn sqlite_identity(s in any::<String>()) {
            let _ = &s;
            panic!("RED — Plan 02 implements");
        }
    }

    proptest! {
        /// Invariant 5: no panic on adversarial / arbitrary `&str` input.
        /// RED until Plan 02.
        #[test]
        fn no_panic_on_arbitrary_input(s in any::<String>()) {
            let _ = &s;
            panic!("RED — Plan 02 implements");
        }
    }
}
