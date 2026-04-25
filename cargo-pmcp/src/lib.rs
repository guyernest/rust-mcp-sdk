//! cargo-pmcp library — loadtest and pentest types plus a narrow subset of
//! the deployment layer (`deployment::config` + `deployment::iam`) for external
//! use by fuzz targets, integration tests, and examples, plus a narrow
//! integration-test seam onto `auth_cmd::cache`.
//!
//! The full `deployment` module tree lives in the bin target and transitively
//! depends on `commands::*`; exposing only the two deployment submodules that
//! Phase 76 Wave 5's fuzz target + example need keeps the lib surface minimal.

pub mod loadtest;
pub mod pentest;

// Phase 76 Wave 5: expose `deployment::config` + `deployment::iam` to the lib
// target. These two modules only cross-depend on each other and on
// `utils::config`, so they can be mounted via `#[path]` without pulling in the
// rest of the `deployment::*` tree (which references `crate::commands::*`,
// bin-only).
pub mod deployment {
    //! Narrow lib-visible view of the deployment subsystem — `config` + `iam`.
    //! The full module is in the bin target; this surface is sufficient for
    //! the Phase 76 fuzz target + `deploy_with_iam` example.

    #[path = "../deployment/config.rs"]
    pub mod config;

    #[path = "../deployment/iam.rs"]
    pub mod iam;
}

pub mod utils {
    //! Narrow lib-visible view of `utils::config` so `deployment::config` can
    //! resolve `crate::utils::config::WorkspaceConfig`.

    #[path = "../utils/config.rs"]
    pub mod config;
}

// Compiled via `#[path]` to bypass the bin-only `commands::auth_cmd` tree,
// which cross-depends on the CLI subsystem and cannot compile in the lib
// target without pulling in the entire command layer.
#[doc(hidden)]
#[path = "commands/auth_cmd/cache.rs"]
pub mod test_support_cache;

#[doc(hidden)]
pub mod test_support {
    pub use crate::test_support_cache as cache;
}
