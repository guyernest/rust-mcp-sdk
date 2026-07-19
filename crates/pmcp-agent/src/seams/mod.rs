//! The three object-safe async effect seams + shared retry classification.
//!
//! The loop performs ALL side effects through these seams:
//! - [`CompletionSource`] — produce the next model completion
//! - [`ToolInvoker`] — dispatch tool calls
//! - [`ConversationStore`] — load/save resumable run state
//!
//! Retry classification crosses the seams as DATA ([`RetryClass`]) — the loop
//! never sleeps or applies a backoff policy; the host does.

mod completion;
mod store;
mod tool;

pub use completion::{CompletionError, CompletionSource};
pub use store::{ConversationStore, InMemoryStore, RunPhase, RunState, StoreError};
pub use tool::{ToolCall, ToolCallResult, ToolError, ToolInvoker};

use serde::{Deserialize, Serialize};

/// Retry classification exposed as data — never a backoff/sleep policy.
///
/// Mirrors the "classification as data" precedent of `pmcp`'s
/// `Task::poll_decision` / `TaskPollDecision`: an exhaustive, `#[non_exhaustive]`
/// enum the loop returns to the host, which decides whether and when to retry.
/// The `attempt_hint` carries the provider's advisory attempt count when known,
/// but implies no timing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum RetryClass {
    /// Not retryable — decode/auth/validation failure.
    Fatal,
    /// Transient failure (transport / 5xx) — retryable.
    Transient {
        /// Advisory attempt count from the provider, if any.
        attempt_hint: u32,
    },
    /// Capacity/rate-limit failure (429 / 529) — retryable, backpressure applies.
    Capacity {
        /// Advisory attempt count from the provider, if any.
        attempt_hint: u32,
    },
}

#[cfg(test)]
mod tests {
    use super::RetryClass;

    #[test]
    fn retry_class_serde_round_trips() {
        for rc in [
            RetryClass::Fatal,
            RetryClass::Transient { attempt_hint: 2 },
            RetryClass::Capacity { attempt_hint: 0 },
        ] {
            let json = serde_json::to_string(&rc).unwrap();
            let back: RetryClass = serde_json::from_str(&json).unwrap();
            assert_eq!(rc, back);
        }
    }
}
