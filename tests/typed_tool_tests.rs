//! Tests for typed tool functionality with automatic schema generation.

#[cfg(all(not(target_arch = "wasm32"), feature = "schema-generation"))]
mod tests {
    use pmcp::{SimpleTool, SimpleToolExt, SyncTool, SyncToolExt, TypedSyncTool, TypedTool};
    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};
    use serde_json::json;

    // Helper function to create a test RequestHandlerExtra
    fn test_extra() -> pmcp::RequestHandlerExtra {
        use tokio_util::sync::CancellationToken;

        // Create a cancellation token
        let cancellation_token = CancellationToken::new();

        // Use the actual constructor
        pmcp::RequestHandlerExtra::new("test-request".to_string(), cancellation_token)
    }

    #[derive(Debug, Deserialize, Serialize, JsonSchema)]
    struct SearchArgs {
        /// The search query
        query: String,
        /// Maximum number of results to return
        #[serde(default = "default_limit")]
        limit: Option<u32>,
        /// Include archived items in search
        #[serde(default)]
        include_archived: bool,
    }

    #[allow(clippy::unnecessary_wraps)]
    fn default_limit() -> Option<u32> {
        Some(10)
    }

    #[tokio::test]
    async fn test_typed_tool_with_schema_generation() {
        use pmcp::ToolHandler;

        let tool = TypedTool::new("search", |args: SearchArgs, _extra| {
            Box::pin(async move {
                Ok(json!({
                    "query": args.query,
                    "limit": args.limit.unwrap_or(10),
                    "include_archived": args.include_archived
                }))
            })
        })
        .with_description("Search for items in the database");

        // Check metadata
        let metadata = tool.metadata().expect("Tool should have metadata");
        assert_eq!(metadata.name, "search");
        assert_eq!(
            metadata.description,
            Some("Search for items in the database".to_string())
        );

        // Check that schema was generated
        let schema = &metadata.input_schema;
        assert!(schema.is_object());

        // The schema should include our fields
        let schema_str = serde_json::to_string(schema).unwrap();
        assert!(schema_str.contains("query"));
        assert!(schema_str.contains("limit"));
        assert!(schema_str.contains("include_archived"));

        // Test with valid arguments
        let args = json!({
            "query": "test search",
            "limit": 5,
            "include_archived": true
        });

        let extra = test_extra();
        let result = tool.handle(args, extra).await.unwrap();

        assert_eq!(result["query"], "test search");
        assert_eq!(result["limit"], 5);
        assert_eq!(result["include_archived"], true);
    }

    #[tokio::test]
    async fn test_typed_tool_validation_error() {
        use pmcp::ToolHandler;

        let tool = TypedTool::new("search", |args: SearchArgs, _extra| {
            Box::pin(async move { Ok(json!({ "query": args.query })) })
        });

        // Test with invalid arguments (missing required field)
        let args = json!({
            "limit": 5
        });

        let extra = test_extra();
        let result = tool.handle(args, extra).await;

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.to_string().contains("Invalid arguments"));
    }

    #[test]
    fn test_typed_sync_tool() {
        use pmcp::ToolHandler;
        use tokio::runtime::Runtime;

        let tool = TypedSyncTool::new("search", |args: SearchArgs, _extra| {
            Ok(json!({
                "query": args.query,
                "limit": args.limit.unwrap_or(10)
            }))
        })
        .with_description("Synchronous search tool");

        // Check metadata
        let metadata = tool.metadata().expect("Tool should have metadata");
        assert_eq!(metadata.name, "search");
        assert_eq!(
            metadata.description,
            Some("Synchronous search tool".to_string())
        );

        // Test execution
        let rt = Runtime::new().unwrap();
        let args = json!({
            "query": "test"
        });

        let extra = test_extra();
        let result = rt.block_on(tool.handle(args, extra)).unwrap();

        assert_eq!(result["query"], "test");
        assert_eq!(result["limit"], 10);
    }

    #[test]
    fn test_simple_tool_with_schema_from() {
        use pmcp::ToolHandler;

        let tool = SimpleTool::new("search", |args, _extra| {
            Box::pin(async move {
                let query = args["query"].as_str().unwrap_or("");
                Ok(json!({ "query": query }))
            })
        })
        .with_description("Search with generated schema")
        .with_schema_from::<SearchArgs>();

        // Check that schema was generated
        let metadata = tool.metadata().expect("Tool should have metadata");
        let schema = &metadata.input_schema;

        // The schema should include our fields
        let schema_str = serde_json::to_string(schema).unwrap();
        assert!(schema_str.contains("query"));
        assert!(schema_str.contains("limit"));
        assert!(schema_str.contains("include_archived"));
    }

    #[test]
    fn test_sync_tool_with_schema_from() {
        use pmcp::ToolHandler;

        let tool = SyncTool::new("search", |args| {
            let query = args["query"].as_str().unwrap_or("");
            Ok(json!({ "query": query }))
        })
        .with_description("Sync search with generated schema")
        .with_schema_from::<SearchArgs>();

        // Check that schema was generated
        let metadata = tool.metadata().expect("Tool should have metadata");
        let schema = &metadata.input_schema;

        // The schema should include our fields
        let schema_str = serde_json::to_string(schema).unwrap();
        assert!(schema_str.contains("query"));
        assert!(schema_str.contains("limit"));
    }

    #[derive(Debug, Deserialize, Serialize, JsonSchema)]
    #[serde(rename_all = "camelCase")]
    struct ComplexArgs {
        /// The action to perform
        action: ActionType,
        /// Target of the action
        target: String,
        /// Optional parameters
        #[serde(default)]
        params: Vec<String>,
        /// Nested configuration
        config: Config,
    }

    #[derive(Debug, Deserialize, Serialize, JsonSchema)]
    #[serde(rename_all = "lowercase")]
    enum ActionType {
        Create,
        Update,
        Delete,
    }

    #[derive(Debug, Deserialize, Serialize, JsonSchema)]
    struct Config {
        /// Enable verbose output
        verbose: bool,
        /// Timeout in seconds
        timeout: u32,
    }

    #[tokio::test]
    async fn test_complex_schema_generation() {
        use pmcp::ToolHandler;

        let tool = TypedTool::new("complex", |args: ComplexArgs, _extra| {
            Box::pin(async move {
                Ok(json!({
                    "action": format!("{:?}", args.action),
                    "target": args.target,
                    "params": args.params,
                    "verbose": args.config.verbose,
                    "timeout": args.config.timeout
                }))
            })
        });

        let metadata = tool.metadata().expect("Tool should have metadata");
        let schema = &metadata.input_schema;

        // The schema should handle enums and nested objects
        let schema_str = serde_json::to_string(schema).unwrap();
        assert!(schema_str.contains("action"));
        assert!(schema_str.contains("target"));
        assert!(schema_str.contains("config"));

        // Test with valid complex arguments
        let args = json!({
            "action": "create",
            "target": "resource-1",
            "params": ["param1", "param2"],
            "config": {
                "verbose": true,
                "timeout": 30
            }
        });

        let extra = test_extra();
        let result = tool.handle(args, extra).await.unwrap();

        assert_eq!(result["action"], "Create");
        assert_eq!(result["target"], "resource-1");
        assert_eq!(result["verbose"], true);
        assert_eq!(result["timeout"], 30);
    }
}
