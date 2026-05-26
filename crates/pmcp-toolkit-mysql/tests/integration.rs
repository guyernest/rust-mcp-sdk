//! Integration anchor — Plan 06 fills.
//!
//! Wave 0 leaves this anchor without `#[test]` functions. Plan 06 adds the
//! D-13 contract tests (construct against mock, `execute()` with `:name`
//! placeholders, `schema_text()` DDL assertions, dialect identification).

#[path = "mock_mysql.rs"]
mod mock_mysql;
