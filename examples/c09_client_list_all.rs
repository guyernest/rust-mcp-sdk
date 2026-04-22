//! Example: typed call helpers and auto-paginating list helpers.
//!
//! Demonstrates:
//!   - `Client::with_client_options` (custom `ClientOptions` + `max_iterations` knob)
//!   - `Client::call_tool_typed` (struct → JSON args)
//!   - `Client::get_prompt_typed` (struct → `HashMap<String, String>`)
//!   - `Client::list_all_tools`, `list_all_prompts`, `list_all_resources`, and
//!     `list_all_resource_templates` (the last uses the distinct
//!     `resources/templates/list` capability).
//!
//! # How to run
//!
//! This example drives an MCP server over **stdio**. It is NOT self-contained —
//! pair it with a compatible stdio MCP server, e.g. `examples/01_server_basic.rs`:
//!
//! ```bash
//! # Terminal A — build a compatible stdio server:
//! cargo build --example 01_server_basic --features full
//!
//! # Terminal B — run c09 paired with the server binary over stdio:
//! cargo run --example c09_client_list_all --features full
//! ```
//!
//! Running without a paired server will block reading from stdio. Pair it with
//! any stdio MCP server that advertises `resources/templates/list` to see real
//! output from every helper.

use pmcp::{Client, ClientCapabilities, ClientOptions, StdioTransport};
use serde::Serialize;

#[derive(Serialize)]
struct SearchArgs {
    query: String,
    limit: u32,
}

#[derive(Serialize)]
struct SummaryArgs {
    topic: String,
    length: u32,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter("pmcp=info")
        .try_init()
        .ok();

    // Custom ClientOptions — lower the safety cap to illustrate the knob.
    // ClientOptions is `#[non_exhaustive]`, so external callers construct it
    // via `::default()` + the builder-style setter.
    let opts = ClientOptions::default().with_max_iterations(50);

    let transport = StdioTransport::new();
    let mut client = Client::with_client_options(transport, opts);
    client.initialize(ClientCapabilities::minimal()).await?;

    // Typed call_tool — no hand-rolled json!({...}).
    let _call = client
        .call_tool_typed(
            "search",
            &SearchArgs {
                query: "rust mcp".into(),
                limit: 10,
            },
        )
        .await?;
    println!("called tool: search");

    // Typed get_prompt — numeric leaves auto-stringified per D-06.
    let _prompt = client
        .get_prompt_typed(
            "summarize",
            &SummaryArgs {
                topic: "rust async".into(),
                length: 200,
            },
        )
        .await?;
    println!("fetched prompt: summarize");

    // Auto-pagination across all four families. `list_all_resource_templates`
    // uses the distinct `resources/templates/list` capability path.
    let tools = client.list_all_tools().await?;
    println!("discovered {} tools across all pages", tools.len());

    let prompts = client.list_all_prompts().await?;
    println!("discovered {} prompts across all pages", prompts.len());

    let resources = client.list_all_resources().await?;
    println!("discovered {} resources across all pages", resources.len());

    let templates = client.list_all_resource_templates().await?;
    println!(
        "discovered {} resource templates across all pages",
        templates.len()
    );

    Ok(())
}
