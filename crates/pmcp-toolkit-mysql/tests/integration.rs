//! Phase 84 CONN-06 integration anchor — MySQL connector against the in-process
//! mock. Per D-13 each test (a) constructs a connector against the mock, (b)
//! calls `execute` with `:name` placeholders, (c) calls `schema_text` and
//! asserts DDL fragments, (d) asserts the dialect ID.
//!
//! REVIEWS H5: the mock lives at `src/dev_mock.rs` and is reachable here via the
//! published `pmcp_toolkit_mysql::dev_mock` path under the `dev_mock` feature —
//! not via a path-include of a sibling test module. No container runtime, no
//! networking — pure in-process.

#![cfg(feature = "dev_mock")]

use pmcp_server_toolkit::sql::{Dialect, SqlConnector};
use pmcp_toolkit_mysql::dev_mock::MysqlMock;
use serde_json::{json, Value};

#[tokio::test]
async fn dialect_is_mysql() {
    let mock = MysqlMock::employee_directory();
    assert_eq!(mock.dialect(), Dialect::MySql);
}

#[tokio::test]
async fn execute_translates_named_to_question_mark() {
    let mock = MysqlMock::employee_directory();
    let rows = mock
        .execute(
            "SELECT * FROM employees WHERE id = :id",
            &[("id".into(), Value::from(1_i64))],
        )
        .await
        .expect("execute");
    let translated = mock.last_translated_sql.lock().unwrap().clone().unwrap();
    assert!(
        translated.contains("WHERE id = ?"),
        "named :id must translate to ?; got: {translated:?}"
    );
    assert!(
        !translated.contains("$1"),
        "MySQL must NOT use Postgres-style $1; got: {translated:?}"
    );
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["name"], Value::from("Ada Lovelace"));
}

#[tokio::test]
async fn schema_text_contains_expected_ddl_with_backticks() {
    let mock = MysqlMock::employee_directory();
    let text = mock.schema_text().await.expect("schema_text");
    assert!(text.contains("CREATE TABLE"), "schema_text: {text:?}");
    assert!(
        text.contains('`'),
        "MySQL DDL must use backtick identifiers; got: {text:?}"
    );
    assert!(
        text.contains("`employees`"),
        "backtick-quoted table name expected; got: {text:?}"
    );
    assert!(
        text.contains("ENGINE=InnoDB"),
        "MySQL InnoDB marker expected; got: {text:?}"
    );
}

#[tokio::test]
async fn repeated_named_params_translate_to_question_marks_in_order() {
    let mock = MysqlMock::employee_directory();
    let _ = mock
        .execute(
            "WHERE a = :a AND b = :b AND c = :a",
            &[("a".into(), json!(1)), ("b".into(), json!(2))],
        )
        .await
        .expect("execute");
    let translated = mock.last_translated_sql.lock().unwrap().clone().unwrap();
    // Three `?` placeholders, none of the Postgres positional indices.
    assert_eq!(
        translated.matches('?').count(),
        3,
        "three ? placeholders for :a, :b, :a; got: {translated:?}"
    );
    assert!(!translated.contains("$1"));
    let args = mock.last_positional_args.lock().unwrap().clone().unwrap();
    assert_eq!(args.len(), 3, "three positional slots for :a, :b, :a");
    assert_eq!(args[0], json!(1));
    assert_eq!(args[2], json!(1), "repeated :a re-binds the same value");
}
