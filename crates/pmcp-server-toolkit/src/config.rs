// Originated from pmcp-run/built-in/shared/mcp-server-common/src/config.rs
// (https://github.com/guyernest/pmcp-run). Lifted into rust-mcp-sdk for Phase 83.

//! `ServerConfig` + sub-sections. Strict `#[serde(deny_unknown_fields)]` per D-13.
//!
//! # Strict-parse discipline (D-13)
//!
//! Every struct in this module carries `#[serde(deny_unknown_fields)]`. A typo
//! in any key (e.g. `auto_aprove_levels` for `auto_approve_levels`) is a
//! **parse error**, not a silent default. This is the defence-in-depth path
//! against the Tampering threat documented in `83-04-PLAN.md` T-83-04-02 —
//! mis-spelled keys MUST NOT degrade security policy.
//!
//! # REF-01 superset invariant
//!
//! `ServerConfig` is a strict **superset** of every key emitted by the three
//! reference config.tomls (`tests/fixtures/{open-images,imdb,msr-vtt}-config.toml`,
//! lifted in Plan 01 Task 4). When a fixture grows a new key, the toolkit grows
//! a new field — typed if known, `toml::Value` if heterogeneous. The invariant
//! is enforced empirically by the [`tests/reference_configs.rs`] integration
//! test (REF-01 superset, D-13, ROADMAP SC-2).
//!
//! **Anti-pattern (RESEARCH §Pitfall 1, PATTERNS §8):** Do NOT loosen
//! `deny_unknown_fields` to make a fixture parse. Always ADD the missing field.
//!
//! # Three entry points
//!
//! | Method | Returns | Use case |
//! |--------|---------|----------|
//! | [`ServerConfig::from_toml`] | `Result<Self, ToolkitError::Parse>` | Programmatic partial-config merge; no semantic checks |
//! | [`ServerConfig::validate`] | `Result<(), ConfigValidationError>` | Post-parse semantic check (run after a merge) |
//! | [`ServerConfig::from_toml_strict_validated`] | `Result<Self, ToolkitError>` | Production entry: parse + validate in one call |
//!
//! Per Phase 83 review R8, `validate()` exists because the `Default` impls on
//! `ServerSection` etc. would otherwise let `[server]` typos land empty
//! `name`/`version` strings without an error. The strict-validated convenience
//! is what production callers should reach for.
//!
//! REF-01 superset enumeration (from `tests/fixtures/{open-images,imdb,msr-vtt}-config.toml`):
//!
//! ```text
//! [server]            : id, name, description, type, version
//! [metadata]          : display_name, short_description, description, tags, author, visibility
//! [database]          : type, database, output_location, workgroup, query_timeout_ms,
//!                       [[database.tables]], [database.pool]
//! [[database.tables]] : name, description
//! [database.pool]     : max_connections, connection_timeout_seconds
//! [code_mode]         : enabled, server_id, allow_writes, allow_deletes, allow_ddl,
//!                       require_limit, max_limit, blocked_tables, sensitive_columns,
//!                       auto_approve_levels, token_ttl_seconds, token_secret,
//!                       [code_mode.limits]
//! [code_mode.limits]  : max_tables_per_query, max_join_depth, max_subquery_depth
//! [[tools]]           : name, description, sql, ui_resource_uri,
//!                       [[tools.parameters]], [tools.annotations]
//! [[tools.parameters]] : name, type, description, required, default, max_length,
//!                       minimum, maximum, enum
//! [tools.annotations] : read_only_hint, destructive_hint, idempotent_hint,
//!                       open_world_hint, cost_hint
//! [[prompts]]         : name, description, include_resources, arguments
//! [[resources]]       : uri, name, description, mime_type, content
//! ```

use serde::{Deserialize, Serialize};

use crate::error::{ConfigValidationError, Result, ToolkitError};

// -----------------------------------------------------------------------------
// Top-level
// -----------------------------------------------------------------------------

/// Top-level `pmcp-server-toolkit` configuration parsed from a `config.toml`.
///
/// One struct parses the entire file in one shot (per D-13). All sub-sections
/// carry `#[serde(deny_unknown_fields)]` — a typo anywhere in the file is a
/// hard parse error.
///
/// # Entry points
///
/// Use [`ServerConfig::from_toml_strict_validated`] for production callers.
/// [`ServerConfig::from_toml`] is the no-validation variant for programmatic
/// merges; [`ServerConfig::validate`] runs the semantic checks separately.
///
/// # Examples
///
/// ```
/// use pmcp_server_toolkit::config::ServerConfig;
///
/// let toml = r#"
///     [server]
///     name = "demo"
///     version = "0.1.0"
/// "#;
/// let cfg = ServerConfig::from_toml_strict_validated(toml)
///     .expect("valid minimum config");
/// assert_eq!(cfg.server.name, "demo");
/// assert_eq!(cfg.server.version, "0.1.0");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct ServerConfig {
    /// `[server]` — identity and version metadata.
    #[serde(default)]
    pub server: ServerSection,

    /// `[metadata]` — admin-facing display defaults.
    #[serde(default)]
    pub metadata: MetadataSection,

    /// `[database]` — backend connection + tables.
    #[serde(default)]
    pub database: DatabaseSection,

    /// `[code_mode]` (optional) — code-mode policy and limits.
    #[serde(default)]
    pub code_mode: Option<CodeModeSection>,

    /// `[[tools]]` — declarative tool surface (TOML-defined handlers).
    #[serde(default)]
    pub tools: Vec<ToolDecl>,

    /// `[[prompts]]` — declarative prompt surface.
    #[serde(default)]
    pub prompts: Vec<PromptDecl>,

    /// `[[resources]]` — declarative resource surface.
    #[serde(default)]
    pub resources: Vec<ResourceDecl>,
}

impl ServerConfig {
    /// Parse `ServerConfig` from a TOML config string.
    ///
    /// Performs **strict parsing** (`#[serde(deny_unknown_fields)]` on every
    /// section, per D-13). Does **not** run semantic validation — callers
    /// wanting required-field guarantees should use
    /// [`Self::from_toml_strict_validated`] instead.
    ///
    /// # Errors
    ///
    /// Returns [`ToolkitError::Parse`] on syntax error or unknown field. A
    /// mis-spelled key (e.g. `auto_aprove_levels` for `auto_approve_levels`)
    /// produces a parse error here, not a silent default.
    ///
    /// # Example
    ///
    /// ```
    /// use pmcp_server_toolkit::config::ServerConfig;
    ///
    /// let toml = r#"
    ///     [server]
    ///     id = "demo"
    ///     name = "Demo"
    ///     version = "0.1.0"
    /// "#;
    /// let cfg = ServerConfig::from_toml(toml).expect("parse");
    /// assert_eq!(cfg.server.name, "Demo");
    /// ```
    pub fn from_toml(toml_str: &str) -> Result<Self> {
        toml::from_str(toml_str).map_err(ToolkitError::Parse)
    }

    /// Parse + validate. Per Phase 83 review R8 — guards against the
    /// missing-required-value trap that the `Default` impls on sub-sections
    /// would otherwise hide behind silent empty strings (e.g. a typo'd
    /// `[serever]` header makes `server.name` default to `""`).
    ///
    /// # Errors
    ///
    /// Returns [`ToolkitError::Parse`] on TOML syntax / unknown-field errors,
    /// or [`ToolkitError::Validation`] (wrapping
    /// [`ConfigValidationError`]) on missing required values
    /// (empty `server.name`, empty `server.version`, empty tool name, empty
    /// table name).
    ///
    /// # Example
    ///
    /// ```
    /// use pmcp_server_toolkit::config::ServerConfig;
    /// let toml = r#"
    ///     [server]
    ///     name = "demo"
    ///     version = "0.1.0"
    /// "#;
    /// let cfg = ServerConfig::from_toml_strict_validated(toml).expect("valid");
    /// # let _ = cfg;
    /// ```
    pub fn from_toml_strict_validated(toml_str: &str) -> Result<Self> {
        let cfg = Self::from_toml(toml_str)?;
        cfg.validate()?;
        Ok(cfg)
    }

    /// Validate required-field semantics that `#[serde(default)]` would
    /// otherwise mask. Per Phase 83 review R8.
    ///
    /// Rules checked, in order:
    /// 1. `server.name` is non-empty (trimmed).
    /// 2. `server.version` is non-empty (trimmed).
    /// 3. Every `[[tools]]` entry has a non-empty `name`.
    /// 4. Every `[[database.tables]]` entry has a non-empty `name`.
    ///
    /// # Errors
    ///
    /// Returns a [`ConfigValidationError`] variant identifying the
    /// first rule violated. Iteration order matches struct field order.
    pub fn validate(&self) -> std::result::Result<(), ConfigValidationError> {
        if self.server.name.trim().is_empty() {
            return Err(ConfigValidationError::EmptyServerName);
        }
        if self.server.version.trim().is_empty() {
            return Err(ConfigValidationError::EmptyServerVersion);
        }
        for (i, tool) in self.tools.iter().enumerate() {
            if tool.name.trim().is_empty() {
                return Err(ConfigValidationError::EmptyToolName(i));
            }
        }
        for (i, table) in self.database.tables.iter().enumerate() {
            if table.name.trim().is_empty() {
                return Err(ConfigValidationError::EmptyTableName(i));
            }
        }
        Ok(())
    }
}

// -----------------------------------------------------------------------------
// [server]
// -----------------------------------------------------------------------------

/// `[server]` section — identity and version metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct ServerSection {
    /// Stable server identifier (e.g. `"open-images"`). Optional in the TOML;
    /// callers that need it should fall back to deriving from `name`.
    #[serde(default)]
    pub id: Option<String>,
    /// Human-readable server name (required for production via [`ServerConfig::validate`]).
    #[serde(default)]
    pub name: String,
    /// Short server description.
    #[serde(default)]
    pub description: Option<String>,
    /// Server flavour (e.g. `"sql-api"`). Free-form for now; future plans may tighten.
    #[serde(default, rename = "type")]
    pub server_type: Option<String>,
    /// Semver version string (required for production via [`ServerConfig::validate`]).
    #[serde(default)]
    pub version: String,
}

// -----------------------------------------------------------------------------
// [metadata]
// -----------------------------------------------------------------------------

/// `[metadata]` section — admin-facing display defaults (visible in the
/// pmcp.run UI before an operator customises them).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct MetadataSection {
    /// Long-form display name shown in the UI.
    #[serde(default)]
    pub display_name: Option<String>,
    /// One-line summary for list views.
    #[serde(default)]
    pub short_description: Option<String>,
    /// Multi-line description for detail pages.
    #[serde(default)]
    pub description: Option<String>,
    /// Tag list for filtering / discovery.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Server author (organisation or individual).
    #[serde(default)]
    pub author: Option<String>,
    /// Visibility flag (e.g. `"public"`, `"private"`).
    #[serde(default)]
    pub visibility: Option<String>,
}

// -----------------------------------------------------------------------------
// [database]
// -----------------------------------------------------------------------------

/// `[database]` section — backend identification and table catalogue.
///
/// Includes Athena-specific keys (`output_location`, `workgroup`) as optional
/// fields per the REF-01 superset invariant — non-Athena backends omit them.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct DatabaseSection {
    /// Backend type (`"athena"`, `"postgres"`, `"mysql"`, `"sqlite"`, …).
    #[serde(default, rename = "type")]
    pub backend_type: Option<String>,
    /// Database / schema name.
    #[serde(default)]
    pub database: Option<String>,
    /// Athena S3 output location for query results.
    #[serde(default)]
    pub output_location: Option<String>,
    /// Athena workgroup name.
    #[serde(default)]
    pub workgroup: Option<String>,
    /// Per-query timeout in milliseconds.
    #[serde(default)]
    pub query_timeout_ms: Option<u64>,
    /// `[[database.tables]]` — declared table catalogue for schema enrichment.
    #[serde(default)]
    pub tables: Vec<DatabaseTableDecl>,
    /// Connection URL for Postgres / MySQL backends. Supports `env:VAR_NAME`
    /// indirection at the consumer-resolution layer (the toolkit parses the
    /// string as-is and leaves resolution to the per-backend connector or
    /// the secret-resolution machinery from P83 R6/R9). Optional/unused for
    /// Athena (uses `region` + `workgroup` + `output_location`) and SQLite
    /// (uses `database` for the file path or `:memory:` literal).
    #[serde(default)]
    pub url: Option<String>,
    /// `[database.pool]` — connection-pool tuning (optional).
    #[serde(default)]
    pub pool: Option<DatabasePoolSection>,
}

/// Single `[[database.tables]]` entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct DatabaseTableDecl {
    /// Table or view name (required for production via [`ServerConfig::validate`]).
    #[serde(default)]
    pub name: String,
    /// Human-readable table description for schema enrichment.
    #[serde(default)]
    pub description: Option<String>,
}

/// `[database.pool]` connection-pool tuning.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct DatabasePoolSection {
    /// Maximum concurrent connections.
    #[serde(default)]
    pub max_connections: Option<u32>,
    /// Connection-acquisition timeout, in seconds.
    #[serde(default)]
    pub connection_timeout_seconds: Option<u64>,
}

// -----------------------------------------------------------------------------
// [code_mode]
// -----------------------------------------------------------------------------

/// `[code_mode]` section — code-mode policy + complexity limits.
///
/// The toolkit uses **unprefixed** field names (REF-01 invariant); the mapping
/// to `pmcp_code_mode::CodeModeConfig`'s prefixed names (`sql_allow_writes`,
/// etc.) is handled by Plan 06's executor wiring.
#[allow(clippy::struct_excessive_bools)]
// Why: REF-01 superset — these bools mirror the reference servers' [code_mode] block 1:1 (CONTEXT.md D-13). Grouping into a sub-struct would break REF-01.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct CodeModeSection {
    /// Master enable flag for code-mode.
    #[serde(default)]
    pub enabled: bool,
    /// Server identifier used by AVP / Cedar policy resolution.
    #[serde(default)]
    pub server_id: Option<String>,
    /// Whether INSERT / UPDATE / MERGE statements are allowed.
    #[serde(default)]
    pub allow_writes: bool,
    /// Whether DELETE statements are allowed.
    #[serde(default)]
    pub allow_deletes: bool,
    /// Whether DDL (CREATE / ALTER / DROP) is allowed.
    #[serde(default)]
    pub allow_ddl: bool,
    /// Whether `SELECT` queries must declare a `LIMIT`.
    #[serde(default)]
    pub require_limit: bool,
    /// Maximum allowed `LIMIT` value.
    #[serde(default)]
    pub max_limit: Option<u64>,
    /// Table names blocked from any query (denylist).
    #[serde(default)]
    pub blocked_tables: Vec<String>,
    /// `table.column` strings stripped from query output.
    #[serde(default)]
    pub sensitive_columns: Vec<String>,
    /// Risk levels eligible for auto-approval (e.g. `["low"]`).
    #[serde(default)]
    pub auto_approve_levels: Vec<String>,
    /// Token TTL, in seconds, for HMAC-signed approval tokens.
    #[serde(default)]
    pub token_ttl_seconds: Option<u64>,
    /// Secret reference (e.g. `"${CODE_MODE_SECRET}"`) for HMAC signing — resolved
    /// at runtime by `SecretsProvider`. NEVER a raw secret value (review R6 +
    /// T-83-04-04 in the plan threat model).
    #[serde(default)]
    pub token_secret: Option<String>,
    /// Per Phase 83 review R9: inline `token_secret = "raw-string"` is REJECTED
    /// by default to prevent secrets from being committed to source-controlled
    /// configs. Set this flag to `true` ONLY in dev/test configs where the
    /// operator explicitly accepts the risk. NEVER set this in a committed
    /// production config — production must use the `env:VAR_NAME` syntax that
    /// resolves at runtime through `SecretsProvider`.
    #[serde(default)]
    pub allow_inline_token_secret_for_dev: bool,
    /// `[code_mode.limits]` — query-complexity caps.
    #[serde(default)]
    pub limits: Option<CodeModeLimits>,
}

/// `[code_mode.limits]` — query-complexity caps.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct CodeModeLimits {
    /// Maximum number of distinct tables referenced in a single query.
    #[serde(default)]
    pub max_tables_per_query: Option<u32>,
    /// Maximum JOIN nesting depth.
    #[serde(default)]
    pub max_join_depth: Option<u32>,
    /// Maximum subquery nesting depth.
    #[serde(default)]
    pub max_subquery_depth: Option<u32>,
}

// -----------------------------------------------------------------------------
// [[tools]]
// -----------------------------------------------------------------------------

/// Single `[[tools]]` entry — a declaratively-defined tool surface.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct ToolDecl {
    /// Tool name (required for production via [`ServerConfig::validate`]).
    #[serde(default)]
    pub name: String,
    /// Human-readable tool description.
    #[serde(default)]
    pub description: Option<String>,
    /// SQL template (uses `:param` placeholders bound by [`ParamDecl`]).
    #[serde(default)]
    pub sql: Option<String>,
    /// Optional UI-resource URI for `structuredContent` widgets.
    #[serde(default)]
    pub ui_resource_uri: Option<String>,
    /// `[[tools.parameters]]` — declared input parameters.
    #[serde(default)]
    pub parameters: Vec<ParamDecl>,
    /// `[tools.annotations]` — MCP `toolAnnotations`.
    #[serde(default)]
    pub annotations: Option<AnnotationsDecl>,
}

/// Single `[[tools.parameters]]` entry.
///
/// The `default` and `enum` fields use [`toml::Value`] because they are
/// heterogeneous in the reference configs (a `default` may be an integer,
/// a string, or a boolean depending on the parameter type).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct ParamDecl {
    /// Parameter name (the `:param` token used in the tool's `sql`).
    #[serde(default)]
    pub name: String,
    /// JSON-schema type (`"string"`, `"integer"`, `"number"`, `"boolean"`).
    #[serde(default, rename = "type")]
    pub param_type: Option<String>,
    /// Human-readable parameter description.
    #[serde(default)]
    pub description: Option<String>,
    /// Whether the parameter is required.
    #[serde(default)]
    pub required: bool,
    /// Optional default value (any TOML type).
    #[serde(default)]
    pub default: Option<toml::Value>,
    /// Maximum string length (string parameters only).
    #[serde(default)]
    pub max_length: Option<u64>,
    /// Inclusive minimum (integer / number parameters only).
    #[serde(default)]
    pub minimum: Option<f64>,
    /// Inclusive maximum (integer / number parameters only).
    #[serde(default)]
    pub maximum: Option<f64>,
    /// Closed set of allowed values (any TOML scalar).
    #[serde(default, rename = "enum")]
    pub enum_values: Option<Vec<toml::Value>>,
}

/// `[tools.annotations]` — MCP `toolAnnotations` hints.
#[allow(clippy::struct_excessive_bools)] // Why: REF-01 superset — mirrors the MCP `toolAnnotations` flag set 1:1.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct AnnotationsDecl {
    /// Whether the tool only reads (never mutates) state.
    #[serde(default)]
    pub read_only_hint: bool,
    /// Whether the tool may destroy data.
    #[serde(default)]
    pub destructive_hint: bool,
    /// Whether repeated calls with the same args produce the same result.
    #[serde(default)]
    pub idempotent_hint: bool,
    /// Whether the tool interacts with an open-world (external) service.
    #[serde(default)]
    pub open_world_hint: bool,
    /// Cost hint (`"low"`, `"medium"`, `"high"`).
    #[serde(default)]
    pub cost_hint: Option<String>,
}

// -----------------------------------------------------------------------------
// [[prompts]]
// -----------------------------------------------------------------------------

/// Single `[[prompts]]` entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct PromptDecl {
    /// Prompt name (the identifier MCP clients call by).
    #[serde(default)]
    pub name: String,
    /// Human-readable prompt description.
    #[serde(default)]
    pub description: Option<String>,
    /// Resource URIs to include in the prompt's assembled body.
    #[serde(default)]
    pub include_resources: Vec<String>,
    /// Declared prompt arguments (MCP `PromptArgument`).
    #[serde(default)]
    pub arguments: Vec<PromptArgumentDecl>,
}

/// Single argument under `[[prompts.arguments]]`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct PromptArgumentDecl {
    /// Argument name.
    #[serde(default)]
    pub name: String,
    /// Human-readable description.
    #[serde(default)]
    pub description: Option<String>,
    /// Whether the argument is required.
    #[serde(default)]
    pub required: bool,
}

// -----------------------------------------------------------------------------
// [[resources]]
// -----------------------------------------------------------------------------

/// Single `[[resources]]` entry — a statically-shipped resource.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct ResourceDecl {
    /// Resource URI (e.g. `"docs://open-images/schema"`).
    #[serde(default)]
    pub uri: String,
    /// Human-readable resource name.
    #[serde(default)]
    pub name: Option<String>,
    /// Resource description.
    #[serde(default)]
    pub description: Option<String>,
    /// MIME type (e.g. `"text/markdown"`).
    #[serde(default)]
    pub mime_type: Option<String>,
    /// Inline resource content (or `"loaded from path.md"` placeholder string —
    /// the toolkit treats the value verbatim; resolution to filesystem reads
    /// is the caller's responsibility).
    #[serde(default)]
    pub content: Option<String>,
}

// -----------------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    const MINIMAL: &str = r#"
        [server]
        name = "demo"
        version = "0.1.0"
    "#;

    #[test]
    fn parse_minimal_config_succeeds() {
        let cfg = ServerConfig::from_toml(MINIMAL).expect("minimal must parse");
        assert_eq!(cfg.server.name, "demo");
        assert_eq!(cfg.server.version, "0.1.0");
        assert!(cfg.tools.is_empty());
        assert!(cfg.code_mode.is_none());
    }

    #[test]
    fn parse_unknown_field_fails() {
        let toml = r#"
            [server]
            name = "demo"
            version = "0.1.0"
            unknown_field = "x"
        "#;
        let err = ServerConfig::from_toml(toml).expect_err("unknown field must fail");
        assert!(matches!(err, ToolkitError::Parse(_)), "got: {err:?}");
    }

    #[test]
    fn parse_typo_in_code_mode_key_fails() {
        // T-83-04-02: defence-in-depth against silent policy widening.
        let toml = r#"
            [server]
            name = "demo"
            version = "0.1.0"
            [code_mode]
            enabled = true
            auto_aprove_levels = ["low"]
        "#;
        let err = ServerConfig::from_toml(toml).expect_err("typo'd code_mode key must be rejected");
        assert!(matches!(err, ToolkitError::Parse(_)));
    }

    #[test]
    fn code_mode_section_optional() {
        let cfg = ServerConfig::from_toml(MINIMAL).expect("parse");
        assert!(cfg.code_mode.is_none());
    }

    #[test]
    fn validate_accepts_valid_config() {
        let cfg = ServerConfig::from_toml(MINIMAL).expect("parse");
        cfg.validate().expect("minimal config must validate");
    }

    #[test]
    fn validate_rejects_empty_server_name() {
        let toml = r#"
            [server]
            name = ""
            version = "0.1.0"
        "#;
        let cfg = ServerConfig::from_toml(toml).expect("parse");
        match cfg.validate() {
            Err(ConfigValidationError::EmptyServerName) => {},
            other => panic!("expected EmptyServerName, got {other:?}"),
        }
    }

    #[test]
    fn validate_rejects_empty_server_version() {
        let toml = r#"
            [server]
            name = "demo"
            version = ""
        "#;
        let cfg = ServerConfig::from_toml(toml).expect("parse");
        match cfg.validate() {
            Err(ConfigValidationError::EmptyServerVersion) => {},
            other => panic!("expected EmptyServerVersion, got {other:?}"),
        }
    }

    #[test]
    fn validate_rejects_empty_tool_name() {
        let toml = r#"
            [server]
            name = "demo"
            version = "0.1.0"

            [[tools]]
            name = "ok"
            description = "first"

            [[tools]]
            name = ""
            description = "second-is-empty"
        "#;
        let cfg = ServerConfig::from_toml(toml).expect("parse");
        match cfg.validate() {
            Err(ConfigValidationError::EmptyToolName(1)) => {},
            other => panic!("expected EmptyToolName(1), got {other:?}"),
        }
    }

    #[test]
    fn validate_rejects_empty_table_name() {
        let toml = r#"
            [server]
            name = "demo"
            version = "0.1.0"

            [[database.tables]]
            name = ""
            description = "missing-name"
        "#;
        let cfg = ServerConfig::from_toml(toml).expect("parse");
        match cfg.validate() {
            Err(ConfigValidationError::EmptyTableName(0)) => {},
            other => panic!("expected EmptyTableName(0), got {other:?}"),
        }
    }

    #[test]
    fn database_url_optional_field_parses() {
        // Phase 84 CONN-04 / D-08: the additive `[database].url` field parses
        // under `#[serde(deny_unknown_fields)]` and carries the `env:VAR_NAME`
        // indirection string verbatim (resolution happens at the consumer layer).
        let toml = r#"
            [server]
            name = "x"
            version = "0.0.1"

            [database]
            url = "env:DATABASE_URL"
        "#;
        let cfg = ServerConfig::from_toml(toml).expect("config with [database].url must parse");
        assert_eq!(cfg.database.url, Some("env:DATABASE_URL".to_string()));
    }

    #[test]
    fn from_toml_strict_validated_rolls_both_errors() {
        // 1. Parse error path (unknown field).
        let bad_toml = r#"
            [server]
            name = "demo"
            version = "0.1.0"
            nonsense = "x"
        "#;
        let err = ServerConfig::from_toml_strict_validated(bad_toml)
            .expect_err("unknown field must surface");
        assert!(matches!(err, ToolkitError::Parse(_)), "got: {err:?}");

        // 2. Validation error path (empty required value).
        let invalid_toml = r#"
            [server]
            name = ""
            version = "0.1.0"
        "#;
        let err = ServerConfig::from_toml_strict_validated(invalid_toml)
            .expect_err("empty name must surface");
        assert!(
            matches!(
                err,
                ToolkitError::Validation(ConfigValidationError::EmptyServerName)
            ),
            "got: {err:?}"
        );
    }

    proptest! {
        /// TEST-02: any valid `ServerConfig` round-trips through TOML.
        ///
        /// Builds a `ServerConfig` from an arbitrary (but valid) `(name, version)`
        /// pair, serializes it, parses it back, and asserts equality on the
        /// load-bearing scalars.
        #[test]
        fn server_config_minimal_round_trips(
            name in "[a-zA-Z0-9_-]{1,32}",
            version in "[0-9]+\\.[0-9]+\\.[0-9]+",
        ) {
            let cfg = ServerConfig {
                server: ServerSection {
                    name: name.clone(),
                    version: version.clone(),
                    ..Default::default()
                },
                ..Default::default()
            };
            let s = toml::to_string(&cfg).unwrap();
            let parsed = ServerConfig::from_toml(&s).unwrap();
            prop_assert_eq!(parsed.server.name, name);
            prop_assert_eq!(parsed.server.version, version);
        }
    }
}
