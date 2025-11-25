//! Example demonstrating type-safe tools with automatic schema generation
//!
//! This example shows how to:
//! - Create typed tools with automatic schema generation
//! - Use SimpleTool with schema generation from types
//! - Validate and handle typed arguments automatically

use anyhow::Result;
use async_trait::async_trait;
use pmcp::{
    RequestHandlerExtra, ServerBuilder, ServerCapabilities, SimpleToolExt, ToolHandler,
    TypedSyncTool, TypedTool,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::info;

// Define argument types with automatic schema generation
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct CalculatorArgs {
    /// The operation to perform
    operation: Operation,
    /// First number
    a: f64,
    /// Second number
    b: f64,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
enum Operation {
    Add,
    Subtract,
    Multiply,
    Divide,
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

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct FileOperationArgs {
    /// The file path to operate on
    path: String,
    /// The operation to perform
    operation: FileOp,
    /// Content for write operations
    content: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
enum FileOp {
    Read,
    Write,
    Delete,
    List,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("Starting typed tools example server");

    // Create a server with typed tools
    let server = ServerBuilder::new()
        .name("typed-tools-example")
        .version("1.0.0")
        .capabilities(ServerCapabilities {
            tools: Some(pmcp::types::ToolCapabilities::default()),
            ..Default::default()
        })
        // TypedTool with async handler and automatic schema generation
        .tool(
            "calculator",
            TypedTool::new("calculator", |args: CalculatorArgs, _extra| {
                Box::pin(async move {
                    let result = match args.operation {
                        Operation::Add => args.a + args.b,
                        Operation::Subtract => args.a - args.b,
                        Operation::Multiply => args.a * args.b,
                        Operation::Divide => {
                            if args.b == 0.0 {
                                return Err(pmcp::Error::Validation(
                                    "Division by zero".to_string(),
                                ));
                            }
                            args.a / args.b
                        }
                    };
                    Ok(json!({
                        "result": result,
                        "operation": format!("{:?}", args.operation).to_lowercase(),
                        "expression": format!("{} {:?} {} = {}", args.a, args.operation, args.b, result)
                    }))
                })
            })
            .with_description("Perform basic arithmetic operations"),
        )
        // TypedSyncTool with synchronous handler
        .tool(
            "search",
            TypedSyncTool::new("search", |args: SearchArgs, _extra| {
                // Simulate a search operation
                let limit = args.limit.unwrap_or(10);
                let mut results = vec![];

                // Mock search results
                for i in 0..limit.min(5) {
                    if args.query.to_lowercase().contains("test")
                        || args.include_archived
                        || i < 3
                    {
                        results.push(json!({
                            "id": i + 1,
                            "title": format!("Result {} for '{}'", i + 1, args.query),
                            "archived": i >= 3
                        }));
                    }
                }

                Ok(json!({
                    "query": args.query,
                    "results": results,
                    "total": results.len(),
                    "limit": limit,
                    "include_archived": args.include_archived
                }))
            })
            .with_description("Search for items with filtering options"),
        )
        // SimpleTool with schema generation from type
        .tool(
            "file_operation",
            pmcp::SimpleTool::new("file_operation", |args, _extra| {
                Box::pin(async move {
                    // Parse the typed arguments
                    let typed_args: FileOperationArgs = serde_json::from_value(args)?;

                    let result = match typed_args.operation {
                        FileOp::Read => {
                            json!({
                                "operation": "read",
                                "path": typed_args.path,
                                "content": "Mock file content here..."
                            })
                        }
                        FileOp::Write => {
                            if typed_args.content.is_none() {
                                return Err(pmcp::Error::Validation(
                                    "Content required for write operation".to_string(),
                                ));
                            }
                            json!({
                                "operation": "write",
                                "path": typed_args.path,
                                "bytes_written": typed_args.content.unwrap().len()
                            })
                        }
                        FileOp::Delete => {
                            json!({
                                "operation": "delete",
                                "path": typed_args.path,
                                "deleted": true
                            })
                        }
                        FileOp::List => {
                            json!({
                                "operation": "list",
                                "path": typed_args.path,
                                "files": ["file1.txt", "file2.rs", "file3.json"]
                            })
                        }
                    };

                    Ok(result)
                })
            })
            .with_description("Perform file operations")
            .with_schema_from::<FileOperationArgs>(),
        )
        // Custom tool with manual validation
        .tool("custom_validated", CustomValidatedTool)
        .build()?;

    info!("Server initialized with typed tools");
    info!("Tools will have automatically generated JSON schemas");

    // Run the server
    server.run_stdio().await?;

    Ok(())
}

// Custom tool with manual validation
struct CustomValidatedTool;

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct CustomArgs {
    /// User's name
    name: String,
    /// User's age
    #[serde(default)]
    age: Option<u32>,
    /// User's email
    email: String,
}

#[async_trait]
impl ToolHandler for CustomValidatedTool {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> Result<Value, pmcp::Error> {
        // Parse and validate the arguments
        let typed_args: CustomArgs = serde_json::from_value(args)
            .map_err(|e| pmcp::Error::Validation(format!("Invalid arguments: {}", e)))?;

        // Additional custom validation
        if !typed_args.email.contains('@') {
            return Err(pmcp::Error::Validation("Invalid email format".to_string()));
        }

        if let Some(age) = typed_args.age {
            if age < 18 {
                return Err(pmcp::Error::Validation(
                    "User must be 18 or older".to_string(),
                ));
            }
        }

        Ok(json!({
            "message": format!("Hello, {}!", typed_args.name),
            "email": typed_args.email,
            "age": typed_args.age,
            "validated": true
        }))
    }

    fn metadata(&self) -> Option<pmcp::types::ToolInfo> {
        // Generate schema for the custom tool
        let schema = schemars::schema_for!(CustomArgs);

        Some(pmcp::types::ToolInfo::new(
            "custom_validated",
            Some("Custom tool with validation".to_string()),
            serde_json::to_value(&schema).unwrap_or_else(|_| {
                json!({
                    "type": "object",
                    "properties": {}
                })
            }),
        ))
    }
}

// Example usage instructions
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_generation() {
        // Test that schemas are generated correctly
        let schema = schemars::schema_for!(CalculatorArgs);
        let json_schema = serde_json::to_value(&schema).unwrap();

        // Verify the schema contains the expected fields
        assert!(json_schema.get("properties").is_some());
        let properties = &json_schema["properties"];
        assert!(properties.get("operation").is_some());
        assert!(properties.get("a").is_some());
        assert!(properties.get("b").is_some());
    }

    #[test]
    fn test_search_args_defaults() {
        // Test default values in SearchArgs
        let json = json!({
            "query": "test query"
        });

        let args: SearchArgs = serde_json::from_value(json).unwrap();
        assert_eq!(args.query, "test query");
        assert_eq!(args.limit, Some(10)); // Default value
        assert_eq!(args.include_archived, false); // Default value
    }
}
