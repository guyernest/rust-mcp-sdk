//! cargo-pmcp library — provides loadtest and pentest types for external use
//! by fuzz targets, integration tests, and examples, plus a narrow
//! integration-test seam onto `auth_cmd::cache` helpers.

pub mod loadtest;
pub mod pentest;

/// Integration test seam — re-exports a minimal subset of internal modules
/// that the test harness under `tests/` needs to exercise end-to-end.
///
/// NOT part of the stable public API. Hidden from rustdoc. Use at your own
/// risk — contents may change without notice.
///
/// Review MED-5 (Codex) — narrowed from the original blanket `pub mod commands;`
/// exposure. Integration tests only need `auth_cmd::cache`; the rest of the
/// `commands/` tree remains an implementation detail.
#[doc(hidden)]
#[path = "commands/auth_cmd/cache.rs"]
pub mod test_support_cache;

/// Narrow integration-test seam.
///
/// Re-exports only the `cache` module needed by the test harness. Compiled
/// via `#[path]` on a single top-level module (see `test_support_cache`) —
/// this bypasses the bin-only `commands::auth_cmd` tree, which has cross-deps
/// on `crate::deployment` / `crate::utils` and cannot compile in the lib
/// target without pulling in the entire CLI dispatch subsystem.
#[doc(hidden)]
pub mod test_support {
    /// Integration-test view of `auth_cmd::cache`. See `TokenCacheV1`,
    /// `TokenCacheEntry`, `normalize_cache_key`, `is_near_expiry`,
    /// `default_multi_cache_path`, `refresh_and_persist`, `REFRESH_WINDOW_SECS`.
    pub use crate::test_support_cache as cache;
}
