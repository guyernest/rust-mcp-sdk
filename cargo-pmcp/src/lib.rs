//! cargo-pmcp library — loadtest and pentest types for external use by
//! fuzz targets, integration tests, and examples, plus a narrow
//! integration-test seam onto `auth_cmd::cache`.

pub mod loadtest;
pub mod pentest;

// Compiled via `#[path]` to bypass the bin-only `commands::auth_cmd` tree,
// which cross-depends on `crate::deployment` / `crate::utils` and cannot
// compile in the lib target without pulling in the entire CLI subsystem.
#[doc(hidden)]
#[path = "commands/auth_cmd/cache.rs"]
pub mod test_support_cache;

#[doc(hidden)]
pub mod test_support {
    pub use crate::test_support_cache as cache;
}
