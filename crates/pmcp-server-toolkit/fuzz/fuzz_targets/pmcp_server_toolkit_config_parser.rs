//! Phase 83 fuzz target — stress `pmcp_server_toolkit::ServerConfig::from_toml`
//! against arbitrary byte sequences. The parser must not panic on adversarial
//! input — `toml::from_str` either succeeds (valid TOML matching schema with
//! `#[serde(deny_unknown_fields)]`) or returns Err.
//!
//! Threat model: T-83-09-01 (adversarial TOML → parser DoS via panic).
//!
//! Note: `cargo fuzz` requires a nightly toolchain (libfuzzer-sys uses
//! `-Z sanitizer`).
//!
//! Run with:
//! ```sh
//! cargo +nightly fuzz run pmcp_server_toolkit_config_parser
//! ```
//!
//! Quick sanity smoke (10 s):
//! ```sh
//! cargo +nightly fuzz run pmcp_server_toolkit_config_parser -- -max_total_time=10
//! ```
//!
//! Full CI gate (60 s minimum per Phase 83 must-haves):
//! ```sh
//! cargo +nightly fuzz run pmcp_server_toolkit_config_parser -- -max_total_time=60
//! ```
//!
//! Per Phase 77 disposition, if `cargo +nightly` is unavailable locally the
//! source still lands and CI nightly exercises the actual fuzz run. Compile
//! verification on stable: `cargo +nightly check --bin pmcp_server_toolkit_config_parser`.

#![no_main]

use libfuzzer_sys::fuzz_target;
use pmcp_server_toolkit::ServerConfig;

fuzz_target!(|data: &[u8]| {
    // Pre-filter non-UTF-8: `toml::from_str` requires `&str`, so the fuzz target
    // intentionally exits early on invalid UTF-8 rather than wrapping in a
    // lossy decode (the parser's contract is "valid UTF-8 TOML or error").
    let Ok(s) = std::str::from_utf8(data) else {
        return;
    };
    // The parser must never panic — `ServerConfig::from_toml` returns
    // `Result<Self, ToolkitError>` on anything but valid TOML matching the
    // strict schema.
    let _: Result<ServerConfig, _> = ServerConfig::from_toml(s);
});
