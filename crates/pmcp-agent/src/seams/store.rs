//! The conversation-store seam — load/save resumable run state.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use pmcp::types::sampling::SamplingMessage;
use serde::{Deserialize, Serialize};

use super::ToolCall;

/// The checkpoint phase of a run — where the engine is in one iteration.
///
/// The engine advances this so a crash-resumed run knows whether tool calls
/// were already dispatched: it saves [`PendingTools`](RunPhase::PendingTools)
/// BEFORE dispatch and [`ToolsCompleted`](RunPhase::ToolsCompleted) after, so a
/// crash between the two does not re-run side-effecting tools (plan 108-03).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum RunPhase {
    /// Ready to request the next completion.
    ReadyForCompletion,
    /// Tool calls have been recorded and are about to (or currently) dispatch.
    PendingTools,
    /// Tool results are in `history`; ready to fold them into the next turn.
    ToolsCompleted,
}

/// Resumable state for one agent run.
///
/// Holds everything needed to resume mid-iteration (D-06): the message
/// `history`, the `iteration` and `tokens_used` counters (counters, NOT
/// timestamps — determinism), the `pending_tool_calls` awaiting dispatch, and
/// the checkpoint `phase`. Contains no floats, `SystemTime`, or `Instant` so it
/// serializes deterministically for replay.
///
/// (No `PartialEq`: the SDK `SamplingMessage` in `history` does not implement
/// it — compare `RunState`s by their serialized form instead.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunState {
    /// The conversation so far (SDK message type).
    pub history: Vec<SamplingMessage>,
    /// Loop iteration counter.
    pub iteration: u32,
    /// Cumulative tokens consumed (a limit input, not a clock).
    pub tokens_used: u32,
    /// Tool calls recorded but not yet folded back as results.
    pub pending_tool_calls: Vec<ToolCall>,
    /// The checkpoint phase this run is paused at.
    pub phase: RunPhase,
}

impl RunState {
    /// A fresh run: empty history, zeroed counters, ready to complete.
    #[must_use]
    pub fn new() -> Self {
        Self {
            history: Vec::new(),
            iteration: 0,
            tokens_used: 0,
            pending_tool_calls: Vec::new(),
            phase: RunPhase::ReadyForCompletion,
        }
    }
}

impl Default for RunState {
    fn default() -> Self {
        Self::new()
    }
}

/// Error from a [`ConversationStore`].
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum StoreError {
    /// The backing store could not be reached or read/written.
    #[error("conversation store backend error: {0}")]
    Backend(String),
    /// A stored `RunState` could not be (de)serialized.
    #[error("conversation store serialization error: {0}")]
    Serialization(String),
}

/// Loads and saves resumable [`RunState`] keyed by run id.
///
/// Object-safe (`Arc<dyn ConversationStore>`). `InMemoryStore` is the trivial
/// laptop default; a durable host substitutes its own backend for mid-iteration
/// resume.
#[async_trait]
pub trait ConversationStore: Send + Sync {
    /// Load the state for `run_id`, or `None` if there is no such run.
    async fn load(&self, run_id: &str) -> Result<Option<RunState>, StoreError>;
    /// Persist `state` for `run_id`, overwriting any prior state.
    async fn save(&self, run_id: &str, state: &RunState) -> Result<(), StoreError>;
}

/// Forward the seam through a shared `Arc<dyn ConversationStore>`.
///
/// The 108-06 adapter shares ONE conversation store across agent runs (so a
/// `run_id` can resume prior history — D-12 continuity) while the generic
/// [`AgentEngine`](crate::iteration::AgentEngine) takes its store by value. This
/// blanket impl lets the engine be parameterised over an erased
/// `Arc<dyn ConversationStore>`.
#[async_trait]
impl ConversationStore for Arc<dyn ConversationStore> {
    async fn load(&self, run_id: &str) -> Result<Option<RunState>, StoreError> {
        (**self).load(run_id).await
    }

    async fn save(&self, run_id: &str, state: &RunState) -> Result<(), StoreError> {
        (**self).save(run_id, state).await
    }
}

/// In-memory [`ConversationStore`] for laptops and tests.
///
/// Backed by `std::sync::Mutex<HashMap<..>>` — no async lock is needed because
/// no `.await` happens while the guard is held (lock → clone/insert → drop).
#[derive(Debug, Default)]
pub struct InMemoryStore {
    runs: Mutex<HashMap<String, RunState>>,
}

impl InMemoryStore {
    /// Create an empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl ConversationStore for InMemoryStore {
    async fn load(&self, run_id: &str) -> Result<Option<RunState>, StoreError> {
        // Scope the guard so it is dropped before returning — never held across
        // an await.
        let state = {
            let guard = self
                .runs
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            guard.get(run_id).cloned()
        };
        Ok(state)
    }

    async fn save(&self, run_id: &str, state: &RunState) -> Result<(), StoreError> {
        {
            let mut guard = self
                .runs
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            guard.insert(run_id.to_string(), state.clone());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{ConversationStore, InMemoryStore, RunPhase, RunState};

    #[tokio::test]
    async fn in_memory_store_round_trips() {
        let store = InMemoryStore::new();
        assert!(store.load("run-1").await.unwrap().is_none());

        let mut state = RunState::new();
        state.iteration = 3;
        state.tokens_used = 42;
        state.phase = RunPhase::PendingTools;
        store.save("run-1", &state).await.unwrap();

        let loaded = store.load("run-1").await.unwrap().unwrap();
        assert_eq!(
            serde_json::to_string(&loaded).unwrap(),
            serde_json::to_string(&state).unwrap()
        );
    }
}
