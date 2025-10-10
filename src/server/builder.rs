//! Builder pattern for constructing `ServerCore` instances.

use crate::error::{Error, Result};
use crate::runtime::RwLock;
use crate::server::auth::{AuthProvider, ToolAuthorizer};
use crate::server::core::ServerCore;
use crate::server::{PromptHandler, ResourceHandler, SamplingHandler, ToolHandler};
use crate::shared::middleware::EnhancedMiddlewareChain;
use crate::types::{Implementation, ServerCapabilities};
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
    resources: Option<Arc<dyn ResourceHandler>>,
    sampling: Option<Arc<dyn SamplingHandler>>,
    auth_provider: Option<Arc<dyn AuthProvider>>,
    tool_authorizer: Option<Arc<dyn ToolAuthorizer>>,
    protocol_middleware: Arc<RwLock<EnhancedMiddlewareChain>>,
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
            resources: None,
            sampling: None,
            auth_provider: None,
            tool_authorizer: None,
            protocol_middleware: Arc::new(RwLock::new(EnhancedMiddlewareChain::new())),
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
        self.tools
            .insert(name.into(), Arc::new(handler) as Arc<dyn ToolHandler>);

        // Update capabilities to include tools
        if self.capabilities.tools.is_none() {
            self.capabilities.tools = Some(crate::types::ToolCapabilities { list_changed: None });
        }

        self
    }

    /// Add a tool handler with an Arc.
    ///
    /// This variant is useful when you need to share the handler across multiple servers.
    pub fn tool_arc(mut self, name: impl Into<String>, handler: Arc<dyn ToolHandler>) -> Self {
        self.tools.insert(name.into(), handler);

        // Update capabilities to include tools
        if self.capabilities.tools.is_none() {
            self.capabilities.tools = Some(crate::types::ToolCapabilities { list_changed: None });
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
        self.prompts
            .insert(name.into(), Arc::new(handler) as Arc<dyn PromptHandler>);

        // Update capabilities to include prompts
        if self.capabilities.prompts.is_none() {
            self.capabilities.prompts =
                Some(crate::types::PromptCapabilities { list_changed: None });
        }

        self
    }

    /// Add a prompt handler with an Arc.
    ///
    /// This variant is useful when you need to share the handler across multiple servers.
    pub fn prompt_arc(mut self, name: impl Into<String>, handler: Arc<dyn PromptHandler>) -> Self {
        self.prompts.insert(name.into(), handler);

        // Update capabilities to include prompts
        if self.capabilities.prompts.is_none() {
            self.capabilities.prompts =
                Some(crate::types::PromptCapabilities { list_changed: None });
        }

        self
    }

    /// Set the resource handler.
    ///
    /// Resources provide access to data that the client can read.
    pub fn resources(mut self, handler: impl ResourceHandler + 'static) -> Self {
        self.resources = Some(Arc::new(handler) as Arc<dyn ResourceHandler>);

        // Update capabilities to include resources
        if self.capabilities.resources.is_none() {
            self.capabilities.resources = Some(crate::types::ResourceCapabilities {
                subscribe: None,
                list_changed: None,
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
        if self.capabilities.resources.is_none() {
            self.capabilities.resources = Some(crate::types::ResourceCapabilities {
                subscribe: None,
                list_changed: None,
            });
        }

        self
    }

    /// Set the sampling handler.
    ///
    /// Sampling provides LLM capabilities for message generation.
    pub fn sampling(mut self, handler: impl SamplingHandler + 'static) -> Self {
        self.sampling = Some(Arc::new(handler) as Arc<dyn SamplingHandler>);

        // Update capabilities to include sampling
        if self.capabilities.sampling.is_none() {
            self.capabilities.sampling = Some(crate::types::SamplingCapabilities { models: None });
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
            self.capabilities.sampling = Some(crate::types::SamplingCapabilities { models: None });
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

    /// Build the `ServerCore` instance.
    ///
    /// # Errors
    ///
    /// Returns an error if required fields (name, version) are not set.
    pub fn build(self) -> Result<ServerCore> {
        let name = self
            .name
            .ok_or_else(|| Error::validation("Server name is required"))?;

        let version = self
            .version
            .ok_or_else(|| Error::validation("Server version is required"))?;

        let info = Implementation { name, version };

        Ok(ServerCore::new(
            info,
            self.capabilities,
            self.tools,
            self.prompts,
            self.resources,
            self.sampling,
            self.auth_provider,
            self.tool_authorizer,
            self.protocol_middleware,
        ))
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
}
