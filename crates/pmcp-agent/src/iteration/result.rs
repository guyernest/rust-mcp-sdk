//! Per-iteration and per-run result data types.
//!
//! Every value here is a plain, `Serialize + Deserialize` data type with NO
//! floats, wall-clock, or RNG — so it round-trips through a checkpoint and
//! replays deterministically (AGNT-03). The RUN-level [`RunOutcome`] carries
//! retry classification as returned DATA ([`RetryClass`]), mirroring the
//! "classification as data" precedent of `pmcp`'s `Task::poll_decision`
//! (AGNT-02): the loop never sleeps or applies a backoff policy — the host does.

use pmcp::types::content::Role;
use pmcp::types::sampling::SamplingMessageContent;
use serde::{Deserialize, Serialize};

use crate::seams::RetryClass;

/// One conversational turn: a role plus its ordered content blocks.
///
/// The SDK `SamplingMessage` carries a SINGLE `SamplingMessageContent`, but a
/// completion turn routinely mixes several blocks (assistant text alongside one
/// or more `tool_use` blocks; a tool-results turn folds one `tool_result` per
/// call). `TurnMessage` formalizes that multi-block turn as a replay-safe data
/// type; the engine flattens it into per-block `SamplingMessage`s for the
/// `RunState.history`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnMessage {
    /// The role that produced this turn.
    pub role: Role,
    /// The ordered content blocks of this turn (text / `tool_use` / `tool_result`).
    pub content: Vec<SamplingMessageContent>,
}

impl TurnMessage {
    /// Build a turn from a role and its content blocks.
    #[must_use]
    pub fn new(role: Role, content: Vec<SamplingMessageContent>) -> Self {
        Self { role, content }
    }
}

/// The data produced by ONE loop iteration.
///
/// `is_final` is the pure termination decision for the iteration; the engine
/// checkpoints this so a resumed run can round-trip it (Pitfall 3). No floats,
/// no timestamps.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IterationResult {
    /// The assistant turn the completion produced.
    pub assistant_message: TurnMessage,
    /// The folded tool-results turn, when the iteration dispatched tools.
    pub tool_results_message: Option<TurnMessage>,
    /// Whether this iteration terminates the run.
    pub is_final: bool,
}

/// The RUN-level outcome (AGNT-02) — retry classification is returned DATA.
///
/// The engine never retries or sleeps: it returns the classification so the
/// host decides whether and when to resume. `#[non_exhaustive]` mirrors
/// `TaskPollDecision` so new terminal states cannot silently break exhaustive
/// matches downstream.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum RunOutcome {
    /// The run reached a terminal completion.
    Completed {
        /// The final iteration's result.
        result: IterationResult,
    },
    /// A configured limit (iterations or tokens) stopped the run.
    LimitReached,
    /// A retryable failure occurred; the host decides whether/when to resume.
    RetryRequired {
        /// How the failure is classified for retry (transient / capacity).
        class: RetryClass,
    },
    /// A non-retryable failure occurred.
    Failed {
        /// A secret-free description of the failure.
        error: String,
    },
}

/// The pure limit decision — continue iterating or stop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum LimitDecision {
    /// Under all limits — keep iterating.
    Continue,
    /// A limit was reached — stop.
    Stop,
}

#[cfg(test)]
mod tests {
    use super::{IterationResult, LimitDecision, RunOutcome, TurnMessage};
    use crate::seams::RetryClass;
    use pmcp::types::content::Role;
    use pmcp::types::sampling::SamplingMessageContent;

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
    fn iteration_result_round_trips() {
        let ir = IterationResult {
            assistant_message: text_turn("hi"),
            tool_results_message: None,
            is_final: true,
        };
        let json = serde_json::to_string(&ir).unwrap();
        let back: IterationResult = serde_json::from_str(&json).unwrap();
        assert!(back.is_final);
        assert_eq!(back.assistant_message.content.len(), 1);
    }

    #[test]
    fn run_outcome_carries_retry_class_as_data() {
        let outcome = RunOutcome::RetryRequired {
            class: RetryClass::Capacity { attempt_hint: 0 },
        };
        let json = serde_json::to_string(&outcome).unwrap();
        let back: RunOutcome = serde_json::from_str(&json).unwrap();
        match back {
            RunOutcome::RetryRequired { class } => {
                assert_eq!(class, RetryClass::Capacity { attempt_hint: 0 });
            },
            other => panic!("expected RetryRequired, got {other:?}"),
        }
    }

    #[test]
    fn limit_decision_round_trips_without_floats() {
        for d in [LimitDecision::Continue, LimitDecision::Stop] {
            let json = serde_json::to_string(&d).unwrap();
            assert!(!json.contains('.'));
            let back: LimitDecision = serde_json::from_str(&json).unwrap();
            assert_eq!(d, back);
        }
    }
}
