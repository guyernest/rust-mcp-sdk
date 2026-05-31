//! Backend dispatch: `[database] type` → `Arc<dyn SqlConnector>` (NOVEL seam).
//!
//! This is the one genuinely-new business seam in the Shape A binary —
//! RESEARCH/PATTERNS flag it as having no analog anywhere in the codebase.
//! Everything else the binary does is wiring of `pmcp-server-toolkit`
//! primitives; this module turns the `[database] type` string into the concrete
//! [`SqlConnector`] the toolkit builder chain (Plan 05) consumes as an
//! `Arc<dyn SqlConnector>`.
//!
//! # Compiled-out backends (D-08)
//!
//! Each backend arm is feature-gated. When a config names a backend whose
//! feature was compiled out, [`dispatch`] returns
//! [`DispatchError::FeatureMissing`] with actionable rebuild guidance naming the
//! missing feature — never a silent fallback to the wrong backend.
//!
//! # Credential safety (V7 / T-85-04-01)
//!
//! [`DispatchError`]'s `Display` NEVER echoes connection URLs, file paths, or
//! credential strings from the config — it names the backend / feature only.
//! The wrapped [`ConnectorError`] is already credential-redacted at the source
//! (the per-backend connectors strip passwords / AWS keys before constructing
//! it, T-84-0{5,6,7}-02).
//!
//! # Offline-safe Athena (D-09 / REVIEW FIX / T-85-04-04)
//!
//! [`AthenaConnector::from_config`] builds an AWS SDK client. In aws-config 1.x
//! the default credentials provider chain resolves credentials *lazily* (on the
//! first API call), so construction does not reach the network — PROVIDED a
//! region is supplied explicitly so `load()` does not probe IMDS for one. The
//! Athena arm therefore resolves the region from `AWS_REGION` /
//! `AWS_DEFAULT_REGION` (falling back to a static default) and passes it
//! through; it never calls `schema_text()` / `execute()` at dispatch time. This
//! keeps Plan 05's SC-1 `tools/list`-with-no-creds test from hanging or reaching
//! the network.

use std::sync::Arc;

use pmcp_server_toolkit::config::ServerConfig;
use pmcp_server_toolkit::sql::{ConnectorError, SqlConnector};

/// Error returned when [`dispatch`] cannot produce a connector for the config.
///
/// # Security
///
/// `Display` names the backend / feature ONLY. It MUST NOT echo the connection
/// URL, SQLite file path, AWS output location, or any credential substring from
/// the config (V7 / T-85-04-01). The wrapped [`ConnectorError`] is redacted at
/// the connector source.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum DispatchError {
    /// The config names a backend whose Cargo feature was compiled out of this
    /// binary. Carries the backend name so the message can guide a rebuild
    /// (D-08). Names the feature only — no config values.
    #[error(
        "config requires backend '{0}' but this binary was built without the '{0}' feature; \
         rebuild with --features {0}"
    )]
    FeatureMissing(String),

    /// The `[database] type` is set to a value no backend recognises. Carries
    /// the offending type string (which is a backend identifier the operator
    /// typed, not a secret — e.g. `"oracle"`), not any URL/credential.
    #[error("unknown [database] type '{0}'; supported: sqlite, postgres, mysql, athena")]
    UnknownBackend(String),

    /// The `[database] type` key is absent. The binary cannot guess a backend.
    #[error("[database] type is required (one of: sqlite, postgres, mysql, athena)")]
    MissingType,

    /// A required config field for the selected backend is absent (e.g. a
    /// Postgres config with no `url`, or a SQLite config with no `file_path`).
    /// Names the field only — never its value.
    #[error("backend '{backend}' requires the config field '{field}'")]
    MissingField {
        /// Backend that needs the field.
        backend: &'static str,
        /// Name of the absent config field.
        field: &'static str,
    },

    /// The SQLite connector could not open the configured database.
    ///
    /// # Security (T-85-04-01)
    ///
    /// `rusqlite`'s open error echoes the file path (e.g. "unable to open
    /// database file: /secret/path/db") — so the SQLite arm maps it into THIS
    /// path-free variant rather than forwarding the raw [`ConnectorError`]. The
    /// path is logged at the binary's discretion (tracing), never surfaced to
    /// MCP clients.
    #[error("backend 'sqlite' failed to open the configured database file")]
    SqliteOpen,

    /// The connector failed to construct. Used by the URL-based backends
    /// (Postgres / MySQL) and Athena, whose [`ConnectorError`] is already
    /// credential-redacted at the connector source (sanitize_url /
    /// strip_aws_credentials, T-84-0{5,6,7}-02). The SQLite open path uses
    /// [`DispatchError::SqliteOpen`] instead because its error carries the file
    /// path verbatim.
    #[error("connector construction failed: {0}")]
    Connector(#[from] ConnectorError),
}

/// Select and construct the [`SqlConnector`] for the configured backend.
///
/// Matches on `cfg.database.backend_type` (the `[database] type` string) and
/// constructs the matching connector behind its feature gate. The returned
/// `Arc<dyn SqlConnector>` feeds the toolkit builder chain in Plan 05.
///
/// Construction is offline-safe for every backend: Postgres / MySQL build lazy
/// pools (no TCP at construction), Athena builds an SDK client with an explicit
/// region (no IMDS / provider-chain probe — see the module docs), and SQLite
/// opens a local file (or `:memory:`). No `schema_text()` / `execute()` call is
/// made here.
///
/// # Errors
///
/// - [`DispatchError::MissingType`] when `[database] type` is absent.
/// - [`DispatchError::UnknownBackend`] for an unrecognised type.
/// - [`DispatchError::FeatureMissing`] when the type names a compiled-out
///   backend (D-08).
/// - [`DispatchError::MissingField`] when a required field for the backend is
///   absent.
/// - [`DispatchError::Connector`] when the connector itself fails to construct.
pub async fn dispatch(cfg: &ServerConfig) -> Result<Arc<dyn SqlConnector>, DispatchError> {
    match cfg.database.backend_type.as_deref() {
        Some("sqlite") => dispatch_sqlite(cfg),
        Some("postgres") => dispatch_postgres(cfg).await,
        Some("mysql") => dispatch_mysql(cfg).await,
        Some("athena") => dispatch_athena(cfg).await,
        Some(other) => Err(DispatchError::UnknownBackend(other.to_string())),
        None => Err(DispatchError::MissingType),
    }
}

/// Construct the SQLite connector (`:memory:` or file-backed). Synchronous.
#[cfg(feature = "sqlite")]
fn dispatch_sqlite(cfg: &ServerConfig) -> Result<Arc<dyn SqlConnector>, DispatchError> {
    use pmcp_server_toolkit::sql::SqliteConnector;
    use std::path::Path;

    // Resolve the path from `file_path` first, then fall back to the documented
    // `database = ":memory:"` / `database = "<path>"` form (config.rs DatabaseSection
    // docs — 85-10 dispatch fix). `file_path` takes precedence when both are set.
    let path = cfg
        .database
        .file_path
        .as_deref()
        .or(cfg.database.database.as_deref())
        .ok_or(DispatchError::MissingField {
            backend: "sqlite",
            field: "file_path` or `database",
        })?;
    // T-85-04-01: rusqlite's open error echoes the file path, so map any open
    // failure to the path-free DispatchError::SqliteOpen. The operator can find
    // the path in their own config / the binary's tracing logs.
    let conn = if path == ":memory:" {
        SqliteConnector::open_in_memory().map_err(|_| DispatchError::SqliteOpen)?
    } else {
        SqliteConnector::open(Path::new(path)).map_err(|_| DispatchError::SqliteOpen)?
    };
    Ok(Arc::new(conn))
}

/// Compiled-out SQLite arm — actionable rebuild error (D-08).
#[cfg(not(feature = "sqlite"))]
fn dispatch_sqlite(_cfg: &ServerConfig) -> Result<Arc<dyn SqlConnector>, DispatchError> {
    Err(DispatchError::FeatureMissing("sqlite".to_string()))
}

/// Construct the Postgres connector (lazy pool; no TCP at construction).
#[cfg(feature = "postgres")]
async fn dispatch_postgres(cfg: &ServerConfig) -> Result<Arc<dyn SqlConnector>, DispatchError> {
    use pmcp_toolkit_postgres::PostgresConnector;

    let url = cfg
        .database
        .url
        .as_deref()
        .ok_or(DispatchError::MissingField {
            backend: "postgres",
            field: "url",
        })?;
    let conn = PostgresConnector::connect(url).await?;
    Ok(Arc::new(conn))
}

/// Compiled-out Postgres arm — actionable rebuild error (D-08).
#[cfg(not(feature = "postgres"))]
async fn dispatch_postgres(_cfg: &ServerConfig) -> Result<Arc<dyn SqlConnector>, DispatchError> {
    Err(DispatchError::FeatureMissing("postgres".to_string()))
}

/// Construct the MySQL connector (lazy pool; no TCP at construction).
#[cfg(feature = "mysql")]
async fn dispatch_mysql(cfg: &ServerConfig) -> Result<Arc<dyn SqlConnector>, DispatchError> {
    use pmcp_toolkit_mysql::MysqlConnector;

    let url = cfg
        .database
        .url
        .as_deref()
        .ok_or(DispatchError::MissingField {
            backend: "mysql",
            field: "url",
        })?;
    let conn = MysqlConnector::connect(url).await?;
    Ok(Arc::new(conn))
}

/// Compiled-out MySQL arm — actionable rebuild error (D-08).
#[cfg(not(feature = "mysql"))]
async fn dispatch_mysql(_cfg: &ServerConfig) -> Result<Arc<dyn SqlConnector>, DispatchError> {
    Err(DispatchError::FeatureMissing("mysql".to_string()))
}

/// Read an env var, returning `None` when it is unset OR set-but-empty /
/// all-whitespace (85-10 region fix). A set-but-empty `AWS_REGION` previously
/// flowed through as a present empty region; treating it as unset lets the
/// `AWS_DEFAULT_REGION` / static-default fallback fire.
#[cfg(feature = "athena")]
fn non_empty_env(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|v| !v.trim().is_empty())
}

/// Resolve the AWS region for offline-safe Athena construction.
///
/// REVIEW FIX (T-85-04-04): an EXPLICIT region keeps `aws_config::load()` from
/// probing IMDS for one, so construction stays offline. Reads `AWS_REGION` then
/// `AWS_DEFAULT_REGION`; falls back to a static default when neither is set so
/// the SC-1 no-creds test never reaches the network. Credentials remain lazy
/// (resolved on the first API call, which dispatch never makes).
///
/// A set-but-EMPTY `AWS_REGION` / `AWS_DEFAULT_REGION` is treated as UNSET
/// (85-10 region fix) so an empty value falls through instead of yielding an
/// empty region.
#[cfg(feature = "athena")]
fn resolve_athena_region() -> String {
    non_empty_env("AWS_REGION")
        .or_else(|| non_empty_env("AWS_DEFAULT_REGION"))
        .unwrap_or_else(|| "us-east-1".to_string())
}

/// Construct the Athena connector (SDK client only; offline-safe, lazy creds).
#[cfg(feature = "athena")]
async fn dispatch_athena(cfg: &ServerConfig) -> Result<Arc<dyn SqlConnector>, DispatchError> {
    use pmcp_toolkit_athena::AthenaConnector;

    let region = resolve_athena_region();
    let workgroup = cfg
        .database
        .workgroup
        .as_deref()
        .ok_or(DispatchError::MissingField {
            backend: "athena",
            field: "workgroup",
        })?;
    // REVIEW FIX (T-85-04-04): from_config builds the SDK client with the
    // explicit region above; credentials resolve lazily on first API call, so
    // this stays offline. We deliberately apply only construction-time builders
    // (output_location/database) and never call execute()/schema_text() here.
    let mut conn = AthenaConnector::from_config(&region, workgroup).await?;
    if let Some(loc) = cfg.database.output_location.as_deref() {
        conn = conn.with_output_location(loc);
    }
    if let Some(db) = cfg.database.database.as_deref() {
        conn = conn.with_database(db);
    }
    Ok(Arc::new(conn))
}

/// Compiled-out Athena arm — actionable rebuild error (D-08).
#[cfg(not(feature = "athena"))]
async fn dispatch_athena(_cfg: &ServerConfig) -> Result<Arc<dyn SqlConnector>, DispatchError> {
    Err(DispatchError::FeatureMissing("athena".to_string()))
}

#[cfg(all(test, feature = "athena"))]
mod region_tests {
    use super::{non_empty_env, resolve_athena_region};
    use std::sync::{Mutex, MutexGuard};

    /// Process-global lock serializing every test that mutates the shared
    /// `AWS_REGION` / `AWS_DEFAULT_REGION` env vars. The default test runner
    /// is multi-threaded, so without this the region tests interleave and
    /// stomp each other's env (e.g. one test setting `AWS_DEFAULT_REGION=""`
    /// clobbers another that just set it to `eu-west-1`), producing flaky
    /// failures. `RegionEnvGuard` holds the lock for the whole test body.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    /// Snapshot + clear the two region env vars, returning a guard that
    /// restores them on drop so the test never bleeds region state into
    /// siblings. Also holds [`ENV_LOCK`] so region tests run serially.
    struct RegionEnvGuard {
        // Declared first so it is the last field dropped: the `Drop` impl
        // restores the env vars while the lock is still held, then this
        // releases it. Poisoning is recovered so a panicking test does not
        // cascade-fail its siblings.
        _lock: MutexGuard<'static, ()>,
        region: Option<String>,
        default_region: Option<String>,
    }

    impl RegionEnvGuard {
        fn take() -> Self {
            let lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
            let guard = Self {
                _lock: lock,
                region: std::env::var("AWS_REGION").ok(),
                default_region: std::env::var("AWS_DEFAULT_REGION").ok(),
            };
            std::env::remove_var("AWS_REGION");
            std::env::remove_var("AWS_DEFAULT_REGION");
            guard
        }
    }

    impl Drop for RegionEnvGuard {
        fn drop(&mut self) {
            match &self.region {
                Some(v) => std::env::set_var("AWS_REGION", v),
                None => std::env::remove_var("AWS_REGION"),
            }
            match &self.default_region {
                Some(v) => std::env::set_var("AWS_DEFAULT_REGION", v),
                None => std::env::remove_var("AWS_DEFAULT_REGION"),
            }
        }
    }

    #[test]
    fn non_empty_env_treats_empty_and_whitespace_as_unset() {
        let _guard = RegionEnvGuard::take();
        std::env::set_var("AWS_REGION", "");
        assert_eq!(non_empty_env("AWS_REGION"), None, "empty must be unset");
        std::env::set_var("AWS_REGION", "   ");
        assert_eq!(
            non_empty_env("AWS_REGION"),
            None,
            "whitespace must be unset"
        );
        std::env::set_var("AWS_REGION", "us-west-2");
        assert_eq!(non_empty_env("AWS_REGION"), Some("us-west-2".to_string()));
    }

    #[test]
    fn empty_aws_region_falls_through_to_default_region() {
        // 85-10: a set-but-EMPTY AWS_REGION must fall through to
        // AWS_DEFAULT_REGION rather than yielding an empty region.
        let _guard = RegionEnvGuard::take();
        std::env::set_var("AWS_REGION", "");
        std::env::set_var("AWS_DEFAULT_REGION", "eu-west-1");
        assert_eq!(resolve_athena_region(), "eu-west-1");
    }

    #[test]
    fn both_region_vars_empty_use_static_default() {
        let _guard = RegionEnvGuard::take();
        std::env::set_var("AWS_REGION", "");
        std::env::set_var("AWS_DEFAULT_REGION", "");
        assert_eq!(resolve_athena_region(), "us-east-1");
    }

    #[test]
    fn aws_region_wins_when_set_non_empty() {
        let _guard = RegionEnvGuard::take();
        std::env::set_var("AWS_REGION", "ap-south-1");
        std::env::set_var("AWS_DEFAULT_REGION", "eu-west-1");
        assert_eq!(resolve_athena_region(), "ap-south-1");
    }
}
