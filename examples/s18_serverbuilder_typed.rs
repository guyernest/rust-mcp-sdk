//! Example demonstrating the new ServerBuilder::tool_typed methods.
//!
//! Shows how to use the ergonomic tool_typed and tool_typed_sync methods
//! for creating type-safe tools with automatic schema generation.

use pmcp::{ServerBuilder, StdioTransport};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;

/// Arguments for the async greeting tool
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct GreetingArgs {
    /// The name of the person to greet
    name: String,
    /// Optional greeting style
    style: Option<GreetingStyle>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
enum GreetingStyle {
    Formal,
    Casual,
    Enthusiastic,
}

/// Arguments for the sync calculator tool
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct CalculatorArgs {
    /// First number
    a: f64,
    /// Second number
    b: f64,
    /// Operation to perform
    operation: Operation,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
enum Operation {
    Add,
    Subtract,
    Multiply,
    Divide,
}

#[tokio::main]
async fn main() -> Result<(), pmcp::Error> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("pmcp=debug".parse().unwrap()),
        )
        .init();

    // Build server with typed tools
    let server = ServerBuilder::new()
        .name("typed-example")
        .version("1.0.0")
        // Async typed tool
        .tool_typed("greeting", |args: GreetingArgs, _extra| {
            Box::pin(async move {
                let style = args.style.unwrap_or(GreetingStyle::Casual);
                let message = match style {
                    GreetingStyle::Formal => format!("Good day, {}.", args.name),
                    GreetingStyle::Casual => format!("Hey {}!", args.name),
                    GreetingStyle::Enthusiastic => format!("🎉 HELLO {}! 🎉", args.name),
                };

                Ok(json!({
                    "message": message,
                    "timestamp": chrono::Utc::now().to_rfc3339()
                }))
            })
        })
        // Sync typed tool
        .tool_typed_sync("calculator", |args: CalculatorArgs, _extra| {
            // Perform validation
            if args.operation == Operation::Divide && args.b == 0.0 {
                return Err(pmcp::Error::Validation(
                    "Cannot divide by zero".to_string(),
                ));
            }

            let result = match args.operation {
                Operation::Add => args.a + args.b,
                Operation::Subtract => args.a - args.b,
                Operation::Multiply => args.a * args.b,
                Operation::Divide => args.a / args.b,
            };

            Ok(json!({
                "result": result,
                "operation": format!("{} {} {} = {}",
                    args.a,
                    match args.operation {
                        Operation::Add => "+",
                        Operation::Subtract => "-",
                        Operation::Multiply => "*",
                        Operation::Divide => "/",
                    },
                    args.b,
                    result
                )
            }))
        })
        .build()?;

    tracing::info!("Server built with typed tools");

    // Start the server with stdio transport
    let transport = StdioTransport::new();
    server.run(transport).await?;

    Ok(())
}

impl PartialEq for Operation {
    fn eq(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Operation::Add, Operation::Add)
                | (Operation::Subtract, Operation::Subtract)
                | (Operation::Multiply, Operation::Multiply)
                | (Operation::Divide, Operation::Divide)
        )
    }
}
