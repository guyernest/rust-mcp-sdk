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
            let is_schema = r.uri.ends_with(SCHEMA_URI_SUFFIX);
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

/// Build the [`StaticResourceHandler`] from the schema-merged resource configs.
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
    let merged = merge_schema_resource(cfg, schema_ddl);
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
    use super::{merge_schema_resource, SCHEMA_HEADER, SCHEMA_URI_SUFFIX};
    use pmcp_server_toolkit::ServerConfig;

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
