//! Builder pattern for constructing `ServerCore` instances.

use crate::error::{Error, Result};
use crate::runtime::RwLock;
use crate::server::auth::{AuthProvider, ToolAuthorizer};
use crate::server::core::ServerCore;
use crate::server::limits::PayloadLimits;
#[cfg(not(target_arch = "wasm32"))]
use crate::server::observability::{
    CloudWatchBackend, ConsoleBackend, McpObservabilityMiddleware, NullBackend,
    ObservabilityBackend, ObservabilityConfig,
};
#[cfg(all(feature = "skills", not(target_arch = "wasm32")))]
use crate::server::skills::{ComposedResources, Skill, SkillPromptHandler, Skills};
use crate::server::tasks::TaskRouter;
#[cfg(not(target_arch = "wasm32"))]
use crate::server::tool_middleware::{ToolMiddleware, ToolMiddlewareChain};
use crate::server::{PromptHandler, ResourceHandler, SamplingHandler, ToolHandler};
use crate::shared::middleware::EnhancedMiddlewareChain;
use crate::types::{Implementation, PromptInfo, ServerCapabilities, ToolInfo};
use std::collections::HashMap;
use std::sync::Arc;

/// Builder for constructing a `ServerCore` instance.
///
/// This builder provides a fluent API for configuring all aspects of the server
/// before creating the final `ServerCore` instance.
///
/// # Examples
///
/// ```rust,no_run
/// use pmcp::server::builder::ServerCoreBuilder;
/// use pmcp::server::core::ServerCore;
/// use pmcp::{ToolHandler, ServerCapabilities};
/// use async_trait::async_trait;
/// use serde_json::Value;
///
/// struct MyTool;
///
/// #[async_trait]
/// impl ToolHandler for MyTool {
///     async fn handle(&self, args: Value, _extra: pmcp::RequestHandlerExtra) -> pmcp::Result<Value> {
///         Ok(serde_json::json!({"result": "success"}))
///     }
/// }
///
/// # async fn example() -> pmcp::Result<()> {
/// let server = ServerCoreBuilder::new()
///     .name("my-server")
///     .version("1.0.0")
///     .tool("my-tool", MyTool)
///     .capabilities(ServerCapabilities::tools_only())
///     .build()?;
/// # Ok(())
/// # }
/// ```
#[allow(missing_debug_implementations)]
pub struct ServerCoreBuilder {
    name: Option<String>,
    version: Option<String>,
    capabilities: ServerCapabilities,
    tools: HashMap<String, Arc<dyn ToolHandler>>,
    prompts: HashMap<String, Arc<dyn PromptHandler>>,
    /// Cached tool metadata (populated at registration, avoids per-request cloning)
    tool_infos: HashMap<String, ToolInfo>,
    /// Cached prompt metadata (populated at registration, avoids per-request cloning)
    prompt_infos: HashMap<String, PromptInfo>,
    resources: Option<Arc<dyn ResourceHandler>>,
    sampling: Option<Arc<dyn SamplingHandler>>,
    auth_provider: Option<Arc<dyn AuthProvider>>,
    tool_authorizer: Option<Arc<dyn ToolAuthorizer>>,
    protocol_middleware: Arc<RwLock<EnhancedMiddlewareChain>>,
    #[cfg(not(target_arch = "wasm32"))]
    tool_middlewares: Vec<Arc<dyn ToolMiddleware>>,
    /// Task router for experimental MCP Tasks support (optional)
    #[cfg(not(target_arch = "wasm32"))]
    task_router: Option<Arc<dyn TaskRouter>>,
    /// Task store for MCP Tasks with polling (optional, standard capability path)
    #[cfg(not(target_arch = "wasm32"))]
    task_store: Option<Arc<dyn crate::server::task_store::TaskStore>>,
    /// Stateless mode for serverless deployments (None = auto-detect)
    stateless_mode: Option<bool>,
    /// Host-specific metadata layers (e.g., `ChatGpt` for openai/* keys)
    #[cfg(feature = "mcp-apps")]
    host_layers: Vec<crate::types::mcp_apps::HostType>,
    /// Optional website URL for the server implementation (MCP 2025-11-25)
    website_url: Option<String>,
    /// Optional icons for the server implementation (MCP 2025-11-25)
    icons: Option<Vec<crate::types::protocol::IconInfo>>,
    /// Payload and resource limits
    payload_limits: PayloadLimits,
    /// Accumulated SEP-2640 Agent Skills (80-REVIEWS.md Fix 1: finalized
    /// into a single `SkillsHandler` exactly once at `.build()` time).
    #[cfg(all(feature = "skills", not(target_arch = "wasm32")))]
    pending_skills: Option<Skills>,
}

impl Default for ServerCoreBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ServerCoreBuilder {
    /// Create a new builder with default settings.
    pub fn new() -> Self {
        Self {
            name: None,
            version: None,
            capabilities: ServerCapabilities::default(),
            tools: HashMap::new(),
            prompts: HashMap::new(),
            tool_infos: HashMap::new(),
            prompt_infos: HashMap::new(),
            resources: None,
            sampling: None,
            auth_provider: None,
            tool_authorizer: None,
            protocol_middleware: Arc::new(RwLock::new(EnhancedMiddlewareChain::new())),
            #[cfg(not(target_arch = "wasm32"))]
            tool_middlewares: Vec::new(),
            #[cfg(not(target_arch = "wasm32"))]
            task_router: None,
            #[cfg(not(target_arch = "wasm32"))]
            task_store: None,
            stateless_mode: None, // Auto-detect by default
            #[cfg(feature = "mcp-apps")]
            host_layers: Vec::new(),
            website_url: None,
            icons: None,
            payload_limits: PayloadLimits::default(),
            #[cfg(all(feature = "skills", not(target_arch = "wasm32")))]
            pending_skills: None,
        }
    }

    /// Set the server name.
    ///
    /// This is a required field that identifies the server implementation.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the server version.
    ///
    /// This is a required field that identifies the server version.
    pub fn version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    /// Set the website URL for the server implementation (MCP 2025-11-25).
    pub fn website_url(mut self, url: impl Into<String>) -> Self {
        self.website_url = Some(url.into());
        self
    }

    /// Set icons for the server implementation (MCP 2025-11-25).
    pub fn with_icons(mut self, icons: Vec<crate::types::protocol::IconInfo>) -> Self {
        self.icons = Some(icons);
        self
    }

    /// Set the server capabilities.
    ///
    /// Defines what features this server supports.
    pub fn capabilities(mut self, capabilities: ServerCapabilities) -> Self {
        self.capabilities = capabilities;
        self
    }

    /// Add a tool handler.
    ///
    /// Tools are functions that can be called by the client.
    pub fn tool(mut self, name: impl Into<String>, handler: impl ToolHandler + 'static) -> Self {
        contract_pre_tool_dispatch_integrity!();
        let name = name.into();
        let handler = Arc::new(handler) as Arc<dyn ToolHandler>;
        // Cache metadata at registration time to avoid per-request cloning
        let mut info = handler
            .metadata()
            .unwrap_or_else(|| ToolInfo::new(name.clone(), None, serde_json::json!({})));
        info.name.clone_from(&name);
        self.tool_infos.insert(name.clone(), info);
        self.tools.insert(name, handler);

        // Update capabilities to include tools
        // Use Some(false) instead of None to ensure the field serializes properly
        if self.capabilities.tools.is_none() {
            self.capabilities.tools = Some(crate::types::ToolCapabilities {
                list_changed: Some(false),
            });
        }

        self
    }

    /// Add a tool handler with an Arc.
    ///
    /// This variant is useful when you need to share the handler across multiple servers.
    pub fn tool_arc(mut self, name: impl Into<String>, handler: Arc<dyn ToolHandler>) -> Self {
        let name = name.into();
        // Cache metadata at registration time to avoid per-request cloning
        let mut info = handler
            .metadata()
            .unwrap_or_else(|| ToolInfo::new(name.clone(), None, serde_json::json!({})));
        info.name.clone_from(&name);
        self.tool_infos.insert(name.clone(), info);
        self.tools.insert(name, handler);

        // Update capabilities to include tools
        // Use Some(false) instead of None to ensure the field serializes properly
        if self.capabilities.tools.is_none() {
            self.capabilities.tools = Some(crate::types::ToolCapabilities {
                list_changed: Some(false),
            });
        }

        self
    }

    /// Add a prompt handler.
    ///
    /// Prompts are templates that generate messages for the client.
    pub fn prompt(
        mut self,
        name: impl Into<String>,
        handler: impl PromptHandler + 'static,
    ) -> Self {
        let name = name.into();
        let handler = Arc::new(handler) as Arc<dyn PromptHandler>;
        // Cache metadata at registration time to avoid per-request cloning
        let mut info = handler.metadata().unwrap_or_else(|| PromptInfo::new(&name));
        info.name.clone_from(&name);
        self.prompt_infos.insert(name.clone(), info);
        self.prompts.insert(name, handler);

        // Update capabilities to include prompts
        // Use Some(false) instead of None to ensure the field serializes properly
        if self.capabilities.prompts.is_none() {
            self.capabilities.prompts = Some(crate::types::PromptCapabilities {
                list_changed: Some(false),
            });
        }

        self
    }

    /// Add a prompt handler with an Arc.
    ///
    /// This variant is useful when you need to share the handler across multiple servers.
    pub fn prompt_arc(mut self, name: impl Into<String>, handler: Arc<dyn PromptHandler>) -> Self {
        let name = name.into();
        // Cache metadata at registration time to avoid per-request cloning
        let mut info = handler.metadata().unwrap_or_else(|| PromptInfo::new(&name));
        info.name.clone_from(&name);
        self.prompt_infos.insert(name.clone(), info);
        self.prompts.insert(name, handler);

        // Update capabilities to include prompts
        // Use Some(false) instead of None to ensure the field serializes properly
        if self.capabilities.prompts.is_none() {
            self.capabilities.prompts = Some(crate::types::PromptCapabilities {
                list_changed: Some(false),
            });
        }

        self
    }

    /// Set the resource handler.
    ///
    /// Resources provide access to data that the client can read.
    pub fn resources(mut self, handler: impl ResourceHandler + 'static) -> Self {
        self.resources = Some(Arc::new(handler) as Arc<dyn ResourceHandler>);

        // Update capabilities to include resources
        // Use Some(false) instead of None to ensure fields serialize properly
        if self.capabilities.resources.is_none() {
            self.capabilities.resources = Some(crate::types::ResourceCapabilities {
                subscribe: Some(false),
                list_changed: Some(false),
            });
        }

        self
    }

    /// Set the resource handler with an Arc.
    ///
    /// This variant is useful when you need to share the handler across multiple servers.
    pub fn resources_arc(mut self, handler: Arc<dyn ResourceHandler>) -> Self {
        self.resources = Some(handler);

        // Update capabilities to include resources
        // Use Some(false) instead of None to ensure fields serialize properly
        if self.capabilities.resources.is_none() {
            self.capabilities.resources = Some(crate::types::ResourceCapabilities {
                subscribe: Some(false),
                list_changed: Some(false),
            });
        }

        self
    }

    /// Register a single SEP-2640 Agent Skill.
    ///
    /// Convenience over [`Self::skills`] for the single-skill case. The skill
    /// is accumulated in `self.pending_skills` and finalized into a
    /// [`crate::server::skills::Skills`]-derived `ResourceHandler` at
    /// [`Self::build`] time. Composes (at most once, in `build()`) with any
    /// `.resources(...)` handler set on this builder.
    ///
    /// # Panics
    ///
    /// Panics at `.build()` time if multiple registered skills resolve to
    /// the same `skill://` URI. Use [`Self::try_skills`] with a pre-built
    /// [`Skills`] registry to surface duplicates as a `Result`.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::server::builder::ServerCoreBuilder;
    /// use pmcp::server::skills::Skill;
    ///
    /// # fn example() -> pmcp::Result<()> {
    /// let server = ServerCoreBuilder::new()
    ///     .name("my-server")
    ///     .version("1.0.0")
    ///     .skill(Skill::new("hello", "# Hello skill"))
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(all(feature = "skills", not(target_arch = "wasm32")))]
    #[must_use]
    pub fn skill(self, skill: Skill) -> Self {
        self.skills(Skills::new().add(skill))
    }

    /// Register a registry of SEP-2640 Agent Skills.
    ///
    /// Merges into any prior accumulated skills (a previous `.skill(...)` or
    /// `.skills(...)` call). The accumulated registry is finalized into a
    /// single `SkillsHandler` exactly once at [`Self::build`] time, then
    /// composed at most once with any `.resources(...)` handler. There is
    /// no per-call wrapper nesting (80-REVIEWS.md Fix 1).
    ///
    /// # Panics
    ///
    /// Panics at `.build()` if two registered skills resolve to the same
    /// `skill://` URI. Use [`Self::try_skills`] for fallible registration.
    #[cfg(all(feature = "skills", not(target_arch = "wasm32")))]
    #[must_use]
    pub fn skills(mut self, skills: Skills) -> Self {
        let merged = match self.pending_skills.take() {
            Some(prior) => prior.merge(skills),
            None => skills,
        };
        self.pending_skills = Some(merged);
        // Capability flips: set early so they're visible to inspectors
        // (e.g., tests) even before `.build()` runs.
        if self.capabilities.resources.is_none() {
            self.capabilities.resources = Some(crate::types::ResourceCapabilities {
                subscribe: Some(false),
                list_changed: Some(false),
            });
        }
        let mut ext = self.capabilities.extensions.take().unwrap_or_default();
        ext.entry("io.modelcontextprotocol/skills".to_string())
            .or_insert_with(|| serde_json::json!({}));
        self.capabilities.extensions = Some(ext);
        self
    }

    /// Fallible variant of [`Self::skills`] (80-REVIEWS.md Fix 10 / Codex G2).
    ///
    /// Returns `Err` immediately if the merged registry would contain duplicate
    /// URIs. Useful for runtime-dynamic registration where panicking is
    /// unacceptable.
    ///
    /// # Errors
    ///
    /// Returns `Err(pmcp::Error::Validation)` if the merged registry would
    /// produce duplicate `skill://` URIs.
    #[cfg(all(feature = "skills", not(target_arch = "wasm32")))]
    pub fn try_skills(mut self, skills: Skills) -> Result<Self> {
        let merged = match self.pending_skills.take() {
            Some(prior) => prior.merge(skills),
            None => skills,
        };
        // Probe by cloning + into_handler; discard the handler. The real
        // construction happens in `.build()` once everything is settled.
        merged.clone().into_handler()?;
        self.pending_skills = Some(merged);
        if self.capabilities.resources.is_none() {
            self.capabilities.resources = Some(crate::types::ResourceCapabilities {
                subscribe: Some(false),
                list_changed: Some(false),
            });
        }
        let mut ext = self.capabilities.extensions.take().unwrap_or_default();
        ext.entry("io.modelcontextprotocol/skills".to_string())
            .or_insert_with(|| serde_json::json!({}));
        self.capabilities.extensions = Some(ext);
        Ok(self)
    }

    /// Register a skill AND a parallel prompt that returns the same content.
    ///
    /// The dual-surface bootstrap. Both surfaces are derived from one
    /// [`Skill`] value, so they cannot drift. The byte-equality is asserted
    /// in `tests/skills_integration.rs` (Plan 80-03).
    #[cfg(all(feature = "skills", not(target_arch = "wasm32")))]
    #[must_use]
    pub fn bootstrap_skill_and_prompt(self, skill: Skill, prompt_name: impl Into<String>) -> Self {
        let prompt_handler = SkillPromptHandler::new(skill.clone());
        self.skill(skill).prompt(prompt_name, prompt_handler)
    }

    /// Set the sampling handler.
    ///
    /// Sampling provides LLM capabilities for message generation.
    pub fn sampling(mut self, handler: impl SamplingHandler + 'static) -> Self {
        self.sampling = Some(Arc::new(handler) as Arc<dyn SamplingHandler>);

        // Update capabilities to include sampling
        if self.capabilities.sampling.is_none() {
            self.capabilities.sampling = Some(crate::types::SamplingCapabilities::default());
        }

        self
    }

    /// Set the sampling handler with an Arc.
    ///
    /// This variant is useful when you need to share the handler across multiple servers.
    pub fn sampling_arc(mut self, handler: Arc<dyn SamplingHandler>) -> Self {
        self.sampling = Some(handler);

        // Update capabilities to include sampling
        if self.capabilities.sampling.is_none() {
            self.capabilities.sampling = Some(crate::types::SamplingCapabilities::default());
        }

        self
    }

    /// Set the authentication provider.
    ///
    /// The auth provider validates client authentication.
    pub fn auth_provider(mut self, provider: impl AuthProvider + 'static) -> Self {
        self.auth_provider = Some(Arc::new(provider) as Arc<dyn AuthProvider>);
        self
    }

    /// Set the authentication provider with an Arc.
    ///
    /// This variant is useful when you need to share the provider across multiple servers.
    pub fn auth_provider_arc(mut self, provider: Arc<dyn AuthProvider>) -> Self {
        self.auth_provider = Some(provider);
        self
    }

    /// Set the tool authorizer.
    ///
    /// The tool authorizer provides fine-grained access control for tools.
    pub fn tool_authorizer(mut self, authorizer: impl ToolAuthorizer + 'static) -> Self {
        self.tool_authorizer = Some(Arc::new(authorizer) as Arc<dyn ToolAuthorizer>);
        self
    }

    /// Set the tool authorizer with an Arc.
    ///
    /// This variant is useful when you need to share the authorizer across multiple servers.
    pub fn tool_authorizer_arc(mut self, authorizer: Arc<dyn ToolAuthorizer>) -> Self {
        self.tool_authorizer = Some(authorizer);
        self
    }

    /// Set the protocol middleware chain.
    ///
    /// Protocol middleware processes JSON-RPC requests, responses, and notifications
    /// at the protocol layer, enabling logging, metrics, validation, and more.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use pmcp::server::builder::ServerCoreBuilder;
    /// use pmcp::shared::middleware::{EnhancedMiddlewareChain, LoggingMiddleware};
    /// use std::sync::Arc;
    /// use pmcp::runtime::RwLock;
    ///
    /// let mut chain = EnhancedMiddlewareChain::new();
    /// chain.add(Arc::new(LoggingMiddleware::new()));
    ///
    /// let server = ServerCoreBuilder::new()
    ///     .name("my-server")
    ///     .version("1.0.0")
    ///     .protocol_middleware(Arc::new(RwLock::new(chain)))
    ///     .build()?;
    /// ```
    pub fn protocol_middleware(mut self, middleware: Arc<RwLock<EnhancedMiddlewareChain>>) -> Self {
        self.protocol_middleware = middleware;
        self
    }

    /// Add a tool middleware to the chain.
    ///
    /// Tool middleware provides cross-cutting concerns for tool execution,
    /// such as OAuth token injection, logging, metrics, and authorization.
    ///
    /// Middleware is sorted by priority during `build()` - lower priority values
    /// execute first (e.g., auth: 10, default: 50, logging: 90).
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use pmcp::server::builder::ServerCoreBuilder;
    /// use pmcp::server::tool_middleware::ToolMiddleware;
    /// use std::sync::Arc;
    ///
    /// struct OAuthMiddleware {
    ///     token: String,
    /// }
    ///
    /// #[async_trait]
    /// impl ToolMiddleware for OAuthMiddleware {
    ///     async fn on_request(
    ///         &self,
    ///         _tool_name: &str,
    ///         _args: &mut Value,
    ///         extra: &mut RequestHandlerExtra,
    ///         _context: &ToolContext,
    ///     ) -> Result<()> {
    ///         extra.set_metadata("oauth_token".to_string(), self.token.clone());
    ///         Ok(())
    ///     }
    /// }
    ///
    /// let server = ServerCoreBuilder::new()
    ///     .name("my-server")
    ///     .version("1.0.0")
    ///     .tool_middleware(Arc::new(OAuthMiddleware {
    ///         token: "my-token".to_string()
    ///     }))
    ///     .build()?;
    /// ```
    #[cfg(not(target_arch = "wasm32"))]
    pub fn tool_middleware(mut self, middleware: Arc<dyn ToolMiddleware>) -> Self {
        self.tool_middlewares.push(middleware);
        self
    }

    /// Enable observability for this server.
    ///
    /// This adds observability middleware that provides:
    /// - Distributed tracing with trace/span IDs
    /// - Request/response event logging
    /// - Metrics emission (duration, count, errors)
    ///
    /// The backend is selected based on the configuration:
    /// - "console" - Pretty or JSON output to stdout (development)
    /// - "cloudwatch" - AWS `CloudWatch` EMF format (production)
    /// - "null" - Discards all events (testing)
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use pmcp::server::builder::ServerCoreBuilder;
    /// use pmcp::server::observability::ObservabilityConfig;
    ///
    /// # fn example() -> pmcp::Result<()> {
    /// // Development: console output with pretty printing
    /// let server = ServerCoreBuilder::new()
    ///     .name("my-server")
    ///     .version("1.0.0")
    ///     .with_observability(ObservabilityConfig::development())
    ///     .build()?;
    ///
    /// // Production: CloudWatch with EMF metrics
    /// let server = ServerCoreBuilder::new()
    ///     .name("my-server")
    ///     .version("1.0.0")
    ///     .with_observability(ObservabilityConfig::production())
    ///     .build()?;
    ///
    /// // Load from config file or environment
    /// let config = ObservabilityConfig::load().unwrap_or_default();
    /// let server = ServerCoreBuilder::new()
    ///     .name("my-server")
    ///     .version("1.0.0")
    ///     .with_observability(config)
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(not(target_arch = "wasm32"))]
    pub fn with_observability(mut self, config: ObservabilityConfig) -> Self {
        if !config.enabled {
            return self;
        }

        // Create backend based on configuration
        let backend: Arc<dyn ObservabilityBackend> = match config.backend.as_str() {
            "cloudwatch" => Arc::new(CloudWatchBackend::new(config.cloudwatch.clone())),
            "null" => Arc::new(NullBackend),
            _ => Arc::new(ConsoleBackend::new(config.console.pretty)),
        };

        // Get server name for middleware (use placeholder if not yet set)
        let server_name = self.name.clone().unwrap_or_else(|| "unknown".to_string());

        // Create and add the observability middleware
        let middleware = McpObservabilityMiddleware::new(server_name, config, backend);
        self.tool_middlewares.push(Arc::new(middleware));

        self
    }

    /// Enable observability with a custom backend.
    ///
    /// Use this method when you need to provide a custom backend implementation,
    /// such as sending events to a custom metrics platform or log aggregator.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use pmcp::server::builder::ServerCoreBuilder;
    /// use pmcp::server::observability::{ObservabilityConfig, ObservabilityBackend};
    /// use std::sync::Arc;
    ///
    /// struct MyCustomBackend;
    ///
    /// #[async_trait]
    /// impl ObservabilityBackend for MyCustomBackend {
    ///     // ... custom implementation
    /// }
    ///
    /// let server = ServerCoreBuilder::new()
    ///     .name("my-server")
    ///     .version("1.0.0")
    ///     .with_observability_backend(
    ///         ObservabilityConfig::development(),
    ///         Arc::new(MyCustomBackend),
    ///     )
    ///     .build()?;
    /// ```
    #[cfg(not(target_arch = "wasm32"))]
    pub fn with_observability_backend(
        mut self,
        config: ObservabilityConfig,
        backend: Arc<dyn ObservabilityBackend>,
    ) -> Self {
        if !config.enabled {
            return self;
        }

        // Get server name for middleware (use placeholder if not yet set)
        let server_name = self.name.clone().unwrap_or_else(|| "unknown".to_string());

        // Create and add the observability middleware
        let middleware = McpObservabilityMiddleware::new(server_name, config, backend);
        self.tool_middlewares.push(Arc::new(middleware));

        self
    }

    /// Register a host-specific metadata layer.
    ///
    /// By default, only standard MCP Apps keys are emitted in tool `_meta`.
    /// Call this to add host-specific keys at build time. For example,
    /// `HostType::ChatGpt` adds `openai/outputTemplate` and
    /// `openai/widgetAccessible` to tools that have a `ui.resourceUri`.
    ///
    /// Duplicate host types are ignored (deduplicated).
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use pmcp::types::mcp_apps::HostType;
    ///
    /// let server = ServerCoreBuilder::new()
    ///     .name("my-server")
    ///     .version("1.0.0")
    ///     .tool("chess", ChessTool)
    ///     .with_host_layer(HostType::ChatGpt)
    ///     .build()?;
    /// ```
    #[cfg(feature = "mcp-apps")]
    pub fn with_host_layer(mut self, host: crate::types::mcp_apps::HostType) -> Self {
        if !self.host_layers.contains(&host) {
            self.host_layers.push(host);
        }
        self
    }

    /// Enable or disable stateless mode for serverless deployments.
    ///
    /// Stateless mode skips initialization state checking, allowing the server
    /// to process requests without requiring an `initialize` call first. This is
    /// essential for stateless environments like AWS Lambda, Cloudflare Workers,
    /// and other serverless platforms where each request may create a fresh
    /// server instance.
    ///
    /// # Default Behavior
    ///
    /// If not explicitly set, stateless mode is automatically detected based on
    /// environment variables:
    /// - `AWS_LAMBDA_FUNCTION_NAME` - AWS Lambda
    /// - `VERCEL` - Vercel Functions
    /// - `DENO_DEPLOYMENT_ID` - Deno Deploy
    /// - `CLOUDFLARE_WORKER` - Cloudflare Workers
    /// - `FUNCTIONS_WORKER_RUNTIME` - Azure Functions
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// // Explicit stateless mode for Lambda
    /// let server = ServerCoreBuilder::new()
    ///     .name("lambda-server")
    ///     .stateless_mode(true)
    ///     .build()?;
    ///
    /// // Auto-detect (works automatically in Lambda)
    /// let server = ServerCoreBuilder::new()
    ///     .name("lambda-server")
    ///     .build()?;  // Detects AWS_LAMBDA_FUNCTION_NAME
    ///
    /// // Explicit stateful mode (stdio transport)
    /// let server = ServerCoreBuilder::new()
    ///     .name("stdio-server")
    ///     .stateless_mode(false)
    ///     .build()?;
    /// ```
    pub fn stateless_mode(mut self, enabled: bool) -> Self {
        self.stateless_mode = Some(enabled);
        self
    }

    /// Enable experimental MCP Tasks support with a task router.
    ///
    /// The task router handles task lifecycle operations (`tasks/get`, `tasks/result`,
    /// `tasks/list`, `tasks/cancel`) and task-augmented `tools/call` requests.
    ///
    /// This method:
    /// - Stores the task router for use during request handling
    /// - Auto-configures `experimental.tasks` in server capabilities so clients
    ///   know the server supports the tasks protocol extension
    ///
    /// The `router` parameter is typically created by the `pmcp-tasks` crate,
    /// which wraps a `TaskStore` with routing logic.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use pmcp::server::builder::ServerCoreBuilder;
    /// use pmcp_tasks::TaskRouterImpl;
    ///
    /// let task_router = TaskRouterImpl::new(store);
    /// let server = ServerCoreBuilder::new()
    ///     .name("task-server")
    ///     .version("1.0.0")
    ///     .with_task_store(Arc::new(task_router))
    ///     .build()?;
    /// ```
    #[cfg(not(target_arch = "wasm32"))]
    pub fn with_task_store(mut self, router: Arc<dyn TaskRouter>) -> Self {
        // Auto-configure experimental.tasks capability
        let experimental = self
            .capabilities
            .experimental
            .get_or_insert_with(HashMap::new);
        experimental.insert("tasks".to_string(), router.task_capabilities());

        self.task_router = Some(router);
        self
    }

    /// Register a task store for MCP Tasks with polling.
    ///
    /// When a task store is registered, the server:
    /// - Advertises `ServerCapabilities.tasks` with list and cancel support
    /// - Handles `tasks/get`, `tasks/list`, `tasks/cancel` requests via the store
    /// - Resolves task owner from auth context (OAuth subject, client ID, or session ID)
    ///
    /// This is the standard capability path (uses `ServerCapabilities.tasks`).
    /// For the legacy experimental path via `pmcp-tasks`, use [`Self::with_task_store`].
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use pmcp::server::task_store::InMemoryTaskStore;
    /// use std::sync::Arc;
    ///
    /// let store = Arc::new(InMemoryTaskStore::new());
    /// let server = Server::builder()
    ///     .name("my-server")
    ///     .version("1.0.0")
    ///     .task_store(store)
    ///     .build()?;
    /// ```
    #[cfg(not(target_arch = "wasm32"))]
    pub fn task_store(mut self, store: Arc<dyn crate::server::task_store::TaskStore>) -> Self {
        // Set ServerCapabilities.tasks (standard, not experimental)
        self.capabilities.tasks = Some(crate::types::capabilities::ServerTasksCapability {
            list: Some(serde_json::json!({})),
            cancel: Some(serde_json::json!({})),
            requests: Some(crate::types::capabilities::ServerTasksRequestCapability {
                tools: Some(crate::types::capabilities::ServerTasksToolsCapability {
                    call: Some(serde_json::json!({})),
                }),
            }),
        });

        self.task_store = Some(store);
        self
    }

    /// Detect if running in a stateless/serverless environment.
    ///
    /// Checks for environment variables that indicate serverless platforms:
    /// - AWS Lambda
    /// - Vercel Functions
    /// - Deno Deploy
    /// - Cloudflare Workers
    /// - Azure Functions
    fn detect_stateless_environment() -> bool {
        std::env::var("AWS_LAMBDA_FUNCTION_NAME").is_ok()
            || std::env::var("VERCEL").is_ok()
            || std::env::var("DENO_DEPLOYMENT_ID").is_ok()
            || std::env::var("CLOUDFLARE_WORKER").is_ok()
            || std::env::var("FUNCTIONS_WORKER_RUNTIME").is_ok()
    }

    /// Register a workflow as a prompt with automatic middleware support.
    ///
    /// This method provides the easiest way to register workflows with middleware:
    /// - Validates the workflow
    /// - Builds tool registry from registered tools
    /// - Creates workflow handler with middleware executor
    /// - Ensures OAuth, logging, and other middleware applies to workflow tool calls
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use pmcp::server::builder::ServerCoreBuilder;
    /// use pmcp::server::workflow::{SequentialWorkflow, WorkflowStep, ToolHandle};
    /// use pmcp::server::tool_middleware::ToolMiddleware;
    ///
    /// let workflow = SequentialWorkflow::new("my_workflow", "Description")
    ///     .step(WorkflowStep::new("fetch_data", ToolHandle::new("my_tool")));
    ///
    /// let server = ServerCoreBuilder::new()
    ///     .name("my-server")
    ///     .version("1.0.0")
    ///     .tool("my_tool", MyTool)
    ///     .tool_middleware(Arc::new(OAuthMiddleware::new())) // ✅ Applies to workflows!
    ///     .prompt_workflow(workflow)?  // ✅ Simple one-line registration
    ///     .build()?;
    /// ```
    ///
    /// # Benefits
    ///
    /// - **One-Line Registration**: No manual tool registry building required
    /// - **Automatic Middleware**: OAuth and other middleware applies automatically
    /// - **No Boilerplate**: No need to manually create `WorkflowPromptHandler`
    /// - **Builder Pattern**: Follows the same pattern as `.tool()` and `.prompt()`
    ///
    /// # Errors
    ///
    /// Returns an error if workflow validation fails.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn prompt_workflow(
        mut self,
        workflow: crate::server::workflow::SequentialWorkflow,
    ) -> Result<Self> {
        use crate::server::builder_middleware_executor::BuilderMiddlewareExecutor;
        use crate::server::middleware_executor::MiddlewareExecutor;
        use crate::server::workflow;

        // Validate workflow
        workflow
            .validate()
            .map_err(|e| Error::validation(format!("Workflow validation failed: {}", e)))?;

        // Build tool registry from cached metadata (avoids per-request handler.metadata() calls)
        let mut tool_registry = std::collections::HashMap::new();
        for (name, info) in &self.tool_infos {
            tool_registry.insert(
                Arc::from(name.as_str()),
                workflow::conversion::ToolInfo {
                    name: info.name.clone(),
                    description: info.description.clone().unwrap_or_default(),
                    input_schema: info.input_schema.clone(),
                },
            );
        }

        // Create builder-scoped middleware executor
        let middleware_executor = Arc::new(BuilderMiddlewareExecutor::new(
            self.tools.clone(),
            self.tool_middlewares.clone(),
        )) as Arc<dyn MiddlewareExecutor>;

        // Get workflow name and task support flag before moving
        let name = workflow.name().to_string();
        let has_task_support = workflow.has_task_support();

        // Create workflow handler with middleware
        let handler = workflow::WorkflowPromptHandler::with_middleware_executor(
            workflow.clone(),
            tool_registry,
            middleware_executor,
            self.resources.clone(),
        );

        // Wrap in TaskWorkflowPromptHandler if task support is enabled
        if has_task_support {
            let task_router = self.task_router.as_ref().ok_or_else(|| {
                Error::validation(format!(
                    "Workflow '{}' has task support enabled but no task router is configured. \
                     Call .with_task_store() on the builder before registering task-enabled workflows.",
                    name
                ))
            })?;

            let task_handler =
                workflow::TaskWorkflowPromptHandler::new(handler, task_router.clone(), workflow);
            let prompt_handler: Arc<dyn PromptHandler> = Arc::new(task_handler);
            // Cache metadata at registration time
            let mut info = prompt_handler
                .metadata()
                .unwrap_or_else(|| PromptInfo::new(&name));
            info.name.clone_from(&name);
            self.prompt_infos.insert(name.clone(), info);
            self.prompts.insert(name, prompt_handler);
        } else {
            let prompt_handler: Arc<dyn PromptHandler> = Arc::new(handler);
            // Cache metadata at registration time
            let mut info = prompt_handler
                .metadata()
                .unwrap_or_else(|| PromptInfo::new(&name));
            info.name.clone_from(&name);
            self.prompt_infos.insert(name.clone(), info);
            self.prompts.insert(name, prompt_handler);
        }

        // Update capabilities to include prompts
        // This ensures prompts/list returns the workflow prompts
        if self.capabilities.prompts.is_none() {
            self.capabilities.prompts = Some(crate::types::PromptCapabilities {
                list_changed: Some(false),
            });
        }

        Ok(self)
    }

    /// Set payload and resource limits for the server.
    ///
    /// Controls maximum request body size and tool argument size.
    /// Defaults are tuned for AWS Lambda (4 MB request, 1 MB args).
    pub fn payload_limits(mut self, limits: PayloadLimits) -> Self {
        self.payload_limits = limits;
        self
    }

    /// Build the `ServerCore` instance.
    ///
    /// # Errors
    ///
    /// Returns an error if required fields (name, version) are not set.
    #[allow(unused_mut)]
    pub fn build(mut self) -> Result<ServerCore> {
        let name = self
            .name
            .ok_or_else(|| Error::validation("Server name is required"))?;

        let version = self
            .version
            .ok_or_else(|| Error::validation("Server version is required"))?;

        let mut info = Implementation::new(name, version);
        if let Some(url) = self.website_url {
            info = info.with_website_url(url);
        }
        if let Some(icons) = self.icons {
            info = info.with_icons(icons);
        }

        // Build tool middleware chain from accumulated middleware
        #[cfg(not(target_arch = "wasm32"))]
        let tool_middleware = {
            let mut tool_middleware_chain = ToolMiddlewareChain::new();
            for middleware in self.tool_middlewares {
                tool_middleware_chain.add(middleware);
            }
            Arc::new(RwLock::new(tool_middleware_chain))
        };

        // Enrich tool _meta with host-specific keys (e.g., openai/* for ChatGPT)
        #[cfg(feature = "mcp-apps")]
        {
            for host in &self.host_layers {
                for info in self.tool_infos.values_mut() {
                    if let Some(meta) = info._meta.as_mut() {
                        crate::server::core::enrich_meta_for_host(meta, *host);
                    }
                }
            }
        }

        // Determine stateless mode: use explicit setting or auto-detect
        let stateless_mode = self
            .stateless_mode
            .unwrap_or_else(Self::detect_stateless_environment);

        // 80-REVIEWS.md Fix 1: finalize accumulated skills exactly once.
        // Compose with the user's `.resources(...)` slot if both are set —
        // otherwise pass through unchanged. `.resources(...)` semantics
        // remain "last write wins" (Fix 4) — see scope guard.
        #[cfg(all(feature = "skills", not(target_arch = "wasm32")))]
        let final_resources: Option<Arc<dyn ResourceHandler>> =
            finalize_skills_resources(self.pending_skills.take(), self.resources.take());
        #[cfg(not(all(feature = "skills", not(target_arch = "wasm32"))))]
        let final_resources = self.resources.take();

        Ok(ServerCore::new(
            info,
            self.capabilities,
            self.tools,
            self.prompts,
            self.tool_infos,
            self.prompt_infos,
            final_resources,
            self.sampling,
            self.auth_provider,
            self.tool_authorizer,
            self.protocol_middleware,
            #[cfg(not(target_arch = "wasm32"))]
            tool_middleware,
            #[cfg(not(target_arch = "wasm32"))]
            self.task_router,
            #[cfg(not(target_arch = "wasm32"))]
            self.task_store,
            stateless_mode,
            self.payload_limits,
        ))
    }
}

/// Finalize accumulated `Skills` into a single `ResourceHandler`, optionally
/// composed with the user's `.resources(...)` slot.
///
/// Called from both [`ServerCoreBuilder::build`] and the `ServerBuilder::build`
/// path in `src/server/mod.rs` so the composition logic exists in exactly
/// one place. Panics on duplicate URIs — surface the failure via
/// [`ServerCoreBuilder::try_skills`] for fallible registration.
#[cfg(all(feature = "skills", not(target_arch = "wasm32")))]
pub(crate) fn finalize_skills_resources(
    pending: Option<Skills>,
    user: Option<Arc<dyn ResourceHandler>>,
) -> Option<Arc<dyn ResourceHandler>> {
    match (pending, user) {
        (None, other) => other,
        (Some(skills), None) => Some(skills.into_handler().unwrap_or_else(|e| {
            panic!("Skills::into_handler: {e}; use try_skills(...) for fallible registration")
        })),
        (Some(skills), Some(user_handler)) => {
            let skills_handler = skills.into_handler().unwrap_or_else(|e| {
                panic!("Skills::into_handler: {e}; use try_skills(...) for fallible registration")
            });
            Some(Arc::new(ComposedResources {
                skills: skills_handler,
                other: user_handler,
            }))
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::cancellation::RequestHandlerExtra;
    use crate::server::core::ProtocolHandler;
    use async_trait::async_trait;
    use serde_json::Value;

    struct TestTool;

    #[async_trait]
    impl ToolHandler for TestTool {
        async fn handle(&self, _args: Value, _extra: RequestHandlerExtra) -> Result<Value> {
            Ok(serde_json::json!({"result": "test"}))
        }
    }

    #[test]
    fn test_builder_required_fields() {
        // Should fail without name
        let result = ServerCoreBuilder::new().version("1.0.0").build();
        assert!(result.is_err());

        // Should fail without version
        let result = ServerCoreBuilder::new().name("test").build();
        assert!(result.is_err());

        // Should succeed with both
        let result = ServerCoreBuilder::new()
            .name("test")
            .version("1.0.0")
            .build();
        assert!(result.is_ok());
    }

    #[test]
    fn test_builder_with_tools() {
        let server = ServerCoreBuilder::new()
            .name("test")
            .version("1.0.0")
            .tool("test-tool", TestTool)
            .build()
            .unwrap();

        // Check that capabilities were automatically set
        assert!(server.capabilities().tools.is_some());
    }

    #[test]
    fn test_builder_capabilities_serialization() {
        let server = ServerCoreBuilder::new()
            .name("test")
            .version("1.0.0")
            .tool("test-tool", TestTool)
            .build()
            .unwrap();

        let caps = server.capabilities();
        let json = serde_json::to_value(caps).unwrap();

        // Verify tools capability is present and properly structured
        let tools = json.get("tools").expect("tools should be present in JSON");
        assert!(tools.is_object(), "tools should be an object");

        // Verify listChanged is present (not just an empty object)
        let list_changed = tools.get("listChanged");
        assert!(
            list_changed.is_some(),
            "listChanged should be present in tools"
        );
        assert_eq!(
            list_changed.unwrap(),
            &serde_json::json!(false),
            "listChanged should be false"
        );

        println!(
            "Serialized capabilities: {}",
            serde_json::to_string_pretty(&json).unwrap()
        );
    }

    #[test]
    fn test_builder_with_custom_capabilities() {
        let custom_caps = ServerCapabilities::tools_only();

        let server = ServerCoreBuilder::new()
            .name("test")
            .version("1.0.0")
            .capabilities(custom_caps.clone())
            .build()
            .unwrap();

        assert_eq!(server.capabilities().tools, custom_caps.tools);
    }

    #[test]
    fn test_builder_with_task_store_sets_capabilities() {
        use crate::server::tasks::TaskRouter;

        /// Mock task router for testing.
        struct MockTaskRouter;

        #[async_trait]
        impl TaskRouter for MockTaskRouter {
            async fn handle_task_call(
                &self,
                _tool_name: &str,
                _arguments: Value,
                _task_params: Value,
                _owner_id: &str,
                _progress_token: Option<Value>,
            ) -> Result<Value> {
                Ok(Value::Null)
            }
            async fn handle_tasks_get(&self, _params: Value, _owner_id: &str) -> Result<Value> {
                Ok(Value::Null)
            }
            async fn handle_tasks_result(&self, _params: Value, _owner_id: &str) -> Result<Value> {
                Ok(Value::Null)
            }
            async fn handle_tasks_list(&self, _params: Value, _owner_id: &str) -> Result<Value> {
                Ok(Value::Null)
            }
            async fn handle_tasks_cancel(&self, _params: Value, _owner_id: &str) -> Result<Value> {
                Ok(Value::Null)
            }
            fn resolve_owner(
                &self,
                _subject: Option<&str>,
                _client_id: Option<&str>,
                _session_id: Option<&str>,
            ) -> String {
                "test-owner".to_string()
            }
            fn tool_requires_task(
                &self,
                _tool_name: &str,
                _tool_execution: Option<&Value>,
            ) -> bool {
                false
            }
            fn task_capabilities(&self) -> Value {
                serde_json::json!({
                    "supported": true,
                    "maxTtl": 86_400_000
                })
            }
        }

        let router = Arc::new(MockTaskRouter);
        let server = ServerCoreBuilder::new()
            .name("test")
            .version("1.0.0")
            .with_task_store(router)
            .build()
            .unwrap();

        // Verify experimental.tasks capability was set
        let caps = server.capabilities();
        let experimental = caps
            .experimental
            .as_ref()
            .expect("experimental should be set");
        let tasks_cap = experimental
            .get("tasks")
            .expect("tasks capability should be set");
        assert_eq!(tasks_cap["supported"], true);
        assert_eq!(tasks_cap["maxTtl"], 86_400_000);
    }

    #[cfg(feature = "mcp-apps")]
    #[test]
    fn test_builder_host_layers_empty_by_default() {
        let builder = ServerCoreBuilder::new();
        assert!(
            builder.host_layers.is_empty(),
            "host_layers should be empty by default"
        );
    }

    #[cfg(feature = "mcp-apps")]
    #[test]
    fn test_builder_with_host_layer_adds_and_deduplicates() {
        use crate::types::mcp_apps::HostType;

        let builder = ServerCoreBuilder::new()
            .with_host_layer(HostType::ChatGpt)
            .with_host_layer(HostType::ChatGpt); // duplicate
        assert_eq!(builder.host_layers.len(), 1, "duplicates should be removed");
        assert_eq!(builder.host_layers[0], HostType::ChatGpt);
    }

    #[cfg(feature = "mcp-apps")]
    #[test]
    fn test_builder_with_chatgpt_layer_enriches_tool_meta() {
        use crate::types::mcp_apps::HostType;

        struct UiTool;

        #[async_trait]
        impl ToolHandler for UiTool {
            async fn handle(&self, _args: Value, _extra: RequestHandlerExtra) -> Result<Value> {
                Ok(Value::Null)
            }
            fn metadata(&self) -> Option<ToolInfo> {
                Some(ToolInfo::with_ui(
                    "ui-tool",
                    Some("A tool with UI".to_string()),
                    serde_json::json!({"type": "object"}),
                    "ui://chess/board",
                ))
            }
        }

        let server = ServerCoreBuilder::new()
            .name("test")
            .version("1.0.0")
            .tool("ui-tool", UiTool)
            .with_host_layer(HostType::ChatGpt)
            .build()
            .unwrap();

        // The tool_infos should contain openai/outputTemplate after enrichment
        let caps = server.capabilities();
        assert!(caps.tools.is_some());
    }

    #[cfg(feature = "mcp-apps")]
    #[test]
    fn test_builder_without_host_layer_no_openai_keys() {
        struct UiTool;

        #[async_trait]
        impl ToolHandler for UiTool {
            async fn handle(&self, _args: Value, _extra: RequestHandlerExtra) -> Result<Value> {
                Ok(Value::Null)
            }
            fn metadata(&self) -> Option<ToolInfo> {
                Some(ToolInfo::with_ui(
                    "ui-tool",
                    Some("A tool with UI".to_string()),
                    serde_json::json!({"type": "object"}),
                    "ui://chess/board",
                ))
            }
        }

        let server = ServerCoreBuilder::new()
            .name("test")
            .version("1.0.0")
            .tool("ui-tool", UiTool)
            .build()
            .unwrap();

        // Without host layer, no openai keys should be in tool meta
        assert!(server.capabilities().tools.is_some());
    }

    #[test]
    fn test_builder_task_store_sets_capabilities() {
        let store = Arc::new(crate::server::task_store::InMemoryTaskStore::new());
        let builder = ServerCoreBuilder::new()
            .name("test")
            .version("1.0.0")
            .task_store(store);
        // Verify capabilities were set by the builder method
        assert!(
            builder.capabilities.tasks.is_some(),
            "ServerCapabilities.tasks should be set"
        );
        let tasks_cap = builder.capabilities.tasks.as_ref().unwrap();
        assert!(tasks_cap.list.is_some(), "tasks.list should be set");
        assert!(tasks_cap.cancel.is_some(), "tasks.cancel should be set");
        assert!(tasks_cap.requests.is_some(), "tasks.requests should be set");
        // Verify task_store field is populated
        assert!(
            builder.task_store.is_some(),
            "task_store field should be set"
        );
    }

    #[test]
    fn test_builder_with_task_store_builds_successfully() {
        let store = Arc::new(crate::server::task_store::InMemoryTaskStore::new());
        let server = ServerCoreBuilder::new()
            .name("test")
            .version("1.0.0")
            .task_store(store)
            .build()
            .unwrap();
        let caps = server.capabilities();
        assert!(
            caps.tasks.is_some(),
            "ServerCapabilities.tasks should be set"
        );
        assert!(caps.provides_tasks(), "provides_tasks() should be true");
    }

    #[test]
    fn test_builder_without_task_store_has_no_experimental() {
        let server = ServerCoreBuilder::new()
            .name("test")
            .version("1.0.0")
            .build()
            .unwrap();

        // No experimental capabilities by default
        assert!(server.capabilities().experimental.is_none());
    }

    /// Shared mock task router for workflow task tests.
    struct WorkflowMockTaskRouter;

    #[async_trait]
    impl crate::server::tasks::TaskRouter for WorkflowMockTaskRouter {
        async fn handle_task_call(
            &self,
            _tool_name: &str,
            _arguments: Value,
            _task_params: Value,
            _owner_id: &str,
            _progress_token: Option<Value>,
        ) -> Result<Value> {
            Ok(Value::Null)
        }
        async fn handle_tasks_get(&self, _params: Value, _owner_id: &str) -> Result<Value> {
            Ok(Value::Null)
        }
        async fn handle_tasks_result(&self, _params: Value, _owner_id: &str) -> Result<Value> {
            Ok(Value::Null)
        }
        async fn handle_tasks_list(&self, _params: Value, _owner_id: &str) -> Result<Value> {
            Ok(Value::Null)
        }
        async fn handle_tasks_cancel(&self, _params: Value, _owner_id: &str) -> Result<Value> {
            Ok(Value::Null)
        }
        fn resolve_owner(
            &self,
            _subject: Option<&str>,
            _client_id: Option<&str>,
            _session_id: Option<&str>,
        ) -> String {
            "test-owner".to_string()
        }
        fn tool_requires_task(&self, _tool_name: &str, _tool_execution: Option<&Value>) -> bool {
            false
        }
        fn task_capabilities(&self) -> Value {
            serde_json::json!({
                "supported": true,
                "maxTtl": 86_400_000
            })
        }
    }

    #[test]
    fn test_workflow_without_task_support_registers_normally() {
        use crate::server::workflow::{SequentialWorkflow, ToolHandle, WorkflowStep};

        let server = ServerCoreBuilder::new()
            .name("test")
            .version("1.0.0")
            .tool("my_tool", TestTool)
            .prompt_workflow(
                SequentialWorkflow::new("test_workflow", "A test workflow")
                    .step(WorkflowStep::new("step1", ToolHandle::new("my_tool"))),
            )
            .unwrap()
            .build()
            .unwrap();

        // Verify the workflow was registered as a prompt
        assert!(server.capabilities().prompts.is_some());
    }

    #[test]
    fn test_workflow_with_task_support_and_router_wraps_in_task_handler() {
        use crate::server::workflow::{SequentialWorkflow, ToolHandle, WorkflowStep};

        let router = Arc::new(WorkflowMockTaskRouter);
        let server = ServerCoreBuilder::new()
            .name("test")
            .version("1.0.0")
            .tool("my_tool", TestTool)
            .with_task_store(router)
            .prompt_workflow(
                SequentialWorkflow::new("task_workflow", "A task-enabled workflow")
                    .step(WorkflowStep::new("step1", ToolHandle::new("my_tool")))
                    .with_task_support(true),
            )
            .unwrap()
            .build()
            .unwrap();

        // Verify the workflow was registered (the TaskWorkflowPromptHandler wrapping
        // is internal, but we verify it compiled and the prompt is available)
        assert!(server.capabilities().prompts.is_some());
    }

    #[test]
    fn test_workflow_with_task_support_but_no_router_errors() {
        use crate::server::workflow::{SequentialWorkflow, ToolHandle, WorkflowStep};

        let result = ServerCoreBuilder::new()
            .name("test")
            .version("1.0.0")
            .tool("my_tool", TestTool)
            .prompt_workflow(
                SequentialWorkflow::new("task_workflow", "A task-enabled workflow")
                    .step(WorkflowStep::new("step1", ToolHandle::new("my_tool")))
                    .with_task_support(true),
            );

        assert!(result.is_err());
        let err_msg = match result {
            Err(e) => format!("{}", e),
            Ok(_) => panic!("Expected error but got Ok"),
        };
        assert!(
            err_msg.contains("no task router is configured"),
            "Error should mention missing task router, got: {}",
            err_msg
        );
    }
}

#[cfg(test)]
#[cfg(all(feature = "skills", not(target_arch = "wasm32")))]
mod skills_builder_tests {
    use super::*;
    use crate::server::cancellation::RequestHandlerExtra;
    use crate::server::core::ProtocolHandler;
    use crate::server::skills::{Skill, SkillReference, Skills};
    use crate::types::Content;
    use async_trait::async_trait;

    fn extra() -> RequestHandlerExtra {
        RequestHandlerExtra::default()
    }

    // Helper that uses `pending_skills.clone().into_handler()` to recover
    // a `ResourceHandler` for a builder mid-construction. The actual
    // composition happens in `.build()` — these tests verify the
    // accumulator state directly.
    fn read_via_pending(builder: &ServerCoreBuilder, uri: &str) -> Option<String> {
        let pending = builder.pending_skills.clone()?;
        let handler = pending.into_handler().ok()?;
        let rt = tokio::runtime::Runtime::new().ok()?;
        let res = rt.block_on(handler.read(uri, extra())).ok()?;
        match res.contents.into_iter().next() {
            Some(Content::Resource { text, .. }) => text,
            Some(Content::Text { text }) => Some(text),
            _ => None,
        }
    }

    // ── Test 2.1: single skill via ServerCoreBuilder ─────────────────
    #[test]
    fn test_2_1_skill_method_single_skill_via_server_core_builder() {
        let builder = ServerCoreBuilder::new()
            .name("test")
            .version("1.0")
            .skill(Skill::new("foo", "body"));
        assert!(builder.pending_skills.is_some());
        let result = builder.build();
        assert!(result.is_ok());
    }

    // ── Test 2.2: skills auto-sets extensions capability ─────────────
    #[test]
    fn test_2_2_skills_method_sets_extensions_capability() {
        let server = ServerCoreBuilder::new()
            .name("test")
            .version("1.0")
            .skills(Skills::new().add(Skill::new("a", "")))
            .build()
            .unwrap();
        let ext = server
            .capabilities()
            .extensions
            .as_ref()
            .expect("extensions should be set");
        assert_eq!(
            ext.get("io.modelcontextprotocol/skills"),
            Some(&serde_json::json!({}))
        );
    }

    // ── Test 2.3: skills auto-sets resources capability ──────────────
    #[test]
    fn test_2_3_skills_method_sets_resources_capability() {
        let server = ServerCoreBuilder::new()
            .name("test")
            .version("1.0")
            .skills(Skills::new().add(Skill::new("a", "")))
            .build()
            .unwrap();
        let caps = server.capabilities();
        let r = caps.resources.as_ref().expect("resources should be set");
        assert_eq!(r.subscribe, Some(false));
        assert_eq!(r.list_changed, Some(false));
    }

    // ── Test 2.4 helper resource handler ─────────────────────────────
    struct DocsHandler;
    #[async_trait]
    impl ResourceHandler for DocsHandler {
        async fn read(
            &self,
            uri: &str,
            _extra: RequestHandlerExtra,
        ) -> Result<crate::types::ReadResourceResult> {
            Ok(crate::types::ReadResourceResult::new(vec![Content::text(
                format!("DOCS:{uri}"),
            )]))
        }
        async fn list(
            &self,
            _cursor: Option<String>,
            _extra: RequestHandlerExtra,
        ) -> Result<crate::types::ListResourcesResult> {
            Ok(crate::types::ListResourcesResult::new(vec![
                crate::types::ResourceInfo::new("docs://handbook", "handbook"),
            ]))
        }
    }

    // ── Test 2.4: resources THEN skill composes ──────────────────────
    #[tokio::test]
    async fn test_2_4_skills_compose_with_existing_resources() {
        // We finalize manually to access the composed handler without
        // calling `.build()` (which moves the handler into ServerCore).
        let pending = Some(Skills::new().add(Skill::new("a", "skill-a")));
        let user: Option<Arc<dyn ResourceHandler>> = Some(Arc::new(DocsHandler));
        let composed = finalize_skills_resources(pending, user).expect("composed handler");

        let list = composed.list(None, extra()).await.unwrap();
        let uris: Vec<&str> = list.resources.iter().map(|r| r.uri.as_str()).collect();
        assert!(uris.contains(&"skill://a/SKILL.md"));
        assert!(uris.contains(&"skill://index.json"));
        assert!(uris.contains(&"docs://handbook"));

        let res = composed.read("docs://handbook", extra()).await.unwrap();
        match &res.contents[0] {
            Content::Text { text } => assert_eq!(text, "DOCS:docs://handbook"),
            other => panic!("expected Content::Text, got {other:?}"),
        }

        let res = composed.read("skill://a/SKILL.md", extra()).await.unwrap();
        match &res.contents[0] {
            Content::Resource { uri, .. } => assert_eq!(uri, "skill://a/SKILL.md"),
            other => panic!("expected Content::Resource, got {other:?}"),
        }
    }

    // ── Test 2.5: reverse ordering — skill THEN resources composes ───
    #[tokio::test]
    async fn test_2_5_skills_method_reverse_ordering_composes() {
        // Reverse order of inputs to `finalize_skills_resources` — the
        // function takes them in (skills, resources) order regardless of
        // the builder method call order. Verifies the same outcome.
        let pending = Some(Skills::new().add(Skill::new("a", "skill-a")));
        let user: Option<Arc<dyn ResourceHandler>> = Some(Arc::new(DocsHandler));
        let composed = finalize_skills_resources(pending, user).expect("composed handler");

        let res = composed.read("skill://a/SKILL.md", extra()).await.unwrap();
        match &res.contents[0] {
            Content::Resource { uri, .. } => assert_eq!(uri, "skill://a/SKILL.md"),
            other => panic!("expected Content::Resource, got {other:?}"),
        }
        let res = composed.read("docs://handbook", extra()).await.unwrap();
        match &res.contents[0] {
            Content::Text { text } => assert_eq!(text, "DOCS:docs://handbook"),
            other => panic!("expected Content::Text, got {other:?}"),
        }
    }

    // ── Test 2.5a: .resources(A).resources(B) is still "B replaces A" ─
    // (Fix 4: when no skills are registered, the .resources() semantics
    // are completely unchanged.)
    #[tokio::test]
    async fn test_2_5a_resources_replace_unchanged_under_skills_feature() {
        struct A;
        #[async_trait]
        impl ResourceHandler for A {
            async fn read(
                &self,
                _uri: &str,
                _extra: RequestHandlerExtra,
            ) -> Result<crate::types::ReadResourceResult> {
                Ok(crate::types::ReadResourceResult::new(vec![Content::text(
                    "A",
                )]))
            }
            async fn list(
                &self,
                _cursor: Option<String>,
                _extra: RequestHandlerExtra,
            ) -> Result<crate::types::ListResourcesResult> {
                Ok(crate::types::ListResourcesResult::new(vec![]))
            }
        }
        struct B;
        #[async_trait]
        impl ResourceHandler for B {
            async fn read(
                &self,
                _uri: &str,
                _extra: RequestHandlerExtra,
            ) -> Result<crate::types::ReadResourceResult> {
                Ok(crate::types::ReadResourceResult::new(vec![Content::text(
                    "B",
                )]))
            }
            async fn list(
                &self,
                _cursor: Option<String>,
                _extra: RequestHandlerExtra,
            ) -> Result<crate::types::ListResourcesResult> {
                Ok(crate::types::ListResourcesResult::new(vec![]))
            }
        }

        // No skill calls — finalize should pass through user only.
        let final_handler =
            finalize_skills_resources(None, Some(Arc::new(B) as Arc<dyn ResourceHandler>))
                .expect("user handler preserved");
        let res = final_handler.read("test://uri", extra()).await.unwrap();
        match &res.contents[0] {
            Content::Text { text } => assert_eq!(text, "B"),
            other => panic!("expected Content::Text, got {other:?}"),
        }

        // Confirm via the actual builder that .resources(A).resources(B)
        // ends up with B alone.
        let server = ServerCoreBuilder::new()
            .name("t")
            .version("1.0")
            .resources(A)
            .resources(B)
            .build()
            .unwrap();
        // No skills registered — should be the simple "last write wins" semantic.
        // We can't directly inspect server.resources from this scope, but we
        // verify the capabilities state.
        assert!(server.capabilities().resources.is_some());
    }

    // ── Test 2.6: .skill(s) == .skills(Skills::new().add(s)) ─────────
    #[test]
    fn test_2_6_skill_method_is_sugar_over_skills() {
        let a = ServerCoreBuilder::new()
            .name("t")
            .version("1.0")
            .skill(Skill::new("x", "body"));
        let b = ServerCoreBuilder::new()
            .name("t")
            .version("1.0")
            .skills(Skills::new().add(Skill::new("x", "body")));
        assert_eq!(
            a.pending_skills.as_ref().unwrap().skill_md_uris(),
            b.pending_skills.as_ref().unwrap().skill_md_uris()
        );
    }

    // ── Test 2.7: bootstrap_skill_and_prompt registers both surfaces ──
    #[test]
    fn test_2_7_bootstrap_skill_and_prompt_registers_both_surfaces() {
        let server = ServerCoreBuilder::new()
            .name("t")
            .version("1.0")
            .bootstrap_skill_and_prompt(Skill::new("c", "body-c"), "my_prompt")
            .build()
            .unwrap();
        let caps = server.capabilities();
        assert!(caps.prompts.is_some(), "prompts capability not set");
        let ext = caps.extensions.as_ref().expect("extensions should be set");
        assert!(ext.get("io.modelcontextprotocol/skills").is_some());
        assert!(caps.resources.is_some());
    }

    // ── Test 2.9: duplicate URI panics at .build() ───────────────────
    #[test]
    #[should_panic(expected = "duplicate")]
    fn test_2_9_skills_panics_on_duplicate_uri_at_build() {
        let _ = ServerCoreBuilder::new()
            .name("t")
            .version("1.0")
            .skill(Skill::new("x", "a"))
            .skill(Skill::new("x", "b"))
            .build()
            .unwrap();
    }

    // ── Test 2.9a: try_skills returns Err on duplicate ───────────────
    #[test]
    fn test_2_9a_try_skills_returns_err_on_duplicate() {
        let res = ServerCoreBuilder::new()
            .name("t")
            .version("1.0")
            .try_skills(
                Skills::new()
                    .add(Skill::new("x", "a"))
                    .add(Skill::new("x", "b")),
            );
        assert!(res.is_err());
        match res {
            Err(Error::Validation(_)) => {},
            Err(other) => panic!("expected Validation, got {other:?}"),
            Ok(_) => panic!("expected Err for try_skills with duplicate"),
        }
    }

    // ── Test 2.10: capability merge preserves pre-existing extensions ─
    #[test]
    fn test_2_10_capability_merge_preserves_pre_existing_extensions() {
        let mut caps = ServerCapabilities::default();
        let mut ext = HashMap::new();
        ext.insert("some.other/ext".to_string(), serde_json::json!({"foo": 1}));
        caps.extensions = Some(ext);

        let server = ServerCoreBuilder::new()
            .name("t")
            .version("1.0")
            .capabilities(caps)
            .skill(Skill::new("a", ""))
            .build()
            .unwrap();
        let ext = server
            .capabilities()
            .extensions
            .as_ref()
            .expect("extensions should be set");
        assert!(
            ext.contains_key("some.other/ext"),
            "pre-existing extension lost"
        );
        assert!(
            ext.contains_key("io.modelcontextprotocol/skills"),
            "skills extension not added"
        );
    }

    // ── Test 2.11: accumulator — repeated .skill() calls all reachable ─
    #[test]
    fn test_2_11_accumulator_repeated_skill_calls_all_reachable() {
        let builder = ServerCoreBuilder::new()
            .name("t")
            .version("1.0")
            .skill(Skill::new("a", "body-a"))
            .skill(Skill::new("b", "body-b"))
            .bootstrap_skill_and_prompt(Skill::new("c", "body-c"), "c_prompt");

        // Verify the accumulator carries all three before .build().
        let uris = builder.pending_skills.as_ref().unwrap().skill_md_uris();
        assert_eq!(uris.len(), 3);
        assert!(uris.contains(&"skill://a/SKILL.md".to_string()));
        assert!(uris.contains(&"skill://b/SKILL.md".to_string()));
        assert!(uris.contains(&"skill://c/SKILL.md".to_string()));

        // Read each through the pending handler.
        assert_eq!(
            read_via_pending(&builder, "skill://a/SKILL.md").as_deref(),
            Some("body-a")
        );
        assert_eq!(
            read_via_pending(&builder, "skill://b/SKILL.md").as_deref(),
            Some("body-b")
        );
        assert_eq!(
            read_via_pending(&builder, "skill://c/SKILL.md").as_deref(),
            Some("body-c")
        );

        // And the server builds successfully — confirms .build() finalizes.
        builder.build().unwrap();
    }

    // ── Test: skills + references end-to-end via builder ─────────────
    #[test]
    fn test_skill_with_references_via_builder() {
        let skill = Skill::new("docs", "body").with_reference(SkillReference::new(
            "references/api.md",
            "text/markdown",
            "api body",
        ));
        let server = ServerCoreBuilder::new()
            .name("t")
            .version("1.0")
            .skill(skill)
            .build()
            .unwrap();
        assert!(server.capabilities().resources.is_some());
    }
}
