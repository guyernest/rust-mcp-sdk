//! Integration anchor — Plan 07 fills.
//!
//! Wave 0 leaves this anchor without `#[test]` functions. Plan 07 adds the
//! D-13 contract tests (construct against mock, `execute()` with `:name`
//! placeholders, `schema_text()` DDL assertions, dialect identification).

#[path = "mock_athena.rs"]
mod mock_athena;
