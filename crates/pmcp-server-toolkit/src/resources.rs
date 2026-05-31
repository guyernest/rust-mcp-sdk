// Originated from pmcp-run/built-in/shared/mcp-server-common/src/resources.rs
// (https://github.com/guyernest/pmcp-run). Lifted into rust-mcp-sdk for Phase 83.

//! Static MCP resources for config-driven servers.
//!
//! [`StaticResourceHandler`] implements [`pmcp::server::ResourceHandler`] over an
//! in-memory [`IndexMap`] of [`LoadedResource`] entries. The handler does NOT
//! redefine the trait — it consumes the trait shape from `pmcp`.
//!
//! # Wire shape — MIME-typed-wire (PATTERNS §5)
//!
//! `read()` returns content via [`Content::resource_with_text`] (NOT
//! `Content::text`) so per-resource MIME types survive the JSON-RPC wire
//! round-trip. Reference files like `schema.graphql` keep their
//! `application/graphql` MIME type rather than being downgraded to
//! `text/plain`.
//!
//! # Determinism (Pattern D)
//!
//! Storage is `IndexMap<String, LoadedResource>` (NOT `HashMap`). This
//! guarantees that `list()` returns resources in deterministic, configuration
//! order — required for snapshot tests, stable example output, and predictable
//! host UX.
//!
//! # Orthogonality with skills
//!
//! `StaticResourceHandler` is independent of [`pmcp::server::skills::Skill`]
//! and `bootstrap_skill_and_prompt`. Downstream consumers can register both
//! surfaces side-by-side; the toolkit makes no assumption about skill
//! registration (RESEARCH §Risks #3).
//!
//! # Example configuration
//!
//! ```toml
//! [[resources]]
//! uri = "docs://policies/guide"
//! name = "Policy Guide"
//! description = "How to interpret policies"
//! mime_type = "text/markdown"
//! content = """
//! # Policy Guide
//! This document explains...
//! """
//! ```

use async_trait::async_trait;
use indexmap::IndexMap;
use pmcp::{
    types::{Content, ListResourcesResult, ReadResourceResult, ResourceInfo},
    ResourceHandler,
};
use serde::{Deserialize, Serialize};

use crate::error::{Result, ToolkitError};

// =============================================================================
// Configuration Types
// =============================================================================

/// MCP Resource configuration.
///
/// Resources provide documentation and context that LLMs can access to better
/// understand how to use the server's tools. Resources are loaded at build
/// time and served via MCP `resources/list` and `resources/read`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ResourceConfig {
    /// Resource URI (e.g., `docs://policies/alcohol-shipment`).
    pub uri: String,

    /// Human-readable name.
    pub name: String,

    /// Description of what this resource contains.
    #[serde(default)]
    pub description: Option<String>,

    /// MIME type (defaults to `text/markdown`).
    #[serde(default = "default_mime_type")]
    pub mime_type: String,

    /// Inline content (mutually exclusive with `content_file`).
    #[serde(default)]
    pub content: Option<String>,

    /// Path to content file, relative to config (mutually exclusive with
    /// `content`).
    ///
    /// Not supported in Lambda; use inline content instead.
    #[serde(default)]
    pub content_file: Option<String>,

    /// Optional metadata map for resource `_meta` (e.g., widget metadata for
    /// MCP Apps).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Map<String, serde_json::Value>>,
}

fn default_mime_type() -> String {
    "text/markdown".to_string()
}

impl ResourceConfig {
    /// Validate the resource configuration.
    ///
    /// Returns [`ToolkitError::Synth`] if neither `content` nor `content_file`
    /// is set, or if both are set.
    pub fn validate(&self) -> Result<()> {
        if self.content.is_none() && self.content_file.is_none() {
            return Err(ToolkitError::Synth(format!(
                "Resource '{}': must specify either 'content' or 'content_file'",
                self.uri
            )));
        }
        if self.content.is_some() && self.content_file.is_some() {
            return Err(ToolkitError::Synth(format!(
                "Resource '{}': cannot specify both 'content' and 'content_file'",
                self.uri
            )));
        }
        Ok(())
    }
}

// =============================================================================
// Loaded Resource
// =============================================================================

/// A loaded resource with resolved content.
#[derive(Debug, Clone)]
pub struct LoadedResource {
    /// Resource URI.
    pub uri: String,
    /// Human-readable name.
    pub name: String,
    /// Optional description.
    pub description: Option<String>,
    /// MIME type.
    pub mime_type: String,
    /// Resolved content.
    pub content: String,
    /// Optional metadata map for resource `_meta`.
    pub meta: Option<serde_json::Map<String, serde_json::Value>>,
}

impl LoadedResource {
    /// Create a `LoadedResource` from config with inline content.
    ///
    /// Returns [`ToolkitError::Synth`] if `config.content` is absent —
    /// `content_file` is not supported in this lift (Lambda runtime
    /// constraint).
    pub fn from_config(config: &ResourceConfig) -> Result<Self> {
        let content = config.content.clone().ok_or_else(|| {
            ToolkitError::Synth(format!(
                "Resource '{}': inline 'content' is required (content_file not supported in Lambda)",
                config.uri
            ))
        })?;

        Ok(Self {
            uri: config.uri.clone(),
            name: config.name.clone(),
            description: config.description.clone(),
            mime_type: config.mime_type.clone(),
            content,
            meta: config.meta.clone(),
        })
    }

    /// Convert to PMCP SDK [`ResourceInfo`] for listing.
    pub fn to_resource_info(&self) -> ResourceInfo {
        let mut info = ResourceInfo::new(&self.uri, &self.name).with_mime_type(&self.mime_type);
        if let Some(ref desc) = self.description {
            info = info.with_description(desc);
        }
        if let Some(ref meta) = self.meta {
            info = info.with_meta(meta.clone());
        }
        info
    }

    /// Convert to PMCP SDK [`Content`] for reading.
    ///
    /// Always uses the MIME-typed-wire shape [`Content::resource_with_text`]
    /// (PATTERNS §5) so per-resource MIME types survive the wire round-trip.
    /// If a downstream consumer needs `_meta` propagation on top of MIME, see
    /// the threat model `T-83-03-03` mitigation in plan 83-03 — a future
    /// follow-up may add a `with_meta` variant. The current lift drops `_meta`
    /// at the read boundary; the resource _meta is exposed through
    /// `to_resource_info()` for `resources/list` only.
    pub fn to_content(&self) -> Content {
        Content::resource_with_text(
            self.uri.clone(),
            self.content.clone(),
            self.mime_type.clone(),
        )
    }
}

// =============================================================================
// Static Resource Handler
// =============================================================================

/// Handler for static resources loaded from configuration.
///
/// Implements the PMCP SDK [`ResourceHandler`] trait for serving pre-loaded
/// resources via MCP `resources/list` and `resources/read`. Storage is an
/// [`IndexMap`] (Pattern D) so iteration order is deterministic across runs.
///
/// # Orthogonality with skills
///
/// `StaticResourceHandler` is independent of [`pmcp::server::skills::Skill`]
/// and `bootstrap_skill_and_prompt`. Downstream consumers can register both
/// surfaces side-by-side; the toolkit makes no assumption about skill
/// registration (RESEARCH §Risks #3).
pub struct StaticResourceHandler {
    // IndexMap — see Pattern D in 83-PATTERNS.md. Insertion order is preserved
    // across iterations so `list()` is deterministic.
    resources: IndexMap<String, LoadedResource>,
}

impl StaticResourceHandler {
    /// Create a handler from a pre-built [`IndexMap`].
    ///
    /// This is the constructor Plan 08 will target from
    /// `impl From<&ServerConfig> for StaticResourceHandler`.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use pmcp_server_toolkit::resources::StaticResourceHandler;
    /// use indexmap::IndexMap;
    /// let map = IndexMap::new();
    /// let handler = StaticResourceHandler::new(map);
    /// # let _ = handler;
    /// ```
    pub fn new(resources: IndexMap<String, LoadedResource>) -> Self {
        Self { resources }
    }

    /// Create a new handler from a list of resource configurations.
    ///
    /// Insertion order is preserved — `list()` reflects the order configs
    /// were supplied in.
    pub fn from_configs(configs: &[ResourceConfig]) -> Result<Self> {
        let mut resources = IndexMap::with_capacity(configs.len());

        for config in configs {
            let loaded = LoadedResource::from_config(config)?;
            resources.insert(loaded.uri.clone(), loaded);
        }

        Ok(Self { resources })
    }

    /// Create an empty handler with no resources.
    pub fn empty() -> Self {
        Self {
            resources: IndexMap::new(),
        }
    }

    /// Returns `true` if there are no resources.
    pub fn is_empty(&self) -> bool {
        self.resources.is_empty()
    }

    /// Returns the number of resources.
    pub fn len(&self) -> usize {
        self.resources.len()
    }

    /// Get a resource by URI (for use outside of the trait).
    pub fn get(&self, uri: &str) -> Option<&LoadedResource> {
        self.resources.get(uri)
    }

    /// Iterate over resource URIs in deterministic insertion order.
    pub fn uris(&self) -> impl Iterator<Item = &str> {
        self.resources.keys().map(String::as_str)
    }
}

// =============================================================================
// Construction from `ServerConfig` (Plan 08 — TKIT-04 completion)
// =============================================================================
//
// `ResourceDecl` is the strict, lifted shape parsed by `ServerConfig`
// (`config::ResourceDecl`), whereas this module's own `ResourceConfig` carries
// the richer fields (`content_file`, `meta`) used for file-backed and widget
// resources. The two shapes are NOT identical — `From<&ServerConfig>` maps the
// configured `[[resources]]` block onto `LoadedResource` directly so callers
// don't have to thread a second config type through their builders.

impl From<&crate::config::ServerConfig> for StaticResourceHandler {
    /// Build a [`StaticResourceHandler`] from a parsed [`crate::config::ServerConfig`].
    ///
    /// Each `[[resources]]` entry in `config` becomes one [`LoadedResource`].
    /// Resources with no `content` field default to an empty body — the
    /// strict-parse path's [`crate::config::ServerConfig::validate`] does not
    /// flag empty resource bodies (operators may use the placeholder form
    /// `"loaded from path.md"` as a stable URI handle), so this construction
    /// follows suit. Resources WITH `content_file` semantics are out of scope
    /// for the lifted shape (Lambda runtime constraint, mirroring
    /// [`LoadedResource::from_config`]).
    ///
    /// Insertion order matches the order of `[[resources]]` declarations,
    /// satisfying Pattern D (deterministic `list()` output).
    ///
    /// # Example
    ///
    /// ```no_run
    /// use pmcp_server_toolkit::{ServerConfig, StaticResourceHandler};
    ///
    /// let cfg = ServerConfig::default();
    /// let handler = StaticResourceHandler::from(&cfg);
    /// assert_eq!(handler.len(), 0); // default config has no [[resources]]
    /// ```
    fn from(cfg: &crate::config::ServerConfig) -> Self {
        let mut resources = IndexMap::with_capacity(cfg.resources.len());
        for r in &cfg.resources {
            let mime = r.mime_type.clone().unwrap_or_else(default_mime_type);
            let loaded = LoadedResource {
                uri: r.uri.clone(),
                name: r.name.clone().unwrap_or_else(|| r.uri.clone()),
                description: r.description.clone(),
                mime_type: mime,
                content: r.content.clone().unwrap_or_default(),
                meta: None,
            };
            resources.insert(r.uri.clone(), loaded);
        }
        Self { resources }
    }
}

#[async_trait]
impl ResourceHandler for StaticResourceHandler {
    async fn list(
        &self,
        _cursor: Option<String>,
        _extra: pmcp::RequestHandlerExtra,
    ) -> pmcp::Result<ListResourcesResult> {
        let resources: Vec<ResourceInfo> = self
            .resources
            .values()
            .map(LoadedResource::to_resource_info)
            .collect();

        Ok(ListResourcesResult::new(resources))
    }

    async fn read(
        &self,
        uri: &str,
        _extra: pmcp::RequestHandlerExtra,
    ) -> pmcp::Result<ReadResourceResult> {
        match self.resources.get(uri) {
            Some(resource) => Ok(ReadResourceResult::new(vec![resource.to_content()])),
            None => Err(pmcp::Error::protocol(
                pmcp::ErrorCode::METHOD_NOT_FOUND,
                format!("Resource not found: {}", uri),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pmcp::RequestHandlerExtra;

    fn mk_extra() -> RequestHandlerExtra {
        RequestHandlerExtra::default()
    }

    fn cfg(uri: &str, mime: &str, body: &str) -> ResourceConfig {
        ResourceConfig {
            uri: uri.to_string(),
            name: uri.to_string(),
            description: None,
            mime_type: mime.to_string(),
            content: Some(body.to_string()),
            content_file: None,
            meta: None,
        }
    }

    #[test]
    fn resource_config_validation() {
        // Valid inline content.
        let c = cfg("docs://test", "text/plain", "hello");
        assert!(c.validate().is_ok());

        // Missing content.
        let mut c = cfg("docs://test", "text/plain", "");
        c.content = None;
        assert!(c.validate().is_err());

        // Both content and content_file.
        let mut c = cfg("docs://test", "text/plain", "hello");
        c.content_file = Some("file.md".to_string());
        assert!(c.validate().is_err());
    }

    #[test]
    fn loaded_resource_from_config() {
        let c = cfg("docs://test", "text/markdown", "# Hello\nWorld");
        let loaded = LoadedResource::from_config(&c).unwrap();
        assert_eq!(loaded.uri, "docs://test");
        assert_eq!(loaded.mime_type, "text/markdown");
        assert_eq!(loaded.content, "# Hello\nWorld");
    }

    /// Requirement: `read()` returns the MIME-typed-wire resource variant
    /// (NOT a bare text payload) so per-resource MIME types survive the wire
    /// round-trip (PATTERNS §5 MIME-typed-wire).
    #[tokio::test]
    async fn read_returns_resource_with_text_and_correct_mime() {
        let handler = StaticResourceHandler::from_configs(&[cfg(
            "schema://main",
            "application/graphql",
            "type Query { hello: String }",
        )])
        .unwrap();

        let result = handler.read("schema://main", mk_extra()).await.unwrap();
        assert_eq!(result.contents.len(), 1);
        match &result.contents[0] {
            Content::Resource {
                uri,
                text,
                mime_type,
                ..
            } => {
                assert_eq!(uri, "schema://main");
                assert_eq!(text.as_deref(), Some("type Query { hello: String }"));
                assert_eq!(mime_type.as_deref(), Some("application/graphql"));
            },
            other => panic!("expected Content::Resource, got {:?}", other),
        }
    }

    /// Requirement: `read()` on a missing URI returns `Err`.
    #[tokio::test]
    async fn read_missing_uri_returns_err() {
        let handler = StaticResourceHandler::empty();
        let result = handler.read("docs://nope", mk_extra()).await;
        assert!(result.is_err());
    }

    /// Requirement: `list()` returns resources in deterministic insertion
    /// order across multiple invocations (Pattern D — IndexMap, not HashMap).
    #[tokio::test]
    async fn list_returns_deterministic_order() {
        let handler = StaticResourceHandler::from_configs(&[
            cfg("docs://a", "text/plain", "A"),
            cfg("docs://b", "text/plain", "B"),
            cfg("docs://c", "text/plain", "C"),
        ])
        .unwrap();

        let first = handler.list(None, mk_extra()).await.unwrap();
        let second = handler.list(None, mk_extra()).await.unwrap();

        let uris1: Vec<&str> = first.resources.iter().map(|r| r.uri.as_str()).collect();
        let uris2: Vec<&str> = second.resources.iter().map(|r| r.uri.as_str()).collect();

        assert_eq!(uris1, vec!["docs://a", "docs://b", "docs://c"]);
        assert_eq!(uris1, uris2);
    }

    #[test]
    fn handler_len_and_empty() {
        let handler = StaticResourceHandler::from_configs(&[
            cfg("docs://one", "text/plain", "Content one"),
            cfg("docs://two", "text/plain", "Content two"),
        ])
        .unwrap();
        assert_eq!(handler.len(), 2);
        assert!(!handler.is_empty());
        assert!(handler.get("docs://one").is_some());
        assert!(handler.get("docs://three").is_none());

        let uris: Vec<&str> = handler.uris().collect();
        assert_eq!(uris, vec!["docs://one", "docs://two"]);
    }
}
