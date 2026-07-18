//! [`ClientToolInvoker`] — the tasks-aware [`ToolInvoker`] over a
//! [`ConnectorClient`] (AGNT-08).
//!
//! Each call goes out as a `tools/call`; if the result carries a related-task
//! envelope ([`CallToolResult::related_task`]), the invoker drives it to
//! terminal via [`ConnectorClient::wait_for_related_task`] with a
//! host-configured hard `max_poll_duration_secs` cap, so it can NEVER poll
//! forever (closing the `WaitForTaskOptions::default()` no-timeout gap). The
//! actual poll loop / `poll_decision` classification lives in the SDK primitive
//! — this invoker only supplies the cap and maps outcomes onto
//! [`ToolCallResult`]s.
//!
//! [`invoke_batch`](ToolInvoker::invoke_batch) overrides the seam's sequential
//! default with BOUNDED-concurrency parallel dispatch (`buffered(N)`),
//! preserving input order and correlating [`ToolCall::id`]s.

use std::sync::Arc;

use async_trait::async_trait;
use futures::stream::{self, StreamExt};

use pmcp::types::{CallToolResult, Content};
use pmcp::WaitForTaskOptions;

use super::factory::{ConnectorClient, InvokerError};
use crate::seams::{ToolCall, ToolCallResult, ToolInvoker};

/// Default bound on concurrent in-flight tool calls in [`invoke_batch`].
const DEFAULT_MAX_CONCURRENCY: usize = 8;

/// A tasks-aware [`ToolInvoker`] over a single [`ConnectorClient`].
///
/// Holds a hard `max_poll_duration_secs` cap that bounds every task poll, and a
/// `max_concurrency` bound for batch dispatch. Both default to safe values and
/// are host-configurable.
pub struct ClientToolInvoker {
    connector: Arc<dyn ConnectorClient>,
    max_poll_duration_secs: u64,
    max_concurrency: usize,
}

impl ClientToolInvoker {
    /// Create an invoker over `connector` with a hard task-poll cap (seconds).
    ///
    /// The cap is the ceiling on how long a single related task may be polled
    /// before the call returns a timeout error entry — the invoker can never
    /// hang on a never-completing task.
    #[must_use]
    pub fn new(connector: Arc<dyn ConnectorClient>, max_poll_duration_secs: u64) -> Self {
        Self {
            connector,
            max_poll_duration_secs,
            max_concurrency: DEFAULT_MAX_CONCURRENCY,
        }
    }

    /// Override the batch concurrency bound (minimum 1).
    #[must_use]
    pub fn with_max_concurrency(mut self, max_concurrency: usize) -> Self {
        self.max_concurrency = max_concurrency.max(1);
        self
    }

    /// Dispatch one call, driving a related task to terminal when present.
    async fn dispatch(&self, call: &ToolCall) -> Result<CallToolResult, InvokerError> {
        let result = self
            .connector
            .call_tool(&call.name, call.arguments.clone())
            .await?;
        // Task-augmented result? Drive it to terminal under our hard cap. The
        // `Some(cap)` wins over any metadata hint (`or_from_metadata` only fills
        // UNSET fields), so the host maximum is authoritative.
        if let Some(meta) = result.related_task() {
            let opts = WaitForTaskOptions {
                max_poll_duration_secs: Some(self.max_poll_duration_secs),
                ..WaitForTaskOptions::default()
            }
            .or_from_metadata(&meta);
            self.connector.wait_for_related_task(&meta, opts).await
        } else {
            Ok(result)
        }
    }

    /// Project a `CallToolResult` payload into the invoker's JSON content shape.
    fn result_value(result: &CallToolResult) -> serde_json::Value {
        result.structured_content.clone().unwrap_or_else(|| {
            serde_json::to_value(&result.content).unwrap_or(serde_json::Value::Null)
        })
    }

    /// Extract a secret-free error message from a tool-side error result.
    fn error_message(result: &CallToolResult) -> String {
        result
            .content
            .iter()
            .find_map(|c| match c {
                Content::Text { text } => Some(text.clone()),
                _ => None,
            })
            .unwrap_or_else(|| "tool reported an error with no text detail".to_string())
    }

    /// Map a dispatch outcome onto a `ToolCallResult`, preserving `id`.
    fn to_result(id: String, outcome: Result<CallToolResult, InvokerError>) -> ToolCallResult {
        match outcome {
            Ok(result) if result.is_error => {
                ToolCallResult::error(id, Self::error_message(&result))
            },
            Ok(result) => ToolCallResult::ok(id, Self::result_value(&result)),
            Err(err) => ToolCallResult::error(id, err.to_string()),
        }
    }
}

#[async_trait]
impl ToolInvoker for ClientToolInvoker {
    async fn invoke(&self, call: ToolCall) -> ToolCallResult {
        let id = call.id.clone();
        let outcome = self.dispatch(&call).await;
        Self::to_result(id, outcome)
    }

    async fn invoke_batch(&self, calls: Vec<ToolCall>) -> Vec<ToolCallResult> {
        // BOUNDED concurrency: `buffered(N)` polls at most N futures at once and
        // yields results in INPUT ORDER, so ids stay correlated to their calls
        // without an explicit index sort (D-07).
        stream::iter(calls)
            .map(|call| self.invoke(call))
            .buffered(self.max_concurrency)
            .collect()
            .await
    }
}
