//! Phase 79 Plan 79-04: fuzz target for [[widgets]] + [post_deploy_tests]
//! TOML parsing.
//!
//! REVISION 3 HIGH-G2: this target ALSO exercises the OnFailure custom
//! Deserialize hard-reject path for `"rollback"` — verifies it errors
//! gracefully on any input without panicking. Mirrors `fuzz_iam_config.rs`
//! shape (Phase 76 Wave 5 precedent).
//!
//! Threat model: T-79-01 (TOML parser DoS — malformed / adversarial TOML
//! crashing or hanging the CLI).
//!
//! Run with: `cargo +nightly fuzz run fuzz_widgets_config`
//! Quick smoke: `cargo +nightly fuzz run fuzz_widgets_config -- -max_total_time=60`

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Guard: only attempt UTF-8; non-UTF-8 byte sequences are not meaningful TOML.
    let Ok(s) = std::str::from_utf8(data) else {
        return;
    };

    // Parse path — must not panic.
    let parsed: Result<cargo_pmcp::deployment::config::DeployConfig, _> = toml::from_str(s);

    // Validate path — only on successful parse. Each WidgetConfig::validate()
    // checks for path-traversal segments and empty argv arrays. Must not panic.
    if let Ok(cfg) = parsed {
        for widget in &cfg.widgets.widgets {
            let _ = widget.validate();
        }
    }

    // REVISION 3 HIGH-G2: also fuzz the OnFailure rollback-rejection path.
    // Synthesise a TOML doc with `on_failure = <fuzzed-string>` so the custom
    // Deserialize impl is exercised on adversarial inputs (NOT just clean
    // string literals). Truncate to 64 chars + escape quotes so the
    // synthetic TOML stays valid syntactically (the value parsing is what
    // we're testing, not the surrounding parser).
    let escaped: String = s.chars().take(64).collect::<String>().replace('"', "\\\"");
    let synthetic = format!(
        "[target]\ntype = \"aws-lambda\"\nversion = \"1\"\n\n\
         [post_deploy_tests]\non_failure = \"{escaped}\"\n"
    );
    let _: Result<cargo_pmcp::deployment::config::DeployConfig, _> =
        toml::from_str(&synthetic);
});
