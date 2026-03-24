//! Example 65: Durable MCP Agent
//!
//! Demonstrates building an MCP agent (LLM + tool loop) using AWS Lambda
//! Durable Execution for checkpointed, replay-safe workflows.
//!
//! The agent:
//! 1. Connects to configured MCP servers and discovers tools
//! 2. Calls the Anthropic Messages API with the tool schemas
//! 3. Executes any tool_use blocks via MCP call_tool
//! 4. Repeats until the LLM returns end_turn
//!
//! Each LLM call is a durable `ctx.step()` with retry. Tool executions
//! use `ctx.map()` for parallel, replay-safe execution. Each iteration
//! is wrapped in `ctx.run_in_child_context()` for deterministic replay.
//!
//! ## Key Durable Execution Concepts
//!
//! - **`ctx.step()`**: Wraps a side-effectful operation (LLM call, tool
//!   discovery) so its result is checkpointed. On Lambda replay, the step
//!   returns the cached result instead of re-executing.
//!
//! - **`ctx.map()`**: Executes multiple items in parallel with replay
//!   isolation. Each tool call gets its own checkpoint slot.
//!
//! - **`ctx.run_in_child_context()`**: Creates an isolated operation-ID
//!   namespace for each loop iteration, ensuring deterministic replay
//!   even when the number of iterations varies across invocations.
//!
//! ## Prerequisites
//!
//! - AWS Lambda with Durable Execution enabled
//! - `ANTHROPIC_API_KEY` environment variable set
//! - One or more MCP servers accessible via HTTP
//!
//! ## Deployment
//!
//! This example runs as an AWS Lambda function. Deploy with SAM:
//! ```bash
//! sam build && sam deploy --guided
//! ```
//!
//! ## Invocation
//!
//! ```json
//! {
//!   "prompt": "What tools do you have? Try using one.",
//!   "mcp_server_urls": ["https://your-mcp-server.example.com/mcp"],
//!   "model": "claude-sonnet-4-20250514",
//!   "max_iterations": 10
//! }
//! ```

use lambda_durable_execution_rust::prelude::*;
use lambda_durable_execution_rust::runtime::with_durable_execution_service;
use pmcp::shared::streamable_http::{StreamableHttpTransport, StreamableHttpTransportConfig};
use pmcp::types::{CallToolResult, Content, Implementation, ToolInfo};
use pmcp::{Client, ClientCapabilities};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::info;

// ---------------------------------------------------------------------------
// Types -- simplified inline versions of the production agent types
// ---------------------------------------------------------------------------

/// Agent input event. Passed as the Lambda invocation payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AgentInput {
    /// The user's prompt to send to the LLM.
    prompt: String,
    /// URLs of MCP servers to connect to (e.g. `["https://mcp.example.com/mcp"]`).
    mcp_server_urls: Vec<String>,
    /// Anthropic model ID. Defaults to `claude-sonnet-4-20250514`.
    #[serde(default = "default_model")]
    model: String,
    /// Maximum agent loop iterations before giving up.
    #[serde(default = "default_max_iterations")]
    max_iterations: u32,
}

fn default_model() -> String {
    "claude-sonnet-4-20250514".to_string()
}
fn default_max_iterations() -> u32 {
    10
}

/// Agent output returned as the Lambda result.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AgentOutput {
    /// The LLM's final text response.
    response: String,
    /// Number of agent loop iterations executed.
    iterations: u32,
    /// Cumulative input tokens across all LLM calls.
    total_input_tokens: u32,
    /// Cumulative output tokens across all LLM calls.
    total_output_tokens: u32,
    /// Tool names invoked (may contain duplicates showing full call history).
    tools_called: Vec<String>,
}

// --- Anthropic API types (minimal, inline) ---

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    messages: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AnthropicResponse {
    content: Vec<ContentBlock>,
    stop_reason: String,
    usage: Usage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Usage {
    input_tokens: u32,
    output_tokens: u32,
}

/// A single extracted tool call from the LLM response.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ToolCall {
    id: String,
    name: String,
    input: serde_json::Value,
}

/// Result of executing a single MCP tool call.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ToolResult {
    tool_use_id: String,
    content: String,
    is_error: bool,
}

/// Tools discovered from MCP servers, with a routing map for dispatch.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ToolsWithRouting {
    /// Tool schemas in Anthropic tool format (name, description, input_schema).
    tools: Vec<serde_json::Value>,
    /// Maps prefixed tool name -> server URL for routing tool calls.
    routing: HashMap<String, String>,
}

// ---------------------------------------------------------------------------
// Lambda entry point
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<(), lambda_runtime::Error> {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init();

    let svc = with_durable_execution_service(agent_handler, None);
    lambda_runtime::run(svc).await
}

// ---------------------------------------------------------------------------
// Agent handler -- the core durable agent loop
// ---------------------------------------------------------------------------

/// The durable agent handler. Each side-effectful operation is wrapped in a
/// durable primitive so that on Lambda replay (after suspension/resumption),
/// cached results are returned instead of re-executing.
async fn agent_handler(
    event: AgentInput,
    ctx: DurableContextHandle,
) -> DurableResult<AgentOutput> {
    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .map_err(|_| DurableError::Internal("ANTHROPIC_API_KEY not set".into()))?;

    info!(
        model = %event.model,
        mcp_servers = event.mcp_server_urls.len(),
        max_iterations = event.max_iterations,
        "Starting durable MCP agent"
    );

    // 1. Discover tools from MCP servers via a durable step.
    //    On replay, this returns the cached discovery result.
    let server_urls = event.mcp_server_urls.clone();
    let tools_with_routing: ToolsWithRouting = ctx
        .step(
            Some("discover-tools"),
            move |_| async move {
                discover_tools(&server_urls)
                    .await
                    .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { e })
            },
            None,
        )
        .await?;

    info!(tools = tools_with_routing.tools.len(), "Tools discovered");

    // 2. Establish MCP connections OUTSIDE durable steps.
    //    Connections are ephemeral -- they are re-created on each Lambda
    //    invocation. Putting them inside a step would cache the connection
    //    object, which becomes stale on replay (Pitfall 1).
    let mcp_clients = connect_mcp_clients(&event.mcp_server_urls)
        .await
        .map_err(|e| DurableError::Internal(e.to_string()))?;

    // 3. Agent loop: LLM call -> tool execution -> repeat
    let http_client = reqwest::Client::new();
    let model = event.model.clone();
    let mut messages: Vec<serde_json::Value> = vec![serde_json::json!({
        "role": "user",
        "content": event.prompt,
    })];

    let mut total_input_tokens: u32 = 0;
    let mut total_output_tokens: u32 = 0;
    let mut tools_called: Vec<String> = Vec::new();

    for i in 0..event.max_iterations {
        // Each iteration gets its own child context so operation IDs
        // (step-0, step-1, ...) restart from 0. This ensures deterministic
        // replay even if earlier iterations produced different numbers of
        // operations on a previous invocation.
        let client = http_client.clone();
        let key = api_key.clone();
        let mdl = model.clone();
        let msgs = messages.clone();
        let tools = tools_with_routing.tools.clone();
        let routing = tools_with_routing.routing.clone();
        let clients = mcp_clients.clone();

        let (response, tool_results) = ctx
            .run_in_child_context(
                Some(&format!("iteration-{i}")),
                move |child_ctx| async move {
                    execute_iteration(
                        &child_ctx,
                        &client,
                        &key,
                        &mdl,
                        &msgs,
                        &tools,
                        &routing,
                        &clients,
                    )
                    .await
                },
                None,
            )
            .await?;

        // Accumulate token counts across iterations (SDK-03).
        total_input_tokens += response.usage.input_tokens;
        total_output_tokens += response.usage.output_tokens;

        // Collect tool names for observability.
        let iteration_tool_calls = extract_tool_calls(&response);
        for tc in &iteration_tool_calls {
            tools_called.push(tc.name.clone());
        }

        // Structured logging per iteration (SDK-03).
        info!(
            iteration = i,
            input_tokens = response.usage.input_tokens,
            output_tokens = response.usage.output_tokens,
            total_input_tokens,
            total_output_tokens,
            tool_count = iteration_tool_calls.len(),
            "Iteration complete"
        );

        // Append assistant message to conversation history.
        messages.push(response_to_assistant_message(&response));

        // If the LLM said "end_turn" or there are no tool calls, we're done.
        if response.stop_reason == "end_turn" || iteration_tool_calls.is_empty() {
            let final_text = extract_text(&response);
            return Ok(AgentOutput {
                response: final_text,
                iterations: i + 1,
                total_input_tokens,
                total_output_tokens,
                tools_called,
            });
        }

        // Append tool results as a user message.
        if let Some(results) = tool_results {
            let tool_result_blocks: Vec<serde_json::Value> = results
                .into_iter()
                .map(|r| {
                    let mut block = serde_json::json!({
                        "type": "tool_result",
                        "tool_use_id": r.tool_use_id,
                        "content": r.content,
                    });
                    if r.is_error {
                        block["is_error"] = serde_json::json!(true);
                    }
                    block
                })
                .collect();
            messages.push(serde_json::json!({
                "role": "user",
                "content": tool_result_blocks,
            }));
        }
    }

    // Max iterations exceeded -- return an error so the caller knows.
    Err(DurableError::Internal(format!(
        "Agent exceeded max iterations ({}) without completing",
        event.max_iterations
    )))
}

// ---------------------------------------------------------------------------
// Iteration execution (runs inside a child context)
// ---------------------------------------------------------------------------

/// Execute a single agent loop iteration:
/// 1. Call the Anthropic API via a durable step (with retry).
/// 2. If the response contains tool_use blocks, execute them via durable map.
/// 3. Return the LLM response and any tool results.
async fn execute_iteration(
    ctx: &DurableContextHandle,
    http_client: &reqwest::Client,
    api_key: &str,
    model: &str,
    messages: &[serde_json::Value],
    tools: &[serde_json::Value],
    routing: &HashMap<String, String>,
    mcp_clients: &Arc<HashMap<String, Client<StreamableHttpTransport>>>,
) -> DurableResult<(AnthropicResponse, Option<Vec<ToolResult>>)> {
    // --- Durable LLM call with exponential backoff retry ---
    // On replay, this returns the cached response. On transient failures
    // (e.g. 429 rate limit), the durable SDK retries with backoff.
    let client = http_client.clone();
    let key = api_key.to_string();
    let mdl = model.to_string();
    let msgs = messages.to_vec();
    let tls = tools.to_vec();

    let retry = ExponentialBackoff::builder()
        .max_attempts(3)
        .initial_delay(Duration::seconds(2))
        .max_delay(Duration::seconds(30))
        .backoff_rate(2.0)
        .build();
    let step_config =
        StepConfig::<AnthropicResponse>::new().with_retry_strategy(Arc::new(retry));

    let response: AnthropicResponse = ctx
        .step(
            Some("llm-call"),
            move |_| async move {
                call_anthropic(&client, &key, &mdl, &msgs, &tls)
                    .await
                    .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { e })
            },
            Some(step_config),
        )
        .await?;

    // --- Execute tool calls in parallel via durable map ---
    let tool_calls = extract_tool_calls(&response);
    if tool_calls.is_empty() {
        return Ok((response, None));
    }

    let routing = Arc::new(routing.clone());
    let clients = mcp_clients.clone();

    let batch = ctx
        .map(
            Some("tools"),
            tool_calls,
            move |call: ToolCall, _item_ctx: DurableContextHandle, _idx: usize| {
                let r = Arc::clone(&routing);
                let c = clients.clone();
                async move {
                    execute_tool_call(&call, &r, &c)
                        .await
                        .map_err(|e| DurableError::Internal(e.to_string()))
                }
            },
            None,
        )
        .await?;

    let results: Vec<ToolResult> = batch.values();
    Ok((response, Some(results)))
}

// ---------------------------------------------------------------------------
// Anthropic API client (inline, self-contained)
// ---------------------------------------------------------------------------

/// Call the Anthropic Messages API directly via reqwest.
///
/// This is intentionally simple -- no abstraction layers. The durable SDK's
/// `ctx.step()` handles retry and caching; this function just makes the
/// HTTP call.
async fn call_anthropic(
    client: &reqwest::Client,
    api_key: &str,
    model: &str,
    messages: &[serde_json::Value],
    tools: &[serde_json::Value],
) -> Result<AnthropicResponse, Box<dyn std::error::Error + Send + Sync>> {
    let body = AnthropicRequest {
        model: model.to_string(),
        max_tokens: 4096,
        system: None,
        messages: messages.to_vec(),
        tools: if tools.is_empty() {
            None
        } else {
            Some(tools.to_vec())
        },
    };

    let resp = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body_text = resp.text().await.unwrap_or_default();
        return Err(format!("Anthropic API error {status}: {body_text}").into());
    }

    Ok(resp.json::<AnthropicResponse>().await?)
}

// ---------------------------------------------------------------------------
// MCP client setup and tool discovery
// ---------------------------------------------------------------------------

/// Discover tools from all configured MCP servers.
///
/// For each server URL: connect, call `list_tools()`, translate schemas
/// to the Anthropic tool format, and build a routing map so tool calls
/// can be dispatched to the correct server.
///
/// Tool names are prefixed with a host identifier to avoid collisions
/// across servers. E.g., a tool "multiply" on "calc.example.com" becomes
/// "calc__multiply".
async fn discover_tools(
    server_urls: &[String],
) -> Result<ToolsWithRouting, Box<dyn std::error::Error + Send + Sync>> {
    let mut all_tools = Vec::new();
    let mut routing = HashMap::new();

    for url_str in server_urls {
        let parsed = url::Url::parse(url_str)?;
        let prefix: String = parsed
            .host_str()
            .unwrap_or("unknown")
            .split('.')
            .next()
            .unwrap_or("unknown")
            .to_string();

        // Create a temporary client for discovery only.
        let config = StreamableHttpTransportConfig {
            url: parsed,
            extra_headers: vec![],
            auth_provider: None,
            session_id: None,
            enable_json_response: false,
            on_resumption_token: None,
            http_middleware_chain: None,
        };
        let transport = StreamableHttpTransport::new(config);
        let mut client = Client::with_info(
            transport,
            Implementation::new("durable-mcp-agent-example", "0.1.0"),
        );
        // Advertise full capabilities including Tasks support (D-08).
        client.initialize(ClientCapabilities::full()).await?;

        // Paginate through all tool pages.
        let mut cursor: Option<String> = None;
        loop {
            let result = client.list_tools(cursor).await?;
            for tool_info in &result.tools {
                let translated = translate_mcp_tool(tool_info, &prefix);
                let prefixed_name = format!("{prefix}__{}", tool_info.name);
                routing.insert(prefixed_name, url_str.clone());
                all_tools.push(translated);
            }
            match result.next_cursor {
                Some(next) => cursor = Some(next),
                None => break,
            }
        }
    }

    Ok(ToolsWithRouting {
        tools: all_tools,
        routing,
    })
}

/// Establish live MCP client connections for tool execution.
///
/// CRITICAL: This runs OUTSIDE any durable step. MCP connections are
/// ephemeral -- they must be re-established on each Lambda invocation.
/// If you put this inside `ctx.step()`, the connection object would be
/// "replayed" from cache on resume, but the actual TCP/HTTP connection
/// would be dead (Pitfall 1).
async fn connect_mcp_clients(
    server_urls: &[String],
) -> Result<Arc<HashMap<String, Client<StreamableHttpTransport>>>, Box<dyn std::error::Error + Send + Sync>>
{
    let mut clients = HashMap::new();

    for url_str in server_urls {
        let parsed = url::Url::parse(url_str)?;
        let config = StreamableHttpTransportConfig {
            url: parsed,
            extra_headers: vec![],
            auth_provider: None,
            session_id: None,
            enable_json_response: false,
            on_resumption_token: None,
            http_middleware_chain: None,
        };
        let transport = StreamableHttpTransport::new(config);
        let mut client = Client::with_info(
            transport,
            Implementation::new("durable-mcp-agent-example", "0.1.0"),
        );
        // Advertise full capabilities including Tasks support (D-08).
        client.initialize(ClientCapabilities::full()).await?;
        clients.insert(url_str.clone(), client);
    }

    Ok(Arc::new(clients))
}

/// Translate an MCP `ToolInfo` into the Anthropic tool JSON format.
///
/// Ensures `input_schema` has `"type": "object"` (some MCP servers omit it)
/// and strips empty `required` arrays that confuse the Anthropic API.
fn translate_mcp_tool(tool_info: &ToolInfo, prefix: &str) -> serde_json::Value {
    let mut schema = tool_info.input_schema.clone();

    // Ensure "type": "object" is present.
    if schema.get("type").is_none() {
        schema["type"] = serde_json::json!("object");
    }

    // Strip empty "required" arrays.
    if let Some(req) = schema.get("required") {
        if req.as_array().is_some_and(|a| a.is_empty()) {
            schema
                .as_object_mut()
                .map(|m| m.remove("required"));
        }
    }

    serde_json::json!({
        "name": format!("{prefix}__{}", tool_info.name),
        "description": tool_info.description.clone().unwrap_or_default(),
        "input_schema": schema,
    })
}

// ---------------------------------------------------------------------------
// Tool execution
// ---------------------------------------------------------------------------

/// Execute a single tool call via the appropriate MCP server.
///
/// Resolves the prefixed tool name (e.g. "calc__multiply") back to the
/// original name ("multiply") and server URL using the routing map, then
/// calls the tool on the cached MCP client.
///
/// MCP tool errors (is_error: true) are passed through as successful
/// results -- the LLM decides how to recover from tool errors.
async fn execute_tool_call(
    call: &ToolCall,
    routing: &HashMap<String, String>,
    mcp_clients: &HashMap<String, Client<StreamableHttpTransport>>,
) -> Result<ToolResult, Box<dyn std::error::Error + Send + Sync>> {
    // Use split_once("__") to correctly handle tool names that contain "__"
    // (Pitfall 4). Only the first "__" separates prefix from original name.
    let (server_url, original_name) = resolve_tool_name(&call.name, routing)?;

    let client = mcp_clients.get(&server_url).ok_or_else(|| {
        format!("No cached MCP client for server URL: {server_url}")
    })?;

    let result: CallToolResult = client
        .call_tool(original_name.clone(), call.input.clone())
        .await
        .map_err(|e| format!("MCP call_tool failed for {original_name}: {e}"))?;

    // Extract text content from the result.
    let text: String = result
        .content
        .iter()
        .filter_map(|c| match c {
            Content::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");

    Ok(ToolResult {
        tool_use_id: call.id.clone(),
        content: text,
        is_error: result.is_error,
    })
}

/// Resolve a prefixed tool name to (server_url, original_name).
///
/// Uses `split_once("__")` which splits at the first occurrence only,
/// correctly handling tool names that themselves contain "__".
fn resolve_tool_name(
    prefixed_name: &str,
    routing: &HashMap<String, String>,
) -> Result<(String, String), Box<dyn std::error::Error + Send + Sync>> {
    let server_url = routing
        .get(prefixed_name)
        .ok_or_else(|| format!("Unknown tool: {prefixed_name}"))?
        .clone();

    let original_name = prefixed_name
        .split_once("__")
        .map(|(_, name)| name.to_string())
        .ok_or_else(|| format!("Invalid tool name format (no __ separator): {prefixed_name}"))?;

    Ok((server_url, original_name))
}

// ---------------------------------------------------------------------------
// Response helpers
// ---------------------------------------------------------------------------

/// Extract tool_use blocks from an Anthropic response.
fn extract_tool_calls(response: &AnthropicResponse) -> Vec<ToolCall> {
    response
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::ToolUse { id, name, input } => Some(ToolCall {
                id: id.clone(),
                name: name.clone(),
                input: input.clone(),
            }),
            _ => None,
        })
        .collect()
}

/// Extract the final text content from an Anthropic response.
fn extract_text(response: &AnthropicResponse) -> String {
    response
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Convert an Anthropic response to an assistant message for conversation history.
fn response_to_assistant_message(response: &AnthropicResponse) -> serde_json::Value {
    let content: Vec<serde_json::Value> = response
        .content
        .iter()
        .map(|block| match block {
            ContentBlock::Text { text } => serde_json::json!({
                "type": "text",
                "text": text,
            }),
            ContentBlock::ToolUse { id, name, input } => serde_json::json!({
                "type": "tool_use",
                "id": id,
                "name": name,
                "input": input,
            }),
        })
        .collect();

    serde_json::json!({
        "role": "assistant",
        "content": content,
    })
}
