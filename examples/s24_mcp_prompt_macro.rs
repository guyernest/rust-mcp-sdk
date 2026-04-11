//! Example demonstrating `#[mcp_prompt]` and mixed `#[mcp_server]` with tools+prompts
//!
//! Shows the DX improvement over manual PromptHandler implementation:
//! - No `HashMap::get("x").ok_or()?.parse()?` boilerplate
//! - Arguments derived from struct fields via JsonSchema
//! - Description enforced at compile time
//!
//! Compare with `examples/06_server_prompts.rs` for the "before" pattern.
//!
//! # Run
//!
//! ```bash
//! cargo run --example s24_mcp_prompt_macro --features full
//! ```

use pmcp::types::{Content, GetPromptResult, PromptMessage};
use pmcp::{mcp_prompt, mcp_server, PromptHandler, ServerBuilder, ServerCapabilities, State};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

// === Argument types with doc comments (descriptions derived via JsonSchema) ===

/// Arguments for the code review prompt.
#[derive(Debug, Deserialize, JsonSchema)]
struct ReviewArgs {
    /// The programming language to review
    language: String,
    /// The code snippet to review
    code: String,
}

/// Arguments for the summarize prompt (demonstrates typed + optional args).
///
/// MCP sends prompt arguments as strings, but the SDK auto-coerces them:
/// - `"42"` → `u32` / `f64` / `i64`
/// - `"true"` / `"false"` → `bool`
/// - Plain text stays as `String`
#[derive(Debug, Deserialize, JsonSchema)]
struct SummarizeArgs {
    /// Text to summarize
    text: String,
    /// Maximum summary length in words (optional, numeric)
    max_length: Option<u32>,
    /// Include bullet points
    #[serde(default)]
    bullet_points: bool,
}

/// Arguments for the add tool.
#[derive(Debug, Deserialize, JsonSchema)]
struct AddArgs {
    /// First number
    a: f64,
    /// Second number
    b: f64,
}

/// Result of the add tool.
#[derive(Debug, Serialize, JsonSchema)]
struct AddResult {
    /// The sum
    result: f64,
}

// === Standalone #[mcp_prompt] functions ===

/// Standalone prompt with typed args -- no HashMap boilerplate.
#[mcp_prompt(description = "Review code for quality issues")]
async fn code_review(args: ReviewArgs) -> pmcp::Result<GetPromptResult> {
    Ok(GetPromptResult::new(
        vec![PromptMessage::user(Content::text(format!(
            "Please review this {} code for quality issues:\n\n```{}\n{}\n```",
            args.language, args.language, args.code
        )))],
        Some("Code Review".to_string()),
    ))
}

/// Prompt with State<T> injection for shared configuration.
#[mcp_prompt(description = "Summarize text with configurable style")]
async fn summarize(args: SummarizeArgs, config: State<AppConfig>) -> pmcp::Result<GetPromptResult> {
    let length_hint = args
        .max_length
        .map(|l| format!(" (max {l} words)"))
        .unwrap_or_default();
    let format_hint = if args.bullet_points {
        " Use bullet points."
    } else {
        ""
    };
    Ok(GetPromptResult::new(
        vec![PromptMessage::user(Content::text(format!(
            "{} style summary{}{format_hint}:\n\n{}",
            config.summary_style, length_hint, args.text
        )))],
        None,
    ))
}

struct AppConfig {
    summary_style: String,
}

// === #[mcp_server] impl block with mixed tools and prompts ===

struct DevServer;

#[mcp_server]
impl DevServer {
    #[mcp_tool(description = "Add two numbers")]
    async fn add(&self, args: AddArgs) -> pmcp::Result<AddResult> {
        Ok(AddResult {
            result: args.a + args.b,
        })
    }

    #[mcp_prompt(description = "Generate a code template")]
    async fn code_template(&self, args: ReviewArgs) -> pmcp::Result<GetPromptResult> {
        Ok(GetPromptResult::new(
            vec![PromptMessage::user(Content::text(format!(
                "Generate a {} code template for:\n{}",
                args.language, args.code
            )))],
            Some("Code Template".to_string()),
        ))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let config = Arc::new(AppConfig {
        summary_style: "Concise".into(),
    });

    // === Method 1: Standalone prompts registered individually ===
    let builder = ServerBuilder::new()
        .name("prompt-macro-example")
        .version("1.0.0")
        .capabilities(ServerCapabilities::default())
        .prompt("code_review", code_review())
        .prompt("summarize", summarize().with_state(config));

    // === Method 2: Impl-block tools+prompts registered in bulk ===
    let dev = DevServer;
    let builder = builder.mcp_server(dev);

    // Build the server
    let _server = builder.build()?;
    tracing::info!("Server built with macro-defined prompts and tools");

    // Quick verification: inspect prompt metadata
    let prompt = code_review();
    let meta = prompt.metadata().expect("should have metadata");
    tracing::info!(
        "Prompt '{}': {}",
        meta.name,
        meta.description.as_deref().unwrap_or("")
    );

    if let Some(ref arguments) = meta.arguments {
        for arg in arguments {
            tracing::info!(
                "  Arg '{}': {} (required: {})",
                arg.name,
                arg.description.as_deref().unwrap_or(""),
                arg.required
            );
        }
    }

    // Call the prompt directly
    let mut args = HashMap::new();
    args.insert("language".to_string(), "rust".to_string());
    args.insert(
        "code".to_string(),
        "fn main() { println!(\"hello\"); }".to_string(),
    );
    let result = prompt
        .handle(args, pmcp::RequestHandlerExtra::default())
        .await?;
    tracing::info!("Prompt result has {} message(s)", result.messages.len());

    Ok(())
}
