//! Capability definitions for MCP clients and servers.
//!
//! This module defines the capability structures that clients and servers
//! use to advertise their supported features.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Client capabilities advertised during initialization.
///
/// # Examples
///
/// ```rust
/// use pmcp::types::ClientCapabilities;
///
/// let mut capabilities = ClientCapabilities::default();
/// capabilities.experimental = Some([("custom-feature".to_string(), serde_json::json!(true))]
///     .into_iter()
///     .collect());
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct ClientCapabilities {
    /// Sampling capabilities (for LLM providers)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sampling: Option<SamplingCapabilities>,

    /// Elicitation capabilities (for user input)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elicitation: Option<ElicitationCapabilities>,

    /// Roots capabilities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roots: Option<RootsCapabilities>,

    /// Task capabilities (MCP 2025-11-25)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tasks: Option<ClientTasksCapability>,

    /// Experimental capabilities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experimental: Option<HashMap<String, serde_json::Value>>,
}

/// Server capabilities advertised during initialization.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct ServerCapabilities {
    /// Tool providing capabilities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<ToolCapabilities>,

    /// Prompt providing capabilities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompts: Option<PromptCapabilities>,

    /// Resource providing capabilities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<ResourceCapabilities>,

    /// Logging capabilities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logging: Option<LoggingCapabilities>,

    /// Completion capabilities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completions: Option<CompletionCapabilities>,

    /// Sampling capabilities (for LLM providers)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sampling: Option<SamplingCapabilities>,

    /// Task capabilities (MCP 2025-11-25)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tasks: Option<ServerTasksCapability>,

    /// Experimental capabilities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experimental: Option<HashMap<String, serde_json::Value>>,

    /// Extension capabilities — reverse-domain-keyed protocol extensions.
    ///
    /// This is the wire-correct home for declarations from the Extensions
    /// Track of MCP (SEPs that ship as extensions rather than as core protocol
    /// changes). Mandated by SEP-2640 §6 for the
    /// `io.modelcontextprotocol/skills` identifier.
    ///
    /// Use `experimental` only for pre-SEP, pre-namespaced flags. New
    /// extensions belong here.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::types::ServerCapabilities;
    /// use std::collections::HashMap;
    ///
    /// let mut caps = ServerCapabilities::default();
    /// let mut ext = HashMap::new();
    /// ext.insert(
    ///     "io.modelcontextprotocol/skills".to_string(),
    ///     serde_json::json!({}),
    /// );
    /// caps.extensions = Some(ext);
    /// ```
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Option<HashMap<String, serde_json::Value>>,
}

/// Tool-related capabilities.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCapabilities {
    /// Whether list changes are supported
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
}

/// Prompt-related capabilities.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptCapabilities {
    /// Whether list changes are supported
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
}

/// Resource-related capabilities.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceCapabilities {
    /// Whether resource subscriptions are supported
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subscribe: Option<bool>,

    /// Whether list changes are supported
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
}

/// Logging capabilities.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoggingCapabilities {
    /// Supported log levels
    #[serde(skip_serializing_if = "Option::is_none")]
    pub levels: Option<Vec<String>>,
}

/// Sampling capabilities for LLM operations (expanded MCP 2025-11-25).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SamplingCapabilities {
    /// Supported model families/providers
    #[serde(skip_serializing_if = "Option::is_none")]
    pub models: Option<Vec<String>>,
    /// Context capabilities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<Value>,
    /// Tool use capabilities during sampling
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Value>,
}

/// Elicitation capabilities for user input (expanded MCP 2025-11-25).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElicitationCapabilities {
    /// Form-based elicitation support
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form: Option<FormElicitationCapability>,
    /// URL-based elicitation support
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<Value>,
}

/// Form-based elicitation capability options.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormElicitationCapability {
    /// Whether the client supports applying default values
    #[serde(skip_serializing_if = "Option::is_none")]
    pub apply_defaults: Option<bool>,
}

/// Server task capabilities (MCP 2025-11-25).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerTasksCapability {
    /// Whether tasks/list is supported
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list: Option<Value>,
    /// Whether tasks/cancel is supported
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cancel: Option<Value>,
    /// Request-specific task capabilities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requests: Option<ServerTasksRequestCapability>,
}

/// Server task request capabilities.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerTasksRequestCapability {
    /// Tool-specific task capabilities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<ServerTasksToolsCapability>,
}

/// Server task tools capabilities.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerTasksToolsCapability {
    /// Whether tools/call can create tasks
    #[serde(skip_serializing_if = "Option::is_none")]
    pub call: Option<Value>,
}

/// Client task capabilities (MCP 2025-11-25).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientTasksCapability {
    /// Whether tasks/list is supported
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list: Option<Value>,
    /// Whether tasks/cancel is supported
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cancel: Option<Value>,
    /// Request-specific task capabilities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requests: Option<ClientTasksRequestCapability>,
}

/// Client task request capabilities.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientTasksRequestCapability {
    /// Sampling-related task capabilities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sampling: Option<Value>,
    /// Elicitation-related task capabilities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elicitation: Option<Value>,
}

/// Roots capabilities.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RootsCapabilities {
    /// Whether list changed notifications are supported
    #[serde(default)]
    pub list_changed: bool,
}

/// Completion capabilities.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionCapabilities {
    /// Placeholder for completion capability options
    #[serde(skip)]
    _reserved: (),
}

impl ClientCapabilities {
    /// Create a minimal set of client capabilities.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::ClientCapabilities;
    ///
    /// // Create minimal capabilities (no features advertised)
    /// let capabilities = ClientCapabilities::minimal();
    /// assert!(!capabilities.supports_sampling());
    /// assert!(!capabilities.supports_elicitation());
    ///
    /// // Use in client initialization
    /// # use pmcp::{Client, StdioTransport};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let transport = StdioTransport::new();
    /// let mut client = Client::new(transport);
    /// let server_info = client.initialize(ClientCapabilities::minimal()).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn minimal() -> Self {
        Self::default()
    }

    /// Create a full set of client capabilities.
    ///
    /// Advertises all standard client capabilities defined in the MCP specification.
    /// Note: Client capabilities indicate what the CLIENT can do (e.g., handle sampling
    /// requests, provide user input). Server capabilities (tools, prompts, resources)
    /// are advertised by servers, not clients.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::ClientCapabilities;
    ///
    /// // Create full capabilities (all client features supported)
    /// let capabilities = ClientCapabilities::full();
    /// assert!(capabilities.supports_sampling());
    /// assert!(capabilities.supports_elicitation());
    ///
    /// // Inspect specific capabilities
    /// assert!(capabilities.roots.unwrap().list_changed);
    ///
    /// // Use in client that supports all MCP client features
    /// # use pmcp::{Client, StdioTransport};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let transport = StdioTransport::new();
    /// let mut client = Client::new(transport);
    /// let server_info = client.initialize(ClientCapabilities::full()).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn full() -> Self {
        Self {
            sampling: Some(SamplingCapabilities::default()),
            elicitation: Some(ElicitationCapabilities::default()),
            roots: Some(RootsCapabilities { list_changed: true }),
            tasks: Some(ClientTasksCapability::default()),
            experimental: None,
        }
    }

    /// Check if the client supports elicitation (user input requests).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::{ClientCapabilities, types::capabilities::ElicitationCapabilities};
    ///
    /// // Check elicitation support
    /// let caps = ClientCapabilities::full();
    /// assert!(caps.supports_elicitation());
    ///
    /// // Build capabilities with elicitation
    /// let mut interactive_client = ClientCapabilities::default();
    /// interactive_client.elicitation = Some(ElicitationCapabilities::default());
    /// assert!(interactive_client.supports_elicitation());
    ///
    /// // Use for interactive tools
    /// # use pmcp::{Client, StdioTransport};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let caps = ClientCapabilities::full();
    /// if caps.supports_elicitation() {
    ///     println!("Client can handle user input requests");
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn supports_elicitation(&self) -> bool {
        self.elicitation.is_some()
    }

    /// Check if the client supports sampling.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::{ClientCapabilities, types::capabilities::SamplingCapabilities};
    ///
    /// // Check sampling support for LLM operations
    /// let caps = ClientCapabilities::full();
    /// assert!(caps.supports_sampling());
    ///
    /// // Build LLM client capabilities
    /// let mut llm_client = ClientCapabilities::default();
    /// llm_client.sampling = Some(SamplingCapabilities {
    ///     models: Some(vec![
    ///         "gpt-4".to_string(),
    ///         "claude-3".to_string(),
    ///         "llama-2".to_string(),
    ///     ]),
    ///     ..SamplingCapabilities::default()
    /// });
    /// assert!(llm_client.supports_sampling());
    ///
    /// // List supported models
    /// if let Some(sampling) = &llm_client.sampling {
    ///     if let Some(models) = &sampling.models {
    ///         println!("Supported models: {:?}", models);
    ///     }
    /// }
    /// ```
    pub fn supports_sampling(&self) -> bool {
        self.sampling.is_some()
    }
}

impl ServerCapabilities {
    /// Create a minimal set of server capabilities.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::ServerCapabilities;
    ///
    /// // Create minimal server with no advertised features
    /// let capabilities = ServerCapabilities::minimal();
    /// assert!(!capabilities.provides_tools());
    /// assert!(!capabilities.provides_prompts());
    /// assert!(!capabilities.provides_resources());
    ///
    /// // Use in server that implements custom protocol extensions
    /// # use pmcp::Server;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let server = Server::builder()
    ///     .name("minimal-server")
    ///     .version("1.0.0")
    ///     .capabilities(ServerCapabilities::minimal())
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn minimal() -> Self {
        Self::default()
    }

    /// Create capabilities for a tool server.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::ServerCapabilities;
    ///
    /// // Create server that only provides tools
    /// let capabilities = ServerCapabilities::tools_only();
    /// assert!(capabilities.provides_tools());
    /// assert!(!capabilities.provides_prompts());
    /// assert!(!capabilities.provides_resources());
    ///
    /// // Use in a tool-focused server
    /// # use pmcp::{Server, ToolHandler};
    /// # use async_trait::async_trait;
    /// # struct CalculatorTool;
    /// # #[async_trait]
    /// # impl ToolHandler for CalculatorTool {
    /// #     async fn handle(&self, args: serde_json::Value, _extra: pmcp::RequestHandlerExtra) -> Result<serde_json::Value, pmcp::Error> {
    /// #         Ok(serde_json::json!({"result": 42}))
    /// #     }
    /// # }
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let server = Server::builder()
    ///     .name("calculator-server")
    ///     .version("1.0.0")
    ///     .capabilities(ServerCapabilities::tools_only())
    ///     .tool("calculate", CalculatorTool)
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn tools_only() -> Self {
        Self {
            tools: Some(ToolCapabilities {
                list_changed: Some(true),
            }),
            ..Default::default()
        }
    }

    /// Create capabilities for a prompt server.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::ServerCapabilities;
    ///
    /// // Create server that only provides prompts
    /// let capabilities = ServerCapabilities::prompts_only();
    /// assert!(!capabilities.provides_tools());
    /// assert!(capabilities.provides_prompts());
    /// assert!(!capabilities.provides_resources());
    ///
    /// // Use in a prompt template server
    /// # use pmcp::{Server, PromptHandler};
    /// # use async_trait::async_trait;
    /// # use pmcp::types::{GetPromptResult, PromptMessage, Role, Content};
    /// # struct GreetingPrompt;
    /// # #[async_trait]
    /// # impl PromptHandler for GreetingPrompt {
    /// #     async fn handle(&self, args: std::collections::HashMap<String, String>, _extra: pmcp::RequestHandlerExtra) -> Result<GetPromptResult, pmcp::Error> {
    /// #         Ok(GetPromptResult::new(
    /// #             vec![PromptMessage::system(Content::text("Hello!"))],
    /// #             Some("Greeting prompt".to_string()),
    /// #         ))
    /// #     }
    /// # }
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let server = Server::builder()
    ///     .name("prompt-server")
    ///     .version("1.0.0")
    ///     .capabilities(ServerCapabilities::prompts_only())
    ///     .prompt("greeting", GreetingPrompt)
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn prompts_only() -> Self {
        Self {
            prompts: Some(PromptCapabilities {
                list_changed: Some(true),
            }),
            ..Default::default()
        }
    }

    /// Create capabilities for a resource server.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::ServerCapabilities;
    ///
    /// // Create server that only provides resources
    /// let capabilities = ServerCapabilities::resources_only();
    /// assert!(!capabilities.provides_tools());
    /// assert!(!capabilities.provides_prompts());
    /// assert!(capabilities.provides_resources());
    ///
    /// // Check subscription support
    /// let resource_caps = capabilities.resources.unwrap();
    /// assert!(resource_caps.subscribe.unwrap());
    /// assert!(resource_caps.list_changed.unwrap());
    ///
    /// // Use in a file system resource server
    /// # use pmcp::{Server, ResourceHandler};
    /// # use async_trait::async_trait;
    /// # use pmcp::types::{ReadResourceResult, ListResourcesResult, ResourceInfo, Content};
    /// # struct FileResource;
    /// # #[async_trait]
    /// # impl ResourceHandler for FileResource {
    /// #     async fn read(&self, uri: &str, _extra: pmcp::RequestHandlerExtra) -> Result<ReadResourceResult, pmcp::Error> {
    /// #         Ok(ReadResourceResult::new(vec![Content::text("File contents")]))
    /// #     }
    /// #     async fn list(&self, _path: Option<String>, _extra: pmcp::RequestHandlerExtra) -> Result<ListResourcesResult, pmcp::Error> {
    /// #         Ok(ListResourcesResult::new(vec![]))
    /// #     }
    /// # }
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let server = Server::builder()
    ///     .name("filesystem-server")
    ///     .version("1.0.0")
    ///     .capabilities(ServerCapabilities::resources_only())
    ///     .resources(FileResource)
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn resources_only() -> Self {
        Self {
            resources: Some(ResourceCapabilities {
                subscribe: Some(true),
                list_changed: Some(true),
            }),
            ..Default::default()
        }
    }

    /// Check if the server provides tools.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::ServerCapabilities;
    ///
    /// // Check different server configurations
    /// let tool_server = ServerCapabilities::tools_only();
    /// assert!(tool_server.provides_tools());
    ///
    /// let minimal_server = ServerCapabilities::minimal();
    /// assert!(!minimal_server.provides_tools());
    ///
    /// // Use in server logic
    /// fn validate_server(caps: &ServerCapabilities) {
    ///     if caps.provides_tools() {
    ///         println!("Server can handle tool calls");
    ///     } else {
    ///         println!("Server does not provide tools");
    ///     }
    /// }
    ///
    /// // Combine multiple capabilities
    /// use pmcp::types::capabilities::{ToolCapabilities, PromptCapabilities};
    /// let mut multi_server = ServerCapabilities::default();
    /// multi_server.tools = Some(ToolCapabilities::default());
    /// multi_server.prompts = Some(PromptCapabilities::default());
    /// assert!(multi_server.provides_tools());
    /// assert!(multi_server.provides_prompts());
    /// ```
    pub fn provides_tools(&self) -> bool {
        self.tools.is_some()
    }

    /// Check if the server provides prompts.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::ServerCapabilities;
    ///
    /// // Check prompt server
    /// let prompt_server = ServerCapabilities::prompts_only();
    /// assert!(prompt_server.provides_prompts());
    /// assert!(!prompt_server.provides_tools());
    ///
    /// // Use in client code to check server features
    /// # use pmcp::{Client, StdioTransport};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let transport = StdioTransport::new();
    /// # let mut client = Client::new(transport);
    /// # let server_info = client.initialize(pmcp::ClientCapabilities::default()).await?;
    /// if server_info.capabilities.provides_prompts() {
    ///     // Server supports prompts, we can list them
    ///     let prompts = client.list_prompts(None).await?;
    ///     println!("Available prompts: {}", prompts.prompts.len());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn provides_prompts(&self) -> bool {
        self.prompts.is_some()
    }

    /// Check if the server provides resources.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::ServerCapabilities;
    ///
    /// // Check resource server capabilities
    /// let resource_server = ServerCapabilities::resources_only();
    /// assert!(resource_server.provides_resources());
    ///
    /// // Check if subscriptions are supported
    /// if resource_server.provides_resources() {
    ///     if let Some(res_caps) = &resource_server.resources {
    ///         if res_caps.subscribe.unwrap_or(false) {
    ///             println!("Server supports resource subscriptions");
    ///         }
    ///     }
    /// }
    ///
    /// // Build a full-featured server
    /// use pmcp::types::capabilities::*;
    /// let mut full_server = ServerCapabilities::default();
    /// full_server.tools = Some(ToolCapabilities::default());
    /// full_server.prompts = Some(PromptCapabilities::default());
    /// let mut res_caps = ResourceCapabilities::default();
    /// res_caps.subscribe = Some(true);
    /// res_caps.list_changed = Some(true);
    /// full_server.resources = Some(res_caps);
    /// assert!(full_server.provides_tools());
    /// assert!(full_server.provides_prompts());
    /// assert!(full_server.provides_resources());
    /// ```
    pub fn provides_resources(&self) -> bool {
        self.resources.is_some()
    }

    /// Check if the server provides task support (MCP 2025-11-25).
    pub fn provides_tasks(&self) -> bool {
        self.tasks.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_capabilities_helpers() {
        let minimal = ClientCapabilities::minimal();
        assert!(!minimal.supports_sampling());
        assert!(!minimal.supports_elicitation());

        let full = ClientCapabilities::full();
        assert!(full.supports_sampling());
        assert!(full.supports_elicitation());
    }

    #[test]
    fn server_capabilities_helpers() {
        let tools_only = ServerCapabilities::tools_only();
        assert!(tools_only.provides_tools());
        assert!(!tools_only.provides_prompts());
        assert!(!tools_only.provides_resources());

        let prompts_only = ServerCapabilities::prompts_only();
        assert!(!prompts_only.provides_tools());
        assert!(prompts_only.provides_prompts());
        assert!(!prompts_only.provides_resources());
    }

    #[test]
    fn capabilities_serialization() {
        let caps = ClientCapabilities {
            sampling: Some(SamplingCapabilities::default()),
            elicitation: Some(ElicitationCapabilities::default()),
            roots: Some(RootsCapabilities { list_changed: true }),
            ..Default::default()
        };

        let json = serde_json::to_value(&caps).unwrap();
        assert!(json.get("sampling").is_some());
        assert!(json.get("elicitation").is_some());
        assert_eq!(json["roots"]["listChanged"], true);
        // Verify invalid fields are not present
        assert!(json.get("tools").is_none());
        assert!(json.get("prompts").is_none());
        assert!(json.get("resources").is_none());
    }

    #[test]
    fn server_capabilities_auto_set_serialization() {
        // Test that auto-set capabilities (with Some(false)) serialize correctly
        let caps = ServerCapabilities {
            tools: Some(ToolCapabilities {
                list_changed: Some(false),
            }),
            prompts: Some(PromptCapabilities {
                list_changed: Some(false),
            }),
            resources: Some(ResourceCapabilities {
                subscribe: Some(false),
                list_changed: Some(false),
            }),
            ..Default::default()
        };

        let json = serde_json::to_value(&caps).unwrap();
        println!(
            "Serialized capabilities: {}",
            serde_json::to_string_pretty(&json).unwrap()
        );

        // Verify tools, prompts, resources are present
        assert!(json.get("tools").is_some(), "tools should be present");
        assert!(json.get("prompts").is_some(), "prompts should be present");
        assert!(
            json.get("resources").is_some(),
            "resources should be present"
        );

        // Verify the listChanged fields are present
        assert_eq!(json["tools"]["listChanged"], false);
        assert_eq!(json["prompts"]["listChanged"], false);
        assert_eq!(json["resources"]["listChanged"], false);
        assert_eq!(json["resources"]["subscribe"], false);
    }

    #[test]
    fn server_tasks_capability_serialization() {
        let caps = ServerCapabilities {
            tasks: Some(ServerTasksCapability {
                list: Some(serde_json::json!({})),
                cancel: Some(serde_json::json!({})),
                requests: Some(ServerTasksRequestCapability {
                    tools: Some(ServerTasksToolsCapability {
                        call: Some(serde_json::json!({})),
                    }),
                }),
            }),
            ..Default::default()
        };
        let json = serde_json::to_value(&caps).unwrap();
        assert!(json.get("tasks").is_some());
        assert!(json["tasks"].get("list").is_some());
        assert!(json["tasks"].get("cancel").is_some());
        assert!(json["tasks"]["requests"]["tools"]["call"].is_object());
        assert!(caps.provides_tasks());
    }

    #[test]
    fn client_tasks_capability_serialization() {
        let caps = ClientCapabilities::full();
        let json = serde_json::to_value(&caps).unwrap();
        assert!(json.get("tasks").is_some());
    }

    #[test]
    fn default_serializes_without_extensions_key() {
        // Test 1.1: default `ServerCapabilities` must NOT emit `extensions` —
        // the `skip_serializing_if = "Option::is_none"` guard must work.
        let caps = ServerCapabilities::default();
        let json = serde_json::to_value(&caps).unwrap();
        assert!(
            json.get("extensions").is_none(),
            "default ServerCapabilities should not serialize an `extensions` key, \
             got: {json}"
        );
    }

    #[test]
    fn extensions_round_trip_byte_equal() {
        // Test 1.2: round-trip with the SEP-2640 key.
        let mut ext = HashMap::new();
        ext.insert(
            "io.modelcontextprotocol/skills".to_string(),
            serde_json::json!({}),
        );
        let caps = ServerCapabilities {
            extensions: Some(ext),
            ..Default::default()
        };
        let json = serde_json::to_value(&caps).unwrap();
        let round: ServerCapabilities = serde_json::from_value(json).unwrap();
        assert_eq!(
            round.extensions
                .as_ref()
                .unwrap()
                .get("io.modelcontextprotocol/skills"),
            Some(&serde_json::json!({})),
            "round-tripped extensions value must equal the original"
        );
    }

    #[test]
    fn extensions_and_experimental_coexist() {
        // Test 1.3: both maps are sibling fields, both survive round-trip.
        let mut exp = HashMap::new();
        exp.insert("old-thing".to_string(), serde_json::json!({"v": 1}));
        let mut ext = HashMap::new();
        ext.insert(
            "io.modelcontextprotocol/skills".to_string(),
            serde_json::json!({}),
        );
        let caps = ServerCapabilities {
            experimental: Some(exp),
            extensions: Some(ext),
            ..Default::default()
        };
        let json = serde_json::to_value(&caps).unwrap();
        // (a) Both top-level keys present.
        assert!(json.get("experimental").is_some(), "experimental missing");
        assert!(json.get("extensions").is_some(), "extensions missing");
        // (b) They are siblings, not nested — i.e. `extensions` is not inside `experimental`.
        assert!(
            json["experimental"].get("extensions").is_none(),
            "extensions must NOT be nested inside experimental"
        );
        assert!(
            json["extensions"].get("experimental").is_none(),
            "experimental must NOT be nested inside extensions"
        );
        // (c) Round-trip preserves both.
        let round: ServerCapabilities = serde_json::from_value(json).unwrap();
        assert!(round.experimental.is_some());
        assert!(round.extensions.is_some());
        assert_eq!(
            round.extensions
                .as_ref()
                .unwrap()
                .get("io.modelcontextprotocol/skills"),
            Some(&serde_json::json!({}))
        );
        assert_eq!(
            round.experimental
                .as_ref()
                .unwrap()
                .get("old-thing"),
            Some(&serde_json::json!({"v": 1}))
        );
    }

    #[test]
    fn extensions_camelcase_serde() {
        // Test 1.4: `#[serde(rename_all = "camelCase")]` must keep `extensions`
        // verbatim on the wire (it's already lowercase) — SEP-2640 §6 wire match.
        let mut ext = HashMap::new();
        ext.insert("k".to_string(), serde_json::json!(1));
        let caps = ServerCapabilities {
            extensions: Some(ext),
            ..Default::default()
        };
        let s = serde_json::to_string(&caps).unwrap();
        assert!(
            s.contains("\"extensions\""),
            "wire form must contain exactly `\"extensions\"`, got: {s}"
        );
        // Sanity: no unwanted casing variants.
        assert!(!s.contains("\"Extensions\""));
        assert!(!s.contains("\"extension\""));
    }

    #[test]
    fn server_capabilities_with_none_fields_serialization() {
        // Test that capabilities with None fields still have the parent object
        let caps = ServerCapabilities {
            tools: Some(ToolCapabilities { list_changed: None }),
            prompts: Some(PromptCapabilities { list_changed: None }),
            resources: Some(ResourceCapabilities {
                subscribe: None,
                list_changed: None,
            }),
            ..Default::default()
        };

        let json = serde_json::to_value(&caps).unwrap();
        println!(
            "Serialized capabilities with None: {}",
            serde_json::to_string_pretty(&json).unwrap()
        );

        // Verify tools, prompts, resources are present (even if empty objects)
        assert!(
            json.get("tools").is_some(),
            "tools should be present even with None fields"
        );
        assert!(
            json.get("prompts").is_some(),
            "prompts should be present even with None fields"
        );
        assert!(
            json.get("resources").is_some(),
            "resources should be present even with None fields"
        );
    }
}
