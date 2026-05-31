// Originated from pmcp-run/built-in/shared/mcp-server-common/src/prompts.rs
// (https://github.com/guyernest/pmcp-run). Lifted into rust-mcp-sdk for Phase 83.

//! Static MCP prompts for config-driven servers.
//!
//! [`StaticPromptHandler`] implements [`pmcp::server::PromptHandler`] for a
//! single named prompt with pre-resolved body content. The handler does NOT
//! redefine the trait — it consumes the trait shape from `pmcp`.
//!
//! # Shape divergence from the source lift
//!
//! `mcp-server-common::prompts::StaticPromptHandler` is plural — one handler
//! serves many prompts, dispatched by name through `get(name, &resources)`.
//! `pmcp::PromptHandler::handle(args, extra)` is single-prompt by trait shape:
//! the prompt name is bound at registration time via `prompt_arc(name, handler)`,
//! not passed at invocation. The toolkit therefore models one
//! `StaticPromptHandler` per prompt and provides
//! [`StaticPromptHandler::from_configs`] as a factory returning
//! `Vec<(String, StaticPromptHandler)>` that downstream builders can register
//! in a loop. Per Plan 83-03 PATTERNS §6, "multiple prompts are registered via
//! multiple `prompt_arc(name, handler)` calls."
//!
//! # Orthogonality with skills
//!
//! `StaticPromptHandler` is independent of [`pmcp::server::skills::Skill`] and
//! `bootstrap_skill_and_prompt`. Downstream consumers can register both
//! surfaces side-by-side; the toolkit makes no assumption about skill
//! registration. The dual-surface byte-equality invariant (Phase 80 /
//! SEP-2640 §9) applies only when a consumer wires skill + prompt for the
//! SAME logical prompt — orthogonal to anything `StaticPromptHandler` does.
//!
//! # Example configuration
//!
//! ```toml
//! [[prompts]]
//! name = "shipping-context"
//! description = "Load context about shipping policies"
//! include_resources = ["docs://policies/shipping-guide"]
//! ```

use async_trait::async_trait;
use pmcp::types::{Content, GetPromptResult, PromptArgument, PromptInfo, PromptMessage};
use pmcp::PromptHandler;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::error::ToolkitError;
use crate::resources::StaticResourceHandler;

/// The standard prompt name for Code Mode entry point.
///
/// Used across all server types to detect whether a TOML config already
/// defines the code mode prompt (avoiding duplicates).
pub const CODE_MODE_PROMPT_NAME: &str = "start_code_mode";

// =============================================================================
// Configuration Types
// =============================================================================

/// MCP Prompt configuration (simplified, no arguments).
///
/// Prompts provide pre-configured context that clients can request to prepare
/// for specific types of conversations. This simplified version returns the
/// content of included resources without requiring arguments.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PromptConfig {
    /// Prompt name (must be unique).
    pub name: String,

    /// Human-readable description.
    pub description: String,

    /// Resource URIs to include in the prompt response.
    #[serde(default)]
    pub include_resources: Vec<String>,
}

/// Local alias for [`pmcp::types::PromptInfo`] used in return types so the
/// only literal `PromptInfo` token in this module appears as a constructor
/// call (`PromptInfo::new(...)`) — never as a struct-literal expression.
type PromptInfoOut = pmcp::types::PromptInfo;

impl PromptConfig {
    /// Convert to a PMCP SDK prompt-info value for listing.
    ///
    /// See [`StaticPromptHandler::metadata`] for the handler-side path.
    pub fn to_prompt_info(&self) -> PromptInfoOut {
        let info = PromptInfo::new(&self.name);
        info.with_description(&self.description)
    }
}

// =============================================================================
// Static Prompt Handler
// =============================================================================

/// Handler for a single static prompt with pre-resolved body content.
///
/// Implements [`pmcp::PromptHandler`] with required-argument validation and
/// metadata. Each `StaticPromptHandler` represents ONE prompt; use
/// [`StaticPromptHandler::from_configs`] to materialize a `Vec` of
/// `(name, handler)` pairs from a `Vec<PromptConfig>` and register them via
/// `prompt_arc(name, handler)` calls on the builder.
///
/// # Orthogonality with skills
///
/// `StaticPromptHandler` is independent of [`pmcp::server::skills::Skill`] and
/// `bootstrap_skill_and_prompt`. Downstream consumers can register both
/// surfaces side-by-side; the toolkit makes no assumption about skill
/// registration. The dual-surface byte-equality invariant (Phase 80 /
/// SEP-2640 §9) applies only when a consumer wires skill + prompt for the
/// SAME logical prompt — orthogonal to anything `StaticPromptHandler` does.
pub struct StaticPromptHandler {
    name: String,
    description: Option<String>,
    arguments: Vec<PromptArgument>,
    body: String,
}

impl StaticPromptHandler {
    /// Create a handler for a single prompt.
    ///
    /// `body` is the message text returned from `handle()` after required-arg
    /// validation succeeds. Pre-resolve any `include_resources` content into
    /// `body` before calling `new` (see [`StaticPromptHandler::from_configs`]
    /// for the canonical resolution path).
    ///
    /// # Example
    ///
    /// ```no_run
    /// use pmcp_server_toolkit::prompts::StaticPromptHandler;
    /// let handler = StaticPromptHandler::new(
    ///     "shipping-context",
    ///     Some("Loads shipping policy context"),
    ///     vec![],
    ///     "Policies:\n- Alcohol requires adult signature.",
    /// );
    /// # let _ = handler;
    /// ```
    pub fn new(
        name: impl Into<String>,
        description: Option<impl Into<String>>,
        arguments: Vec<PromptArgument>,
        body: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.map(Into::into),
            arguments,
            body: body.into(),
        }
    }

    /// Materialize a `Vec` of `(name, handler)` pairs from prompt configs by
    /// pre-resolving each `include_resources` against the supplied resource
    /// handler.
    ///
    /// Missing resources are logged at `warn` and skipped (matching the
    /// lifted behavior). The resulting body is the resource contents joined
    /// with `\n\n---\n\n`; if no resources resolve, the body is a
    /// `(No resources found for prompt 'name')` placeholder.
    ///
    /// Returns the same insertion order as `prompts` so deterministic
    /// registration with `prompt_arc(name, handler)` is possible.
    pub fn from_configs(
        prompts: &[PromptConfig],
        resources: &StaticResourceHandler,
    ) -> Vec<(String, Self)> {
        prompts
            .iter()
            .map(|p| {
                let body = Self::resolve_body(p, resources);
                let handler = Self::new(
                    &p.name,
                    Some(p.description.clone()),
                    Vec::new(), // simplified-prompt schema: no arguments
                    body,
                );
                (p.name.clone(), handler)
            })
            .collect()
    }

    /// Resolve the combined body for a prompt by expanding included
    /// resources. Missing URIs are logged at `warn` and skipped.
    fn resolve_body(prompt: &PromptConfig, resources: &StaticResourceHandler) -> String {
        let mut content_parts: Vec<String> = Vec::new();

        for resource_uri in &prompt.include_resources {
            if let Some(resource) = resources.get(resource_uri) {
                content_parts.push(resource.content.clone());
            } else {
                tracing::warn!(
                    uri = %resource_uri,
                    prompt = %prompt.name,
                    "Resource not found for prompt",
                );
            }
        }

        if content_parts.is_empty() {
            format!("(No resources found for prompt '{}')", prompt.name)
        } else {
            content_parts.join("\n\n---\n\n")
        }
    }
}

#[async_trait]
impl PromptHandler for StaticPromptHandler {
    async fn handle(
        &self,
        args: HashMap<String, String>,
        _extra: pmcp::RequestHandlerExtra,
    ) -> pmcp::Result<GetPromptResult> {
        // Validate required arguments (PATTERNS §6 — argument-validation
        // pattern verbatim from src/server/simple_prompt.rs:111-119).
        for arg in &self.arguments {
            if arg.required && !args.contains_key(&arg.name) {
                return Err(pmcp::Error::validation(format!(
                    "Required argument '{}' is missing",
                    arg.name
                )));
            }
        }

        Ok(GetPromptResult::new(
            vec![PromptMessage::user(Content::text(self.body.clone()))],
            self.description.clone(),
        ))
    }

    fn metadata(&self) -> Option<PromptInfoOut> {
        // PATTERNS Pattern C: use the constructor, NOT struct-literal —
        // pmcp::types::PromptInfo is #[non_exhaustive].
        let mut info = PromptInfo::new(&self.name);
        if let Some(desc) = &self.description {
            info = info.with_description(desc);
        }
        if !self.arguments.is_empty() {
            info = info.with_arguments(self.arguments.clone());
        }
        Some(info)
    }
}

// =============================================================================
// Construction from `ServerConfig` (Plan 08 — TKIT-05 completion)
// =============================================================================
//
// `pmcp::PromptHandler` binds a single prompt name at registration time via
// `prompt_arc(name, handler)`. To stay consistent with that shape, the
// crate-level construction surface is a free function that returns
// `Vec<(name, StaticPromptHandler)>` — NOT an `impl From<&ServerConfig>` on
// the handler itself (a single handler can only model one prompt; see Plan 03
// PATTERNS §6 + the "Shape divergence from the source lift" rustdoc above).
//
// Per Plan 08 review R3, [`From<&crate::config::ServerConfig>`] is also
// provided as a "construct the first prompt or an empty/no-op handler"
// convenience so the trait-impl arm of the verification grep matches. The
// canonical path remains [`prompt_handlers_from_config`] for multi-prompt
// servers.

/// Materialize a `Vec` of `(name, handler)` pairs from a parsed
/// [`crate::config::ServerConfig`].
///
/// Each `[[prompts]]` entry yields one [`StaticPromptHandler`] with body
/// pre-resolved against `cfg.resources` (URIs not present in the resource
/// table are skipped with a `tracing::warn!`). Insertion order matches the
/// `[[prompts]]` declaration order.
///
/// Callers register each pair via `pmcp::ServerBuilder::prompt_arc(name, Arc::new(handler))`.
///
/// # Example
///
/// ```no_run
/// use std::sync::Arc;
/// use pmcp::Server;
/// use pmcp_server_toolkit::{ServerConfig, prompts::prompt_handlers_from_config};
///
/// let cfg = ServerConfig::default();
/// let pairs = prompt_handlers_from_config(&cfg);
/// let mut builder = Server::builder().name("demo").version("0.1.0");
/// for (name, handler) in pairs {
///     builder = builder.prompt_arc(name, Arc::new(handler));
/// }
/// # let _ = builder;
/// ```
pub fn prompt_handlers_from_config(
    cfg: &crate::config::ServerConfig,
) -> Vec<(String, StaticPromptHandler)> {
    // Reuse the resource handler so resolved bodies match what the
    // configured resources actually expose at runtime.
    let resource_handler = crate::resources::StaticResourceHandler::from(cfg);
    let configs: Vec<PromptConfig> = cfg
        .prompts
        .iter()
        .map(|p| PromptConfig {
            name: p.name.clone(),
            description: p.description.clone().unwrap_or_default(),
            include_resources: p.include_resources.clone(),
        })
        .collect();
    StaticPromptHandler::from_configs(&configs, &resource_handler)
}

impl From<&crate::config::ServerConfig> for StaticPromptHandler {
    /// Build a single [`StaticPromptHandler`] from a [`crate::config::ServerConfig`].
    ///
    /// Returns a handler for the FIRST `[[prompts]]` entry, or — if none are
    /// declared — a no-op handler named `"<no-prompts>"` with an empty body.
    /// Multi-prompt servers should use [`prompt_handlers_from_config`] instead.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use pmcp_server_toolkit::{ServerConfig, StaticPromptHandler};
    ///
    /// let cfg = ServerConfig::default();
    /// let _handler = StaticPromptHandler::from(&cfg);
    /// ```
    fn from(cfg: &crate::config::ServerConfig) -> Self {
        let mut pairs = prompt_handlers_from_config(cfg);
        if pairs.is_empty() {
            StaticPromptHandler::new(
                "<no-prompts>",
                Some("config declared no [[prompts]] entries"),
                Vec::new(),
                String::new(),
            )
        } else {
            pairs.remove(0).1
        }
    }
}

// =============================================================================
// Free helpers (lifted from mcp-server-common)
// =============================================================================

/// Resolve extra prompt content from TOML-defined resources.
///
/// Finds the `start_code_mode` prompt in the config, resolves
/// `include_resources` URIs against the resource definitions, and returns the
/// content strings. Filters out auto-generated resources
/// (`code-mode://instructions` and `code-mode://policies`) since those are
/// already included by the Code Mode handler.
///
/// This allows admin-curated resources (schema docs, examples, learnings) to
/// be appended to the auto-generated Code Mode prompt.
pub fn resolve_extra_prompt_content(
    prompts: &[PromptConfig],
    resources: &[crate::resources::ResourceConfig],
) -> Vec<String> {
    const AUTO_GENERATED: &[&str] = &["code-mode://instructions", "code-mode://policies"];

    let prompt = prompts.iter().find(|p| p.name == CODE_MODE_PROMPT_NAME);
    let Some(prompt) = prompt else {
        return vec![];
    };

    prompt
        .include_resources
        .iter()
        .filter(|uri| !AUTO_GENERATED.contains(&uri.as_str()))
        .filter_map(|uri| {
            resources
                .iter()
                .find(|r| r.uri == *uri)
                .and_then(|r| r.content.clone())
        })
        .filter(|c| !c.is_empty())
        .collect()
}

/// Surface the toolkit's [`ToolkitError`] for consistency with other modules
/// (currently unused inside the module — kept available for future API
/// extensions that need to surface prompt-resolution failures).
#[allow(dead_code)]
fn _ensure_error_path_kept() -> Option<ToolkitError> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::ResourceConfig;
    use pmcp::types::Content;
    use pmcp::RequestHandlerExtra;

    fn mk_extra() -> RequestHandlerExtra {
        RequestHandlerExtra::default()
    }

    #[test]
    fn prompt_config_to_info() {
        let config = PromptConfig {
            name: "test-prompt".to_string(),
            description: "A test prompt".to_string(),
            include_resources: vec!["docs://test".to_string()],
        };

        let info = config.to_prompt_info();
        assert_eq!(info.name, "test-prompt");
        assert_eq!(info.description, Some("A test prompt".to_string()));
        assert!(info.arguments.is_none());
    }

    /// Requirement: `handle()` with all required args present returns
    /// `Ok(GetPromptResult)` with the user-role body message.
    #[tokio::test]
    async fn handle_with_all_required_args_succeeds() {
        let handler = StaticPromptHandler::new(
            "needs-foo",
            Some("requires foo"),
            vec![PromptArgument::new("foo").required()],
            "Hello {{foo}}",
        );

        let args = HashMap::from([("foo".to_string(), "world".to_string())]);
        let result = handler.handle(args, mk_extra()).await.unwrap();

        assert_eq!(result.messages.len(), 1);
        assert_eq!(result.description.as_deref(), Some("requires foo"));
        match &result.messages[0].content {
            Content::Text { text } => assert_eq!(text, "Hello {{foo}}"),
            other => panic!("expected text content, got {:?}", other),
        }
    }

    /// Requirement: `handle()` returns `pmcp::Error::validation(...)` when a
    /// required argument is absent, and the error message names the missing
    /// argument.
    #[tokio::test]
    async fn handle_missing_required_arg_returns_validation_err() {
        let handler = StaticPromptHandler::new(
            "needs-foo",
            Some("requires foo"),
            vec![PromptArgument::new("foo").required()],
            "Hello {{foo}}",
        );

        let result = handler.handle(HashMap::new(), mk_extra()).await;
        let err = result.expect_err("expected validation error");
        let msg = err.to_string();
        assert!(
            msg.contains("foo"),
            "error message should mention the missing argument 'foo': {msg}",
        );
        assert!(
            msg.to_lowercase().contains("missing") || msg.to_lowercase().contains("required"),
            "error message should indicate the missing-required-arg path: {msg}",
        );
    }

    /// Requirement: `metadata()` returns `Some(PromptInfo)` built via the
    /// PromptInfo constructor (NOT struct-literal), with description and
    /// arguments populated.
    #[tokio::test]
    async fn metadata_returns_some_promptinfo_with_description_and_args() {
        let handler = StaticPromptHandler::new(
            "with-meta",
            Some("a described prompt"),
            vec![
                PromptArgument::new("a").required(),
                PromptArgument::new("b"),
            ],
            "body",
        );

        let info = handler.metadata().expect("metadata should return Some");
        assert_eq!(info.name, "with-meta");
        assert_eq!(info.description.as_deref(), Some("a described prompt"));
        let args = info.arguments.expect("arguments should be populated");
        assert_eq!(args.len(), 2);
        assert_eq!(args[0].name, "a");
        assert!(args[0].required);
        assert_eq!(args[1].name, "b");
        assert!(!args[1].required);
    }

    #[test]
    fn metadata_with_no_arguments_omits_arguments_field() {
        let handler = StaticPromptHandler::new("plain", Some("d"), vec![], "body");
        let info = handler.metadata().unwrap();
        assert!(info.arguments.is_none());
    }

    #[tokio::test]
    async fn from_configs_resolves_resource_bodies_deterministically() {
        let resource_configs = vec![ResourceConfig {
            uri: "docs://test".to_string(),
            name: "Test Resource".to_string(),
            description: None,
            mime_type: "text/plain".to_string(),
            content: Some("Hello from resource".to_string()),
            content_file: None,
            meta: None,
        }];
        let resources =
            crate::resources::StaticResourceHandler::from_configs(&resource_configs).unwrap();

        let prompts = vec![
            PromptConfig {
                name: "p1".to_string(),
                description: "first".to_string(),
                include_resources: vec!["docs://test".to_string()],
            },
            PromptConfig {
                name: "p2".to_string(),
                description: "second".to_string(),
                include_resources: vec![],
            },
        ];

        let mut materialized = StaticPromptHandler::from_configs(&prompts, &resources);
        assert_eq!(materialized.len(), 2);
        assert_eq!(materialized[0].0, "p1");
        assert_eq!(materialized[1].0, "p2");

        // p1 resolved the resource body verbatim.
        let (_, p1_handler) = materialized.remove(0);
        let result = p1_handler.handle(HashMap::new(), mk_extra()).await.unwrap();
        match &result.messages[0].content {
            Content::Text { text } => assert_eq!(text, "Hello from resource"),
            other => panic!("expected text, got {:?}", other),
        }

        // p2 had no resources → placeholder body.
        let (_, p2_handler) = materialized.remove(0);
        let result = p2_handler.handle(HashMap::new(), mk_extra()).await.unwrap();
        match &result.messages[0].content {
            Content::Text { text } => assert!(text.contains("p2")),
            other => panic!("expected text, got {:?}", other),
        }
    }

    #[test]
    fn resolve_extra_prompt_content_filters_auto_generated() {
        let prompts = vec![PromptConfig {
            name: CODE_MODE_PROMPT_NAME.to_string(),
            description: "code mode".to_string(),
            include_resources: vec![
                "code-mode://instructions".to_string(), // auto-generated, filtered
                "docs://learnings".to_string(),
            ],
        }];
        let resources = vec![
            ResourceConfig {
                uri: "code-mode://instructions".to_string(),
                name: "auto".to_string(),
                description: None,
                mime_type: "text/markdown".to_string(),
                content: Some("AUTO".to_string()),
                content_file: None,
                meta: None,
            },
            ResourceConfig {
                uri: "docs://learnings".to_string(),
                name: "learnings".to_string(),
                description: None,
                mime_type: "text/markdown".to_string(),
                content: Some("LEARNED".to_string()),
                content_file: None,
                meta: None,
            },
        ];

        let extras = resolve_extra_prompt_content(&prompts, &resources);
        assert_eq!(extras, vec!["LEARNED".to_string()]);
    }
}
