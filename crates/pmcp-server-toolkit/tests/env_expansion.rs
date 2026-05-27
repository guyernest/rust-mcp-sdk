//! REF-01 Gap #3 — `${VAR}` token-secret env-expansion (Plan 85-01 Task 2).
//!
//! The four reference configs all set `token_secret = "${CODE_MODE_SECRET}"`,
//! which the toolkit rejected before Plan 85-01 (only `env:VAR` was accepted).
//! This test drives `${VAR}` resolution through the public
//! [`validation_pipeline_from_config`] entry point (the function that consumes
//! `resolve_token_secret`), proving:
//!
//! 1. `${MY_SECRET}` resolves from the named env var (≥16 bytes per
//!    `HmacTokenGenerator::MIN_SECRET_LEN`).
//! 2. `${MISSING_VAR}` (unset) returns `Err(ToolkitError::CodeMode)` — never a panic
//!    (threat-model item T-85-01-01: never fall back to a weak/empty secret).
//! 3. The pre-existing `env:VAR` convention still resolves (no regression).
//! 4. The R9 inline-secret rejection still fires for a bare literal.
//! 5. Proptest: arbitrary `${name}`-ish token-secret strings never panic
//!    (the pipeline returns `Ok` or `Err`, never unwinds).
//!
//! Tests share process env, so the suite MUST run with `--test-threads=1`
//! (matches the project-wide CI convention).

#![cfg(feature = "code-mode")]

use pmcp_server_toolkit::code_mode::validation_pipeline_from_config;
use pmcp_server_toolkit::config::ServerConfig;
use pmcp_server_toolkit::ToolkitError;
use proptest::prelude::*;

/// Build a minimal `[code_mode]`-enabled config with the given verbatim
/// `token_secret` value. `validation_pipeline_from_config` resolves that secret.
fn config_with_token_secret(token_secret: &str) -> ServerConfig {
    let toml = format!(
        r#"
        [server]
        name = "env-expansion-test"
        version = "0.1.0"

        [code_mode]
        enabled = true
        token_secret = "{token_secret}"
        "#
    );
    ServerConfig::from_toml_strict_validated(&toml).expect("config parses + validates")
}

#[test]
fn braced_var_resolves_from_env() {
    const VAR: &str = "PMCP_TOOLKIT_BRACED_VAR_RESOLVES";
    std::env::set_var(VAR, "a-test-secret-bytes-16-or-more");
    let cfg = config_with_token_secret(&format!("${{{VAR}}}"));
    let result = validation_pipeline_from_config(&cfg);
    std::env::remove_var(VAR);
    assert!(
        result.is_ok(),
        "${{{VAR}}} must resolve from the named env var; got: {:?}",
        result.err().map(|e| e.to_string())
    );
}

#[test]
fn braced_var_missing_errors_without_panic() {
    // T-85-01-01: a missing/unset ${VAR} must error cleanly, never panic and
    // never fall back to a weak/empty secret.
    let cfg = config_with_token_secret("${PMCP_TOOLKIT_DEFINITELY_NOT_SET_BRACED}");
    match validation_pipeline_from_config(&cfg) {
        Ok(_) => panic!("missing ${{VAR}} must error"),
        Err(ToolkitError::CodeMode(msg)) => {
            assert!(
                msg.contains("PMCP_TOOLKIT_DEFINITELY_NOT_SET_BRACED"),
                "error must name the missing var; got: {msg}"
            );
        },
        Err(other) => panic!("expected ToolkitError::CodeMode, got: {other:?}"),
    }
}

#[test]
fn env_prefix_still_resolves() {
    // No regression: the existing `env:VAR` convention still works.
    const VAR: &str = "PMCP_TOOLKIT_ENV_PREFIX_STILL_WORKS";
    std::env::set_var(VAR, "a-test-secret-bytes-16-or-more");
    let cfg = config_with_token_secret(&format!("env:{VAR}"));
    let result = validation_pipeline_from_config(&cfg);
    std::env::remove_var(VAR);
    assert!(
        result.is_ok(),
        "env:VAR must still resolve; got: {:?}",
        result.err().map(|e| e.to_string())
    );
}

#[test]
fn inline_literal_still_rejected_r9() {
    // R9 unchanged: a bare inline literal (not env:/not ${...}) is still rejected.
    let cfg = config_with_token_secret("raw-inline-secret-not-allowed");
    match validation_pipeline_from_config(&cfg) {
        Ok(_) => panic!("bare inline literal must be rejected (R9)"),
        Err(ToolkitError::Validation(
            pmcp_server_toolkit::ConfigValidationError::InlineSecretRejected,
        )) => {},
        Err(other) => panic!("expected InlineSecretRejected, got: {other:?}"),
    }
}

proptest! {
    /// T-85-01: arbitrary `${name}`-shaped token-secret strings never panic.
    /// The pipeline returns Ok or Err — it must never unwind. The proptest
    /// uses var names overwhelmingly unlikely to be set in CI, so the expected
    /// outcome is a clean Err for almost every input; the invariant under test
    /// is *no panic*.
    #[test]
    fn braced_var_resolution_never_panics(name in "[A-Za-z_][A-Za-z0-9_]{0,32}") {
        let cfg = config_with_token_secret(&format!("${{{name}}}"));
        // Materialize the Result — Ok or Err, never a panic.
        let _ = validation_pipeline_from_config(&cfg);
    }
}
