//! Simple Calculator template - starting point for learning MCP
//!
//! Demonstrates the most basic MCP server:
//! - Single tool: add
//! - Simple input/output
//! - Foundation for understanding MCP concepts

pub const CALCULATOR_LIB: &str = r####"//! Simple Calculator MCP Server
//!
//! A minimal MCP server that demonstrates:
//! - Tool definition with TypedTool
//! - JSON schema generation
//! - Basic arithmetic operations
//!
//! This is the starting point for learning MCP. From here, you can:
//! 1. Add more arithmetic operations (subtract, multiply, divide)
//! 2. Add resources (e.g., calculation history)
//! 3. Add workflow prompts for multi-step calculations

use pmcp::{Result, Server, TypedTool};
use pmcp::types::{ServerCapabilities, ToolCapabilities};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// ============================================================================
// TYPE DEFINITIONS
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct AddInput {
    /// First number
    #[schemars(description = "The first number to add")]
    pub a: f64,

    /// Second number
    #[schemars(description = "The second number to add")]
    pub b: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AddResult {
    /// The sum of the two numbers
    pub result: f64,
}

// ============================================================================
// TOOL IMPLEMENTATIONS
// ============================================================================

async fn add_tool(input: AddInput, _extra: pmcp::RequestHandlerExtra) -> Result<AddResult> {
    Ok(AddResult {
        result: input.a + input.b,
    })
}

// ============================================================================
// SERVER BUILDER
// ============================================================================

/// Build the calculator server
pub fn build_calculator_server() -> Result<Server> {
    Server::builder()
        .name("calculator")
        .version("1.0.0")
        .capabilities(ServerCapabilities {
            tools: Some(ToolCapabilities {
                list_changed: Some(true)
            }),
            ..Default::default()
        })
        .tool(
            "add",
            TypedTool::new("add", |input: AddInput, extra| {
                Box::pin(add_tool(input, extra))
            })
            .with_description("Add two numbers together")
        )
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_add() {
        let input = AddInput { a: 5.0, b: 3.0 };
        let result = add_tool(input, pmcp::RequestHandlerExtra::default())
            .await
            .unwrap();
        assert_eq!(result.result, 8.0);
    }

    #[tokio::test]
    async fn test_add_negative() {
        let input = AddInput { a: 10.0, b: -3.0 };
        let result = add_tool(input, pmcp::RequestHandlerExtra::default())
            .await
            .unwrap();
        assert_eq!(result.result, 7.0);
    }

    #[tokio::test]
    async fn test_server_builds() {
        let server = build_calculator_server();
        assert!(server.is_ok());
    }
}
"####;
