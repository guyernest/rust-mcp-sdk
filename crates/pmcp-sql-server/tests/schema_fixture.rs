//! Fixture-shape sanity tests for the vendored Chinook parity fixtures (REF-02).
//!
//! This is the Wave 1 (Plan 85-03) foundation for the self-contained parity
//! harness; the actual scenario replay against a live server is Plan 85-06.
//! These tests prove the FOUR vendored fixtures are sound:
//!
//! 1. `tests/fixtures/chinook.db` is **data-bearing** (~1 MB, not a schema-only
//!    stub): opened via [`SqliteConnector::open`], a curated `SELECT` returns
//!    the real `"Rock"` genre that `generated.yaml` asserts on (REVIEW FIX #1 —
//!    an empty DB would make the parity replay FAIL).
//! 2. A `search_tracks`-style join returns the `"AC/DC"`/`"For Those About To
//!    Rock"` rows the scenarios expect.
//! 3. `tests/fixtures/chinook.ddl` is the SEPARATE `--schema` text input
//!    (D-06), DISTINCT from the `.db` data file: it is non-empty and names the
//!    curated tables.
//! 4. The DDL is a valid **standalone** schema — loading it into a fresh
//!    in-memory SQLite DB via `execute_batch` creates >= 11 tables.
//! 5. The vendored `generated.yaml` parses via [`mcp_tester::TestScenario`] —
//!    the parity contract is consumable.

#![cfg(feature = "sqlite")]

use std::path::Path;

use pmcp_server_toolkit::sql::{SqlConnector, SqliteConnector};

const CHINOOK_DB: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/chinook.db"
);

const CHINOOK_DDL: &str = include_str!("fixtures/chinook.ddl");
const GENERATED_YAML: &str = include_str!("fixtures/generated.yaml");

/// The committed `chinook.db` must be the real ~1 MB data-bearing DB, not an
/// empty schema-only stub. (REVIEW FIX #1 — the parity replay asserts on real
/// data values.)
#[test]
fn chinook_db_is_data_bearing_blob() {
    let meta = std::fs::metadata(CHINOOK_DB).expect("chinook.db fixture must exist");
    assert!(
        meta.len() >= 500_000,
        "chinook.db must be the data-bearing DB (>= 500 KB), got {} bytes — a \
         schema-only stub would make the parity replay return no rows",
        meta.len()
    );
}

/// Opening the populated DB through the toolkit connector and running a curated
/// `SELECT` returns the real `"Rock"` genre — proves the fixture is
/// data-bearing through the SAME connector path the parity harness uses.
#[tokio::test]
async fn populated_db_returns_real_genre_rows() {
    let conn = SqliteConnector::open(Path::new(CHINOOK_DB)).expect("open chinook.db");
    let rows = conn
        .execute("SELECT Name FROM Genre WHERE Name = :g", &[("g".into(), "Rock".into())])
        .await
        .expect("curated Genre SELECT must succeed against the populated DB");
    assert_eq!(
        rows.len(),
        1,
        "the populated chinook.db must contain the 'Rock' genre generated.yaml asserts on"
    );
    assert_eq!(rows[0]["Name"], serde_json::json!("Rock"));
}

/// A `search_tracks`-style join (the shape the curated `search_tracks` tool
/// runs) returns the `AC/DC` / `For Those About To Rock` rows the scenarios
/// assert on — confirming the data the parity contract expects is present.
#[tokio::test]
async fn search_tracks_shape_returns_acdc_rows() {
    let conn = SqliteConnector::open(Path::new(CHINOOK_DB)).expect("open chinook.db");
    let rows = conn
        .execute(
            "SELECT t.Name FROM Track t \
             JOIN Album al ON t.AlbumId = al.AlbumId \
             JOIN Artist ar ON al.ArtistId = ar.ArtistId \
             WHERE ar.Name LIKE :artist",
            &[("artist".into(), "%AC/DC%".into())],
        )
        .await
        .expect("search_tracks-style join must succeed");
    assert!(
        !rows.is_empty(),
        "the populated DB must return AC/DC tracks for the search_tracks parity scenarios"
    );
    let titles: Vec<String> = rows
        .iter()
        .filter_map(|r| r["Name"].as_str().map(str::to_owned))
        .collect();
    assert!(
        titles.iter().any(|t| t == "For Those About To Rock (We Salute You)"),
        "expected the 'For Those About To Rock (We Salute You)' track in AC/DC results, got {titles:?}"
    );
}

/// The DDL fixture is the SEPARATE `--schema` text input (D-06), non-empty and
/// naming the curated-tool tables — DISTINCT from the `.db` data file.
#[test]
fn ddl_fixture_names_curated_tables() {
    assert!(!CHINOOK_DDL.trim().is_empty(), "chinook.ddl must be non-empty");
    for table in ["Artist", "Album", "Track", "Genre"] {
        assert!(
            CHINOOK_DDL.contains(table),
            "chinook.ddl must name the curated table {table:?}"
        );
    }
    let create_count = CHINOOK_DDL.matches("CREATE TABLE").count();
    assert!(
        create_count >= 11,
        "chinook.ddl must define >= 11 tables, found {create_count}"
    );
}

/// The DDL is a valid STANDALONE schema: loading it into a fresh in-memory
/// SQLite DB succeeds and creates >= 11 tables. This proves the `--schema`
/// input is self-sufficient, independent of the populated `.db`.
#[test]
fn ddl_builds_valid_standalone_schema() {
    let conn = rusqlite::Connection::open_in_memory().expect("open in-memory sqlite");
    conn.execute_batch(CHINOOK_DDL)
        .expect("the vendored DDL must be a valid standalone schema");
    let table_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table'",
            [],
            |row| row.get(0),
        )
        .expect("count tables");
    assert!(
        table_count >= 11,
        "loading chinook.ddl must create >= 11 tables, created {table_count}"
    );
}

/// The vendored `generated.yaml` parses via [`mcp_tester::TestScenario`] —
/// confirming the 29-scenario parity contract is consumable by the Plan 06
/// harness with no cross-repo dependency.
#[test]
fn generated_yaml_parses_as_test_scenario() {
    let scenario: mcp_tester::TestScenario =
        serde_yaml::from_str(GENERATED_YAML).expect("generated.yaml must parse as a TestScenario");
    assert!(
        !scenario.steps.is_empty(),
        "the vendored parity contract must declare scenario steps"
    );
}
