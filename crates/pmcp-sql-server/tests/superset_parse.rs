//! SC-2 (Plan 85-05) — superset parse + dispatch at the binary boundary (REF-01).
//!
//! All FOUR reference configs parse via [`ServerConfig::from_toml_strict_validated`]
//! and [`dispatch`] resolves the correct connector dialect for each:
//!
//! | config | backend | expected [`Dialect`] |
//! |--------|---------|----------------------|
//! | chinook reference-config.toml | sqlite | [`Dialect::Sqlite`] |
//! | open-images-config.toml | athena | [`Dialect::Athena`] |
//! | imdb-config.toml | athena | [`Dialect::Athena`] |
//! | msr-vtt-config.toml | athena | [`Dialect::Athena`] |
//!
//! Runs under DEFAULT features so all four backends compile in (REF-01 SC-2 at
//! the binary boundary — no renames; every reference config the proto-SDK ships
//! parses and dispatches the right connector through the published binary).
//!
//! Run with:
//! ```sh
//! cargo test -p pmcp-sql-server --test superset_parse -- --test-threads=1
//! ```

#![cfg(all(feature = "sqlite", feature = "athena"))]

use pmcp_server_toolkit::sql::Dialect;
use pmcp_server_toolkit::ServerConfig;
use pmcp_sql_server::dispatch;

// The Chinook reference config is vendored in this crate (Plan 03); the three
// Athena configs live in the toolkit's fixtures dir (Plan 83 D-03 set).
const CHINOOK_CONFIG: &str = include_str!("fixtures/reference-config.toml");
const OPEN_IMAGES_CONFIG: &str =
    include_str!("../../pmcp-server-toolkit/tests/fixtures/open-images-config.toml");
const IMDB_CONFIG: &str = include_str!("../../pmcp-server-toolkit/tests/fixtures/imdb-config.toml");
const MSR_VTT_CONFIG: &str =
    include_str!("../../pmcp-server-toolkit/tests/fixtures/msr-vtt-config.toml");

fn chinook_db_path() -> String {
    format!("{}/tests/fixtures/chinook.db", env!("CARGO_MANIFEST_DIR"))
}

/// Parse a config, dispatch its connector, and assert the resolved dialect.
///
/// No credentials are set — every backend's constructor is offline-safe
/// (Postgres/MySQL lazy pools, Athena explicit-region lazy creds, SQLite local
/// file). Dispatch never calls `execute()`/`schema_text()`.
async fn assert_dispatch_dialect(
    config_toml: &str,
    file_path_override: Option<&str>,
    expected: Dialect,
) {
    let mut cfg = ServerConfig::from_toml_strict_validated(config_toml)
        .expect("config must parse + validate");
    if let Some(path) = file_path_override {
        cfg.database.file_path = Some(path.to_string());
    }
    let connector = dispatch(&cfg)
        .await
        .expect("dispatch must resolve a connector");
    assert_eq!(
        connector.dialect(),
        expected,
        "dispatched connector must carry the backend's dialect"
    );
}

#[tokio::test]
async fn chinook_parses_and_dispatches_sqlite() {
    let path = chinook_db_path();
    assert_dispatch_dialect(CHINOOK_CONFIG, Some(&path), Dialect::Sqlite).await;
}

#[tokio::test]
async fn open_images_parses_and_dispatches_athena() {
    assert_dispatch_dialect(OPEN_IMAGES_CONFIG, None, Dialect::Athena).await;
}

#[tokio::test]
async fn imdb_parses_and_dispatches_athena() {
    assert_dispatch_dialect(IMDB_CONFIG, None, Dialect::Athena).await;
}

#[tokio::test]
async fn msr_vtt_parses_and_dispatches_athena() {
    assert_dispatch_dialect(MSR_VTT_CONFIG, None, Dialect::Athena).await;
}
