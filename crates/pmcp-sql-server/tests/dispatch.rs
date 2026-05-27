//! Tests for the NOVEL `[database] type` → `Arc<dyn SqlConnector>` dispatch.
//!
//! Run under `--no-default-features --features sqlite` (the lean single-backend
//! build) so the `#[cfg(feature = "athena")]`-off arm is exercised by the
//! compiled-out-feature test. The athena-feature-ON offline test is gated on
//! the `athena` feature so it only runs in a full-feature build.

use std::sync::Arc;

use pmcp_server_toolkit::config::ServerConfig;
use pmcp_server_toolkit::sql::{Dialect, SqlConnector};
use pmcp_sql_server::dispatch::{dispatch, DispatchError};

/// Build a minimal `ServerConfig` from a `[database]` body (server header added).
fn config_with_database(database_body: &str) -> ServerConfig {
    let toml = format!(
        "[server]\nname = \"dispatch-test\"\nversion = \"0.1.0\"\n\n[database]\n{database_body}"
    );
    ServerConfig::from_toml(&toml).expect("test config must parse")
}

/// Extract the [`DispatchError`] from a dispatch result, panicking with `ctx` if
/// it unexpectedly succeeded. `Arc<dyn SqlConnector>` is not `Debug`, so the
/// stdlib `expect_err`/`unwrap_err` cannot be used directly here.
fn expect_dispatch_err(
    result: Result<Arc<dyn SqlConnector>, DispatchError>,
    ctx: &str,
) -> DispatchError {
    match result {
        Ok(_) => panic!("{ctx}"),
        Err(e) => e,
    }
}

#[tokio::test(flavor = "current_thread")]
async fn sqlite_file_path_yields_sqlite_connector() {
    // A real on-disk SQLite file: dispatch must open it and report Sqlite.
    let tmp = tempfile::NamedTempFile::new().expect("temp db");
    let path = tmp.path().to_str().expect("utf8 path");
    let cfg = config_with_database(&format!("type = \"sqlite\"\nfile_path = \"{path}\""));
    let conn = dispatch(&cfg).await.expect("sqlite dispatch must succeed");
    assert_eq!(conn.dialect(), Dialect::Sqlite);
}

#[tokio::test(flavor = "current_thread")]
async fn sqlite_memory_uses_open_in_memory() {
    let cfg = config_with_database("type = \"sqlite\"\nfile_path = \":memory:\"");
    let conn = dispatch(&cfg)
        .await
        .expect(":memory: dispatch must succeed");
    assert_eq!(conn.dialect(), Dialect::Sqlite);
}

#[tokio::test(flavor = "current_thread")]
async fn sqlite_without_file_path_reports_missing_field() {
    let cfg = config_with_database("type = \"sqlite\"");
    let err = expect_dispatch_err(dispatch(&cfg).await, "missing file_path must err");
    assert!(matches!(
        err,
        DispatchError::MissingField {
            backend: "sqlite",
            field: "file_path"
        }
    ));
}

#[tokio::test(flavor = "current_thread")]
async fn unknown_backend_names_the_type() {
    let cfg = config_with_database("type = \"oracle\"");
    let err = expect_dispatch_err(dispatch(&cfg).await, "unknown type must err");
    match err {
        DispatchError::UnknownBackend(ref t) => assert_eq!(t, "oracle"),
        other => panic!("expected UnknownBackend, got {other:?}"),
    }
    assert!(format!("{err}").contains("oracle"));
}

#[tokio::test(flavor = "current_thread")]
async fn missing_type_reports_missing_type() {
    let cfg = config_with_database("database = \"x\"");
    let err = expect_dispatch_err(dispatch(&cfg).await, "absent type must err");
    assert!(matches!(err, DispatchError::MissingType));
}

/// Compiled-out backend (D-08): under `--features sqlite` (athena OFF), an
/// athena config returns a feature-missing error naming the 'athena' feature
/// with rebuild guidance — never a silent fallback.
#[cfg(not(feature = "athena"))]
#[tokio::test(flavor = "current_thread")]
async fn athena_config_without_feature_reports_feature_missing() {
    let cfg = config_with_database(
        "type = \"athena\"\nworkgroup = \"primary\"\noutput_location = \"s3://b/r/\"",
    );
    let err = expect_dispatch_err(dispatch(&cfg).await, "compiled-out athena must err");
    match err {
        DispatchError::FeatureMissing(ref f) => assert_eq!(f, "athena"),
        other => panic!("expected FeatureMissing, got {other:?}"),
    }
    let msg = format!("{err}");
    assert!(msg.contains("athena"), "names the backend: {msg}");
    assert!(
        msg.contains("--features athena"),
        "gives rebuild guidance: {msg}"
    );
}

/// REVIEW FIX (T-85-04-04): with the athena feature ON and NO AWS creds in the
/// env, dispatching an athena config must construct offline WITHOUT hanging.
/// The construction is wrapped in a short timeout — if `from_config` reached the
/// AWS provider chain / IMDS it would stall past the timeout. The Plan 05 SC-1
/// startup test additionally guards the full `tools/list` path.
#[cfg(feature = "athena")]
#[tokio::test(flavor = "current_thread")]
async fn athena_dispatch_is_offline_safe_with_no_creds() {
    // Deliberately do NOT set AWS_ACCESS_KEY_ID / AWS_SECRET_ACCESS_KEY.
    // resolve_athena_region() falls back to a static region so load() never
    // probes IMDS for one; credentials resolve lazily on first API call, which
    // dispatch never makes.
    let cfg = config_with_database(
        "type = \"athena\"\nworkgroup = \"primary\"\noutput_location = \"s3://b/r/\"\ndatabase = \"analytics\"",
    );
    let dispatched = tokio::time::timeout(std::time::Duration::from_secs(10), dispatch(&cfg)).await;
    let result = dispatched.expect("athena dispatch must not hang (offline-safe construction)");
    match result {
        Ok(conn) => assert_eq!(conn.dialect(), Dialect::Athena),
        // A clean, fast, non-network error is also acceptable per the plan —
        // what matters is that we did not hang reaching the provider chain.
        Err(e) => {
            let msg = format!("{e}");
            assert!(
                !msg.contains("password") && !msg.contains("AWS_SECRET_ACCESS_KEY"),
                "error must not leak credentials: {msg}"
            );
        },
    }
}

/// Property (V7 / T-85-04-01): no DispatchError Display echoes the connection
/// URL, file path, AWS output location, or any credential substring from the
/// config. We feed adversarial secret-bearing values and assert the rendered
/// error never contains them.
mod no_credential_leak {
    use super::{config_with_database, dispatch};
    use proptest::prelude::*;

    /// Build a tokio current-thread runtime to drive the async dispatch inside
    /// the synchronous proptest body.
    fn block_on<F: std::future::Future>(fut: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
            .block_on(fut)
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        /// A postgres config whose URL carries a secret password must never have
        /// that secret appear in the DispatchError Display. Postgres is gated;
        /// under sqlite-only this exercises the FeatureMissing arm, under the
        /// postgres feature the (lazy, offline) connect arm — both must stay
        /// leak-free.
        #[test]
        fn postgres_url_secret_never_in_error(secret in "[a-zA-Z0-9]{12,24}") {
            let url = format!("postgres://user:{secret}@db.internal:5432/app");
            let cfg = config_with_database(&format!("type = \"postgres\"\nurl = \"{url}\""));
            let rendered = match block_on(dispatch(&cfg)) {
                Ok(_) => String::new(), // lazy pool built fine — no error to inspect
                Err(e) => format!("{e}"),
            };
            prop_assert!(
                !rendered.contains(&secret),
                "DispatchError leaked the URL password: {rendered:?}"
            );
            prop_assert!(
                !rendered.contains(&url),
                "DispatchError leaked the raw URL: {rendered:?}"
            );
        }

        /// A sqlite config whose file_path looks secret-bearing must never have
        /// that path appear in the DispatchError Display when construction errs.
        #[test]
        fn sqlite_path_never_in_error(token in "[a-zA-Z0-9]{12,24}") {
            // A non-existent nested path under a token-named dir forces an open
            // error; the path (and token) must not surface in the Display.
            let path = format!("/nonexistent-{token}/deeper/{token}/db.sqlite");
            let cfg = config_with_database(&format!("type = \"sqlite\"\nfile_path = \"{path}\""));
            // Arc<dyn SqlConnector> is not Debug, so inspect the Err arm directly.
            if let Err(e) = block_on(dispatch(&cfg)) {
                let rendered = format!("{e}");
                prop_assert!(
                    !rendered.contains(&token),
                    "DispatchError leaked the file path token: {rendered:?}"
                );
            }
        }
    }
}
