//! Phase 77 fuzz target: stress the `~/.pmcp/config.toml` parser against arbitrary
//! byte sequences. The parser must not panic on adversarial input — `toml::from_str`
//! either succeeds (valid TOML matching schema) or returns Err (anything else).
//!
//! Threat model: T-77-02-A (parser DoS — adversarial TOML input panics the parser).
//!
//! Note: `cargo fuzz` requires a nightly toolchain (libfuzzer-sys uses `-Z sanitizer`).
//! Run with: `cargo +nightly fuzz run pmcp_config_toml_parser`
//! Quick smoke: `cargo +nightly fuzz run pmcp_config_toml_parser -- -max_total_time=60`
//!
//! On stable, `cargo +nightly check --bin pmcp_config_toml_parser` from the
//! `cargo-pmcp/fuzz/` directory verifies the target compiles. CI / nightly will
//! exercise the actual fuzz run; this file lands the source even when local stable
//! cannot build it.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(s) = std::str::from_utf8(data) else {
        return;
    };
    let _: Result<cargo_pmcp::test_support::configure_config::TargetConfigV1, _> =
        toml::from_str(s);
});
