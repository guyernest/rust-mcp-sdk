//! Dialect-aware placeholder translation (CONN-03).
//!
//! Translates canonical `:name` named placeholders in a SQL string into the
//! positional form each backend dialect expects, while preserving the binding
//! order so the per-backend `execute()` impl can build a positional argument
//! list from the caller's `&[(String, serde_json::Value)]`.
//!
//! The translation walks the SQL char-by-char with a small state machine
//! ([`SqlWalker`]) that tracks string literals, line/block comments, and the
//! placeholder substate so a `:name` inside `'...'`, `"..."`, `-- ...`, or
//! `/* ... */` is NEVER rewritten. Each helper stays under PMAT cog 25 via the
//! split-helper form (PATTERNS Pattern G) — no cognitive-complexity allow
//! attribute is needed anywhere in this module.
//!
//! **Placeholder-recognition rule (REVIEWS H7):** in `Normal` state a `:` only
//! begins a placeholder if the NEXT char is `[A-Za-z_]`. A `::` is a Postgres
//! cast (consumed verbatim, the following type identifier is swallowed by the
//! transitional `CastTypeName` state), a `:=` is a MySQL session-var assignment,
//! and `:1bad` is malformed — all three emit the bare `:` verbatim and stay in
//! `Normal`. This eliminates the `::text` mis-translation regression class.
//!
//! Public surface lives at `pmcp_server_toolkit::sql::translate_placeholders`
//! (D-05): a free helper, NOT a trait method — every connector calls it the
//! same way, so putting it on the trait would invite per-backend drift.

// Why: dialect display names are proper nouns that clippy::doc_markdown
// otherwise flags as needing back-ticks.
#![allow(clippy::doc_markdown)]

use super::Dialect;
use std::fmt::Write as _;
use std::iter::Peekable;
use std::str::Chars;

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
/// let translated: TranslatedSql =
///     translate_placeholders("SELECT :id FROM t", Dialect::Postgres);
/// assert_eq!(translated.sql, "SELECT $1 FROM t");
/// assert_eq!(translated.ordered_params, vec!["id".to_string()]);
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
/// Placeholders inside string literals (`'...'`, `"..."`), line comments
/// (`-- ...`), and block comments (`/* ... */`, nested) are left verbatim. A
/// `::text` Postgres cast, a `:=` MySQL session-var, and a malformed `:1bad`
/// are all emitted verbatim per REVIEWS H7.
///
/// # Example
///
/// ```
/// use pmcp_server_toolkit::sql::{translate_placeholders, Dialect};
///
/// // Repeated names get a fresh positional index per appearance.
/// let t = translate_placeholders("WHERE a = :a AND b = :b AND c = :a", Dialect::Postgres);
/// assert_eq!(t.sql, "WHERE a = $1 AND b = $2 AND c = $3");
/// assert_eq!(t.ordered_params, vec!["a", "b", "a"]);
///
/// // SQLite keeps the SQL byte-identical but still records bind order.
/// let s = translate_placeholders("SELECT :id FROM t", Dialect::Sqlite);
/// assert_eq!(s.sql, "SELECT :id FROM t");
/// assert_eq!(s.ordered_params, vec!["id"]);
/// ```
#[must_use]
pub fn translate_placeholders(sql: &str, dialect: Dialect) -> TranslatedSql {
    let mut walker = SqlWalker::new(sql, dialect);
    walker.run();
    walker.into_translated()
}

/// State of the [`SqlWalker`] char-by-char scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    /// Default scanning state — placeholders are recognized here.
    Normal,
    /// Inside a `'...'` or `"..."` literal (the quote char is carried).
    StringLiteral(char),
    /// Inside a `-- ...` line comment (ends at `\n`).
    LineComment,
    /// Inside a `/* ... */` block comment; the `usize` tracks nesting depth.
    BlockComment(usize),
    /// Reading a placeholder identifier into `pending_name`.
    Placeholder,
    /// Transitional state after `::` — swallows the following type identifier
    /// (e.g. `text` in `::text`) so no placeholder lookup happens mid-cast.
    CastTypeName,
}

/// Char-by-char SQL scanner that rewrites `:name` placeholders into a dialect's
/// positional form while skipping literals and comments.
struct SqlWalker<'a> {
    chars: Peekable<Chars<'a>>,
    state: State,
    out: String,
    order: Vec<String>,
    pg_index: usize,
    dialect: Dialect,
    pending_name: String,
}

impl<'a> SqlWalker<'a> {
    /// Initialize the walker over `sql` for `dialect`.
    fn new(sql: &'a str, dialect: Dialect) -> Self {
        Self {
            chars: sql.chars().peekable(),
            state: State::Normal,
            out: String::with_capacity(sql.len()),
            order: Vec::new(),
            pg_index: 0,
            dialect,
            pending_name: String::new(),
        }
    }

    /// Drive the scan to completion, dispatching each char by current state.
    fn run(&mut self) {
        while let Some(c) = self.chars.next() {
            match self.state {
                State::Normal => self.handle_normal(c),
                State::StringLiteral(q) => self.handle_string(c, q),
                State::LineComment => self.handle_line_comment(c),
                State::BlockComment(depth) => self.handle_block_comment(c, depth),
                State::Placeholder => self.handle_placeholder(c),
                State::CastTypeName => self.handle_cast_type_name(c),
            }
        }
        // EOF inside a placeholder: emit whatever was accumulated.
        if self.state == State::Placeholder {
            self.emit_placeholder_from_pending();
        }
    }

    /// Handle a char in `Normal` state. Delegates the `:`-precedence logic to
    /// [`Self::dispatch_colon`] (REVIEWS H7).
    fn handle_normal(&mut self, c: char) {
        match c {
            '\'' | '"' => {
                self.out.push(c);
                self.state = State::StringLiteral(c);
            },
            '-' if self.chars.peek() == Some(&'-') => {
                self.out.push(c);
                self.out.push('-');
                self.chars.next();
                self.state = State::LineComment;
            },
            '/' if self.chars.peek() == Some(&'*') => {
                self.out.push(c);
                self.out.push('*');
                self.chars.next();
                self.state = State::BlockComment(1);
            },
            ':' => self.dispatch_colon(),
            _ => self.out.push(c),
        }
    }

    /// REVIEWS H7 colon-precedence: decide whether a `:` begins a placeholder,
    /// a `::` cast, or is a verbatim character. Assumes the `:` was just
    /// consumed by `run()` and `self.state == Normal`.
    fn dispatch_colon(&mut self) {
        match self.chars.peek().copied() {
            Some(':') => {
                // `::` cast prefix — emit both colons verbatim and swallow the
                // following type identifier so no placeholder lookup occurs.
                self.out.push(':');
                self.out.push(':');
                self.chars.next();
                self.state = State::CastTypeName;
            },
            Some(n) if is_ident_start(n) => {
                // Valid placeholder start — consume the `:` (do not emit) and
                // begin reading the identifier.
                self.pending_name.clear();
                self.state = State::Placeholder;
            },
            _ => self.out.push(':'),
        }
    }

    /// Read a placeholder identifier. On a non-identifier char, flush the
    /// placeholder and re-dispatch that char from `Normal`.
    fn handle_placeholder(&mut self, c: char) {
        if is_ident_continue(c) {
            self.pending_name.push(c);
        } else {
            self.emit_placeholder_from_pending();
            self.handle_normal(c);
        }
    }

    /// Swallow the type identifier following a `::` cast, then return to Normal.
    fn handle_cast_type_name(&mut self, c: char) {
        self.out.push(c);
        if !is_ident_continue(c) {
            self.state = State::Normal;
        }
    }

    /// Emit the dialect's positional form for `pending_name`, record the bind
    /// order, and return to `Normal`. Entering `Placeholder` already required a
    /// valid identifier-start, so `pending_name` is never empty here.
    fn emit_placeholder_from_pending(&mut self) {
        match self.dialect {
            Dialect::Postgres => {
                self.pg_index += 1;
                // Writing a formatted integer into a String never fails.
                let _ = write!(self.out, "${}", self.pg_index);
            },
            Dialect::MySql | Dialect::Athena => self.out.push('?'),
            Dialect::Sqlite => {
                let _ = write!(self.out, ":{}", self.pending_name);
            },
        }
        self.order.push(std::mem::take(&mut self.pending_name));
        self.state = State::Normal;
    }

    /// Handle a char inside a `'...'` / `"..."` literal. A doubled quote
    /// (`''` / `""`) is an escape and stays inside the literal.
    fn handle_string(&mut self, c: char, q: char) {
        self.out.push(c);
        if c == q {
            if self.chars.peek() == Some(&q) {
                self.out.push(q);
                self.chars.next();
            } else {
                self.state = State::Normal;
            }
        }
    }

    /// Handle a char inside a `-- ...` line comment; ends at newline.
    fn handle_line_comment(&mut self, c: char) {
        self.out.push(c);
        if c == '\n' {
            self.state = State::Normal;
        }
    }

    /// Handle a char inside a `/* ... */` block comment, tracking nesting.
    fn handle_block_comment(&mut self, c: char, depth: usize) {
        self.out.push(c);
        if c == '*' && self.chars.peek() == Some(&'/') {
            self.out.push('/');
            self.chars.next();
            self.state = if depth <= 1 {
                State::Normal
            } else {
                State::BlockComment(depth - 1)
            };
        } else if c == '/' && self.chars.peek() == Some(&'*') {
            self.out.push('*');
            self.chars.next();
            self.state = State::BlockComment(depth + 1);
        }
    }

    /// Consume the walker into its [`TranslatedSql`] result.
    fn into_translated(self) -> TranslatedSql {
        TranslatedSql {
            sql: self.out,
            ordered_params: self.order,
        }
    }
}

/// `true` if `c` can start a `:name` placeholder identifier (`[A-Za-z_]`).
fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

/// `true` if `c` can continue a placeholder identifier (`[A-Za-z0-9_]`).
fn is_ident_continue(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// Invariant 1: idempotence for `:name`-free SQL — `translate.sql == input`
        /// for every dialect, with no recorded binds.
        #[test]
        fn idempotence_no_placeholders(s in "[A-Za-z0-9 _\\.,;\\(\\)=]*") {
            for d in [Dialect::Postgres, Dialect::MySql, Dialect::Athena, Dialect::Sqlite] {
                let t = translate_placeholders(&s, d);
                prop_assert_eq!(&t.sql, &s);
                prop_assert!(t.ordered_params.is_empty());
            }
        }

        /// Invariant 2: bind-order preservation — `ordered_params` lists
        /// placeholder names left-to-right in their textual order.
        #[test]
        fn bind_order_preserved(names in proptest::collection::vec("[a-z]{1,5}", 1..=5)) {
            let sql = names.iter().map(|n| format!(":{n}")).collect::<Vec<_>>().join(", ");
            let t = translate_placeholders(&sql, Dialect::Postgres);
            prop_assert_eq!(t.ordered_params, names);
        }

        /// Invariant 3: Postgres positional indexing — `$1..=$n` are present and
        /// contiguous, and their count equals `ordered_params.len()`.
        #[test]
        fn postgres_positional_indexing(names in proptest::collection::vec("[a-z]{1,5}", 1..=5)) {
            let sql = names.iter().map(|n| format!(":{n}")).collect::<Vec<_>>().join(", ");
            let t = translate_placeholders(&sql, Dialect::Postgres);
            prop_assert_eq!(t.ordered_params.len(), names.len());
            for i in 1..=names.len() {
                let token = format!("${i}");
                prop_assert!(t.sql.contains(&token));
            }
            // No gap above n.
            let above = format!("${}", names.len() + 1);
            prop_assert!(!t.sql.contains(&above));
        }

        /// Invariant 4: SQLite identity — `Dialect::Sqlite` keeps SQL
        /// byte-identical; only `ordered_params` differs.
        #[test]
        fn sqlite_identity(s in any::<String>()) {
            let t = translate_placeholders(&s, Dialect::Sqlite);
            prop_assert_eq!(t.sql, s);
        }

        /// Invariant 5: no panic on arbitrary `&str` input across all dialects.
        #[test]
        fn no_panic_on_arbitrary_input(s in any::<String>()) {
            for d in [Dialect::Postgres, Dialect::MySql, Dialect::Athena, Dialect::Sqlite] {
                let _ = translate_placeholders(&s, d);
            }
        }
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    fn t(sql: &str, d: Dialect) -> TranslatedSql {
        translate_placeholders(sql, d)
    }

    #[test]
    fn empty_input_is_identity() {
        let r = t("", Dialect::Postgres);
        assert_eq!(r.sql, "");
        assert!(r.ordered_params.is_empty());
    }

    #[test]
    fn no_placeholder_is_identity_mysql() {
        let r = t("SELECT 1", Dialect::MySql);
        assert_eq!(r.sql, "SELECT 1");
        assert!(r.ordered_params.is_empty());
    }

    #[test]
    fn single_placeholder_postgres() {
        let r = t("SELECT :id FROM t", Dialect::Postgres);
        assert_eq!(r.sql, "SELECT $1 FROM t");
        assert_eq!(r.ordered_params, vec!["id"]);
    }

    #[test]
    fn single_placeholder_mysql() {
        let r = t("SELECT :id FROM t", Dialect::MySql);
        assert_eq!(r.sql, "SELECT ? FROM t");
        assert_eq!(r.ordered_params, vec!["id"]);
    }

    #[test]
    fn single_placeholder_athena() {
        let r = t("SELECT :id FROM t", Dialect::Athena);
        assert_eq!(r.sql, "SELECT ? FROM t");
        assert_eq!(r.ordered_params, vec!["id"]);
    }

    #[test]
    fn single_placeholder_sqlite_is_identity_with_bind_order() {
        let r = t("SELECT :id FROM t", Dialect::Sqlite);
        assert_eq!(r.sql, "SELECT :id FROM t");
        assert_eq!(r.ordered_params, vec!["id"]);
    }

    #[test]
    fn repeated_name_gets_fresh_index_postgres() {
        let r = t("WHERE a = :a AND b = :b AND c = :a", Dialect::Postgres);
        assert_eq!(r.sql, "WHERE a = $1 AND b = $2 AND c = $3");
        assert_eq!(r.ordered_params, vec!["a", "b", "a"]);
    }

    #[test]
    fn three_distinct_names_all_dialects_match_must_haves() {
        let sql = "SELECT :id FROM t WHERE x = :x AND y = :id";
        let pg = t(sql, Dialect::Postgres);
        assert_eq!(pg.sql, "SELECT $1 FROM t WHERE x = $2 AND y = $3");
        assert_eq!(pg.ordered_params, vec!["id", "x", "id"]);

        let my = t(sql, Dialect::MySql);
        assert_eq!(my.sql, "SELECT ? FROM t WHERE x = ? AND y = ?");
        assert_eq!(my.ordered_params, vec!["id", "x", "id"]);

        let at = t(sql, Dialect::Athena);
        assert_eq!(at.sql, "SELECT ? FROM t WHERE x = ? AND y = ?");
        assert_eq!(at.ordered_params, vec!["id", "x", "id"]);

        let lite = t(sql, Dialect::Sqlite);
        assert_eq!(lite.sql, sql);
        assert_eq!(lite.ordered_params, vec!["id", "x", "id"]);
    }

    #[test]
    fn placeholder_inside_string_literal_not_translated() {
        let r = t("SELECT 'WHERE name = :foo' AS x", Dialect::Postgres);
        assert_eq!(r.sql, "SELECT 'WHERE name = :foo' AS x");
        assert!(r.ordered_params.is_empty());
    }

    #[test]
    fn doubled_single_quote_escape_stays_in_literal() {
        let r = t("SELECT 'it''s :foo' AS x", Dialect::Postgres);
        assert_eq!(r.sql, "SELECT 'it''s :foo' AS x");
        assert!(r.ordered_params.is_empty());
    }

    #[test]
    fn double_quoted_identifier_skips_placeholder() {
        let r = t("SELECT \"col:name\" FROM t", Dialect::Postgres);
        assert_eq!(r.sql, "SELECT \"col:name\" FROM t");
        assert!(r.ordered_params.is_empty());
    }

    #[test]
    fn placeholder_in_line_comment_not_translated() {
        let r = t("SELECT 1 -- bind :id here", Dialect::Postgres);
        assert_eq!(r.sql, "SELECT 1 -- bind :id here");
        assert!(r.ordered_params.is_empty());
    }

    #[test]
    fn line_comment_ends_at_newline() {
        let r = t("SELECT 1 -- :a\nWHERE x = :b", Dialect::Postgres);
        assert_eq!(r.sql, "SELECT 1 -- :a\nWHERE x = $1");
        assert_eq!(r.ordered_params, vec!["b"]);
    }

    #[test]
    fn placeholder_in_block_comment_not_translated() {
        let r = t("SELECT /* :foo */ 1", Dialect::Postgres);
        assert_eq!(r.sql, "SELECT /* :foo */ 1");
        assert!(r.ordered_params.is_empty());
    }

    #[test]
    fn nested_block_comment_tracked_via_depth() {
        let r = t("SELECT /* /* :foo */ :bar */ :baz", Dialect::Postgres);
        assert_eq!(r.sql, "SELECT /* /* :foo */ :bar */ $1");
        assert_eq!(r.ordered_params, vec!["baz"]);
    }

    // ---- REVIEWS H7 mandatory named tests ----

    #[test]
    fn postgres_double_colon_cast_preserves_text_identifier() {
        let r = t("SELECT :id::text FROM t", Dialect::Postgres);
        assert_eq!(
            r,
            TranslatedSql {
                sql: "SELECT $1::text FROM t".into(),
                ordered_params: vec!["id".into()],
            }
        );
    }

    #[test]
    fn postgres_double_colon_int_cast_no_placeholder() {
        let r = t("SELECT 1::int", Dialect::Postgres);
        assert_eq!(
            r,
            TranslatedSql {
                sql: "SELECT 1::int".into(),
                ordered_params: vec![],
            }
        );
    }

    #[test]
    fn mysql_session_variable_assignment_not_a_placeholder() {
        let r = t("SET @x := 5", Dialect::MySql);
        assert_eq!(
            r,
            TranslatedSql {
                sql: "SET @x := 5".into(),
                ordered_params: vec![],
            }
        );
    }

    #[test]
    fn colon_followed_by_digit_emits_verbatim() {
        let r = t("SELECT :1bad FROM t", Dialect::Postgres);
        assert_eq!(
            r,
            TranslatedSql {
                sql: "SELECT :1bad FROM t".into(),
                ordered_params: vec![],
            }
        );
    }

    #[test]
    fn string_literal_cast_both_colons_verbatim() {
        let r = t("SELECT 'foo'::text", Dialect::Postgres);
        assert_eq!(r.sql, "SELECT 'foo'::text");
        assert!(r.ordered_params.is_empty());
    }

    #[test]
    fn placeholder_then_cast_then_placeholder() {
        let r = t("SELECT :a::text, :b FROM t", Dialect::Postgres);
        assert_eq!(r.sql, "SELECT $1::text, $2 FROM t");
        assert_eq!(r.ordered_params, vec!["a", "b"]);
    }

    #[test]
    fn lone_colon_at_eof_emits_verbatim() {
        let r = t("SELECT 1:", Dialect::Postgres);
        assert_eq!(r.sql, "SELECT 1:");
        assert!(r.ordered_params.is_empty());
    }

    #[test]
    fn underscore_leading_placeholder_name() {
        let r = t("WHERE x = :_id", Dialect::Postgres);
        assert_eq!(r.sql, "WHERE x = $1");
        assert_eq!(r.ordered_params, vec!["_id"]);
    }

    #[test]
    fn unterminated_literal_does_not_panic() {
        let r = t("SELECT 'unterminated :foo", Dialect::Postgres);
        // Remainder is treated as literal content; :foo is NOT translated.
        assert_eq!(r.sql, "SELECT 'unterminated :foo");
        assert!(r.ordered_params.is_empty());
    }

    #[test]
    fn unterminated_block_comment_does_not_panic() {
        let r = t("SELECT /* :foo", Dialect::Postgres);
        assert_eq!(r.sql, "SELECT /* :foo");
        assert!(r.ordered_params.is_empty());
    }

    #[test]
    fn placeholder_at_eof_is_emitted() {
        let r = t("WHERE id = :id", Dialect::Postgres);
        assert_eq!(r.sql, "WHERE id = $1");
        assert_eq!(r.ordered_params, vec!["id"]);
    }
}
