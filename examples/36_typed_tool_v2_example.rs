//! Example demonstrating TypedToolWithOutput with input and output typing
//!
//! This example shows how to use the .tool_typed_with_output() builder method
//! for tools with both input and output type safety. It also demonstrates
//! the new description variants for better tool documentation.
//!
//! Available description methods:
//! - `.tool_typed_with_description(name, desc, handler)` - For input typing only
//! - `.tool_typed_sync_with_description(name, desc, handler)` - For sync input typing
//! - `.tool_typed_with_output_and_description(name, desc, handler)` - For full I/O typing

use anyhow::Result;
use pmcp::{ServerBuilder, ServerCapabilities};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tracing::info;

// Input type with validation
#[derive(Debug, Deserialize, JsonSchema)]
struct MathInput {
    /// First number
    a: f64,
    /// Second number
    b: f64,
    /// Operation to perform: add, subtract, multiply, divide
    operation: String,
}

// Output type for type safety
#[derive(Debug, Serialize, JsonSchema)]
struct MathOutput {
    /// The result of the operation
    result: f64,
    /// The operation that was performed
    operation: String,
    /// The expression that was calculated
    expression: String,
}

// User profile input
#[derive(Debug, Deserialize, JsonSchema)]
struct UserInput {
    /// User's name
    name: String,
    /// User's age
    age: u32,
    /// User's email address
    email: String,
}

// User profile output
#[derive(Debug, Serialize, JsonSchema)]
struct UserProfile {
    /// Formatted name
    display_name: String,
    /// Age category
    age_category: String,
    /// Whether email is valid
    email_valid: bool,
    /// Generated user ID
    user_id: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("Starting TypedToolWithOutput example server");

    // Create a server with typed tools using the .tool_typed_with_output() method
    let server = ServerBuilder::new()
        .name("typed-tool-example")
        .version("1.0.0")
        .capabilities(ServerCapabilities {
            tools: Some(pmcp::types::ToolCapabilities::default()),
            ..Default::default()
        })
        // Math calculator with full input/output typing
        .tool_typed_with_output_and_description::<MathInput, MathOutput>(
            "calculator",
            "Performs basic mathematical operations (add, subtract, multiply, divide) on two numbers",
            |args, _extra| {
            Box::pin(async move {
                let result = match args.operation.as_str() {
                    "add" => args.a + args.b,
                    "subtract" => args.a - args.b,
                    "multiply" => args.a * args.b,
                    "divide" => {
                        if args.b == 0.0 {
                            return Err(pmcp::Error::Validation(
                                "Division by zero".to_string(),
                            ));
                        }
                        args.a / args.b
                    }
                    _ => {
                        return Err(pmcp::Error::Validation(format!(
                            "Unknown operation: {}. Supported: add, subtract, multiply, divide",
                            args.operation
                        )));
                    }
                };

                Ok(MathOutput {
                    result,
                    operation: args.operation.clone(),
                    expression: format!("{} {} {} = {}", args.a, args.operation, args.b, result),
                })
            })
        })
        // User profile processor with validation
        .tool_typed_with_output_and_description::<UserInput, UserProfile>(
            "create_profile",
            "Creates a user profile with validation and categorization based on age",
            |args, _extra| {
            Box::pin(async move {
                // Basic validation
                if args.name.trim().is_empty() {
                    return Err(pmcp::Error::Validation("Name cannot be empty".to_string()));
                }

                if args.age < 13 {
                    return Err(pmcp::Error::Validation("User must be at least 13 years old".to_string()));
                }

                let email_valid = args.email.contains('@') && args.email.contains('.');

                let age_category = match args.age {
                    13..=17 => "Teen",
                    18..=24 => "Young Adult",
                    25..=54 => "Adult",
                    55..=64 => "Middle Age",
                    _ => "Senior",
                };

                let user_id = format!("user_{}", args.name.to_lowercase().replace(' ', "_"));

                Ok(UserProfile {
                    display_name: format!("{} ({})", args.name, age_category),
                    age_category: age_category.to_string(),
                    email_valid,
                    user_id,
                })
            })
        })
        .build()?;

    info!("Server initialized with TypedToolWithOutput tools");
    info!("Both input and output schemas are automatically generated");
    info!("Tools: calculator, create_profile");

    // Run the server
    server.run_stdio().await?;

    Ok(())
}
