//! [`AnthropicSource`] (AGNT-06) — a [`CompletionSource`] against the Anthropic
//! Messages API, behind the `anthropic` feature.
//!
//! The Messages API is stricter than the OpenAI shape: messages must strictly
//! alternate user/assistant, the system prompt is a **top-level** field (not a
//! message), and `tool_result` blocks must ride in the `user` turn that
//! immediately follows the `assistant` turn carrying the matching `tool_use`.
//! A raw MCP history (e.g. a parallel-tool turn producing several consecutive
//! `tool_result` messages) would 400 the API, so
//! [`normalize_history`](normalize_history) rewrites it:
//!
//! 1. hoist every `system` message into the top-level `system` field;
//! 2. merge consecutive same-role turns into one (packs parallel `tool_result`
//!    blocks into a single `user` turn after the matching `tool_use`);
//! 3. treat `tool_result` as a `user` block regardless of its source role.
//!
//! Endpoint policy, request timeout, bounded body, and `SecretString` key
//! handling mirror [`OpenAiCompatSource`](super::OpenAiCompatSource).
#![cfg(feature = "anthropic")]

use async_trait::async_trait;
use pmcp::types::sampling::{
    CreateMessageParams, CreateMessageResultWithTools, SamplingMessage, SamplingMessageContent,
};
use pmcp::types::Role;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::http_common::{
    build_client, classify_status, map_reqwest_error, read_bounded_body, tool_result_text,
    validate_endpoint, HttpSourceOptions,
};
use super::SecretString;
use crate::seams::{CompletionError, CompletionSource};

/// Default Anthropic Messages endpoint.
const DEFAULT_BASE_URL: &str = "https://api.anthropic.com/v1/messages";
/// Default `anthropic-version` header value.
const DEFAULT_VERSION: &str = "2023-06-01";
/// Anthropic requires `max_tokens`; used when the params omit it.
const DEFAULT_MAX_TOKENS: u32 = 1024;

/// A [`CompletionSource`] speaking the Anthropic Messages API.
///
/// Construct with [`AnthropicSource::new`] (defaults) or
/// [`AnthropicSource::with_options`]. The `base_url` is the full Messages
/// endpoint (default [`DEFAULT_BASE_URL`]).
pub struct AnthropicSource {
    client: reqwest::Client,
    base_url: String,
    model: String,
    key: SecretString,
    version: String,
    max_body_bytes: usize,
}

impl std::fmt::Debug for AnthropicSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnthropicSource")
            .field("base_url", &self.base_url)
            .field("model", &self.model)
            .field("key", &self.key)
            .field("version", &self.version)
            .field("max_body_bytes", &self.max_body_bytes)
            .finish()
    }
}

impl AnthropicSource {
    /// Create a source with default [`HttpSourceOptions`].
    ///
    /// # Errors
    /// Returns [`CompletionError::Decode`] if the endpoint scheme policy is
    /// violated, or [`CompletionError::Transport`] if the client fails to build.
    pub fn new(
        base_url: impl Into<String>,
        model: impl Into<String>,
        key: SecretString,
    ) -> Result<Self, CompletionError> {
        Self::with_options(base_url, model, key, HttpSourceOptions::default())
    }

    /// Create a source targeting the default Anthropic endpoint.
    ///
    /// # Errors
    /// See [`AnthropicSource::new`].
    pub fn with_default_endpoint(
        model: impl Into<String>,
        key: SecretString,
    ) -> Result<Self, CompletionError> {
        Self::new(DEFAULT_BASE_URL, model, key)
    }

    /// Create a source with explicit [`HttpSourceOptions`].
    ///
    /// # Errors
    /// See [`AnthropicSource::new`].
    pub fn with_options(
        base_url: impl Into<String>,
        model: impl Into<String>,
        key: SecretString,
        options: HttpSourceOptions,
    ) -> Result<Self, CompletionError> {
        let base_url = base_url.into();
        validate_endpoint(&base_url, options.allow_insecure_http)?;
        let client = build_client(options.timeout)?;
        Ok(Self {
            client,
            base_url,
            model: model.into(),
            key,
            version: DEFAULT_VERSION.to_string(),
            max_body_bytes: options.max_body_bytes,
        })
    }
}

#[async_trait]
impl CompletionSource for AnthropicSource {
    async fn create_message(
        &self,
        params: CreateMessageParams,
    ) -> Result<CreateMessageResultWithTools, CompletionError> {
        let body = build_request(&self.model, &params);

        let resp = self
            .client
            .post(&self.base_url)
            .header("x-api-key", self.key.expose())
            .header("anthropic-version", &self.version)
            .json(&body)
            .send()
            .await
            .map_err(|e| map_reqwest_error(&e))?;

        let status = resp.status().as_u16();
        let bytes = read_bounded_body(resp, self.max_body_bytes).await?;
        if let Some(err) = classify_status(status) {
            return Err(err);
        }

        let parsed: MessagesResponse = serde_json::from_slice(&bytes)
            .map_err(|e| CompletionError::Decode(format!("response decode failed: {e}")))?;
        response_to_result(&self.model, parsed)
    }
}

// ---------------------------------------------------------------------------
// History normalization (pure).
// ---------------------------------------------------------------------------

/// A normalized (role-alternating) turn destined for the Messages API.
#[derive(Debug, Clone)]
pub(crate) struct NormalizedTurn {
    /// Only `User` or `Assistant` after normalization.
    pub role: Role,
    /// One or more content blocks packed into this turn.
    pub blocks: Vec<SamplingMessageContent>,
}

/// Normalize an MCP history for the Anthropic Messages API.
///
/// Returns the hoisted `system` prompt (if any) plus role-alternating turns:
/// system messages are removed and concatenated into `system`; `tool_result`
/// blocks are forced to the `user` role; consecutive same-role turns are merged
/// (which packs parallel `tool_result`s into one `user` turn after the matching
/// `tool_use`). Pure — no I/O, deterministic.
pub(crate) fn normalize_history(
    system_prompt: Option<&str>,
    messages: &[SamplingMessage],
) -> (Option<String>, Vec<NormalizedTurn>) {
    let mut system_parts: Vec<String> = Vec::new();
    if let Some(s) = system_prompt {
        system_parts.push(s.to_string());
    }
    let mut turns: Vec<NormalizedTurn> = Vec::new();

    for msg in messages {
        // Hoist system text; drop the message from the turn stream.
        if msg.role == Role::System {
            if let SamplingMessageContent::Text { text, .. } = &msg.content {
                system_parts.push(text.clone());
            }
            continue;
        }
        // tool_result is always a user block, regardless of its source role.
        let role = match &msg.content {
            SamplingMessageContent::ToolResult { .. } => Role::User,
            _ => msg.role,
        };
        match turns.last_mut() {
            Some(last) if last.role == role => last.blocks.push(msg.content.clone()),
            _ => turns.push(NormalizedTurn {
                role,
                blocks: vec![msg.content.clone()],
            }),
        }
    }

    let system = (!system_parts.is_empty()).then(|| system_parts.join("\n"));
    (system, turns)
}

// ---------------------------------------------------------------------------
// Request transform.
// ---------------------------------------------------------------------------

/// Build the Messages request body from sampling params.
fn build_request(model: &str, params: &CreateMessageParams) -> Value {
    let (system, turns) = normalize_history(params.system_prompt.as_deref(), &params.messages);
    let messages: Vec<Value> = turns.iter().map(turn_to_anthropic).collect();

    let mut body = json!({
        "model": model,
        "max_tokens": params.max_tokens.unwrap_or(DEFAULT_MAX_TOKENS),
        "messages": messages,
    });
    if let Some(system) = system {
        body["system"] = json!(system);
    }
    if let Some(temp) = params.temperature {
        body["temperature"] = json!(temp);
    }
    if let Some(tools) = &params.tools {
        body["tools"] = tools_to_anthropic(tools);
    }
    body
}

/// Convert a normalized turn into an Anthropic message object.
fn turn_to_anthropic(turn: &NormalizedTurn) -> Value {
    let blocks: Vec<Value> = turn.blocks.iter().map(block_to_anthropic).collect();
    json!({ "role": role_str(turn.role), "content": blocks })
}

/// Convert one content block into an Anthropic content block.
fn block_to_anthropic(block: &SamplingMessageContent) -> Value {
    match block {
        SamplingMessageContent::Text { text, .. } => json!({ "type": "text", "text": text }),
        SamplingMessageContent::ToolUse {
            id, name, input, ..
        } => json!({ "type": "tool_use", "id": id, "name": name, "input": input }),
        SamplingMessageContent::ToolResult {
            tool_use_id,
            content,
            is_error,
            ..
        } => {
            let mut v = json!({
                "type": "tool_result",
                "tool_use_id": tool_use_id,
                "content": tool_result_text(content),
            });
            if let Some(true) = is_error {
                v["is_error"] = json!(true);
            }
            v
        },
        SamplingMessageContent::Image {
            data, mime_type, ..
        } => json!({
            "type": "image",
            "source": { "type": "base64", "media_type": mime_type, "data": data },
        }),
        SamplingMessageContent::Audio { data, .. } => {
            json!({ "type": "text", "text": format!("[audio {} bytes]", data.len()) })
        },
    }
}

/// Map MCP tool definitions into Anthropic `tools`.
fn tools_to_anthropic(tools: &[pmcp::types::tools::ToolInfo]) -> Value {
    let arr: Vec<Value> = tools
        .iter()
        .map(|t| {
            let mut tool = json!({
                "name": t.name,
                "input_schema": t.input_schema,
            });
            if let Some(desc) = &t.description {
                tool["description"] = json!(desc);
            }
            tool
        })
        .collect();
    json!(arr)
}

fn role_str(role: Role) -> &'static str {
    match role {
        Role::Assistant => "assistant",
        // System is hoisted out before this point; treat anything else as user.
        Role::User | Role::System => "user",
    }
}

// ---------------------------------------------------------------------------
// Response transform.
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize)]
struct MessagesResponse {
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    stop_reason: Option<String>,
    #[serde(default)]
    content: Vec<RespBlock>,
    /// Provider token accounting — mapped into the result's `_meta` so the loop's
    /// cumulative token budget can see it. Anthropic reports `input_tokens` +
    /// `output_tokens` separately; the loop's budget reads their sum.
    #[serde(default)]
    usage: Option<AnthropicUsage>,
}

#[derive(Debug, Deserialize, Serialize)]
struct AnthropicUsage {
    #[serde(default)]
    input_tokens: Option<u64>,
    #[serde(default)]
    output_tokens: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
enum RespBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        #[serde(default)]
        input: Value,
    },
    #[serde(other)]
    Other,
}

/// Convert a parsed Messages response into the `WithTools` result.
fn response_to_result(
    fallback_model: &str,
    resp: MessagesResponse,
) -> Result<CreateMessageResultWithTools, CompletionError> {
    let model = resp.model.unwrap_or_else(|| fallback_model.to_string());
    let mut content: Vec<SamplingMessageContent> = Vec::new();
    for block in resp.content {
        match block {
            RespBlock::Text { text } => {
                content.push(SamplingMessageContent::Text { text, meta: None });
            },
            RespBlock::ToolUse { id, name, input } => {
                content.push(SamplingMessageContent::ToolUse {
                    name,
                    id,
                    input,
                    meta: None,
                });
            },
            RespBlock::Other => {},
        }
    }
    let mut result = CreateMessageResultWithTools::new(model, Role::Assistant, content);
    result.stop_reason = resp.stop_reason;
    if let Some(usage) = resp.usage {
        let total = usage.input_tokens.unwrap_or(0) + usage.output_tokens.unwrap_or(0);
        if total > 0 {
            result.meta = Some(super::http_common::usage_meta(total));
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pmcp::types::Content;
    use serde_json::json;

    fn tool_use_msg(id: &str) -> SamplingMessage {
        SamplingMessage::new(
            Role::Assistant,
            SamplingMessageContent::ToolUse {
                name: "search".to_string(),
                id: id.to_string(),
                input: json!({}),
                meta: None,
            },
        )
    }

    fn tool_result_msg(id: &str) -> SamplingMessage {
        SamplingMessage::new(
            Role::User,
            SamplingMessageContent::ToolResult {
                tool_use_id: id.to_string(),
                content: vec![Content::text("result")],
                structured_content: None,
                is_error: None,
                meta: None,
            },
        )
    }

    #[test]
    fn parallel_tool_results_pack_into_one_user_turn() {
        // assistant(tool_use a), assistant(tool_use b), user(result a), user(result b)
        let history = vec![
            tool_use_msg("a"),
            tool_use_msg("b"),
            tool_result_msg("a"),
            tool_result_msg("b"),
        ];
        let (system, turns) = normalize_history(None, &history);
        assert!(system.is_none());
        assert_eq!(turns.len(), 2, "must collapse to assistant + user");
        assert_eq!(turns[0].role, Role::Assistant);
        assert_eq!(turns[0].blocks.len(), 2, "both tool_use packed");
        assert_eq!(turns[1].role, Role::User);
        assert_eq!(turns[1].blocks.len(), 2, "both tool_result packed");
    }

    #[test]
    fn system_messages_are_hoisted() {
        let history = vec![
            SamplingMessage::new(
                Role::System,
                SamplingMessageContent::Text {
                    text: "you are terse".to_string(),
                    meta: None,
                },
            ),
            SamplingMessage::new(
                Role::User,
                SamplingMessageContent::Text {
                    text: "hi".to_string(),
                    meta: None,
                },
            ),
        ];
        let (system, turns) = normalize_history(Some("base"), &history);
        assert_eq!(system.as_deref(), Some("base\nyou are terse"));
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].role, Role::User);
    }

    #[test]
    fn consecutive_same_role_text_merges() {
        let history = vec![
            SamplingMessage::new(
                Role::User,
                SamplingMessageContent::Text {
                    text: "a".to_string(),
                    meta: None,
                },
            ),
            SamplingMessage::new(
                Role::User,
                SamplingMessageContent::Text {
                    text: "b".to_string(),
                    meta: None,
                },
            ),
        ];
        let (_s, turns) = normalize_history(None, &history);
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].blocks.len(), 2);
    }

    #[test]
    fn request_hoists_system_to_top_level() {
        let params = CreateMessageParams::new(vec![SamplingMessage::new(
            Role::User,
            SamplingMessageContent::Text {
                text: "hi".to_string(),
                meta: None,
            },
        )])
        .with_system_prompt("be nice")
        .with_max_tokens(256);
        let body = build_request("claude-x", &params);
        assert_eq!(body["model"], "claude-x");
        assert_eq!(body["system"], "be nice");
        assert_eq!(body["max_tokens"], 256);
        // system must NOT be a message.
        let msgs = body["messages"].as_array().unwrap();
        assert!(msgs.iter().all(|m| m["role"] != "system"));
    }

    #[test]
    fn request_defaults_max_tokens_when_absent() {
        let params = CreateMessageParams::new(vec![SamplingMessage::new(
            Role::User,
            SamplingMessageContent::Text {
                text: "hi".to_string(),
                meta: None,
            },
        )]);
        let body = build_request("m", &params);
        assert_eq!(body["max_tokens"], DEFAULT_MAX_TOKENS);
    }

    #[test]
    fn tool_use_block_maps_to_anthropic() {
        let block = SamplingMessageContent::ToolUse {
            name: "search".to_string(),
            id: "tu-1".to_string(),
            input: json!({"q": "rust"}),
            meta: None,
        };
        let v = block_to_anthropic(&block);
        assert_eq!(v["type"], "tool_use");
        assert_eq!(v["id"], "tu-1");
        assert_eq!(v["name"], "search");
        assert_eq!(v["input"]["q"], "rust");
    }

    #[test]
    fn response_maps_tool_use_block() {
        let resp: MessagesResponse = serde_json::from_value(json!({
            "model": "claude-x",
            "stop_reason": "tool_use",
            "role": "assistant",
            "content": [
                { "type": "text", "text": "sure" },
                { "type": "tool_use", "id": "tu-9", "name": "lookup", "input": {"x": 1} }
            ]
        }))
        .unwrap();
        let result = response_to_result("fallback", resp).unwrap();
        assert_eq!(result.model, "claude-x");
        assert_eq!(result.stop_reason.as_deref(), Some("tool_use"));
        let tu = result.content.iter().find_map(|c| match c {
            SamplingMessageContent::ToolUse { id, name, .. } => Some((id.clone(), name.clone())),
            _ => None,
        });
        assert_eq!(tu, Some(("tu-9".to_string(), "lookup".to_string())));
    }

    #[test]
    fn response_ignores_unknown_block_types() {
        let resp: MessagesResponse = serde_json::from_value(json!({
            "content": [ { "type": "thinking", "thinking": "hmm" } ]
        }))
        .unwrap();
        let result = response_to_result("m", resp).unwrap();
        assert!(result.content.is_empty());
    }

    #[test]
    fn response_maps_usage_into_meta_so_budget_can_read_it() {
        // Anthropic reports input_tokens + output_tokens separately; the result's
        // `_meta.usage.totalTokens` must carry their sum so the loop's cumulative
        // token budget can trip (previously usage was discarded, so it never did).
        let resp: MessagesResponse = serde_json::from_value(json!({
            "content": [ { "type": "text", "text": "hi" } ],
            "usage": { "input_tokens": 30, "output_tokens": 12 }
        }))
        .unwrap();
        let result = response_to_result("m", resp).unwrap();
        assert_eq!(
            crate::iteration::extract_token_usage(&result),
            42,
            "input+output tokens must reach the budget via _meta"
        );
    }

    #[test]
    fn remote_http_rejected_at_construction() {
        let err = AnthropicSource::new(
            "http://api.example.com/v1/messages",
            "m",
            SecretString::new("k"),
        )
        .unwrap_err();
        assert!(matches!(err, CompletionError::Decode(_)));
    }

    #[test]
    fn key_absent_from_debug() {
        let src =
            AnthropicSource::with_default_endpoint("claude-x", SecretString::new("sk-ant-xyz"))
                .unwrap();
        let dbg = format!("{src:?}");
        assert!(!dbg.contains("sk-ant-xyz"));
        assert!(dbg.contains("SecretString(***)"));
    }
}
