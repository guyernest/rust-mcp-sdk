//! Server assembly: `ServerConfig` + connector + `--schema` → built [`pmcp::Server`].
//!
//! This module is the heart of the Shape A binary's "no Rust required" promise.
//! It takes the parsed config, the dispatched [`SqlConnector`], and the
//! operator-supplied `--schema` DDL text and assembles a complete
//! [`pmcp::Server`] — curated `[[tools]]`, the code-mode `validate_code` /
//! `execute_code` pair, the full configured resource surface, and the
//! configured prompts — through the `pmcp-server-toolkit` builder chain alone.
//!
//! # Resource preservation (REVIEW FIX #2)
//!
//! The reference config declares THREE `[[resources]]` — `docs://chinook/schema`,
//! `docs://chinook/examples`, and `code-mode://learnings`. ALL THREE survive
//! assembly. Only the schema resource's `content` is overridden with the
//! `--schema` DDL (prefixed with the same `# Database Schema` header the
//! code-mode prompt seam uses, so prompt + resource parity holds). The other
//! resources pass through verbatim. See [`merge_schema_resource`].
//!
//! # Prompt preservation (REVIEW FIX #3)
//!
//! The configured `start_code_mode` prompt — with its `include_resources` list —
//! is preserved. Prompts are built via
//! [`pmcp_server_toolkit::prompts::StaticPromptHandler::from_configs`], which
//! resolves each prompt's `include_resources` against the MERGED resource
//! handler (so the schema portion of the prompt body reflects the `--schema`
//! content). The toolkit's `from_configs` is the canonical resolution path; the
//! prompt body therefore comes from config, not from a standalone generated
//! body. See [`register_prompts`].
//!
//! # Code-mode (REVIEW FIX #4)
//!
//! Code-mode is wired via the LOCKED, connector-aware
//! [`ServerBuilderExt::try_code_mode_from_config_with_connector`] (Plan 02) so
//! `validate_code` + `execute_code` are actually registered — NOT the
//! connectorless `try_code_mode_from_config`, which registers no tools.

use std::sync::Arc;

// SINGLE crate-root toolkit import (D-15): every toolkit symbol the assembly
// touches resolves from `pmcp_server_toolkit::*` with no module-path
// qualification, the binding witness of the headline DX promise.
use pmcp_server_toolkit::{
    ServerBuilderExt, ServerConfig, SqlConnector, StaticPromptHandler, StaticResourceHandler,
};
// The resource/prompt config shapes are module-pathed because they are NOT part
// of the crate-root re-export surface (D-15 covers the builder chain, not the
// intermediate config structs the merge helper rebuilds).
use pmcp_server_toolkit::prompts::PromptConfig;
use pmcp_server_toolkit::resources::ResourceConfig;

use pmcp::Server;

/// The `# Database Schema` markdown header prefixed onto the `--schema` DDL when
/// it becomes the schema resource's content.
///
/// Kept byte-identical to the header
/// [`pmcp_server_toolkit::code_mode::assemble_code_mode_prompt_with_schema`]
/// folds in (`code_mode.rs` `SCHEMA_HEADER`), so the schema text the client sees
/// through `resources/read` matches what the code-mode prompt seam would
/// produce — prompt + resource parity (REVIEW FIX #2 / D-05).
const SCHEMA_HEADER: &str = "# Database Schema\n\n";

/// Suffix of the schema resource URI that [`merge_schema_resource`] overrides.
///
/// The reference config's schema resource is `docs://chinook/schema`; matching
/// on the `/schema` suffix keeps the merge robust across backends whose schema
/// resource URI uses a different namespace (e.g. `docs://open-images/schema`).
const SCHEMA_URI_SUFFIX: &str = "/schema";

/// Error assembling a [`pmcp::Server`] from config + connector + schema.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum AssembleError {
    /// A toolkit builder step (tool synthesis, code-mode wiring, resource
    /// loading) failed. Carries the toolkit's own error for diagnostics.
    #[error("toolkit assembly step failed: {0}")]
    Toolkit(#[from] pmcp_server_toolkit::ToolkitError),

    /// The final `pmcp::Server::builder().build()` failed (e.g. an internal
    /// pmcp invariant). Carries the pmcp error.
    #[error("pmcp server build failed: {0}")]
    Build(#[from] pmcp::Error),
}

/// Clone the configured `[[resources]]` into [`ResourceConfig`]s, overriding ONLY
/// the schema resource's content with the (header-prefixed) `--schema` DDL.
///
/// REVIEW FIX #2: ALL configured resources are preserved. The resource whose URI
/// ends with [`SCHEMA_URI_SUFFIX`] (e.g. `docs://chinook/schema`) has its
/// `content` replaced with `# Database Schema\n\n` + `schema_ddl`; every other
/// resource (`docs://chinook/examples`, `code-mode://learnings`) passes through
/// with its CONFIGURED content intact.
///
/// If the config declares no schema resource, one is appended so the schema is
/// always reachable — but the reference config has it, so the override path is
/// primary.
#[must_use]
pub fn merge_schema_resource(cfg: &ServerConfig, schema_ddl: &str) -> Vec<ResourceConfig> {
    let merged_schema_content = format!("{SCHEMA_HEADER}{schema_ddl}");

    let mut found_schema = false;
    let mut configs: Vec<ResourceConfig> = cfg
        .resources
        .iter()
        .map(|r| {
            // Scope the override to the SINGLE intended schema resource: only the
            // FIRST `/schema`-suffixed resource is overridden with the --schema
            // DDL. Any subsequent `/schema`-suffixed resource passes through with
            // its configured content (defends against a config declaring more
            // than one such URI — REVIEW Gap 3 follow-up).
            let is_schema = !found_schema && r.uri.ends_with(SCHEMA_URI_SUFFIX);
            if is_schema {
                found_schema = true;
            }
            ResourceConfig {
                uri: r.uri.clone(),
                name: r.name.clone().unwrap_or_else(|| r.uri.clone()),
                description: r.description.clone(),
                mime_type: r
                    .mime_type
                    .clone()
                    .unwrap_or_else(|| "text/markdown".to_string()),
                content: if is_schema {
                    Some(merged_schema_content.clone())
                } else {
                    // Preserve the configured content verbatim. A resource with
                    // no content gets an empty body (mirrors the toolkit's
                    // `From<&ServerConfig>` behaviour).
                    Some(r.content.clone().unwrap_or_default())
                },
                content_file: None,
                meta: None,
            }
        })
        .collect();

    if !found_schema {
        // The config has no schema resource — append one so the --schema DDL is
        // still served (defensive; the reference config always declares it).
        configs.push(ResourceConfig {
            uri: format!("docs://schema{SCHEMA_URI_SUFFIX}"),
            name: "Database Schema".to_string(),
            description: Some("Database schema (DDL) for code-mode SQL generation".to_string()),
            mime_type: "text/markdown".to_string(),
            content: Some(merged_schema_content),
            content_file: None,
            meta: None,
        });
    }

    configs
}

/// URI of the synthesized code-mode instructions resource.
const INSTRUCTIONS_URI: &str = "code-mode://instructions";

/// URI of the synthesized code-mode policies resource.
const POLICIES_URI: &str = "code-mode://policies";

/// Synthesize the `code-mode://instructions` resource from the `[code_mode]`
/// config + the configured backend dialect (REVIEW Gap 3).
///
/// Returns `None` when `[code_mode]` is absent — backward-compatible: with no
/// code-mode there is nothing to instruct, and assembly does NOT synthesize.
///
/// The reference config documents that this resource is "auto-generated from
/// templates at server startup … with `{dialect}` dialect"; this helper is that
/// template. The body explains the `validate_code` → token → `execute_code`
/// flow and names the dialect derived from `cfg.database.backend_type`.
#[must_use]
fn synthesize_instructions_resource(cfg: &ServerConfig) -> Option<ResourceConfig> {
    if cfg.code_mode.is_none() {
        return None;
    }
    let dialect = dialect_label(cfg);
    let body = format!(
        "# Code Mode Instructions — {dialect}\n\n\
         Code Mode generates and runs {dialect} SQL on your behalf. The flow is:\n\n\
         1. Call `validate_code` with your SQL. It is checked against the active\n   \
         policy (see the Code Mode Policies section). On success you receive an\n   \
         approval token.\n\
         2. Call `execute_code` with that token to run the validated SQL and\n   \
         receive the rows.\n\n\
         Generate a single statement per call. Honor the policy: do not attempt\n\
         writes, deletes, or DDL the policy forbids, and include a `LIMIT` when\n\
         the policy requires one.\n"
    );
    Some(ResourceConfig {
        uri: INSTRUCTIONS_URI.to_string(),
        name: "Code Mode Instructions".to_string(),
        description: Some("How to use Code Mode (validate_code → execute_code)".to_string()),
        mime_type: "text/markdown".to_string(),
        content: Some(body),
        content_file: None,
        meta: None,
    })
}

/// Synthesize the `code-mode://policies` resource from the `[code_mode]` config
/// fields (REVIEW Gap 3).
///
/// Returns `None` when `[code_mode]` is absent.
///
/// T-85-09-01: this renders only NON-secret policy settings — `token_secret`
/// is NEVER read or emitted. The body is a deterministic Markdown key/value
/// list so a content assertion can lock specific field text.
#[must_use]
fn synthesize_policies_resource(cfg: &ServerConfig) -> Option<ResourceConfig> {
    let cm = cfg.code_mode.as_ref()?;

    let mut body = String::with_capacity(512);
    body.push_str("# Code Mode Policies\n\n");
    body.push_str("The active authorization policy for generated SQL:\n\n");
    let line = |b: &mut String, k: &str, v: &str| {
        b.push_str("- `");
        b.push_str(k);
        b.push_str("`: ");
        b.push_str(v);
        b.push('\n');
    };
    line(&mut body, "enabled", &cm.enabled.to_string());
    line(&mut body, "allow_writes", &cm.allow_writes.to_string());
    line(&mut body, "allow_deletes", &cm.allow_deletes.to_string());
    line(&mut body, "allow_ddl", &cm.allow_ddl.to_string());
    line(&mut body, "require_limit", &cm.require_limit.to_string());
    line(&mut body, "max_limit", &opt_to_string(cm.max_limit));
    line(
        &mut body,
        "blocked_tables",
        &join_or_none(&cm.blocked_tables),
    );
    line(
        &mut body,
        "sensitive_columns",
        &join_or_none(&cm.sensitive_columns),
    );
    line(
        &mut body,
        "auto_approve_levels",
        &join_or_none(&cm.auto_approve_levels),
    );
    line(
        &mut body,
        "token_ttl_seconds",
        &opt_to_string(cm.token_ttl_seconds),
    );
    if let Some(limits) = &cm.limits {
        body.push_str("\n## Complexity Limits\n\n");
        line(
            &mut body,
            "max_tables_per_query",
            &opt_to_string(limits.max_tables_per_query),
        );
        line(
            &mut body,
            "max_join_depth",
            &opt_to_string(limits.max_join_depth),
        );
        line(
            &mut body,
            "max_subquery_depth",
            &opt_to_string(limits.max_subquery_depth),
        );
    }

    Some(ResourceConfig {
        uri: POLICIES_URI.to_string(),
        name: "Code Mode Policies".to_string(),
        description: Some("Active code-mode authorization policy".to_string()),
        mime_type: "text/markdown".to_string(),
        content: Some(body),
        content_file: None,
        meta: None,
    })
}

/// Human-readable dialect label for the prompt, derived from
/// `cfg.database.backend_type` (e.g. `"sqlite"` → `"SQLite"`). Defaults to
/// `"SQL"` when no backend is declared. For Shape A the backend string is
/// sufficient to label the instructions — no live connector is needed.
fn dialect_label(cfg: &ServerConfig) -> &'static str {
    match cfg.database.backend_type.as_deref() {
        Some("sqlite") => "SQLite",
        Some("postgres") => "PostgreSQL",
        Some("mysql") => "MySQL",
        Some("athena") => "Athena",
        _ => "SQL",
    }
}

/// Render an `Option<u64>`/`Option<u32>` policy value as `"none"` or its number.
fn opt_to_string<T: std::fmt::Display>(value: Option<T>) -> String {
    value.map_or_else(|| "none".to_string(), |v| v.to_string())
}

/// Join a policy string list as a comma-separated value, or `"none"` when empty.
fn join_or_none(values: &[String]) -> String {
    if values.is_empty() {
        "none".to_string()
    } else {
        values.join(", ")
    }
}

/// Assemble the full resource config set the prompt resolves against and the
/// server serves: the schema-merged configured resources PLUS the synthesized
/// `code-mode://instructions` + `code-mode://policies` resources.
///
/// Synthesized resources are appended ONLY when their URI is not already
/// declared by the config — an operator `[[resources]]` block with the same URI
/// WINS (T-85-09-03; dedup-by-exact-URI), matching the reference config's
/// documented override path. With no `[code_mode]`, nothing is synthesized
/// (backward-compatible).
#[must_use]
fn merged_resource_configs(cfg: &ServerConfig, schema_ddl: &str) -> Vec<ResourceConfig> {
    let mut configs = merge_schema_resource(cfg, schema_ddl);
    for synthesized in [
        synthesize_instructions_resource(cfg),
        synthesize_policies_resource(cfg),
    ]
    .into_iter()
    .flatten()
    {
        if !configs.iter().any(|r| r.uri == synthesized.uri) {
            configs.push(synthesized);
        }
    }
    configs
}

/// Build the [`StaticResourceHandler`] from the schema-merged + synthesized
/// resource configs (so both prompt resolution AND the served resource surface
/// see the synthesized code-mode instructions/policies).
///
/// # Errors
///
/// Returns [`AssembleError::Toolkit`] if a resource config is invalid (the
/// merge always supplies `content`, so this is effectively infallible for
/// well-formed configs).
fn build_resource_handler(
    cfg: &ServerConfig,
    schema_ddl: &str,
) -> Result<StaticResourceHandler, AssembleError> {
    let merged = merged_resource_configs(cfg, schema_ddl);
    Ok(StaticResourceHandler::from_configs(&merged)?)
}

/// Map the parsed `[[prompts]]` declarations onto the toolkit's
/// [`PromptConfig`] shape (resolving the `Option<String>` description to the
/// `String` the toolkit expects).
fn prompt_configs(cfg: &ServerConfig) -> Vec<PromptConfig> {
    cfg.prompts
        .iter()
        .map(|p| PromptConfig {
            name: p.name.clone(),
            description: p.description.clone().unwrap_or_default(),
            include_resources: p.include_resources.clone(),
        })
        .collect()
}

/// Register every configured prompt on `builder`, resolving each prompt's
/// `include_resources` against the MERGED resource handler.
///
/// REVIEW FIX #3: prompts come from `cfg.prompts` via
/// [`StaticPromptHandler::from_configs`], NOT a standalone generated body, so
/// the `start_code_mode` prompt's `include_resources` (including the merged
/// schema resource) drive the body. Because the resolution runs against the
/// merged handler, the schema portion of the prompt reflects the `--schema`
/// content.
fn register_prompts(
    mut builder: pmcp::ServerBuilder,
    cfg: &ServerConfig,
    resources: &StaticResourceHandler,
) -> pmcp::ServerBuilder {
    let configs = prompt_configs(cfg);
    for (name, handler) in StaticPromptHandler::from_configs(&configs, resources) {
        builder = builder.prompt_arc(name, Arc::new(handler));
    }
    builder
}

/// Assemble a [`pmcp::Server`] from config + connector + the `--schema` DDL.
///
/// Wires, in order:
/// 1. Curated `[[tools]]` via the connector-aware
///    [`ServerBuilderExt::try_tools_from_config_with_connector`].
/// 2. Code-mode `validate_code` + `execute_code` via the LOCKED, connector-aware
///    [`ServerBuilderExt::try_code_mode_from_config_with_connector`] (REVIEW
///    FIX #4) — a no-op when `[code_mode]` is absent.
/// 3. The full configured resource surface, with ONLY the schema resource's
///    content replaced by the header-prefixed `--schema` DDL (REVIEW FIX #2).
/// 4. The configured prompts, resolved against the merged resources (REVIEW
///    FIX #3).
///
/// # Errors
///
/// Returns [`AssembleError::Toolkit`] if a toolkit step fails (e.g. a
/// `[code_mode]` `token_secret` env var is unset) or [`AssembleError::Build`]
/// if the final `pmcp::Server` build fails.
pub fn build_server(
    cfg: &ServerConfig,
    connector: Arc<dyn SqlConnector>,
    schema_ddl: String,
) -> Result<Server, AssembleError> {
    // Build the merged resource handler ONCE: it backs both the server's
    // resource surface AND the prompt resolution (so the two agree on the
    // schema content the client sees).
    let resources = build_resource_handler(cfg, &schema_ddl)?;

    let builder = Server::builder()
        .name(&cfg.server.name)
        .version(&cfg.server.version)
        .try_tools_from_config_with_connector(cfg, connector.clone())?
        .try_code_mode_from_config_with_connector(cfg, connector)?;

    // Register prompts against the merged resources BEFORE consuming `resources`
    // into the builder's resource handler.
    let builder = register_prompts(builder, cfg, &resources);

    let server = builder.resources_arc(Arc::new(resources)).build()?;
    Ok(server)
}

#[cfg(test)]
mod tests {
    use super::{
        merge_schema_resource, merged_resource_configs, prompt_configs, SCHEMA_HEADER,
        SCHEMA_URI_SUFFIX,
    };
    use pmcp::PromptHandler;
    use pmcp_server_toolkit::{ServerConfig, StaticPromptHandler, StaticResourceHandler};

    fn cfg_with_resources() -> ServerConfig {
        let toml = r#"
[server]
name = "t"
version = "0.1.0"

[[resources]]
uri = "docs://chinook/schema"
name = "Schema"
content = "OLD SCHEMA"

[[resources]]
uri = "docs://chinook/examples"
name = "Examples"
content = "EXAMPLES BODY"

[[resources]]
uri = "code-mode://learnings"
name = "Learnings"
content = "LEARNINGS BODY"
"#;
        ServerConfig::from_toml_strict_validated(toml).expect("parse")
    }

    #[test]
    fn merge_overrides_only_schema_content() {
        let cfg = cfg_with_resources();
        let merged = merge_schema_resource(&cfg, "CREATE TABLE t (id INT);");

        // All three resources preserved.
        assert_eq!(
            merged.len(),
            3,
            "all configured resources must survive merge"
        );

        let schema = merged
            .iter()
            .find(|r| r.uri.ends_with(SCHEMA_URI_SUFFIX))
            .expect("schema resource present");
        let body = schema.content.as_deref().unwrap();
        assert!(
            body.starts_with(SCHEMA_HEADER),
            "schema body carries the header"
        );
        assert!(
            body.contains("CREATE TABLE t"),
            "schema body carries the --schema DDL"
        );
        assert!(
            !body.contains("OLD SCHEMA"),
            "configured schema content is overridden"
        );

        // The other two resources keep their CONFIGURED content.
        let examples = merged
            .iter()
            .find(|r| r.uri == "docs://chinook/examples")
            .expect("examples preserved");
        assert_eq!(examples.content.as_deref(), Some("EXAMPLES BODY"));

        let learnings = merged
            .iter()
            .find(|r| r.uri == "code-mode://learnings")
            .expect("learnings preserved");
        assert_eq!(learnings.content.as_deref(), Some("LEARNINGS BODY"));
    }

    /// A config mirroring the reference `[code_mode]` block + the three declared
    /// resources + the `start_code_mode` prompt whose `include_resources` lists
    /// all five URIs (two of which are template-generated, not declared).
    fn cfg_reference_shaped() -> ServerConfig {
        let toml = r#"
[server]
name = "t"
version = "0.1.0"

[database]
type = "sqlite"

[code_mode]
enabled = true
allow_writes = true
allow_deletes = false
allow_ddl = false
require_limit = true
max_limit = 1000
blocked_tables = []
sensitive_columns = ["Password", "Phone", "Email"]
auto_approve_levels = ["low"]
token_ttl_seconds = 300
token_secret = "${CODE_MODE_SECRET}"

[code_mode.limits]
max_tables_per_query = 5
max_join_depth = 5
max_subquery_depth = 3

[[resources]]
uri = "docs://chinook/schema"
name = "Schema"
content = "OLD SCHEMA"

[[resources]]
uri = "docs://chinook/examples"
name = "Examples"
content = "EXAMPLES BODY"

[[resources]]
uri = "code-mode://learnings"
name = "Learnings"
content = "LEARNINGS BODY"

[[prompts]]
name = "start_code_mode"
description = "Load all context needed for Code Mode SQL generation"
include_resources = [
    "docs://chinook/schema",
    "code-mode://instructions",
    "code-mode://policies",
    "docs://chinook/examples",
    "code-mode://learnings",
]
"#;
        ServerConfig::from_toml_strict_validated(toml).expect("parse")
    }

    /// Resolve the `start_code_mode` prompt body the SAME way the server serves
    /// it: build the merged resource handler, run
    /// `StaticPromptHandler::from_configs` against it, find the named prompt, and
    /// drive its `PromptHandler::handle` to extract the served message text.
    /// This asserts the REAL served body (the private `body` field is exposed
    /// only through `handle`), so the test exercises the production path.
    async fn resolved_prompt_body(cfg: &ServerConfig, schema_ddl: &str, name: &str) -> String {
        let merged = merged_resource_configs(cfg, schema_ddl);
        let handler = StaticResourceHandler::from_configs(&merged).expect("resource handler");
        let (_, prompt) = StaticPromptHandler::from_configs(&prompt_configs(cfg), &handler)
            .into_iter()
            .find(|(n, _)| n == name)
            .expect("prompt present");

        let result = prompt
            .handle(
                std::collections::HashMap::new(),
                pmcp::RequestHandlerExtra::default(),
            )
            .await
            .expect("prompt handle");

        // Join every text content part of the served messages.
        result
            .messages
            .iter()
            .filter_map(|m| match &m.content {
                pmcp::types::Content::Text { text } => Some(text.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[tokio::test]
    async fn prompt_body_carries_synthesized_instructions_and_policies() {
        let cfg = cfg_reference_shaped();
        let body = resolved_prompt_body(&cfg, "CREATE TABLE t (id INT);", "start_code_mode").await;

        // Instructions marker: synthesized from [code_mode] + dialect.
        assert!(
            body.contains("Code Mode Instructions"),
            "prompt body carries the synthesized instructions marker, got:\n{body}"
        );
        assert!(
            body.contains("validate_code") && body.contains("execute_code"),
            "instructions explain the validate -> token -> execute flow"
        );
        assert!(
            body.contains("SQLite"),
            "instructions name the dialect derived from backend_type"
        );

        // Policy text reflecting the config: at least three distinct fields.
        assert!(
            body.contains("Code Mode Policies"),
            "prompt body carries the synthesized policies marker"
        );
        assert!(
            body.contains("require_limit"),
            "policy body lists require_limit"
        );
        assert!(body.contains("max_limit"), "policy body lists max_limit");
        assert!(
            body.contains("allow_writes"),
            "policy body lists allow_writes"
        );
        assert!(
            body.contains("Email"),
            "policy body lists a sensitive column name"
        );

        // T-85-09-01: the HMAC secret reference is NEVER rendered into the body.
        assert!(
            !body.contains("CODE_MODE_SECRET") && !body.contains("token_secret"),
            "policy body MUST NOT leak the token_secret reference"
        );
    }

    #[test]
    fn merged_set_preserves_configured_resources_and_schema_override() {
        let cfg = cfg_reference_shaped();
        let merged = merged_resource_configs(&cfg, "CREATE TABLE t (id INT);");

        // Schema override survives (no regression to Plan 05).
        let schema = merged
            .iter()
            .find(|r| r.uri == "docs://chinook/schema")
            .expect("schema present");
        let schema_body = schema.content.as_deref().unwrap();
        assert!(schema_body.starts_with(SCHEMA_HEADER));
        assert!(schema_body.contains("CREATE TABLE t"));
        assert!(!schema_body.contains("OLD SCHEMA"));

        // The two other configured resources survive verbatim.
        assert!(merged
            .iter()
            .any(|r| r.uri == "docs://chinook/examples"
                && r.content.as_deref() == Some("EXAMPLES BODY")));
        assert!(merged
            .iter()
            .any(|r| r.uri == "code-mode://learnings"
                && r.content.as_deref() == Some("LEARNINGS BODY")));

        // The two synthesized resources are now present.
        assert!(merged.iter().any(|r| r.uri == "code-mode://instructions"));
        assert!(merged.iter().any(|r| r.uri == "code-mode://policies"));
    }

    #[test]
    fn no_code_mode_does_not_synthesize() {
        let cfg = cfg_with_resources(); // no [code_mode] block
        let merged = merged_resource_configs(&cfg, "DDL");
        assert!(!merged.iter().any(|r| r.uri == "code-mode://instructions"));
        assert!(!merged.iter().any(|r| r.uri == "code-mode://policies"));
        // Still resolves whatever URIs it can (backward compatible) — only the
        // three configured resources are present.
        assert_eq!(merged.len(), 3);
    }

    #[test]
    fn operator_override_of_synthesized_uri_wins() {
        // T-85-09-03: a config-declared code-mode://policies wins over synthesis.
        let toml = r#"
[server]
name = "t"
version = "0.1.0"

[database]
type = "sqlite"

[code_mode]
enabled = true
require_limit = true

[[resources]]
uri = "code-mode://policies"
name = "Custom Policies"
content = "OPERATOR-OVERRIDDEN POLICIES"
"#;
        let cfg = ServerConfig::from_toml_strict_validated(toml).expect("parse");
        let merged = merged_resource_configs(&cfg, "DDL");
        let policies: Vec<_> = merged
            .iter()
            .filter(|r| r.uri == "code-mode://policies")
            .collect();
        assert_eq!(policies.len(), 1, "no duplicate policies resource");
        assert_eq!(
            policies[0].content.as_deref(),
            Some("OPERATOR-OVERRIDDEN POLICIES"),
            "the configured resource wins over synthesis"
        );
    }

    #[test]
    fn merge_appends_schema_when_absent() {
        let toml = r#"
[server]
name = "t"
version = "0.1.0"
"#;
        let cfg = ServerConfig::from_toml_strict_validated(toml).expect("parse");
        let merged = merge_schema_resource(&cfg, "DDL");
        assert_eq!(
            merged.len(),
            1,
            "a schema resource is appended when none configured"
        );
        assert!(merged[0].uri.ends_with(SCHEMA_URI_SUFFIX));
        assert!(merged[0].content.as_deref().unwrap().contains("DDL"));
    }
}
