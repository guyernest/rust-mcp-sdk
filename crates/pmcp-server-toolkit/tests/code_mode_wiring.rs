//! TKIT-09 integration test — [code_mode] config drives policy enforcement.
//!
//! Verifies that a ServerConfig with `[code_mode] allow_writes = false` causes
//! the synthesized [`pmcp_server_toolkit::code_mode::ValidationPipeline`] to
//! REJECT an INSERT statement at validation time, without any per-server Rust
//! glue. This is the ROADMAP SC-3 anchor for Plan 06.
//!
//! Validation entry point per CODE_MODE_API_NOTES.md Section 2:
//! `validation_pipeline_from_config(&cfg).unwrap().validate_sql_query(sql, &ctx)`
//! returns a `ValidationResult` whose `valid` flag is `false` and whose
//! `violations` array contains an entry with rule "writes_disabled" when the
//! config disallows writes.

#![cfg(feature = "code-mode")]

use pmcp_server_toolkit::code_mode::{
    register_code_mode_tools, validation_pipeline_from_config, ValidationContext,
};
use pmcp_server_toolkit::config::ServerConfig;
use pmcp_server_toolkit::{ConfigValidationError, ToolkitError};

mod support;

const CONFIG_WRITES_DISALLOWED: &str = r#"
[server]
name = "Test"
version = "0.1.0"

[code_mode]
enabled = true
allow_writes = false
allow_deletes = false
allow_ddl = false
token_secret = "env:PMCP_TOOLKIT_TEST_SECRET"
"#;

/// Ensure the HMAC env var is set for tests in this file. The 16-byte minimum
/// (HmacTokenGenerator::MIN_SECRET_LEN) is enforced by pmcp-code-mode.
fn ensure_hmac_env() {
    std::env::set_var(
        "PMCP_TOOLKIT_TEST_SECRET",
        "test-secret-bytes-16-or-more-chars",
    );
}

#[test]
fn allow_writes_false_rejects_insert() {
    // SC-3 anchor: writes-disallowed config REJECTS an INSERT statement at
    // validation time.
    let _env = support::env_lock();
    ensure_hmac_env();
    let cfg = ServerConfig::from_toml_strict_validated(CONFIG_WRITES_DISALLOWED)
        .expect("config parses + validates");
    let pipeline = validation_pipeline_from_config(&cfg).expect("pipeline builds");

    let ctx = ValidationContext::new("test-user", "test-session", "schema-hash", "perms-hash");
    let result = pipeline
        .validate_sql_query("INSERT INTO foo VALUES (1, 2, 3);", &ctx)
        .expect("validation runs (returns failure, not Err)");

    assert!(
        !result.is_valid,
        "ValidationResult.is_valid must be false when allow_writes=false rejects an INSERT"
    );
    assert!(
        result
            .violations
            .iter()
            .any(|v| v.rule == "writes_disabled"),
        "violations must include a 'writes_disabled' rule, got: {:?}",
        result.violations
    );
}

#[test]
fn allow_writes_true_permits_insert() {
    // Inverse — when writes are enabled, the same INSERT validates successfully.
    let _env = support::env_lock();
    ensure_hmac_env();
    let toml = CONFIG_WRITES_DISALLOWED.replace("allow_writes = false", "allow_writes = true");
    let cfg = ServerConfig::from_toml_strict_validated(&toml).expect("config parses + validates");
    let pipeline = validation_pipeline_from_config(&cfg).expect("pipeline builds");

    let ctx = ValidationContext::new("test-user", "test-session", "schema-hash", "perms-hash");
    let result = pipeline
        .validate_sql_query("INSERT INTO foo VALUES (1, 2, 3);", &ctx)
        .expect("validation runs");

    assert!(
        result.is_valid,
        "ValidationResult.is_valid must be true when allow_writes=true permits an INSERT, \
         got violations: {:?}",
        result.violations
    );
}

#[test]
fn select_is_always_permitted_under_default_config() {
    // Sanity check: a basic SELECT is permitted under the default
    // writes-disallowed config (proves the pipeline isn't blanket-rejecting).
    let _env = support::env_lock();
    ensure_hmac_env();
    let cfg = ServerConfig::from_toml_strict_validated(CONFIG_WRITES_DISALLOWED)
        .expect("config parses + validates");
    let pipeline = validation_pipeline_from_config(&cfg).expect("pipeline builds");

    let ctx = ValidationContext::new("test-user", "test-session", "schema-hash", "perms-hash");
    let result = pipeline
        .validate_sql_query("SELECT * FROM foo LIMIT 10;", &ctx)
        .expect("validation runs");

    assert!(
        result.is_valid,
        "SELECT must be permitted under default config, got violations: {:?}",
        result.violations
    );
}

#[test]
fn inline_token_secret_without_dev_flag_rejected_by_register() {
    // R9 enforcement at the builder-extension entry point: a `[code_mode]`
    // with an inline literal token_secret (no `env:` prefix) and no
    // `allow_inline_token_secret_for_dev = true` must surface
    // `ConfigValidationError::InlineSecretRejected` from
    // `register_code_mode_tools`. This catches misconfigured servers at
    // builder time, not at first request.
    let toml = r#"
[server]
name = "T"
version = "0.1.0"
[code_mode]
enabled = true
token_secret = "raw-string-that-should-be-rejected"
"#;
    let cfg = ServerConfig::from_toml(toml).expect("parse succeeds");
    let builder = pmcp::Server::builder().name("t").version("0.1.0");
    let result = register_code_mode_tools(builder, &cfg);
    match result {
        Ok(_) => panic!("must reject inline literal token_secret without dev flag"),
        Err(ToolkitError::Validation(ConfigValidationError::InlineSecretRejected)) => {},
        Err(other) => panic!("expected InlineSecretRejected, got {other:?}"),
    }
}

#[test]
fn inline_token_secret_with_dev_flag_passes_register() {
    // R9 dev-flag escape hatch: when the operator explicitly sets
    // `allow_inline_token_secret_for_dev = true`, the inline literal is
    // accepted and `register_code_mode_tools` returns the builder unchanged.
    let toml = r#"
[server]
name = "T"
version = "0.1.0"
[code_mode]
enabled = true
token_secret = "test-secret-bytes-16-or-more-chars"
allow_inline_token_secret_for_dev = true
"#;
    let cfg = ServerConfig::from_toml(toml).expect("parse succeeds");
    let builder = pmcp::Server::builder().name("t").version("0.1.0");
    let result = register_code_mode_tools(builder, &cfg);
    assert!(
        result.is_ok(),
        "dev-flag must accept inline token_secret, got: {:?}",
        result.err().map(|e| e.to_string())
    );
}

#[test]
fn register_code_mode_tools_no_op_when_section_absent() {
    // Tolerant builder-extension behaviour: when `[code_mode]` is absent
    // entirely, `register_code_mode_tools` returns the builder unchanged.
    // This lets Plan 08's `code_mode_from_config(&cfg)` be invoked
    // unconditionally without an `if cfg.code_mode.is_some()` ceremony at
    // every call site.
    let toml = r#"
[server]
name = "T"
version = "0.1.0"
"#;
    let cfg = ServerConfig::from_toml(toml).expect("parse succeeds");
    assert!(cfg.code_mode.is_none());
    let builder = pmcp::Server::builder().name("t").version("0.1.0");
    let result = register_code_mode_tools(builder, &cfg);
    assert!(
        result.is_ok(),
        "no-op path must succeed when [code_mode] is absent"
    );
}
