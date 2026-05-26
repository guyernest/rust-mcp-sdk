//! Integration anchor — Plan 05 fills.
//!
//! Wave 0 leaves this anchor without `#[test]` functions. Plan 05 adds the
//! D-13 contract tests (construct against mock, `execute()` with `:name`
//! placeholders, `schema_text()` DDL assertions, dialect identification).

#[path = "mock_postgres.rs"]
mod mock_postgres;
