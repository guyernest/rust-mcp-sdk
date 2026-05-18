// Net-new code for Phase 83 TKIT-06 / TKIT-09 (code-mode wiring surface).
//
// Bridges `[code_mode]` config blocks into pmcp-code-mode's `ValidationPipeline`
// + HMAC token machinery, with every public type RE-EXPORTED from pmcp-code-mode
// per D-16 (NO duplicate HMAC / token code per PATTERNS §"Anti-Patterns" #2).
//
// Per Phase 83 review R1, the preflight at
// `.planning/phases/83-toolkit-core-lift-pmcp-server-toolkit/CODE_MODE_API_NOTES.md`
// determined the wiring strategy: **R1 split** —
// `validation_pipeline_from_config(&ServerConfig) -> Result<ValidationPipeline>`
// + `code_mode_tools_from_executor(executor, config) -> Result<...>` — because
// `pmcp-code-mode`'s `CodeExecutor` trait requires backend injection
// (`HttpExecutor`, `SdkExecutor`, `McpExecutor`) and no config-only constructor
// exists.

//! Code-mode wiring: bridges `[code_mode]` config blocks into pmcp-code-mode's
//! validation pipeline + HMAC token machinery, with policy / executor /
//! validation types re-exported verbatim (NO duplicate impl per RESEARCH
//! §"Anti-Patterns" #2).
//!
//! # R1 split (per `CODE_MODE_API_NOTES.md` Section 6)
//!
//! - [`validation_pipeline_from_config`] builds a [`ValidationPipeline`] from a
//!   parsed [`crate::config::ServerConfig`]. This is the entry point Shape A /
//!   Shape C consumers reach for — no per-server Rust glue needed.
//! - [`code_mode_tools_from_executor`] composes a caller-supplied
//!   [`CodeExecutor`] (Plan 08 wires this into `pmcp::ServerBuilder` via
//!   `code_mode_from_config`).
//! - [`register_code_mode_tools`] is the tolerant builder-extension entry
//!   point: a no-op when `[code_mode]` is absent, an R9 enforcement gate when
//!   present.
//!
//! # Security invariants (R6 + R9)
//!
//! - **R6 — toolkit-owned secret type.** `token_secret` resolution flows
//!   through [`crate::secrets::SecretValue`] (feature-independent) and
//!   converts to [`TokenSecret`] via `From` only at the HMAC boundary. This
//!   keeps `--no-default-features` stable.
//! - **R9 — inline-secret rejection.** A `[code_mode] token_secret = "raw"`
//!   literal is REJECTED at validation/resolve time unless the operator
//!   explicitly sets `allow_inline_token_secret_for_dev = true`. Default-deny;
//!   warnings are not protection.

#![cfg(feature = "code-mode")]

// === Re-exports (TKIT-06 + D-16) ===
//
// Every symbol below is a pure re-export of `pmcp_code_mode::*`. Plan 06 ships
// NO duplicate HMAC / token / policy / pipeline code (PATTERNS §"Anti-Patterns"
// #2 — duplicating these would create two copies of a security-critical
// invariant set).
//
// Symbols verified against `crates/pmcp-code-mode/src/lib.rs` per
// CODE_MODE_API_NOTES.md Section 7.

pub use pmcp_code_mode::{
    canonicalize_code, compute_context_hash, hash_code, ApprovalToken, AuthorizationDecision,
    CodeExecutor, CodeModeConfig, HmacTokenGenerator, NoopPolicyEvaluator, PolicyEvaluator,
    TokenGenerator, TokenSecret, ValidationContext, ValidationPipeline,
};

#[cfg(feature = "avp")]
pub use pmcp_code_mode::{AvpClient, AvpConfig, AvpPolicyEvaluator};

use crate::config::{CodeModeSection, ServerConfig};
use crate::error::{ConfigValidationError, Result, ToolkitError};
use crate::secrets::SecretValue;

// =============================================================================
// R1 split — validation_pipeline_from_config + code_mode_tools_from_executor
// =============================================================================

/// Build a [`ValidationPipeline`] from a [`ServerConfig`]'s `[code_mode]` block.
///
/// Maps every reference-server [`CodeModeSection`] field onto
/// [`CodeModeConfig`] per the verified construction surface in
/// `CODE_MODE_API_NOTES.md` Section 2. The pipeline's HMAC token machinery is
/// keyed by the resolved [`TokenSecret`] (derived from a toolkit-owned
/// [`SecretValue`] per review R6).
///
/// Per Phase 83 review R1 — the preflight selected the R1 split because
/// `pmcp-code-mode`'s [`CodeExecutor`] requires backend injection
/// (`HttpExecutor` / `SdkExecutor` / `McpExecutor`); no config-only executor
/// constructor exists. This function delivers the validation surface; the
/// caller supplies the executor (see [`code_mode_tools_from_executor`]).
///
/// # Errors
///
/// - [`ToolkitError::CodeMode`] if `config.code_mode` is `None`.
/// - [`ToolkitError::Validation`] wrapping
///   [`ConfigValidationError::InlineSecretRejected`] when `token_secret` is an
///   inline literal without `allow_inline_token_secret_for_dev` (review R9).
/// - [`ToolkitError::CodeMode`] if the env var referenced by `env:VAR_NAME` is
///   unset, or if the resolved secret is shorter than
///   [`HmacTokenGenerator::MIN_SECRET_LEN`] (16 bytes).
///
/// # Example
///
/// ```no_run
/// use pmcp_server_toolkit::code_mode::validation_pipeline_from_config;
/// use pmcp_server_toolkit::config::ServerConfig;
///
/// // ServerConfig with a [code_mode] block + env:-style token_secret
/// // resolves into a ValidationPipeline ready to validate SQL / GraphQL.
/// let toml = r#"
/// [server]
/// name = "demo"
/// version = "0.1.0"
/// [code_mode]
/// enabled = true
/// token_secret = "env:DEMO_HMAC_SECRET"
/// "#;
/// std::env::set_var("DEMO_HMAC_SECRET", "demo-secret-that-is-long-enough");
/// let cfg = ServerConfig::from_toml_strict_validated(toml).unwrap();
/// let _pipeline = validation_pipeline_from_config(&cfg).unwrap();
/// ```
pub fn validation_pipeline_from_config(config: &ServerConfig) -> Result<ValidationPipeline> {
    let section = config.code_mode.as_ref().ok_or_else(|| {
        ToolkitError::CodeMode("ServerConfig has no [code_mode] block".to_string())
    })?;
    let cm_config = build_cm_config(section);
    let secret_value = resolve_token_secret(section)?;
    let token_secret: TokenSecret = secret_value.into(); // R6 conversion
    ValidationPipeline::from_token_secret(cm_config, &token_secret)
        .map_err(|e| ToolkitError::CodeMode(format!("ValidationPipeline construction failed: {e}")))
}

/// Compose toolkit-side tool registration on top of a caller-supplied
/// [`CodeExecutor`].
///
/// Use this when the underlying execution backend (DB driver, AWS SDK, MCP
/// router) cannot be constructed from `&ServerConfig` alone — the most common
/// production case per `CODE_MODE_API_NOTES.md` Section 1. Plan 08's
/// `ServerBuilderExt::code_mode_from_config` is where the executor is wired
/// into `pmcp::ServerBuilder::tool_arc`. For Plan 06 the helper is shape-
/// preserving: it surfaces the config-driven validation pipeline construction
/// (so R9 errors fire) and returns the executor back to the caller unchanged.
///
/// # Errors
///
/// Surfaces every error from [`validation_pipeline_from_config`] when
/// `config.code_mode.is_some()`. When `config.code_mode.is_none()` the helper
/// is a pass-through (returns the executor) since absence of the block means
/// "code-mode is not configured for this server", which is a legitimate state.
pub fn code_mode_tools_from_executor(
    executor: Box<dyn CodeExecutor>,
    config: &ServerConfig,
) -> Result<Box<dyn CodeExecutor>> {
    if config.code_mode.is_some() {
        // Drive the pipeline construction so R9 (inline-secret) + secret-resolution
        // errors surface here, BEFORE Plan 08 hands the executor over to
        // `pmcp::ServerBuilder::tool_arc`.
        let _pipeline = validation_pipeline_from_config(config)?;
    }
    Ok(executor)
}

/// Tolerant builder-extension entry point for `[code_mode]` config.
///
/// Used by Plan 08's `ServerBuilderExt::code_mode_from_config` to apply
/// `[code_mode]`-driven behaviour to a `pmcp::ServerBuilder`. The function is
/// deliberately tolerant of `config.code_mode = None` (returns the builder
/// unchanged) so callers can invoke it unconditionally — code-mode is opt-in
/// at the config level.
///
/// When `[code_mode]` IS present, this helper drives
/// [`validation_pipeline_from_config`] to surface R9 enforcement errors
/// (inline `token_secret` rejection) before the builder reaches `.build()`.
/// Actual tool registration on the `pmcp::ServerBuilder` lands in Plan 08 once
/// the executor injection contract is fixed.
///
/// # Errors
///
/// Returns every error from [`validation_pipeline_from_config`] when
/// `config.code_mode.is_some()`. No errors when `config.code_mode.is_none()`.
pub fn register_code_mode_tools(
    builder: pmcp::ServerBuilder,
    config: &ServerConfig,
) -> Result<pmcp::ServerBuilder> {
    if config.code_mode.is_none() {
        return Ok(builder); // no-op when block absent
    }
    // R9 enforcement gate — must run BEFORE the builder is returned so that a
    // misconfigured `[code_mode] token_secret = "inline-string"` is caught at
    // builder-time, not at first request.
    let _pipeline = validation_pipeline_from_config(config)?;
    // Plan 08 (`code_mode_from_config` builder extension) will register the
    // validate/execute tools on `builder` here once executor injection is wired.
    // Plan 06's deliverable is the validation pipeline + re-exports; the helper
    // is shape-preserving for now.
    Ok(builder)
}

// =============================================================================
// Helpers (Pattern G — cog ≤25 each, kept small + explicit)
// =============================================================================

/// Translate unprefixed toolkit [`CodeModeSection`] fields into pmcp-code-mode's
/// `sql_`-prefixed [`CodeModeConfig`].
///
/// Mapping is **explicit field-by-field** (PATTERNS §10 + D-13). Silent serde
/// aliasing would couple the toolkit's stable surface to pmcp-code-mode's
/// internal field names — undesirable. Fields on `CodeModeSection` without a
/// `CodeModeConfig` counterpart are noted in inline comments rather than
/// silently dropped (review R1 + threat T-83-06-04).
fn build_cm_config(section: &CodeModeSection) -> CodeModeConfig {
    let mut cfg = CodeModeConfig::default();
    cfg.enabled = section.enabled;
    if let Some(ref sid) = section.server_id {
        cfg.server_id = Some(sid.clone());
    }
    // SQL policy bits — toolkit's unprefixed names → pmcp_code_mode's sql_-prefixed.
    cfg.sql_allow_writes = section.allow_writes;
    cfg.sql_allow_deletes = section.allow_deletes;
    cfg.sql_allow_ddl = section.allow_ddl;
    cfg.sql_blocked_tables = section.blocked_tables.iter().cloned().collect();
    cfg.sql_blocked_columns = section.sensitive_columns.iter().cloned().collect();
    // Token TTL — both sides use seconds, but pmcp_code_mode uses i64 and the
    // toolkit uses Option<u64>. Saturate to i64::MAX rather than wrap.
    if let Some(ttl) = section.token_ttl_seconds {
        cfg.token_ttl_seconds = i64::try_from(ttl).unwrap_or(i64::MAX);
    }
    // Auto-approval — toolkit ships risk-level names as strings; the
    // pmcp_code_mode side wants RiskLevel enums. Best-effort parse; unrecognised
    // entries are silently skipped (operator typos surface as "nothing auto-
    // approved" rather than a parse error — by design, since the registry is
    // open-ended).
    map_auto_approve_levels(&section.auto_approve_levels, &mut cfg);
    // `max_limit` (toolkit) corresponds to `sql_max_rows` (pmcp_code_mode).
    if let Some(max) = section.max_limit {
        cfg.sql_max_rows = max;
    }
    // `require_limit` (toolkit) — pmcp_code_mode has no direct equivalent; the
    // closest semantically is enforced via `sql_max_rows`. Documented gap; not
    // silently dropped.
    let _require_limit_gap = section.require_limit;
    // [code_mode.limits] — pmcp_code_mode's CodeModeConfig has `max_depth` and
    // `max_field_count` (GraphQL-flavoured) but no direct counterparts for
    // `max_tables_per_query` / `max_join_depth` / `max_subquery_depth`. These
    // toolkit fields are exposed for forward compatibility with Phase 84's
    // SQL connector enforcement; they are NOT silently mapped here.
    if let Some(ref limits) = section.limits {
        let _gap_max_tables = limits.max_tables_per_query;
        let _gap_max_join = limits.max_join_depth;
        let _gap_max_subquery = limits.max_subquery_depth;
    }
    cfg
}

/// Decompose auto-approve-level parsing to keep [`build_cm_config`] under
/// Pattern G's cog ≤25 budget.
fn map_auto_approve_levels(levels: &[String], cfg: &mut CodeModeConfig) {
    use pmcp_code_mode::RiskLevel;
    let mut out = Vec::with_capacity(levels.len());
    for level in levels {
        match level.to_ascii_lowercase().as_str() {
            "low" => out.push(RiskLevel::Low),
            "medium" => out.push(RiskLevel::Medium),
            "high" => out.push(RiskLevel::High),
            "critical" => out.push(RiskLevel::Critical),
            _ => {
                tracing::debug!(
                    target: "pmcp_server_toolkit::code_mode",
                    "[code_mode] auto_approve_levels: unrecognised level '{}' — skipping",
                    level
                );
            },
        }
    }
    if !out.is_empty() {
        cfg.auto_approve_levels = out;
    }
}

/// Per review R9: `token_secret` is `env:`-only by default. Inline literals
/// are REJECTED at config-validation time unless
/// `allow_inline_token_secret_for_dev` is set. Returns the resolved bytes
/// wrapped in the toolkit-owned [`SecretValue`] (per review R6).
///
/// Accepted forms:
/// - `token_secret = "env:VAR_NAME"` — reads `VAR_NAME` from the process env.
/// - `token_secret = "raw-string"` — REJECTED unless
///   `allow_inline_token_secret_for_dev = true`.
fn resolve_token_secret(section: &CodeModeSection) -> Result<SecretValue> {
    let raw = section.token_secret.as_ref().ok_or_else(|| {
        ToolkitError::CodeMode(
            "[code_mode] token_secret is required when code-mode is enabled".to_string(),
        )
    })?;
    if let Some(var) = raw.strip_prefix("env:") {
        return std::env::var(var)
            .map(|s| SecretValue::new(s.into_bytes()))
            .map_err(|_| {
                ToolkitError::CodeMode(format!("env var '{var}' not set for token_secret"))
            });
    }
    if section.allow_inline_token_secret_for_dev {
        tracing::warn!(
            target: "pmcp_server_toolkit::code_mode",
            "[code_mode] token_secret is inline AND allow_inline_token_secret_for_dev=true; \
             accepting under dev/test exception — NEVER set this flag in a committed \
             production config"
        );
        return Ok(SecretValue::new(raw.as_bytes().to_vec()));
    }
    Err(ToolkitError::Validation(
        ConfigValidationError::InlineSecretRejected,
    ))
}

// =============================================================================
// Unit tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{CodeModeLimits, CodeModeSection};

    /// Compile-only assertion that the headline re-exports resolve at the
    /// `code_mode::*` path (TKIT-06 + D-16 + R3).
    #[allow(dead_code)]
    const _RE_EXPORTS_COMPILE: fn() = || {
        let _: Option<Box<dyn CodeExecutor>> = None;
        let _: Option<Box<dyn PolicyEvaluator>> = None;
        let _: Option<ApprovalToken> = None;
        let _: Option<HmacTokenGenerator> = None;
        let _: Option<TokenSecret> = None;
        let _: Option<NoopPolicyEvaluator> = None;
        let _: Option<ValidationPipeline> = None;
        let _: Option<ValidationContext> = None;
        let _: Option<CodeModeConfig> = None;
        let _: Option<AuthorizationDecision> = None;
        let _hash = canonicalize_code;
        let _ctx = compute_context_hash;
        let _h = hash_code;
    };

    /// Lightweight test fixture: a `CodeModeSection` with all required fields
    /// populated for env-style secret resolution.
    fn env_section(var: &str) -> CodeModeSection {
        CodeModeSection {
            enabled: true,
            server_id: Some("test-server".to_string()),
            allow_writes: false,
            allow_deletes: false,
            allow_ddl: false,
            require_limit: false,
            max_limit: Some(1000),
            blocked_tables: vec![],
            sensitive_columns: vec![],
            auto_approve_levels: vec!["low".to_string()],
            token_ttl_seconds: Some(300),
            token_secret: Some(format!("env:{var}")),
            allow_inline_token_secret_for_dev: false,
            limits: Some(CodeModeLimits {
                max_tables_per_query: Some(5),
                max_join_depth: Some(3),
                max_subquery_depth: Some(2),
            }),
        }
    }

    #[test]
    fn build_cm_config_maps_allow_writes() {
        let mut section = env_section("UNUSED");
        section.allow_writes = true;
        let cfg = build_cm_config(&section);
        assert!(
            cfg.sql_allow_writes,
            "unprefixed allow_writes=true must map to sql_allow_writes=true"
        );
        assert!(cfg.enabled);
        assert_eq!(cfg.server_id.as_deref(), Some("test-server"));
        // max_limit → sql_max_rows
        assert_eq!(cfg.sql_max_rows, 1000);
        // token_ttl_seconds → i64
        assert_eq!(cfg.token_ttl_seconds, 300);
    }

    #[test]
    fn build_cm_config_propagates_blocked_tables() {
        let mut section = env_section("UNUSED");
        section.blocked_tables = vec!["users".into(), "secrets".into()];
        section.sensitive_columns = vec!["users.password".into()];
        let cfg = build_cm_config(&section);
        assert!(cfg.sql_blocked_tables.contains("users"));
        assert!(cfg.sql_blocked_tables.contains("secrets"));
        assert!(cfg.sql_blocked_columns.contains("users.password"));
    }

    #[test]
    fn resolve_token_secret_env_reference_succeeds() {
        const VAR: &str = "PMCP_TOOLKIT_CODE_MODE_TEST_RESOLVE_ENV";
        // Long enough to satisfy HmacTokenGenerator::MIN_SECRET_LEN (16 bytes).
        std::env::set_var(VAR, "a-test-secret-bytes-16-or-more");
        let section = env_section(VAR);
        let resolved = resolve_token_secret(&section).expect("env resolution must succeed");
        assert_eq!(resolved.expose_secret(), b"a-test-secret-bytes-16-or-more");
        std::env::remove_var(VAR);
    }

    #[test]
    fn resolve_token_secret_inline_without_dev_flag_rejected() {
        // R9 — inline literal + flag absent → InlineSecretRejected.
        let mut section = env_section("UNUSED");
        section.token_secret = Some("raw-string-that-should-be-rejected".to_string());
        section.allow_inline_token_secret_for_dev = false;
        // SecretValue intentionally does not implement Debug (R5 invariant),
        // so we cannot use `expect_err` directly on Result<SecretValue, _>.
        match resolve_token_secret(&section) {
            Ok(_) => panic!("must reject inline literal"),
            Err(ToolkitError::Validation(ConfigValidationError::InlineSecretRejected)) => {},
            Err(other) => panic!("expected InlineSecretRejected, got {other:?}"),
        }
    }

    #[test]
    fn resolve_token_secret_inline_with_dev_flag_accepted() {
        // R9 — inline literal + dev flag → accepted (with tracing::warn).
        let mut section = env_section("UNUSED");
        section.token_secret = Some("a-test-secret-bytes-16-or-more".to_string());
        section.allow_inline_token_secret_for_dev = true;
        let resolved = resolve_token_secret(&section).expect("dev flag must permit inline literal");
        assert_eq!(resolved.expose_secret(), b"a-test-secret-bytes-16-or-more");
    }

    #[test]
    fn resolve_token_secret_missing_env_var_surfaces_error() {
        // Use a var name that is overwhelmingly unlikely to be set in CI.
        let section = env_section("PMCP_TOOLKIT_DEFINITELY_NOT_SET_FOR_TEST");
        // SecretValue has no Debug — pattern-match instead of expect_err.
        match resolve_token_secret(&section) {
            Ok(_) => panic!("missing env var must error"),
            Err(ToolkitError::CodeMode(msg)) => {
                assert!(
                    msg.contains("PMCP_TOOLKIT_DEFINITELY_NOT_SET_FOR_TEST"),
                    "error message must name the missing env var, got: {msg}"
                );
            },
            Err(other) => panic!("expected CodeMode error, got {other:?}"),
        }
    }
}
