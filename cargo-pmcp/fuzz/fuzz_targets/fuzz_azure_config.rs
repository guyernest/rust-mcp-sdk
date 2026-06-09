//! Phase 80 Plan 04: fuzz target for AzureConfig / DeployConfig TOML parsing.
//!
//! Feeds arbitrary byte sequences to `toml::from_str::<DeployConfig>()` and
//! verifies the parse never panics. This covers AzureConfig's serde surface
//! (resource_group/environment/location/target_port/min_replicas) transitively,
//! exactly as `fuzz_iam_config` covers the `[iam]` surface. On a successful
//! parse it also reads back the `[azure]` fields and calls
//! `cfg.azure.is_empty()` — both must not panic.
//!
//! It does NOT call the Dockerfile generator or the `az`-arg builders: those
//! are NOT lib-public, so a fuzz crate compiled against `cargo_pmcp` cannot
//! reach them. The Dockerfile / arg-builder never-panic property is covered
//! IN-CRATE by the Plan 04 proptests instead.
//!
//! Threat model: T-80-02 (parser DoS — malformed / adversarial `[azure]` TOML
//! crashing or hanging the CLI).
//!
//! Run with: `cargo +nightly fuzz run fuzz_azure_config`
//! Quick smoke: `cargo +nightly fuzz run fuzz_azure_config -- -max_total_time=30`

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Guard: only attempt UTF-8; non-UTF-8 byte sequences are not meaningful TOML.
    let Ok(s) = std::str::from_utf8(data) else {
        return;
    };

    // Parse path — must not panic.
    let parsed: Result<cargo_pmcp::deployment::config::DeployConfig, _> = toml::from_str(s);

    // On a successful parse, exercise the [azure] read-back surface. Must not panic.
    if let Ok(cfg) = parsed {
        let _ = cfg.azure.is_empty();
        let _ = cfg.azure.resource_group.as_deref();
        let _ = cfg.azure.environment.as_deref();
        let _ = cfg.azure.location.len();
        let _ = cfg.azure.target_port;
        let _ = cfg.azure.min_replicas;
    }
});
