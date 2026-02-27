//! Fuzz target for TOML config parsing.
//!
//! Feeds arbitrary byte sequences to `LoadTestConfig::from_toml()` and
//! verifies it never panics. Errors (parse failures, validation failures)
//! are expected and acceptable -- panics are not.
//!
//! Run with: `cargo +nightly fuzz run fuzz_config_parse`

#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Only attempt parsing if the input is valid UTF-8.
    // Non-UTF-8 bytes are not meaningful TOML input.
    if let Ok(s) = std::str::from_utf8(data) {
        // Must not panic -- errors are fine, panics are not.
        let _ = cargo_pmcp::loadtest::config::LoadTestConfig::from_toml(s);
    }
});
