//! Pure, side-effect-free decision functions — the replay-deterministic core.
//!
//! Every function here is synchronous and total on its inputs: NO `std::time`,
//! NO RNG, NO HashMap-iteration-order reliance (the crate builds `serde_json`
//! with `preserve_order`). This is exactly what makes AGNT-03 replay-safety
//! hold — feeding identical effect results to these functions yields identical
//! decisions. They are small and separate so each stays well under PMAT
//! cognitive-complexity 25 and is independently unit-testable.

use pmcp::types::content::{Content, Role};
use pmcp::types::sampling::{CreateMessageResultWithTools, SamplingMessageContent};
use serde_json::Value;

use crate::seams::{CompletionError, ToolCall, ToolCallResult, ToolError};

use super::result::{LimitDecision, TurnMessage};

/// Pure end-turn detection: only the model's terminal stop reasons end a turn.
///
/// Mirrors the reference loop's `matches!(stop_reason, Some("end_turn" | "stop"))`.
/// `tool_use` and an absent stop reason are NOT terminal.
#[must_use]
pub fn is_end_turn(stop_reason: Option<&str>) -> bool {
    matches!(stop_reason, Some("end_turn" | "stop"))
}

/// Pure, counter-based limit check: stop at OR past either limit.
///
/// Uses only integer counters (iteration index, cumulative tokens) — never a
/// clock — so the decision is replay-deterministic.
#[must_use]
pub fn check_limits(
    iteration: u32,
    max_iterations: u32,
    tokens_used: u32,
    token_budget: Option<u32>,
) -> LimitDecision {
    let over_budget = token_budget.is_some_and(|budget| tokens_used >= budget);
    if iteration >= max_iterations || over_budget {
        LimitDecision::Stop
    } else {
        LimitDecision::Continue
    }
}

/// A coarse, replay-safe error signal the loop classifies for retry.
///
/// Defined locally so [`classify_retry`] can match it EXHAUSTIVELY with no `_`
/// arm — the classification cannot silently drift when a variant is added.
/// Seam errors (which are `#[non_exhaustive]` and owned elsewhere) are funneled
/// into this enum via [`ErrorSignal::from_completion`] / [`from_tool`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErrorSignal {
    /// Transport / 5xx failure — transient, retryable.
    ServerError,
    /// Rate-limit / capacity (429 / 529) — retryable with backpressure.
    RateLimited,
    /// Decode / auth / bad-request — non-retryable.
    Fatal,
}

impl ErrorSignal {
    /// Funnel a [`CompletionError`] into an [`ErrorSignal`].
    #[must_use]
    pub fn from_completion(err: &CompletionError) -> Self {
        // Exhaustive in-crate (the `#[non_exhaustive]` only constrains downstream
        // crates): a new variant here is a compile error, forcing a deliberate
        // classification rather than a silent fatal default.
        match err {
            CompletionError::Transport(_) => Self::ServerError,
            CompletionError::Capacity(_) => Self::RateLimited,
            CompletionError::Decode(_) | CompletionError::Auth => Self::Fatal,
        }
    }

    /// Funnel a [`ToolError`] into an [`ErrorSignal`].
    #[must_use]
    pub fn from_tool(err: &ToolError) -> Self {
        match err {
            ToolError::Transport(_) => Self::ServerError,
            ToolError::Capacity(_) => Self::RateLimited,
            ToolError::Fatal(_) => Self::Fatal,
        }
    }
}

/// Classify an [`ErrorSignal`] into a [`RetryClass`] returned as DATA.
///
/// Total and exhaustive over the LOCAL signal enum — no `_` arm, so the mapping
/// cannot drift. The loop returns the class; it never sleeps or backs off.
#[must_use]
pub fn classify_retry(signal: ErrorSignal) -> crate::seams::RetryClass {
    use crate::seams::RetryClass;
    match signal {
        ErrorSignal::ServerError => RetryClass::Transient { attempt_hint: 0 },
        ErrorSignal::RateLimited => RetryClass::Capacity { attempt_hint: 0 },
        ErrorSignal::Fatal => RetryClass::Fatal,
    }
}

/// Extract the tool calls the model requested from a completion.
///
/// Yields one [`ToolCall`] per `tool_use` block, preserving `id`/`name`/input;
/// non-`tool_use` blocks are skipped and a completion with no tool blocks yields
/// an empty vec. Typed throughout — never unwraps untrusted content, never
/// panics.
#[must_use]
pub fn extract_tool_calls(result: &CreateMessageResultWithTools) -> Vec<ToolCall> {
    result
        .content
        .iter()
        .filter_map(|block| match block {
            SamplingMessageContent::ToolUse {
                id, name, input, ..
            } => Some(ToolCall {
                id: id.clone(),
                name: name.clone(),
                arguments: input.clone(),
                connector: None,
            }),
            _ => None,
        })
        .collect()
}

/// Capture the assistant turn (role + all content blocks) from a completion.
///
/// The engine appends this to the run history; it keeps every block (text and
/// `tool_use`) so a resumed run sees the exact turn.
#[must_use]
pub fn assistant_turn(result: &CreateMessageResultWithTools) -> TurnMessage {
    TurnMessage::new(result.role, result.content.clone())
}

/// Deterministic token accounting for a completion.
///
/// `CreateMessageResultWithTools` carries no `usage` field, so we read an
/// optional advisory `_meta.usage.totalTokens` (or `total_tokens`) when the
/// source provides one, defaulting to `0`. This is a counter input, never a
/// clock, and is fully deterministic for replay.
#[must_use]
pub fn extract_token_usage(result: &CreateMessageResultWithTools) -> u32 {
    result
        .meta
        .as_ref()
        .and_then(|m| m.get("usage"))
        .and_then(|usage| {
            usage
                .get("totalTokens")
                .or_else(|| usage.get("total_tokens"))
        })
        .and_then(Value::as_u64)
        .map_or(0, |n| u32::try_from(n).unwrap_or(u32::MAX))
}

/// Fold tool results back into a `user`-role tool-results turn (pure transform).
///
/// Each [`ToolCallResult`] becomes a `tool_result` block whose `tool_use_id`
/// preserves the originating call's `id` — so the correlation survives into the
/// next completion. A failed call is carried as `is_error = true`, never a
/// panic. Preserves input order.
#[must_use]
pub fn digest_tool_results(results: Vec<ToolCallResult>) -> TurnMessage {
    let content = results
        .into_iter()
        .map(|r| {
            let text = if r.is_error {
                r.error.clone().unwrap_or_else(|| "tool error".to_string())
            } else {
                r.content.to_string()
            };
            SamplingMessageContent::ToolResult {
                tool_use_id: r.id,
                content: vec![Content::Text { text }],
                structured_content: Some(r.content),
                is_error: Some(r.is_error),
                meta: None,
            }
        })
        .collect();
    TurnMessage::new(Role::User, content)
}

/// Extract the assistant's structured output candidate for schema validation.
///
/// Returns the first text block parsed as JSON (the conventional carrier of a
/// structured final answer), or `None` when no block parses. Pure and total.
#[must_use]
fn structured_output_candidate(assistant_message: &TurnMessage) -> Option<Value> {
    assistant_message
        .content
        .iter()
        .find_map(|block| match block {
            SamplingMessageContent::Text { text, .. } => serde_json::from_str::<Value>(text).ok(),
            _ => None,
        })
}

/// Decide whether the assistant turn satisfies an optional output schema.
///
/// Takes NO stop reason (a schema decision cannot depend on a signal it never
/// receives — reference `evaluate_submit_result`). When `output_schema` is
/// `None` this is a no-op that never terminates (`false`). When a schema is
/// present, the assistant's structured output must both parse AND validate via
/// the real `jsonschema` validator for the turn to be final.
#[must_use]
pub fn evaluate_submit_result(
    assistant_message: &TurnMessage,
    output_schema: Option<&Value>,
) -> bool {
    let Some(schema) = output_schema else {
        return false;
    };
    let Some(instance) = structured_output_candidate(assistant_message) else {
        return false;
    };
    match jsonschema::validator_for(schema) {
        Ok(validator) => validator.is_valid(&instance),
        Err(_) => false,
    }
}

/// Parse a raw completion payload into the typed shape.
///
/// The `serde_json::from_slice` boundary that turns untrusted bytes into a
/// [`CreateMessageResultWithTools`]. Malformed bytes yield an `Err`, never a
/// panic — fuzz-hardened in `tests/digest_fuzz.rs`.
///
/// # Errors
/// Returns the underlying `serde_json` error when `bytes` is not a valid
/// completion payload.
pub fn parse_completion(bytes: &[u8]) -> Result<CreateMessageResultWithTools, serde_json::Error> {
    serde_json::from_slice(bytes)
}

/// Parse a raw tool-result payload into the typed shape.
///
/// # Errors
/// Returns the underlying `serde_json` error when `bytes` is not a valid
/// tool-result payload.
pub fn parse_tool_result(bytes: &[u8]) -> Result<ToolCallResult, serde_json::Error> {
    serde_json::from_slice(bytes)
}

#[cfg(test)]
mod tests {
    use super::{
        assistant_turn, check_limits, classify_retry, digest_tool_results, evaluate_submit_result,
        extract_token_usage, extract_tool_calls, is_end_turn, parse_completion, ErrorSignal,
    };
    use crate::iteration::result::{LimitDecision, TurnMessage};
    use crate::seams::{RetryClass, ToolCallResult};
    use pmcp::types::content::Role;
    use pmcp::types::sampling::{CreateMessageResultWithTools, SamplingMessageContent};
    use serde_json::json;

    #[test]
    fn is_end_turn_matches_only_terminal_reasons() {
        assert!(is_end_turn(Some("end_turn")));
        assert!(is_end_turn(Some("stop")));
        assert!(!is_end_turn(Some("tool_use")));
        assert!(!is_end_turn(Some("length")));
        assert!(!is_end_turn(None));
    }

    #[test]
    fn check_limits_stops_at_or_past_either_limit() {
        assert_eq!(check_limits(0, 5, 0, Some(100)), LimitDecision::Continue);
        assert_eq!(check_limits(4, 5, 99, Some(100)), LimitDecision::Continue);
        // iteration boundary
        assert_eq!(check_limits(5, 5, 0, Some(100)), LimitDecision::Stop);
        assert_eq!(check_limits(6, 5, 0, Some(100)), LimitDecision::Stop);
        // token boundary
        assert_eq!(check_limits(0, 5, 100, Some(100)), LimitDecision::Stop);
        assert_eq!(check_limits(0, 5, 101, Some(100)), LimitDecision::Stop);
        // no budget → only iterations bound the run (tokens never stop it)
        assert_eq!(check_limits(0, 5, 1_000_000, None), LimitDecision::Continue);
        assert_eq!(check_limits(5, 5, 1_000_000, None), LimitDecision::Stop);
    }

    #[test]
    fn classify_retry_is_exhaustive_over_signals() {
        assert_eq!(
            classify_retry(ErrorSignal::ServerError),
            RetryClass::Transient { attempt_hint: 0 }
        );
        assert_eq!(
            classify_retry(ErrorSignal::RateLimited),
            RetryClass::Capacity { attempt_hint: 0 }
        );
        assert_eq!(classify_retry(ErrorSignal::Fatal), RetryClass::Fatal);
    }

    fn result_with(content: Vec<SamplingMessageContent>) -> CreateMessageResultWithTools {
        CreateMessageResultWithTools::new("m", Role::Assistant, content)
    }

    #[test]
    fn extract_tool_calls_yields_tool_use_blocks_only() {
        let result = result_with(vec![
            SamplingMessageContent::Text {
                text: "thinking".into(),
                meta: None,
            },
            SamplingMessageContent::ToolUse {
                name: "search".into(),
                id: "tu-1".into(),
                input: json!({"q": "rust"}),
                meta: None,
            },
        ]);
        let calls = extract_tool_calls(&result);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].id, "tu-1");
        assert_eq!(calls[0].name, "search");
        assert_eq!(calls[0].arguments, json!({"q": "rust"}));
    }

    #[test]
    fn extract_tool_calls_empty_when_no_tool_use() {
        let result = result_with(vec![SamplingMessageContent::Text {
            text: "no tools".into(),
            meta: None,
        }]);
        assert!(extract_tool_calls(&result).is_empty());
    }

    #[test]
    fn assistant_turn_keeps_role_and_all_blocks() {
        let result = result_with(vec![
            SamplingMessageContent::Text {
                text: "a".into(),
                meta: None,
            },
            SamplingMessageContent::ToolUse {
                name: "t".into(),
                id: "i".into(),
                input: json!({}),
                meta: None,
            },
        ]);
        let turn = assistant_turn(&result);
        assert!(matches!(turn.role, Role::Assistant));
        assert_eq!(turn.content.len(), 2);
    }

    #[test]
    fn extract_token_usage_reads_meta_or_defaults_zero() {
        let plain = result_with(vec![]);
        assert_eq!(extract_token_usage(&plain), 0);

        let mut meta = serde_json::Map::new();
        meta.insert("usage".into(), json!({ "totalTokens": 42 }));
        let with_usage = result_with(vec![]).with_meta(meta);
        assert_eq!(extract_token_usage(&with_usage), 42);
    }

    #[test]
    fn digest_tool_results_preserves_id_correlation_and_order() {
        let results = vec![
            ToolCallResult::ok("tu-1", json!({"a": 1})),
            ToolCallResult::error("tu-2", "boom"),
        ];
        let turn = digest_tool_results(results);
        assert!(matches!(turn.role, Role::User));
        assert_eq!(turn.content.len(), 2);
        match &turn.content[0] {
            SamplingMessageContent::ToolResult {
                tool_use_id,
                is_error,
                ..
            } => {
                assert_eq!(tool_use_id, "tu-1");
                assert_eq!(*is_error, Some(false));
            },
            other => panic!("expected tool_result, got {other:?}"),
        }
        match &turn.content[1] {
            SamplingMessageContent::ToolResult {
                tool_use_id,
                is_error,
                ..
            } => {
                assert_eq!(tool_use_id, "tu-2");
                assert_eq!(*is_error, Some(true));
            },
            other => panic!("expected tool_result, got {other:?}"),
        }
    }

    #[test]
    fn digest_tool_results_survives_arbitrary_content_without_panic() {
        // Nested/odd JSON must not panic the digestion path.
        let weird = json!({ "nested": [null, {"k": [1, 2, 3]}], "deep": {"x": {"y": {}}} });
        let turn = digest_tool_results(vec![ToolCallResult::ok("id", weird)]);
        assert_eq!(turn.content.len(), 1);
    }

    fn text_turn(text: &str) -> TurnMessage {
        TurnMessage::new(
            Role::Assistant,
            vec![SamplingMessageContent::Text {
                text: text.into(),
                meta: None,
            }],
        )
    }

    #[test]
    fn evaluate_submit_result_is_noop_without_schema() {
        assert!(!evaluate_submit_result(&text_turn("{}"), None));
    }

    #[test]
    fn evaluate_submit_result_validates_against_schema() {
        let schema = json!({
            "type": "object",
            "required": ["answer"],
            "properties": { "answer": { "type": "string" } }
        });
        // Valid structured output → final.
        assert!(evaluate_submit_result(
            &text_turn(r#"{"answer":"forty-two"}"#),
            Some(&schema)
        ));
        // Missing required field → not final.
        assert!(!evaluate_submit_result(
            &text_turn(r#"{"other":1}"#),
            Some(&schema)
        ));
        // Non-JSON text → not final (no candidate).
        assert!(!evaluate_submit_result(
            &text_turn("plain answer"),
            Some(&schema)
        ));
    }

    #[test]
    fn error_signal_funnels_seam_errors() {
        use crate::seams::{CompletionError, ToolError};
        assert_eq!(
            ErrorSignal::from_completion(&CompletionError::Transport("x".into())),
            ErrorSignal::ServerError
        );
        assert_eq!(
            ErrorSignal::from_completion(&CompletionError::Capacity("x".into())),
            ErrorSignal::RateLimited
        );
        assert_eq!(
            ErrorSignal::from_completion(&CompletionError::Auth),
            ErrorSignal::Fatal
        );
        assert_eq!(
            ErrorSignal::from_tool(&ToolError::Transport("x".into())),
            ErrorSignal::ServerError
        );
    }

    #[test]
    fn parse_completion_errors_on_malformed_bytes_without_panic() {
        assert!(parse_completion(b"not json at all").is_err());
        assert!(parse_completion(b"").is_err());
        assert!(parse_completion(b"{}").is_err()); // missing required fields
    }
}
