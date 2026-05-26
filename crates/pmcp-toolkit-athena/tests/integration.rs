//! Phase 84 CONN-07 integration anchor — Athena connector against in-process mock.
//!
//! REVIEWS H5: the mock lives at `src/dev_mock.rs` and is reached via the
//! `dev_mock` feature — NOT a `#[path = "../tests/..."]` include. No Docker, no
//! networking, no live AWS (D-07 + `feedback_avoid_docker_pure_rust_lambda`).
//! REVIEWS M5: includes a multi-page pagination test.

#![cfg(feature = "dev_mock")]

use pmcp_server_toolkit::sql::{Dialect, SqlConnector};
use pmcp_toolkit_athena::dev_mock::{AthenaMock, PAGINATED_QUERY_MARKER};
use serde_json::{json, Value};

#[tokio::test]
async fn dialect_is_athena() {
    assert_eq!(AthenaMock::open_images_fixture().dialect(), Dialect::Athena);
}

#[tokio::test]
async fn execute_translates_named_to_question_mark() {
    let mock = AthenaMock::open_images_fixture();
    let rows = mock
        .execute(
            "SELECT * FROM images WHERE image_id = :id",
            &[("id".to_string(), Value::from("img1"))],
        )
        .await
        .expect("execute");

    let translated = mock
        .last_translated_sql
        .lock()
        .unwrap()
        .clone()
        .expect("translated SQL recorded");
    assert!(translated.contains('?'), "named → ? : {translated}");
    assert!(!translated.contains(":id"), "no named placeholder left: {translated}");

    assert_eq!(rows.len(), 1, "exactly one matching row");
    assert_eq!(rows[0]["label"], json!("cat"));
}

#[tokio::test]
async fn schema_text_contains_external_table_markers() {
    let mock = AthenaMock::open_images_fixture();
    let text = mock.schema_text().await.expect("schema_text");
    assert!(text.contains("CREATE EXTERNAL TABLE"), "{text}");
    assert!(text.contains("PARTITIONED BY"), "{text}");
    assert!(text.contains("LOCATION 's3://"), "{text}");
}

#[tokio::test]
async fn repeated_named_params_translate_to_question_marks_in_order() {
    let mock = AthenaMock::open_images_fixture();
    mock.execute(
        "SELECT * FROM t WHERE a = :a AND b = :b AND c = :a",
        &[("a".to_string(), json!(1)), ("b".to_string(), json!(2))],
    )
    .await
    .expect("execute");

    let translated = mock
        .last_translated_sql
        .lock()
        .unwrap()
        .clone()
        .expect("translated SQL recorded");
    assert_eq!(
        translated.matches('?').count(),
        3,
        ":a, :b, :a → three ? placeholders: {translated}"
    );

    let positional = mock
        .last_positional_args
        .lock()
        .unwrap()
        .clone()
        .expect("positional args recorded");
    assert_eq!(positional.len(), 3, "three positional binds");
    assert_eq!(positional[0], json!(1), "position 0 binds :a");
    assert_eq!(positional[2], json!(1), "position 2 re-binds :a");
}

// REVIEWS M5: the pagination seam returns the concatenation of all pages.
#[tokio::test]
async fn multi_page_query_returns_all_pages_combined() {
    let mock = AthenaMock::open_images_fixture().with_pages(vec![
        vec![json!({"id": 1}), json!({"id": 2})], // page 1: 2 rows
        vec![json!({"id": 3}), json!({"id": 4}), json!({"id": 5})], // page 2: 3 rows
    ]);
    let sql = format!("SELECT id FROM big_table {PAGINATED_QUERY_MARKER}");
    let rows = mock.execute(&sql, &[]).await.expect("execute");
    assert_eq!(rows.len(), 5, "pagination must concatenate all pages");
    assert_eq!(rows[0]["id"], json!(1));
    assert_eq!(rows[4]["id"], json!(5));
}
