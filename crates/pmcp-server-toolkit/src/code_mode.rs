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
    CodeExecutor, CodeModeConfig, ExecutionError, HmacTokenGenerator, NoopPolicyEvaluator,
    PolicyEvaluator, TokenGenerator, TokenSecret, ValidationContext, ValidationPipeline,
};

#[cfg(feature = "avp")]
pub use pmcp_code_mode::{AvpClient, AvpConfig, AvpPolicyEvaluator};

use std::sync::Arc;

use crate::config::{CodeModeSection, ServerConfig};
use crate::error::{ConfigValidationError, Result, ToolkitError};
use crate::secrets::SecretValue;
use crate::sql::SqlConnector;

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
// SHAP-A-01 — SqlCodeExecutor (Plan 85-02 Task 1)
// =============================================================================

/// [`CodeExecutor`] adapter bridging the toolkit's single-method
/// [`SqlConnector`] to the code-mode `validate_code` / `execute_code` flow.
///
/// # Re-derived for the single-method trait
///
/// The production reference (`mcp-sql-server-core::SqlCodeModeHandler`) is
/// written over a 2-method `DatabaseConnector` (`execute_query` /
/// `execute_statement`) and dispatches by [`crate::sql`]'s
/// `QueryType`. The toolkit's [`SqlConnector`] exposes a SINGLE
/// [`SqlConnector::execute`] entry point, so this adapter collapses that
/// 2-method dispatch into one `connector.execute(sql, &[])` call regardless of
/// statement type — re-validating the SQL FIRST for defense-in-depth.
///
/// # Defense-in-depth re-validation (threat T-85-02-01)
///
/// Before touching the connector, [`SqlCodeExecutor::execute`] re-runs the
/// `[code_mode]` policy against the supplied SQL via the same
/// [`ValidationPipeline`] the `validate_code` tool used. The code-mode
/// framework already verified the approval token + code hash before calling
/// this method, but re-validation guards against a token issued for an
/// allowed statement being replayed with a different (e.g. mutating)
/// statement. A policy violation returns `Err(ExecutionError::BackendError)`
/// BEFORE the connector is reached — a config-driven server cannot bypass the
/// write/DDL guards (SC-3, threat T-85-02-02).
///
/// # Observable result shape (REVIEW FIX Codex MEDIUM #6b)
///
/// The production handler returns
/// `{"columns": [...], "rows": [...], "rows_affected": N}` because its
/// 2-method connector surfaces columns + affected-row counts separately. The
/// toolkit's [`SqlConnector::execute`] returns `Vec<Value>` (one JSON object
/// per row, keyed by column name) with no separate columns/rows_affected
/// channel, so this adapter mirrors production's OBSERVABLE `"rows"` key:
/// `{"rows": <values>}`. The parity replay (Plan 06) only exercises
/// `execute_code` with an INVALID token (asserts `failure`), so this success
/// shape is not asserted by `generated.yaml`; mirroring production keeps the
/// executor correct for any future success-path scenario and for the direct
/// unit assertions in this crate.
pub struct SqlCodeExecutor {
    connector: Arc<dyn SqlConnector>,
    config: ServerConfig,
}

impl SqlCodeExecutor {
    /// Construct an executor over `connector`, enforcing the `[code_mode]`
    /// policy carried by `config` on every [`SqlCodeExecutor::execute`] call.
    #[must_use]
    pub fn new(connector: Arc<dyn SqlConnector>, config: ServerConfig) -> Self {
        Self { connector, config }
    }

    /// Defense-in-depth re-validation of `code` against the `[code_mode]`
    /// policy (threat T-85-02-01). Returns `Err` BEFORE any connector call when
    /// the statement violates the static policy (e.g. a DELETE under
    /// `allow_deletes = false`) or fails to parse.
    fn revalidate(&self, code: &str) -> std::result::Result<(), ExecutionError> {
        let pipeline = validation_pipeline_from_config(&self.config).map_err(|e| {
            ExecutionError::BackendError(format!("re-validation pipeline unavailable: {e}"))
        })?;
        let ctx = ValidationContext::new(
            "code-mode-executor",
            "code-mode-session",
            "schema-hash",
            "perms-hash",
        );
        let result = pipeline
            .validate_sql_query(code, &ctx)
            .map_err(|e| ExecutionError::BackendError(format!("SQL validation failed: {e}")))?;
        if !result.is_valid {
            return Err(ExecutionError::BackendError(
                "SQL rejected by [code_mode] policy on re-validation".to_string(),
            ));
        }
        Ok(())
    }
}

#[pmcp_code_mode::async_trait]
impl CodeExecutor for SqlCodeExecutor {
    /// Re-validate the SQL against the `[code_mode]` policy, then execute it via
    /// the single-method [`SqlConnector::execute`].
    ///
    /// # Errors
    ///
    /// Returns [`ExecutionError::BackendError`] when re-validation rejects the
    /// statement (policy violation or parse failure) or when the connector
    /// surfaces a [`crate::sql::ConnectorError`]. Connector error messages are
    /// surfaced verbatim from the toolkit's already-sanitized
    /// `ConnectorError` Display (T-84-01-01 / threat T-85-02-04) — no raw
    /// backend credentials are echoed.
    async fn execute(
        &self,
        code: &str,
        _variables: Option<&serde_json::Value>,
    ) -> std::result::Result<serde_json::Value, ExecutionError> {
        // (1) Defense-in-depth re-validation BEFORE the connector is reached.
        self.revalidate(code)?;
        // (2) Single entry point — the toolkit trait has one execute() method.
        let rows =
            self.connector.execute(code, &[]).await.map_err(|e| {
                ExecutionError::BackendError(format!("connector execute failed: {e}"))
            })?;
        // (3) Mirror production's observable `"rows"` key (REVIEW FIX #6b).
        Ok(serde_json::json!({ "rows": rows }))
    }
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

/// Extract `NAME` from a string of the exact shape `${NAME}`.
///
/// Returns `Some(name)` only when `raw` both starts with `${` and ends with `}`
/// AND `name` is non-empty. A string that merely *contains* `${` (e.g. an
/// Athena `output_location` substring, or a malformed `${` without a closing
/// brace) returns `None`, so it falls through to the existing inline-secret
/// handling (still rejected unless the dev flag is set). This is what scopes
/// `${VAR}` expansion to `token_secret` only and preserves the R9 guarantee
/// (REVIEW FIX #6).
fn expand_braced_var(raw: &str) -> Option<&str> {
    let inner = raw.strip_prefix("${")?.strip_suffix('}')?;
    if inner.is_empty() {
        return None;
    }
    Some(inner)
}

/// Per review R9: `token_secret` is `env:`- or `${VAR}`-only by default. Inline
/// literals are REJECTED at config-validation time unless
/// `allow_inline_token_secret_for_dev` is set. Returns the resolved bytes
/// wrapped in the toolkit-owned [`SecretValue`] (per review R6).
///
/// Accepted forms:
/// - `token_secret = "env:VAR_NAME"` — reads `VAR_NAME` from the process env.
/// - `token_secret = "${VAR_NAME}"` — reads `VAR_NAME` from the process env
///   (the form every reference SQL-API config emits, Plan 85-01 Gap #3).
/// - `token_secret = "raw-string"` — REJECTED unless
///   `allow_inline_token_secret_for_dev = true`.
///
/// A missing/unset env var (either form) returns
/// [`ToolkitError::CodeMode`] — never a panic, never a fall-back to a weak or
/// empty secret (threat-model item T-85-01-01).
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
    if let Some(var) = expand_braced_var(raw) {
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
// TKIT-10 — assemble_code_mode_prompt (D-12 / review R2)
// =============================================================================

/// TKIT-10: assemble the code-mode bootstrap prompt body from a connector's
/// [`SqlConnector::schema_text`] + curated `[[database.tables]]` descriptions.
///
/// Per Phase 83 review R2 (BOTH reviewers HIGH severity), this function calls
/// ONLY [`SqlConnector::schema_text`] — never `execute()`, which is deferred
/// to Phase 84. Dialect-aware placeholder GUIDANCE is included even though
/// `translate_placeholders` is deferred, because the LLM still benefits from
/// knowing the eventual binding shape.
///
/// # Output structure
///
/// ```text
/// # Code Mode — {dialect.name()}
///
/// {dialect.placeholder_guidance()}
///
/// ## Schema
///
/// {connector.schema_text()}
///
/// ## Curated Tables
///
/// - `table_a`: description A
/// - `table_b`: description B
/// ```
///
/// The "Curated Tables" section is omitted entirely when
/// `config.database.tables` is empty OR every entry has no `description`.
/// Entries with `description = None` are skipped individually.
///
/// # Errors
///
/// Returns [`ToolkitError::CodeMode`] if `connector.schema_text()` fails.
/// The toolkit does not retry; callers should ensure the connector is ready
/// before assembling.
///
/// # Example
///
/// ```no_run
/// use pmcp_server_toolkit::code_mode::assemble_code_mode_prompt;
/// use pmcp_server_toolkit::config::ServerConfig;
/// use pmcp_server_toolkit::sql::SqlConnector;
///
/// async fn assemble<C: SqlConnector>(connector: &C, config: &ServerConfig) {
///     let prompt = assemble_code_mode_prompt(connector, config).await.unwrap();
///     assert!(prompt.contains("# Code Mode"));
/// }
/// ```
pub async fn assemble_code_mode_prompt(
    connector: &(dyn SqlConnector + '_),
    config: &ServerConfig,
) -> Result<String> {
    let dialect = connector.dialect();
    let schema_text = connector
        .schema_text()
        .await
        .map_err(|e| ToolkitError::CodeMode(format!("schema_text failed: {e}")))?;

    let curated = format_curated_tables(config);

    let mut out = String::with_capacity(schema_text.len() + curated.len() + 256);
    out.push_str("# Code Mode — ");
    out.push_str(dialect.name());
    out.push_str("\n\n");
    out.push_str(dialect.placeholder_guidance());
    out.push_str("\n\n## Schema\n\n");
    out.push_str(&schema_text);
    if !curated.is_empty() {
        out.push_str("\n\n## Curated Tables\n\n");
        out.push_str(&curated);
    }
    out.push('\n');
    Ok(out)
}

/// Alias for [`assemble_code_mode_prompt`] satisfying CONN-04's literal naming.
///
/// Identical behavior; both names are valid public surface. Per Phase 84 D-12 +
/// RESEARCH §"Open Questions" Q2 / Landmine #15 the recommendation is an
/// alias-next-to (no deprecation attribute on either name), matching the P83
/// dual-naming precedent (`register_code_mode_tools` vs
/// `code_mode_tools_from_executor`).
///
/// # Errors
///
/// Returns [`ToolkitError::CodeMode`] if `connector.schema_text()` fails —
/// surfaced verbatim from [`assemble_code_mode_prompt`].
///
/// # Example
///
/// ```no_run
/// use pmcp_server_toolkit::code_mode::build_code_mode_prompt;
/// use pmcp_server_toolkit::config::ServerConfig;
/// use pmcp_server_toolkit::sql::SqlConnector;
///
/// async fn assemble<C: SqlConnector>(connector: &C, config: &ServerConfig) {
///     let prompt = build_code_mode_prompt(connector, config).await.unwrap();
///     assert!(prompt.contains("# Code Mode"));
/// }
/// ```
pub async fn build_code_mode_prompt(
    connector: &(dyn SqlConnector + '_),
    config: &ServerConfig,
) -> Result<String> {
    assemble_code_mode_prompt(connector, config).await
}

/// Format the `[[database.tables]]` curated descriptions as a Markdown list.
///
/// Entries with no `description` are skipped. Returns an empty string when no
/// described entries exist; callers use that as the signal to omit the whole
/// "Curated Tables" section (keeping the prompt body tight).
fn format_curated_tables(config: &ServerConfig) -> String {
    config
        .database
        .tables
        .iter()
        .filter_map(|t| {
            t.description
                .as_deref()
                .filter(|d| !d.is_empty())
                .map(|d| format!("- `{}`: {}", t.name, d))
        })
        .collect::<Vec<_>>()
        .join("\n")
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

// =============================================================================
// SHAP-A-01 — SqlCodeExecutor unit tests (Plan 85-02 Task 1)
// =============================================================================

#[cfg(all(test, feature = "sqlite"))]
mod sql_code_executor_tests {
    use super::*;
    use crate::config::{CodeModeSection, ServerConfig, ServerSection};
    use crate::sql::SqliteConnector;

    const TEST_SECRET_VAR: &str = "PMCP_TOOLKIT_SQL_EXECUTOR_TEST_SECRET";

    fn ensure_secret() {
        std::env::set_var(TEST_SECRET_VAR, "executor-test-secret-16-or-more");
    }

    /// A read-only `[code_mode]` config (no writes/deletes/DDL) plus an
    /// in-memory SQLite connector seeded with a single `Artist` row.
    async fn read_only_executor() -> SqlCodeExecutor {
        ensure_secret();
        let connector = SqliteConnector::open_in_memory().expect("open in-memory sqlite");
        connector
            .execute(
                "CREATE TABLE Artist (ArtistId INTEGER PRIMARY KEY, Name TEXT)",
                &[],
            )
            .await
            .expect("create table");
        connector
            .execute(
                "INSERT INTO Artist (ArtistId, Name) VALUES (1, 'AC/DC')",
                &[],
            )
            .await
            .expect("seed row");

        let config = ServerConfig {
            server: ServerSection {
                name: "executor-test".to_string(),
                version: "0.1.0".to_string(),
                ..Default::default()
            },
            code_mode: Some(CodeModeSection {
                enabled: true,
                server_id: Some("executor-test".to_string()),
                allow_writes: false,
                allow_deletes: false,
                allow_ddl: false,
                token_secret: Some(format!("env:{TEST_SECRET_VAR}")),
                ..Default::default()
            }),
            ..Default::default()
        };
        SqlCodeExecutor::new(Arc::new(connector), config)
    }

    #[tokio::test]
    async fn read_only_select_returns_rows() {
        let executor = read_only_executor().await;
        let result = executor
            .execute("SELECT ArtistId, Name FROM Artist", None)
            .await
            .expect("read-only SELECT must succeed under a read-only policy");
        // Mirrors production's observable `"rows"` key (REVIEW FIX #6b).
        let rows = result.get("rows").expect("payload has a `rows` key");
        let arr = rows.as_array().expect("`rows` is an array");
        assert_eq!(arr.len(), 1, "one seeded row expected, got {arr:?}");
        assert_eq!(arr[0]["Name"], "AC/DC");
    }

    #[tokio::test]
    async fn delete_rejected_before_connector_under_read_only_policy() {
        // allow_deletes=false → re-validation rejects DELETE BEFORE the
        // connector is reached (threat T-85-02-01 / SC-3).
        let executor = read_only_executor().await;
        let err = executor
            .execute("DELETE FROM Artist WHERE ArtistId = 1", None)
            .await
            .expect_err("DELETE must be rejected when allow_deletes=false");
        assert!(
            matches!(err, ExecutionError::BackendError(_)),
            "expected a policy-rejection BackendError, got {err:?}"
        );
        // The row must still be present — proving the connector was never reached.
        let still_there = executor
            .connector
            .execute("SELECT COUNT(*) AS n FROM Artist", &[])
            .await
            .expect("count query");
        assert_eq!(still_there[0]["n"], 1, "DELETE must not have run");
    }

    #[tokio::test]
    async fn ddl_rejected_under_read_only_policy() {
        // allow_ddl=false → re-validation rejects DROP TABLE.
        let executor = read_only_executor().await;
        let err = executor
            .execute("DROP TABLE Artist", None)
            .await
            .expect_err("DROP must be rejected when allow_ddl=false");
        assert!(matches!(err, ExecutionError::BackendError(_)));
    }

    #[tokio::test]
    async fn malformed_sql_returns_err_never_panics() {
        let executor = read_only_executor().await;
        let result = executor.execute("SELEC nonsense FRM", None).await;
        assert!(
            result.is_err(),
            "malformed SQL must surface an Err, never panic"
        );
    }
}

// =============================================================================
// TKIT-10 — assemble_code_mode_prompt integration tests
// =============================================================================

#[cfg(test)]
mod tkit10_tests {
    use super::*;
    use crate::config::{DatabaseSection, DatabaseTableDecl, ServerConfig, ServerSection};
    use crate::sql::{Dialect, MockSqlConnector};

    fn make_cfg(tables: Vec<DatabaseTableDecl>) -> ServerConfig {
        ServerConfig {
            server: ServerSection {
                name: "test".to_string(),
                version: "0.1.0".to_string(),
                ..Default::default()
            },
            database: DatabaseSection {
                tables,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn assemble_includes_schema_text_and_dialect_name() {
        let connector = MockSqlConnector {
            dialect: Dialect::Postgres,
            schema: "CREATE TABLE users (id SERIAL PRIMARY KEY);".to_string(),
        };
        let cfg = make_cfg(vec![]);
        let prompt = assemble_code_mode_prompt(&connector, &cfg).await.unwrap();
        assert!(
            prompt.contains("# Code Mode — PostgreSQL"),
            "prompt missing dialect header: {prompt}"
        );
        assert!(
            prompt.contains("CREATE TABLE users"),
            "prompt missing schema body: {prompt}"
        );
        assert!(
            prompt.contains("$1"),
            "Postgres guidance should mention $1: {prompt}"
        );
    }

    #[tokio::test]
    async fn assemble_includes_curated_descriptions() {
        let connector = MockSqlConnector {
            dialect: Dialect::Athena,
            schema: "(see Glue catalog)".to_string(),
        };
        let cfg = make_cfg(vec![
            DatabaseTableDecl {
                name: "users".to_string(),
                description: Some("App users".to_string()),
            },
            DatabaseTableDecl {
                name: "orders".to_string(),
                description: Some("Customer orders".to_string()),
            },
        ]);
        let prompt = assemble_code_mode_prompt(&connector, &cfg).await.unwrap();
        assert!(
            prompt.contains("## Curated Tables"),
            "prompt missing curated header: {prompt}"
        );
        assert!(
            prompt.contains("`users`: App users"),
            "prompt missing users description: {prompt}"
        );
        assert!(
            prompt.contains("`orders`: Customer orders"),
            "prompt missing orders description: {prompt}"
        );
        // Athena uses ? placeholders, not $1
        assert!(
            prompt.contains("Amazon Athena"),
            "prompt missing Athena dialect name: {prompt}"
        );
    }

    #[tokio::test]
    async fn assemble_omits_curated_section_when_tables_empty() {
        let connector = MockSqlConnector {
            dialect: Dialect::Sqlite,
            schema: "CREATE TABLE t (id INTEGER PRIMARY KEY);".to_string(),
        };
        let cfg = make_cfg(vec![]);
        let prompt = assemble_code_mode_prompt(&connector, &cfg).await.unwrap();
        assert!(
            !prompt.contains("## Curated Tables"),
            "empty [[database.tables]] must omit curated section: {prompt}"
        );
        assert!(
            prompt.contains("SQLite"),
            "prompt missing SQLite dialect name: {prompt}"
        );
    }

    #[tokio::test]
    async fn assemble_skips_tables_without_descriptions() {
        // A described entry mixed with an undescribed one — only the described
        // row should render. Curated section still emits because at least one
        // row qualifies.
        let connector = MockSqlConnector {
            dialect: Dialect::MySql,
            schema: "CREATE TABLE t (id INT);".to_string(),
        };
        let cfg = make_cfg(vec![
            DatabaseTableDecl {
                name: "with_desc".to_string(),
                description: Some("has description".to_string()),
            },
            DatabaseTableDecl {
                name: "no_desc".to_string(),
                description: None,
            },
        ]);
        let prompt = assemble_code_mode_prompt(&connector, &cfg).await.unwrap();
        assert!(prompt.contains("`with_desc`: has description"));
        assert!(
            !prompt.contains("`no_desc`"),
            "undescribed table must not appear in curated section: {prompt}"
        );
    }
}
