//! Phase 98 (DSTK-04) fuzz target: stress the `[metadata]` block of a
//! `.pmcp/deploy.toml` against arbitrary byte sequences. The parser must not
//! panic on adversarial input — `toml::from_str::<DeployConfig>` either
//! succeeds (valid TOML matching schema) or returns Err (anything else).
//!
//! The `[metadata]` block (`server_type`, `snapshot_baked`) is operator-supplied
//! and flows into the rendered `stack.ts` synth context, so it is a tampering
//! trust boundary: an attacker who can influence the deploy.toml could feed
//! malformed TOML hoping to crash the CLI before the render guard runs.
//!
//! Threat model: T-98-01 (parser DoS — adversarial `[metadata]` TOML panics the
//! parser). Mirrors the existing `fuzz_iam_config` / `pmcp_config_toml_parser`
//! style; on a successful parse it additionally invokes the lib-public
//! `MetadataConfig::is_empty()` accessor to exercise the post-parse surface the
//! `#[serde(skip_serializing_if)]` backward-compat contract depends on.
//!
//! Note: `cargo fuzz` requires a nightly toolchain (libfuzzer-sys uses
//! `-Z sanitizer`).
//! Run with: `cargo +nightly fuzz run fuzz_metadata_config`
//! Quick smoke: `cargo +nightly fuzz run fuzz_metadata_config -- -max_total_time=60`
//!
//! On stable, `cargo +nightly check --bin fuzz_metadata_config` from the
//! `cargo-pmcp/fuzz/` directory verifies the target compiles. CI / nightly
//! exercises the actual fuzz run; this file lands the source even when local
//! stable cannot build it.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Guard: only attempt UTF-8; non-UTF-8 byte sequences are not meaningful TOML.
    let Ok(s) = std::str::from_utf8(data) else {
        return;
    };

    // Parse path — must not panic. Covers the full DeployConfig serde surface
    // including the `[metadata]` sub-struct (server_type / snapshot_baked).
    let parsed: Result<cargo_pmcp::deployment::config::DeployConfig, _> = toml::from_str(s);

    // Post-parse accessor path — only exercised on a successful parse. The
    // `is_empty()` predicate backs the byte-identity backward-compat contract
    // (DSTK-02) and must never panic on any parsed metadata.
    if let Ok(cfg) = parsed {
        let _ = cfg.metadata.is_empty();
    }
});
