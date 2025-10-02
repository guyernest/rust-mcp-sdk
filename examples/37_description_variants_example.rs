//! Example demonstrating all description variants for typed tools
//!
//! This example showcases the three new description builder methods:
//! - `.tool_typed_with_description()` - For async tools with input typing
//! - `.tool_typed_sync_with_description()` - For sync tools with input typing
//! - `.tool_typed_with_output_and_description()` - For tools with full I/O typing

use anyhow::Result;
use pmcp::{ServerBuilder, ServerCapabilities};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tracing::info;

// Simple input-only structures
#[derive(Debug, Deserialize, JsonSchema)]
struct EchoInput {
    /// The message to echo back
    message: String,
    /// Optional prefix to add
    prefix: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct CountInput {
    /// The text to count characters in
    text: String,
}

// Full input/output structures for the calculator
#[derive(Debug, Deserialize, JsonSchema)]
struct CalcInput {
    /// First number
    a: f64,
    /// Second number
    b: f64,
    /// Operation: add, subtract, multiply, divide
    op: String,
}

#[derive(Debug, Serialize, JsonSchema)]
struct CalcOutput {
    /// The calculated result
    result: f64,
    /// The operation performed
    operation: String,
    /// Human-readable expression
    expression: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("Starting description variants example server");

    // Create a server demonstrating all three description variants
    let server = ServerBuilder::new()
        .name("description-example")
        .version("1.0.0")
        .capabilities(ServerCapabilities {
            tools: Some(pmcp::types::ToolCapabilities::default()),
            ..Default::default()
        })

        // 1. Async tool with input typing and description
        .tool_typed_with_description(
            "echo",
            "Echoes back a message with an optional prefix",
            |args: EchoInput, _extra| {
                Box::pin(async move {
                    let message = match args.prefix {
                        Some(prefix) => format!("{}: {}", prefix, args.message),
                        None => args.message,
                    };
                    Ok(serde_json::json!({ "echoed": message }))
                })
            }
        )

        // 2. Sync tool with input typing and description
        .tool_typed_sync_with_description(
            "count_chars",
            "Counts the number of characters in the provided text",
            |args: CountInput, _extra| {
                let char_count = args.text.chars().count();
                let word_count = args.text.split_whitespace().count();
                Ok(serde_json::json!({
                    "text": args.text,
                    "character_count": char_count,
                    "word_count": word_count
                }))
            }
        )

        // 3. Full I/O typed tool with description
        .tool_typed_with_output_and_description::<CalcInput, CalcOutput>(
            "calculator",
            "Performs mathematical operations with full type safety and structured output",
            |args, _extra| {
                Box::pin(async move {
                    let result = match args.op.as_str() {
                        "add" => args.a + args.b,
                        "subtract" => args.a - args.b,
                        "multiply" => args.a * args.b,
                        "divide" => {
                            if args.b == 0.0 {
                                return Err(pmcp::Error::Validation(
                                    "Division by zero is not allowed".to_string()
                                ));
                            }
                            args.a / args.b
                        }
                        _ => {
                            return Err(pmcp::Error::Validation(format!(
                                "Unknown operation '{}'. Supported: add, subtract, multiply, divide",
                                args.op
                            )));
                        }
                    };

                    Ok(CalcOutput {
                        result,
                        operation: args.op.clone(),
                        expression: format!("{} {} {} = {}", args.a, args.op, args.b, result),
                    })
                })
            }
        )
        .build()?;

    info!("Server initialized with three tools showcasing description variants:");
    info!("  - echo: async tool with input typing and description");
    info!("  - count_chars: sync tool with input typing and description");
    info!("  - calculator: full I/O typed tool with description");
    info!("All tools have rich descriptions that will appear in tool listings");

    // Run the server
    server.run_stdio().await?;

    Ok(())
}
