//! The tool-invocation seam — dispatch tool calls.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::RetryClass;

/// A single tool call the model requested.
///
/// `id` correlates the emitted `tool_use` block with the returned
/// `tool_result` (the SDK models this as `SamplingMessageContent::ToolUse{ id }`
/// ↔ `ToolResult{ tool_use_id }`). It MUST be preserved onto the matching
/// [`ToolCallResult`] so parallel dispatch can reattach results to their calls.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCall {
    /// Correlation id — matches `ToolUse.id` and the result's `id`.
    pub id: String,
    /// Tool name.
    pub name: String,
    /// Tool input arguments.
    pub arguments: serde_json::Value,
    /// Optional connector name selecting which client dispatches this call.
    pub connector: Option<String>,
}

/// The result of a single [`ToolCall`], carrying the same correlation `id`.
///
/// Errors are data, not panics: a failed call becomes a [`ToolCallResult`] with
/// `is_error = true` and an `error` string that never contains secret material.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCallResult {
    /// Correlation id — matches the originating [`ToolCall::id`].
    pub id: String,
    /// The tool's returned content (JSON).
    pub content: serde_json::Value,
    /// Whether the call failed.
    pub is_error: bool,
    /// Error detail when `is_error` — never contains secret material.
    pub error: Option<String>,
}

impl ToolCallResult {
    /// Build a successful result carrying `call_id` and `content`.
    #[must_use]
    pub fn ok(call_id: impl Into<String>, content: serde_json::Value) -> Self {
        Self {
            id: call_id.into(),
            content,
            is_error: false,
            error: None,
        }
    }

    /// Build an error result carrying `call_id` and a (secret-free) message.
    #[must_use]
    pub fn error(call_id: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            id: call_id.into(),
            content: serde_json::Value::Null,
            is_error: true,
            error: Some(message.into()),
        }
    }
}

/// Dispatches tool calls on behalf of the loop.
///
/// [`invoke`](ToolInvoker::invoke) is the required single-call primitive.
/// [`invoke_batch`](ToolInvoker::invoke_batch) crosses the seam as a batch (D-07)
/// with a DEFAULT body that dispatches SEQUENTIALLY over `invoke`; the SDK
/// `ClientToolInvoker` (plan 108-05) overrides it with bounded-concurrency
/// parallel dispatch, and the platform maps the one seam call onto durable
/// `ctx.map`.
///
/// **`invoke_batch` contract:** returns EXACTLY ONE result per input, in INPUT
/// ORDER, with each result's `id` MATCHING its input [`ToolCall::id`].
#[async_trait]
pub trait ToolInvoker: Send + Sync {
    /// Dispatch a single tool call, preserving `call.id` onto the result.
    async fn invoke(&self, call: ToolCall) -> ToolCallResult;

    /// Dispatch a batch of calls. DEFAULT: sequential over [`invoke`](Self::invoke).
    ///
    /// Returns one result per input, in input order, with matching ids.
    async fn invoke_batch(&self, calls: Vec<ToolCall>) -> Vec<ToolCallResult> {
        let mut out = Vec::with_capacity(calls.len());
        for call in calls {
            out.push(self.invoke(call).await);
        }
        out
    }
}

/// Error from a [`ToolInvoker`] connection/transport (distinct from a per-call
/// tool error, which is carried in [`ToolCallResult`]).
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ToolError {
    /// Transport / connection / 5xx failure — transient.
    #[error("tool transport error: {0}")]
    Transport(String),
    /// Rate-limited / capacity — retryable with backpressure.
    #[error("tool capacity error: {0}")]
    Capacity(String),
    /// Non-retryable failure (bad request, decode, missing connector).
    #[error("tool fatal error: {0}")]
    Fatal(String),
}

impl ToolError {
    /// Classify this error for the loop's retry-as-data contract.
    #[must_use]
    pub fn retry_class(&self) -> RetryClass {
        match self {
            Self::Transport(_) => RetryClass::Transient { attempt_hint: 0 },
            Self::Capacity(_) => RetryClass::Capacity { attempt_hint: 0 },
            Self::Fatal(_) => RetryClass::Fatal,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ToolCall, ToolCallResult, ToolInvoker};
    use async_trait::async_trait;

    /// Echo invoker: preserves the call id and echoes the name into content.
    struct EchoInvoker;

    #[async_trait]
    impl ToolInvoker for EchoInvoker {
        async fn invoke(&self, call: ToolCall) -> ToolCallResult {
            ToolCallResult::ok(call.id, serde_json::json!({ "echoed": call.name }))
        }
    }

    fn call(id: &str, name: &str) -> ToolCall {
        ToolCall {
            id: id.into(),
            name: name.into(),
            arguments: serde_json::Value::Null,
            connector: None,
        }
    }

    #[tokio::test]
    async fn default_invoke_batch_is_one_per_input_ordered_and_id_matched() {
        let inv = EchoInvoker;
        let calls = vec![call("a", "alpha"), call("b", "beta"), call("c", "gamma")];
        let results = inv.invoke_batch(calls.clone()).await;

        // Exactly one result per input.
        assert_eq!(results.len(), calls.len());
        // Input order + id correlation preserved.
        for (input, result) in calls.iter().zip(results.iter()) {
            assert_eq!(input.id, result.id);
            assert!(!result.is_error);
            assert_eq!(result.content, serde_json::json!({ "echoed": input.name }));
        }
    }

    #[test]
    fn tool_call_result_serde_round_trips_without_floats() {
        let r = ToolCallResult::error("x", "boom");
        let json = serde_json::to_string(&r).unwrap();
        assert!(!json.contains('.') || !json.contains("e-")); // no float notation
        let back: ToolCallResult = serde_json::from_str(&json).unwrap();
        assert_eq!(r, back);
    }
}
