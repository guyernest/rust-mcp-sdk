//! [`OpenAiCompatSource`] (AGNT-05) — a [`CompletionSource`] against any
//! OpenAI-compatible `/chat/completions` endpoint (Ollama, vLLM, OpenRouter,
//! xAI, DeepSeek, …), behind the `openai-compat` feature.
//!
//! Hardening (per the 108-04 threat register):
//! - **Endpoint policy** (T-108-04-03/05): plain `http://` only for
//!   loopback/localhost (Ollama) or with an explicit `allow_insecure_http`;
//!   otherwise `https://` is required.
//! - **Timeout + bounded body** (T-108-04-04): the reqwest client carries a
//!   request timeout; the response body is read with a hard size cap.
//! - **Key secrecy** (T-108-04-01): the API key lives in [`SecretString`] and is
//!   only exposed when building the `Authorization` header — never logged.
//!
//! No streaming this phase.
#![cfg(feature = "openai-compat")]

use async_trait::async_trait;
use pmcp::types::sampling::{
    CreateMessageParams, CreateMessageResultWithTools, SamplingMessage, SamplingMessageContent,
    ToolChoiceMode,
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

/// A [`CompletionSource`] speaking the OpenAI `/chat/completions` shape.
///
/// Construct with [`OpenAiCompatSource::new`] (defaults) or
/// [`OpenAiCompatSource::with_options`]. The `base_url` is the endpoint root
/// (e.g. `https://api.openai.com/v1` or `http://localhost:11434/v1`);
/// `/chat/completions` is appended.
pub struct OpenAiCompatSource {
    client: reqwest::Client,
    base_url: String,
    model: String,
    key: SecretString,
    max_body_bytes: usize,
}

impl std::fmt::Debug for OpenAiCompatSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never render the key (it is a SecretString, but be explicit).
        f.debug_struct("OpenAiCompatSource")
            .field("base_url", &self.base_url)
            .field("model", &self.model)
            .field("key", &self.key)
            .field("max_body_bytes", &self.max_body_bytes)
            .finish()
    }
}

impl OpenAiCompatSource {
    /// Create a source with default [`HttpSourceOptions`] (60 s timeout, 8 MiB
    /// body cap, loopback-only HTTP).
    ///
    /// # Errors
    /// Returns [`CompletionError::Decode`] if the endpoint scheme policy is
    /// violated, or [`CompletionError::Transport`] if the HTTP client fails to
    /// build.
    pub fn new(
        base_url: impl Into<String>,
        model: impl Into<String>,
        key: SecretString,
    ) -> Result<Self, CompletionError> {
        Self::with_options(base_url, model, key, HttpSourceOptions::default())
    }

    /// Create a source with explicit [`HttpSourceOptions`].
    ///
    /// # Errors
    /// See [`OpenAiCompatSource::new`].
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
            base_url: base_url.trim_end_matches('/').to_string(),
            model: model.into(),
            key,
            max_body_bytes: options.max_body_bytes,
        })
    }

    fn endpoint(&self) -> String {
        format!("{}/chat/completions", self.base_url)
    }
}

#[async_trait]
impl CompletionSource for OpenAiCompatSource {
    async fn create_message(
        &self,
        params: CreateMessageParams,
    ) -> Result<CreateMessageResultWithTools, CompletionError> {
        let body = build_request(&self.model, &params);

        let resp = self
            .client
            .post(self.endpoint())
            .bearer_auth(self.key.expose())
            .json(&body)
            .send()
            .await
            .map_err(|e| map_reqwest_error(&e))?;

        let status = resp.status().as_u16();
        let bytes = read_bounded_body(resp, self.max_body_bytes).await?;
        if let Some(err) = classify_status(status) {
            return Err(err);
        }

        let parsed: ChatResponse = serde_json::from_slice(&bytes)
            .map_err(|e| CompletionError::Decode(format!("response decode failed: {e}")))?;
        response_to_result(&self.model, parsed)
    }
}

// ---------------------------------------------------------------------------
// Request transform: CreateMessageParams -> OpenAI chat-completions body.
// ---------------------------------------------------------------------------

/// Build the `/chat/completions` request body from sampling params.
fn build_request(model: &str, params: &CreateMessageParams) -> Value {
    let mut messages: Vec<Value> = Vec::new();
    if let Some(system) = &params.system_prompt {
        messages.push(json!({ "role": "system", "content": system }));
    }
    for msg in &params.messages {
        messages.push(message_to_openai(msg));
    }

    let mut body = json!({
        "model": model,
        "messages": messages,
    });
    if let Some(max) = params.max_tokens {
        body["max_tokens"] = json!(max);
    }
    if let Some(temp) = params.temperature {
        body["temperature"] = json!(temp);
    }
    if let Some(tools) = &params.tools {
        body["tools"] = tools_to_openai(tools);
    }
    if let Some(choice) = &params.tool_choice {
        if let Some(mode) = choice.mode {
            body["tool_choice"] = json!(tool_choice_str(mode));
        }
    }
    body
}

/// Map a single [`SamplingMessage`] into an OpenAI message object.
fn message_to_openai(msg: &SamplingMessage) -> Value {
    match &msg.content {
        SamplingMessageContent::Text { text, .. } => {
            json!({ "role": role_str(msg.role), "content": text })
        },
        SamplingMessageContent::ToolUse {
            id, name, input, ..
        } => json!({
            "role": "assistant",
            "tool_calls": [{
                "id": id,
                "type": "function",
                "function": { "name": name, "arguments": input.to_string() },
            }],
        }),
        SamplingMessageContent::ToolResult {
            tool_use_id,
            content,
            ..
        } => json!({
            "role": "tool",
            "tool_call_id": tool_use_id,
            "content": tool_result_text(content),
        }),
        SamplingMessageContent::Image { data, .. } | SamplingMessageContent::Audio { data, .. } => {
            // Non-text modalities are not part of the chat-completions text path
            // this phase; forward a placeholder so history stays well-formed.
            json!({ "role": role_str(msg.role), "content": format!("[binary {} bytes]", data.len()) })
        },
    }
}

/// Map MCP tool definitions into OpenAI `tools`.
fn tools_to_openai(tools: &[pmcp::types::tools::ToolInfo]) -> Value {
    let arr: Vec<Value> = tools
        .iter()
        .map(|t| {
            let mut function = json!({
                "name": t.name,
                "parameters": t.input_schema,
            });
            if let Some(desc) = &t.description {
                function["description"] = json!(desc);
            }
            json!({ "type": "function", "function": function })
        })
        .collect();
    json!(arr)
}

fn role_str(role: Role) -> &'static str {
    match role {
        Role::User => "user",
        Role::Assistant => "assistant",
        Role::System => "system",
    }
}

fn tool_choice_str(mode: ToolChoiceMode) -> &'static str {
    match mode {
        ToolChoiceMode::Auto => "auto",
        ToolChoiceMode::Required => "required",
        ToolChoiceMode::None => "none",
    }
}

// ---------------------------------------------------------------------------
// Response transform: OpenAI response -> CreateMessageResultWithTools.
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize)]
struct ChatResponse {
    #[serde(default)]
    choices: Vec<Choice>,
    #[serde(default)]
    model: Option<String>,
    /// Provider token accounting — mapped into the result's `_meta` so the loop's
    /// cumulative token budget can see it (OpenAI-compatible endpoints return
    /// `usage.total_tokens`).
    #[serde(default)]
    usage: Option<OpenAiUsage>,
}

#[derive(Debug, Deserialize, Serialize)]
struct OpenAiUsage {
    #[serde(default)]
    total_tokens: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Choice {
    message: RespMessage,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct RespMessage {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<RespToolCall>>,
}

#[derive(Debug, Deserialize, Serialize)]
struct RespToolCall {
    id: String,
    function: RespFunction,
}

#[derive(Debug, Deserialize, Serialize)]
struct RespFunction {
    name: String,
    #[serde(default)]
    arguments: String,
}

/// Convert a parsed OpenAI response into the `WithTools` result, taking the
/// first choice. Malformed `tool_calls.function.arguments` JSON is a `Fatal`
/// decode error (never a panic).
fn response_to_result(
    fallback_model: &str,
    resp: ChatResponse,
) -> Result<CreateMessageResultWithTools, CompletionError> {
    let model = resp.model.unwrap_or_else(|| fallback_model.to_string());
    let usage_total = resp.usage.and_then(|u| u.total_tokens);
    let choice = resp
        .choices
        .into_iter()
        .next()
        .ok_or_else(|| CompletionError::Decode("response has no choices".to_string()))?;

    let mut content: Vec<SamplingMessageContent> = Vec::new();
    if let Some(text) = choice.message.content {
        if !text.is_empty() {
            content.push(SamplingMessageContent::Text { text, meta: None });
        }
    }
    for call in choice.message.tool_calls.unwrap_or_default() {
        content.push(tool_call_to_content(call)?);
    }

    let mut result = CreateMessageResultWithTools::new(model, Role::Assistant, content);
    result.stop_reason = choice.finish_reason;
    if let Some(total) = usage_total {
        result.meta = Some(super::http_common::usage_meta(total));
    }
    Ok(result)
}

/// Map one OpenAI `tool_call` into a `ToolUse` block, parsing its `arguments`
/// JSON string.
fn tool_call_to_content(call: RespToolCall) -> Result<SamplingMessageContent, CompletionError> {
    let input: Value = if call.function.arguments.trim().is_empty() {
        json!({})
    } else {
        serde_json::from_str(&call.function.arguments)
            .map_err(|e| CompletionError::Decode(format!("malformed tool_call arguments: {e}")))?
    };
    Ok(SamplingMessageContent::ToolUse {
        name: call.function.name,
        id: call.id,
        input,
        meta: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pmcp::types::Content;

    fn params_with_tools() -> CreateMessageParams {
        use pmcp::types::sampling::ToolChoice;
        CreateMessageParams::new(vec![SamplingMessage::new(
            Role::User,
            SamplingMessageContent::Text {
                text: "hello".to_string(),
                meta: None,
            },
        )])
        .with_system_prompt("be terse")
        .with_max_tokens(256)
        .with_tool_choice(ToolChoice::required())
    }

    #[test]
    fn request_hoists_system_and_sets_fields() {
        let body = build_request("gpt-x", &params_with_tools());
        assert_eq!(body["model"], "gpt-x");
        assert_eq!(body["messages"][0]["role"], "system");
        assert_eq!(body["messages"][0]["content"], "be terse");
        assert_eq!(body["messages"][1]["role"], "user");
        assert_eq!(body["max_tokens"], 256);
        assert_eq!(body["tool_choice"], "required");
    }

    #[test]
    fn tool_use_message_becomes_tool_calls() {
        let msg = SamplingMessage::new(
            Role::Assistant,
            SamplingMessageContent::ToolUse {
                name: "search".to_string(),
                id: "tc-1".to_string(),
                input: json!({"q": "rust"}),
                meta: None,
            },
        );
        let v = message_to_openai(&msg);
        assert_eq!(v["role"], "assistant");
        assert_eq!(v["tool_calls"][0]["id"], "tc-1");
        assert_eq!(v["tool_calls"][0]["function"]["name"], "search");
        // arguments must be a JSON *string*.
        assert!(v["tool_calls"][0]["function"]["arguments"].is_string());
    }

    #[test]
    fn tool_result_message_becomes_tool_role() {
        let msg = SamplingMessage::new(
            Role::User,
            SamplingMessageContent::ToolResult {
                tool_use_id: "tc-1".to_string(),
                content: vec![Content::text("42")],
                structured_content: None,
                is_error: None,
                meta: None,
            },
        );
        let v = message_to_openai(&msg);
        assert_eq!(v["role"], "tool");
        assert_eq!(v["tool_call_id"], "tc-1");
        assert_eq!(v["content"], "42");
    }

    #[test]
    fn tool_choice_modes_map() {
        assert_eq!(tool_choice_str(ToolChoiceMode::Auto), "auto");
        assert_eq!(tool_choice_str(ToolChoiceMode::Required), "required");
        assert_eq!(tool_choice_str(ToolChoiceMode::None), "none");
    }

    #[test]
    fn response_maps_tool_calls_to_tool_use() {
        let resp: ChatResponse = serde_json::from_value(json!({
            "model": "gpt-x",
            "choices": [{
                "message": {
                    "content": "calling",
                    "tool_calls": [{
                        "id": "call-7",
                        "type": "function",
                        "function": { "name": "lookup", "arguments": "{\"id\":5}" }
                    }]
                },
                "finish_reason": "tool_calls"
            }]
        }))
        .unwrap();
        let result = response_to_result("fallback", resp).unwrap();
        assert_eq!(result.model, "gpt-x");
        assert_eq!(result.stop_reason.as_deref(), Some("tool_calls"));
        let tu = result.content.iter().find_map(|c| match c {
            SamplingMessageContent::ToolUse {
                id, name, input, ..
            } => Some((id.clone(), name.clone(), input.clone())),
            _ => None,
        });
        let (id, name, input) = tu.expect("tool_use present");
        assert_eq!(id, "call-7");
        assert_eq!(name, "lookup");
        assert_eq!(input["id"], 5);
    }

    #[test]
    fn response_takes_first_of_multiple_choices() {
        let resp: ChatResponse = serde_json::from_value(json!({
            "choices": [
                { "message": { "content": "first" } },
                { "message": { "content": "second" } }
            ]
        }))
        .unwrap();
        let result = response_to_result("m", resp).unwrap();
        assert!(matches!(
            result.content.first(),
            Some(SamplingMessageContent::Text { text, .. }) if text == "first"
        ));
    }

    #[test]
    fn response_missing_usage_and_finish_reason_is_ok() {
        let resp: ChatResponse = serde_json::from_value(json!({
            "choices": [{ "message": { "content": "ok" } }]
        }))
        .unwrap();
        let result = response_to_result("m", resp).unwrap();
        assert_eq!(result.model, "m");
        assert!(result.stop_reason.is_none());
        // No usage reported → the budget reads 0 (only iterations bound the run).
        assert_eq!(crate::iteration::extract_token_usage(&result), 0);
    }

    #[test]
    fn response_maps_usage_total_tokens_into_meta() {
        // `usage.total_tokens` must reach the result's `_meta.usage.totalTokens`
        // so the loop's cumulative token budget can trip (previously discarded).
        let resp: ChatResponse = serde_json::from_value(json!({
            "choices": [{ "message": { "content": "ok" }, "finish_reason": "stop" }],
            "usage": { "prompt_tokens": 40, "completion_tokens": 17, "total_tokens": 57 }
        }))
        .unwrap();
        let result = response_to_result("m", resp).unwrap();
        assert_eq!(crate::iteration::extract_token_usage(&result), 57);
    }

    #[test]
    fn response_no_choices_is_decode_error() {
        let resp: ChatResponse = serde_json::from_value(json!({ "choices": [] })).unwrap();
        let err = response_to_result("m", resp).unwrap_err();
        assert!(matches!(err, CompletionError::Decode(_)));
    }

    #[test]
    fn malformed_tool_arguments_error_no_panic() {
        let call = RespToolCall {
            id: "x".to_string(),
            function: RespFunction {
                name: "n".to_string(),
                arguments: "{not json".to_string(),
            },
        };
        let err = tool_call_to_content(call).unwrap_err();
        assert!(matches!(err, CompletionError::Decode(_)));
    }

    #[test]
    fn empty_tool_arguments_default_to_empty_object() {
        let call = RespToolCall {
            id: "x".to_string(),
            function: RespFunction {
                name: "n".to_string(),
                arguments: "".to_string(),
            },
        };
        let content = tool_call_to_content(call).unwrap();
        assert!(matches!(
            content,
            SamplingMessageContent::ToolUse { input, .. } if input == json!({})
        ));
    }

    #[test]
    fn remote_http_rejected_at_construction() {
        let err = OpenAiCompatSource::new("http://api.example.com/v1", "m", SecretString::new("k"))
            .unwrap_err();
        assert!(matches!(err, CompletionError::Decode(_)));
    }

    #[test]
    fn localhost_http_accepted_at_construction() {
        assert!(
            OpenAiCompatSource::new("http://localhost:11434/v1", "m", SecretString::new("k"))
                .is_ok()
        );
    }

    #[test]
    fn key_absent_from_debug() {
        let src = OpenAiCompatSource::new(
            "https://api.example.com/v1",
            "m",
            SecretString::new("sk-abc"),
        )
        .unwrap();
        let dbg = format!("{src:?}");
        assert!(!dbg.contains("sk-abc"));
        assert!(dbg.contains("SecretString(***)"));
    }
}
