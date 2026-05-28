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
    //! Narrow lib-visible view of the deployment subsystem: `config`, `iam`,
    //! `widgets`, and `post_deploy_tests`. The full module is in the bin
    //! target; this surface is sufficient for the Phase 76 fuzz target and
    //! the `deploy_with_iam` example, plus the Phase 79 Wave-1 schema types
    //! that `config.rs` references via `use crate::deployment::widgets::*`
    //! and `use crate::deployment::post_deploy_tests::*`.

    #[path = "../deployment/config.rs"]
    pub mod config;

    // Re-export the schema types the mounted Cloud Run Dockerfile generator
    // references via `crate::deployment::*` (`dockerfile.rs` imports
    // `crate::deployment::{DeployConfig, LayoutConfig}`).
    pub use config::{DeployConfig, LayoutConfig};

    #[path = "../deployment/iam.rs"]
    pub mod iam;

    /// Narrow lib-visible view of the Google Cloud Run Dockerfile generator.
    ///
    /// The full `targets::*` tree is bin-only (it cross-depends on
    /// `commands::*`), but the Dockerfile / cloudbuild rendering for Cloud Run
    /// only needs `deployment::config` + a sibling `env` helper. Mounting just
    /// these two leaf files via `#[path]` lets the env-gated
    /// `cloud_run_local_build` integration test render the multi-crate-isolated
    /// Dockerfile via the real generator (issue #258) without pulling in the
    /// command layer.
    #[path = "../deployment/targets/google_cloud_run"]
    pub mod google_cloud_run {
        pub mod env;

        pub mod dockerfile;
    }

    // Phase 79 Wave 1: schema types required by `config.rs` so the lib
    // target compiles. These modules are leaf — they cross-depend only on
    // serde and stdlib, so mounting them here does not drag in any further
    // bin-only `commands::*` references.
    #[path = "../deployment/widgets.rs"]
    pub mod widgets;

    #[path = "../deployment/post_deploy_tests.rs"]
    pub mod post_deploy_tests;

    // `widgets::enumerate_workspace_bin_crates` delegates here.
    #[path = "../deployment/naming.rs"]
    pub mod naming;
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

// Compiled via `#[path]` to bypass the bin-only `commands::configure` tree.
// Mirrors the test_support_cache pattern (see lib.rs above for the established convention).
// Only the leaf `config.rs` schema is bridged — the full configure command tree stays bin-only.
#[doc(hidden)]
#[path = "commands/configure/config.rs"]
pub mod test_support_configure;

#[doc(hidden)]
pub mod test_support {
    pub use crate::test_support_cache as cache;
    pub use crate::test_support_configure as configure_config;
}
