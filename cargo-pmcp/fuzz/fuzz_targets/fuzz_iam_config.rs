//! Phase 76 Wave 5: fuzz target for IamConfig / DeployConfig TOML parsing.
//!
//! Feeds arbitrary byte sequences to `toml::from_str::<DeployConfig>()` and
//! verifies the parse never panics. This covers IamConfig's serde surface and
//! all its sub-structs (TablePermission, BucketPermission, IamStatement)
//! transitively. Also invokes `cargo_pmcp::deployment::iam::validate` to
//! exercise the validator on whatever IamConfig the parser constructs.
//!
//! Threat model: T-76-03 (parser DoS — malformed / adversarial TOML crashing
//! or hanging the CLI).
//!
//! Run with: `cargo +nightly fuzz run fuzz_iam_config`
//! Quick smoke: `cargo +nightly fuzz run fuzz_iam_config -- -max_total_time=60`

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Guard: only attempt UTF-8; non-UTF-8 byte sequences are not meaningful TOML.
    let Ok(s) = std::str::from_utf8(data) else {
        return;
    };

    // Parse path — must not panic.
    let parsed: Result<cargo_pmcp::deployment::config::DeployConfig, _> = toml::from_str(s);

    // Validator path — only exercised on successful parse. Must not panic.
    if let Ok(cfg) = parsed {
        let _ = cargo_pmcp::deployment::iam::validate(&cfg.iam);
    }
});
