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
//! - **`ctx.wait_for_condition()`**: Polls for a condition (e.g. MCP Task
//!   completion) with Lambda suspension between polls. Zero compute cost
//!   during waits -- the checkpoint system remembers where we are.
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
use pmcp::shared::streamable_http::{
    StreamableHttpTransport, StreamableHttpTransportConfigBuilder,
};
use pmcp::types::tasks::{Task, TaskStatus};
use pmcp::types::{CallToolResult, Content, Implementation, ToolInfo};
use pmcp::{Client, ClientCapabilities, ToolCallResponse};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::info;

const AGENT_NAME: &str = "durable-mcp-agent-example";
const AGENT_VERSION: &str = "0.1.0";
const MAX_TOKENS: u32 = 4096;

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
    response: String,
    iterations: u32,
    total_input_tokens: u32,
    total_output_tokens: u32,
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
    stop_reason: StopReason,
    usage: Usage,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum StopReason {
    EndTurn,
    ToolUse,
    MaxTokens,
    StopSequence,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ToolCall {
    id: String,
    name: String,
    input: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ToolResult {
    tool_use_id: String,
    content: String,
    is_error: bool,
}

/// Tools discovered from MCP servers, with a routing map for dispatch.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ToolsWithRouting {
    tools: Vec<serde_json::Value>,
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

    // Discover tools from MCP servers via a durable step.
    // On replay, this returns the cached discovery result.
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

    // Establish MCP connections OUTSIDE durable steps. Connections are
    // ephemeral -- they die when Lambda suspends. Caching them inside a
    // step would produce stale connection objects on replay.
    let mcp_clients = connect_mcp_clients(&event.mcp_server_urls)
        .await
        .map_err(|e| DurableError::Internal(e.to_string()))?;

    // Wrap immutable data in Arc to avoid deep clones on each iteration.
    let api_key = Arc::<str>::from(api_key);
    let model = Arc::<str>::from(event.model.as_str());
    let tools = Arc::new(tools_with_routing.tools);
    let routing = Arc::new(tools_with_routing.routing);

    let http_client = reqwest::Client::new();
    let mut messages: Vec<serde_json::Value> = vec![serde_json::json!({
        "role": "user",
        "content": event.prompt,
    })];

    let mut total_input_tokens: u32 = 0;
    let mut total_output_tokens: u32 = 0;
    let mut tools_called: Vec<String> = Vec::new();

    for i in 0..event.max_iterations {
        // Each iteration gets its own child context so operation IDs restart
        // from 0, ensuring deterministic replay even if earlier iterations
        // produced different numbers of operations on a previous invocation.
        let client = http_client.clone();
        let key = Arc::clone(&api_key);
        let mdl = Arc::clone(&model);
        let msgs = messages.clone();
        let tls = Arc::clone(&tools);
        let rte = Arc::clone(&routing);
        let clients = mcp_clients.clone();

        let (response, iteration_tool_calls, tool_results) = ctx
            .run_in_child_context(
                Some(&format!("iteration-{i}")),
                move |child_ctx| async move {
                    execute_iteration(
                        &child_ctx,
                        &client,
                        &key,
                        &mdl,
                        &msgs,
                        &tls,
                        &rte,
                        &clients,
                    )
                    .await
                },
                None,
            )
            .await?;

        total_input_tokens += response.usage.input_tokens;
        total_output_tokens += response.usage.output_tokens;

        for tc in &iteration_tool_calls {
            tools_called.push(tc.name.clone());
        }

        info!(
            iteration = i,
            input_tokens = response.usage.input_tokens,
            output_tokens = response.usage.output_tokens,
            total_input_tokens,
            total_output_tokens,
            tool_count = iteration_tool_calls.len(),
            "Iteration complete"
        );

        messages.push(response_to_assistant_message(&response));

        if response.stop_reason == StopReason::EndTurn || iteration_tool_calls.is_empty() {
            let final_text = extract_text(&response);
            return Ok(AgentOutput {
                response: final_text,
                iterations: i + 1,
                total_input_tokens,
                total_output_tokens,
                tools_called,
            });
        }

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
/// 3. Return the LLM response, extracted tool calls, and any tool results.
async fn execute_iteration(
    ctx: &DurableContextHandle,
    http_client: &reqwest::Client,
    api_key: &str,
    model: &str,
    messages: &[serde_json::Value],
    tools: &[serde_json::Value],
    routing: &HashMap<String, String>,
    mcp_clients: &Arc<HashMap<String, Client<StreamableHttpTransport>>>,
) -> DurableResult<(AnthropicResponse, Vec<ToolCall>, Option<Vec<ToolResult>>)> {
    // Durable LLM call with exponential backoff retry.
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

    let tool_calls = extract_tool_calls(&response);
    if tool_calls.is_empty() {
        return Ok((response, tool_calls, None));
    }

    // Execute tool calls in parallel via durable map.
    let routing = Arc::new(routing.clone());
    let clients = mcp_clients.clone();

    let batch = ctx
        .map(
            Some("tools"),
            tool_calls.clone(),
            move |call: ToolCall, item_ctx: DurableContextHandle, _idx: usize| {
                let r = Arc::clone(&routing);
                let c = clients.clone();
                async move { execute_tool_call(&call, &r, &c, &item_ctx).await }
            },
            None,
        )
        .await?;

    let results: Vec<ToolResult> = batch.values();
    Ok((response, tool_calls, Some(results)))
}

// ---------------------------------------------------------------------------
// Anthropic API client (inline, self-contained)
// ---------------------------------------------------------------------------

/// Call the Anthropic Messages API directly via reqwest.
///
/// Intentionally simple -- no abstraction layers. The durable SDK's
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
        max_tokens: MAX_TOKENS,
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

/// Create an initialized MCP client for the given server URL.
async fn create_mcp_client(
    url_str: &str,
) -> Result<Client<StreamableHttpTransport>, Box<dyn std::error::Error + Send + Sync>> {
    let parsed = url::Url::parse(url_str)?;
    let config = StreamableHttpTransportConfigBuilder::new(parsed).build();
    let transport = StreamableHttpTransport::new(config);
    let mut client = Client::with_info(
        transport,
        Implementation::new(AGENT_NAME, AGENT_VERSION),
    );
    client.initialize(ClientCapabilities::default()).await?;
    Ok(client)
}

/// Discover tools from all configured MCP servers (in parallel).
///
/// For each server: connect, call `list_tools()`, translate schemas to the
/// Anthropic tool format, and build a routing map for dispatch.
///
/// Tool names are prefixed with a host identifier to avoid collisions
/// across servers. E.g., "multiply" on "calc.example.com" becomes
/// "calc__multiply".
async fn discover_tools(
    server_urls: &[String],
) -> Result<ToolsWithRouting, Box<dyn std::error::Error + Send + Sync>> {
    let futures: Vec<_> = server_urls
        .iter()
        .map(|url_str| async move {
            let parsed = url::Url::parse(url_str)?;
            let prefix = parsed
                .host_str()
                .unwrap_or("unknown")
                .split('.')
                .next()
                .unwrap_or("unknown")
                .to_string();

            let client = create_mcp_client(url_str).await?;

            let mut tools = Vec::new();
            let mut routing = Vec::new();
            let mut cursor: Option<String> = None;
            loop {
                let result = client.list_tools(cursor).await?;
                for tool_info in &result.tools {
                    let prefixed_name = format!("{prefix}__{}", tool_info.name);
                    routing.push((prefixed_name, url_str.clone()));
                    tools.push(translate_mcp_tool(tool_info, &prefix));
                }
                match result.next_cursor {
                    Some(next) => cursor = Some(next),
                    None => break,
                }
            }
            Ok::<_, Box<dyn std::error::Error + Send + Sync>>((tools, routing))
        })
        .collect();

    let results = futures::future::join_all(futures).await;

    let mut all_tools = Vec::new();
    let mut all_routing = HashMap::new();
    for result in results {
        let (tools, routing) = result?;
        all_tools.extend(tools);
        for (name, url) in routing {
            all_routing.insert(name, url);
        }
    }

    Ok(ToolsWithRouting {
        tools: all_tools,
        routing: all_routing,
    })
}

/// Establish live MCP client connections for tool execution (in parallel).
///
/// CRITICAL: This runs OUTSIDE any durable step. MCP connections are
/// ephemeral -- they die when Lambda suspends. Caching them inside a
/// `ctx.step()` would produce stale connection objects on replay.
async fn connect_mcp_clients(
    server_urls: &[String],
) -> Result<
    Arc<HashMap<String, Client<StreamableHttpTransport>>>,
    Box<dyn std::error::Error + Send + Sync>,
> {
    let futures: Vec<_> = server_urls
        .iter()
        .map(|url_str| async move {
            let client = create_mcp_client(url_str).await?;
            Ok::<_, Box<dyn std::error::Error + Send + Sync>>((url_str.clone(), client))
        })
        .collect();

    let results = futures::future::join_all(futures).await;
    let mut clients = HashMap::with_capacity(server_urls.len());
    for result in results {
        let (url, client) = result?;
        clients.insert(url, client);
    }

    Ok(Arc::new(clients))
}

/// Translate an MCP `ToolInfo` into the Anthropic tool JSON format.
fn translate_mcp_tool(tool_info: &ToolInfo, prefix: &str) -> serde_json::Value {
    let mut schema = tool_info.input_schema.clone();

    if schema.get("type").is_none() {
        schema["type"] = serde_json::json!("object");
    }

    if let Some(req) = schema.get("required") {
        if req.as_array().is_some_and(|a| a.is_empty()) {
            if let Some(m) = schema.as_object_mut() {
                m.remove("required");
            }
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

/// Execute a single tool call via the appropriate MCP server, with
/// automatic handling of both immediate results and long-running tasks.
///
/// Uses `call_tool_with_task()` to signal MCP Tasks support. The response
/// is either an immediate `Result` or an async `Task` that requires polling.
///
/// NOTE: The PMCP SDK provides `client.call_tool_and_poll()` for simple
/// task polling using `tokio::sleep`. We use `ctx.wait_for_condition()`
/// instead because it *suspends* the Lambda between polls (zero compute
/// cost), while `call_tool_and_poll` keeps the Lambda running and billing.
async fn execute_tool_call(
    call: &ToolCall,
    routing: &HashMap<String, String>,
    mcp_clients: &HashMap<String, Client<StreamableHttpTransport>>,
    ctx: &DurableContextHandle,
) -> DurableResult<ToolResult> {
    let (server_url, original_name) = resolve_tool_name(&call.name, routing)
        .map_err(|e| DurableError::Internal(e.to_string()))?;

    let client = mcp_clients.get(&server_url).ok_or_else(|| {
        DurableError::Internal(format!(
            "No cached MCP client for server URL: {server_url}"
        ))
    })?;

    let response = client
        .call_tool_with_task(original_name.clone(), call.input.clone())
        .await
        .map_err(|e| {
            DurableError::Internal(format!("MCP call_tool failed for {original_name}: {e}"))
        })?;

    match response {
        ToolCallResponse::Result(result) => {
            let text = extract_text_content(&result);
            Ok(ToolResult {
                tool_use_id: call.id.clone(),
                content: text,
                is_error: result.is_error,
            })
        }

        // MCP Tasks: when an MCP server needs time to process, it returns a
        // Task instead of an immediate result. We use ctx.wait_for_condition()
        // to poll -- Lambda suspends between polls with zero compute cost.
        ToolCallResponse::Task(initial_task) => {
            let task_id = initial_task.task_id.clone();
            let poll_ms = initial_task.poll_interval.unwrap_or(5000);
            let poll_secs = std::cmp::max(1, (poll_ms / 1000) as u32);

            info!(
                task_id = %task_id,
                poll_interval_secs = poll_secs,
                "Tool returned task -- polling with wait_for_condition"
            );

            let wait_strategy = Arc::new(move |task: &Task, _attempt: u32| {
                if task.status.is_terminal() {
                    WaitConditionDecision::Stop
                } else {
                    WaitConditionDecision::Continue {
                        delay: Duration::seconds(poll_secs),
                    }
                }
            });

            let config = WaitConditionConfig::new(initial_task, wait_strategy)
                .with_max_attempts(60);

            // Each poll is a durable step -- already-completed polls are
            // replayed from cache if Lambda suspends and resumes.
            let client_clone = client.clone();
            let tid = task_id.clone();
            let final_task = ctx
                .wait_for_condition(
                    Some(&format!("poll-task-{}", &task_id)),
                    move |_current: Task, _step_ctx: StepContext| {
                        let c = client_clone.clone();
                        let id = tid.clone();
                        async move {
                            c.tasks_get(&id)
                                .await
                                .map_err(|e| DurableError::Internal(e.to_string()))
                        }
                    },
                    config,
                )
                .await?;

            match final_task.status {
                TaskStatus::Completed => {
                    let result = client
                        .tasks_result(&final_task.task_id)
                        .await
                        .map_err(|e| DurableError::Internal(e.to_string()))?;
                    let text = extract_text_content(&result);
                    Ok(ToolResult {
                        tool_use_id: call.id.clone(),
                        content: text,
                        is_error: result.is_error,
                    })
                }
                status => {
                    // Failed or Cancelled -- the LLM decides how to recover.
                    Ok(ToolResult {
                        tool_use_id: call.id.clone(),
                        content: format!(
                            "Task {} ended with status: {}",
                            final_task.task_id, status
                        ),
                        is_error: true,
                    })
                }
            }
        }
    }
}

/// Extract text content from an MCP `CallToolResult`.
fn extract_text_content(result: &CallToolResult) -> String {
    let mut out = String::new();
    for c in &result.content {
        if let Content::Text { text } = c {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(text);
        }
    }
    out
}

/// Resolve a prefixed tool name to (server_url, original_name).
///
/// Uses `split_once("__")` so tool names that themselves contain "__"
/// are handled correctly -- only the first "__" is the separator.
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

fn extract_text(response: &AnthropicResponse) -> String {
    let mut out = String::new();
    for block in &response.content {
        if let ContentBlock::Text { text } = block {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(text);
        }
    }
    out
}

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
