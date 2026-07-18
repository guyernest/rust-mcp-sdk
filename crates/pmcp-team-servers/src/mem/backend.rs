//! The storage-agnostic team-memory backend trait plus its dev-grade
//! in-memory implementation.
//!
//! [`TeamMemoryBackend`] is an object-safe async trait defining the six
//! `mem__*` operations of the `mem-mcp` reference server (`add`, `get`,
//! `search`, `list_recent`, `delete`, `complete_task`). The SDK ships one
//! dev-grade implementation, [`InMemoryMemoryBackend`], guarded by a
//! `parking_lot::RwLock` (the explicitly declared sync primitive — 109-03
//! review), whose `search` ranks stored memories with the hand-rolled,
//! dependency-free [`crate::mem::bm25`] scorer (NO embedder).
//!
//! # Determinism (109-03 review)
//!
//! Ids come from an injectable [`IdSource`] seam. Production uses
//! [`UuidIdSource`] (random UUIDv4); conformance and examples use
//! [`SequentialIdSource`], which mints stable ids (`mem-001`, `mem-002`, …) so
//! fixtures are reproducible. Creation order is tracked by a monotonic ordinal
//! independent of the id, giving `list_recent` a stable newest-first order and
//! `search` a stable tie-break.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use crate::mem::bm25::{tokenize, Bm25Index};

/// A stored memory record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Memory {
    /// Stable identifier minted by the backend's [`IdSource`].
    pub id: String,
    /// The free-text memory content.
    pub text: String,
    /// Optional tags associated with the memory.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Monotonic creation ordinal (1-based). Higher == newer. Independent of
    /// [`Memory::id`] so ordering is stable even under a random id source.
    pub created_ordinal: u64,
}

/// The terminal record returned by [`TeamMemoryBackend::complete_task`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskCompletion {
    /// The id of the task that was marked complete.
    pub task_id: String,
    /// Terminal status (always `"completed"` for this dev backend).
    pub status: String,
    /// Optional related task carried through under SEP-1686 semantics; the
    /// server layer surfaces it under the SDK related-task `_meta` key.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub related_task_id: Option<String>,
}

/// Errors returned by [`TeamMemoryBackend`] operations.
#[derive(Debug, thiserror::Error)]
pub enum MemError {
    /// No memory exists for the requested id.
    #[error("memory not found: {0}")]
    NotFound(String),

    /// The tool arguments were invalid (missing/ill-typed/empty fields).
    #[error("invalid arguments: {0}")]
    InvalidArgs(String),

    /// A configurable dev limit was exceeded (item count, text length, query
    /// length, or result limit). Keeps the in-memory dev backend bounded
    /// (T-109-03-01).
    #[error("dev limit exceeded: {0}")]
    LimitExceeded(String),
}

/// A source of memory ids — the deterministic ID seam (109-03 review).
///
/// Object-safe so a backend can hold `Arc<dyn IdSource>`. Implementations use
/// interior mutability, so `next_id` takes `&self`.
pub trait IdSource: Send + Sync {
    /// Returns the next id. Must never return the same id twice.
    fn next_id(&self) -> String;
}

/// Production id source: a fresh random UUIDv4 per call.
#[derive(Debug, Default, Clone, Copy)]
pub struct UuidIdSource;

impl IdSource for UuidIdSource {
    fn next_id(&self) -> String {
        uuid::Uuid::new_v4().to_string()
    }
}

/// Deterministic id source for conformance/examples: `mem-001`, `mem-002`, ….
///
/// The counter starts at 1 and is zero-padded to at least three digits.
#[derive(Debug)]
pub struct SequentialIdSource {
    prefix: String,
    counter: AtomicU64,
}

impl SequentialIdSource {
    /// Creates a sequential source with the default `mem` prefix, starting at 1.
    #[must_use]
    pub fn new() -> Self {
        Self::with_prefix("mem")
    }

    /// Creates a sequential source with a custom prefix, starting at 1.
    #[must_use]
    pub fn with_prefix(prefix: impl Into<String>) -> Self {
        Self {
            prefix: prefix.into(),
            counter: AtomicU64::new(1),
        }
    }
}

impl Default for SequentialIdSource {
    fn default() -> Self {
        Self::new()
    }
}

impl IdSource for SequentialIdSource {
    fn next_id(&self) -> String {
        let n = self.counter.fetch_add(1, Ordering::SeqCst);
        format!("{}-{n:03}", self.prefix)
    }
}

/// Configurable dev limits keeping the in-memory backend bounded (T-109-03-01).
#[derive(Debug, Clone)]
pub struct MemLimits {
    /// Maximum number of stored memories before `add` returns [`MemError::LimitExceeded`].
    pub max_items: usize,
    /// Maximum accepted memory text length in bytes.
    pub max_text_len: usize,
    /// Maximum accepted search-query length in bytes.
    pub max_query_len: usize,
    /// Upper bound applied to any caller-supplied result `limit`.
    pub max_result_limit: usize,
}

impl Default for MemLimits {
    fn default() -> Self {
        Self {
            max_items: 10_000,
            max_text_len: 100_000,
            max_query_len: 10_000,
            max_result_limit: 1_000,
        }
    }
}

/// The storage-agnostic team-memory backend contract.
///
/// Implementations perform the actual storage + ranking behind the six
/// `mem__*` tools. They are `Send + Sync` for concurrent request handling and
/// object-safe (usable as `Arc<dyn TeamMemoryBackend>`).
#[async_trait]
pub trait TeamMemoryBackend: Send + Sync {
    /// Stores `text` (with optional `tags`) as a new memory and returns it.
    ///
    /// # Errors
    ///
    /// - [`MemError::InvalidArgs`] if `text` is empty.
    /// - [`MemError::LimitExceeded`] if the item count or text-length dev limit
    ///   would be exceeded.
    async fn add(&self, text: String, tags: Vec<String>) -> Result<Memory, MemError>;

    /// Returns the memory with the given `id`.
    ///
    /// # Errors
    ///
    /// - [`MemError::NotFound`] if no memory has that id.
    async fn get(&self, id: &str) -> Result<Memory, MemError>;

    /// Returns up to `limit` memories ranked by keyword relevance to `query`,
    /// highest score first. Non-matching memories (score `0.0`) are omitted.
    /// Ties break by creation ordinal ascending, then id.
    ///
    /// # Errors
    ///
    /// - [`MemError::LimitExceeded`] if the query-length dev limit is exceeded.
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<Memory>, MemError>;

    /// Returns up to `limit` memories, newest first (by creation ordinal).
    ///
    /// # Errors
    ///
    /// Returns [`MemError`] only if a future backend adds fallible storage; the
    /// in-memory backend is infallible here.
    async fn list_recent(&self, limit: usize) -> Result<Vec<Memory>, MemError>;

    /// Deletes the memory with `id`. Returns `true` if it existed (idempotent).
    ///
    /// # Errors
    ///
    /// Returns [`MemError`] only if a future backend adds fallible storage; the
    /// in-memory backend is infallible here.
    async fn delete(&self, id: &str) -> Result<bool, MemError>;

    /// Marks `task_id` complete (SEP-1686), optionally linking `related_task_id`.
    ///
    /// # Errors
    ///
    /// - [`MemError::InvalidArgs`] if `task_id` is empty.
    async fn complete_task(
        &self,
        task_id: &str,
        related_task_id: Option<String>,
    ) -> Result<TaskCompletion, MemError>;
}

/// The mutable, lock-guarded state of [`InMemoryMemoryBackend`].
#[derive(Debug, Default)]
struct State {
    memories: Vec<Memory>,
    next_ordinal: u64,
}

/// Dev-grade in-memory [`TeamMemoryBackend`].
///
/// State lives behind a single `parking_lot::RwLock` (the declared sync
/// primitive). `search` rebuilds a [`Bm25Index`] from the live corpus each call
/// — acceptable at the bounded dev scale enforced by [`MemLimits`].
pub struct InMemoryMemoryBackend {
    state: RwLock<State>,
    id_source: Arc<dyn IdSource>,
    limits: MemLimits,
}

impl std::fmt::Debug for InMemoryMemoryBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InMemoryMemoryBackend")
            .field("state", &self.state)
            .field("limits", &self.limits)
            .finish_non_exhaustive()
    }
}

impl InMemoryMemoryBackend {
    /// Creates a backend using the production [`UuidIdSource`] and default limits.
    #[must_use]
    pub fn new() -> Self {
        Self::with_id_source(Arc::new(UuidIdSource))
    }

    /// Creates a backend with deterministic (`mem-001`, …) ids and default
    /// limits — for conformance fixtures and examples.
    #[must_use]
    pub fn deterministic() -> Self {
        Self::with_id_source(Arc::new(SequentialIdSource::new()))
    }

    /// Creates a backend with a custom [`IdSource`] seam and default limits.
    #[must_use]
    pub fn with_id_source(id_source: Arc<dyn IdSource>) -> Self {
        Self {
            state: RwLock::new(State {
                memories: Vec::new(),
                next_ordinal: 1,
            }),
            id_source,
            limits: MemLimits::default(),
        }
    }

    /// Overrides the dev limits (builder-style).
    #[must_use]
    pub fn with_limits(mut self, limits: MemLimits) -> Self {
        self.limits = limits;
        self
    }
}

impl Default for InMemoryMemoryBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TeamMemoryBackend for InMemoryMemoryBackend {
    async fn add(&self, text: String, tags: Vec<String>) -> Result<Memory, MemError> {
        if text.is_empty() {
            return Err(MemError::InvalidArgs("text must be non-empty".to_string()));
        }
        if text.len() > self.limits.max_text_len {
            return Err(MemError::LimitExceeded(format!(
                "text length {} exceeds max {}",
                text.len(),
                self.limits.max_text_len
            )));
        }
        let mut state = self.state.write();
        if state.memories.len() >= self.limits.max_items {
            return Err(MemError::LimitExceeded(format!(
                "item count {} at max {}",
                state.memories.len(),
                self.limits.max_items
            )));
        }
        let id = self.id_source.next_id();
        let ordinal = state.next_ordinal;
        state.next_ordinal += 1;
        let memory = Memory {
            id,
            text,
            tags,
            created_ordinal: ordinal,
        };
        state.memories.push(memory.clone());
        Ok(memory)
    }

    async fn get(&self, id: &str) -> Result<Memory, MemError> {
        self.state
            .read()
            .memories
            .iter()
            .find(|m| m.id == id)
            .cloned()
            .ok_or_else(|| MemError::NotFound(id.to_string()))
    }

    async fn search(&self, query: &str, limit: usize) -> Result<Vec<Memory>, MemError> {
        if query.len() > self.limits.max_query_len {
            return Err(MemError::LimitExceeded(format!(
                "query length {} exceeds max {}",
                query.len(),
                self.limits.max_query_len
            )));
        }
        let limit = limit.min(self.limits.max_result_limit);
        let state = self.state.read();
        let query_terms = tokenize(query);

        // Rebuild the index over the live corpus (doc_id == position).
        let mut index = Bm25Index::new();
        for memory in &state.memories {
            index.push_text(&memory.text);
        }

        // Score, keep only matches (score > 0.0).
        let mut scored: Vec<(usize, f64)> = (0..state.memories.len())
            .map(|doc_id| (doc_id, index.score(&query_terms, doc_id)))
            .filter(|(_, score)| *score > 0.0)
            .collect();

        // Stable tie-break: score desc, then creation ordinal asc, then id.
        scored.sort_by(|(a, sa), (b, sb)| {
            sb.partial_cmp(sa)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    state.memories[*a]
                        .created_ordinal
                        .cmp(&state.memories[*b].created_ordinal)
                })
                .then_with(|| state.memories[*a].id.cmp(&state.memories[*b].id))
        });

        Ok(scored
            .into_iter()
            .take(limit)
            .map(|(doc_id, _)| state.memories[doc_id].clone())
            .collect())
    }

    async fn list_recent(&self, limit: usize) -> Result<Vec<Memory>, MemError> {
        let limit = limit.min(self.limits.max_result_limit);
        let state = self.state.read();
        let mut items = state.memories.clone();
        // Newest first; ties (impossible for distinct ordinals) fall back to id.
        items.sort_by(|a, b| {
            b.created_ordinal
                .cmp(&a.created_ordinal)
                .then_with(|| a.id.cmp(&b.id))
        });
        items.truncate(limit);
        Ok(items)
    }

    async fn delete(&self, id: &str) -> Result<bool, MemError> {
        let mut state = self.state.write();
        let before = state.memories.len();
        state.memories.retain(|m| m.id != id);
        Ok(state.memories.len() != before)
    }

    async fn complete_task(
        &self,
        task_id: &str,
        related_task_id: Option<String>,
    ) -> Result<TaskCompletion, MemError> {
        if task_id.is_empty() {
            return Err(MemError::InvalidArgs(
                "taskId must be non-empty".to_string(),
            ));
        }
        Ok(TaskCompletion {
            task_id: task_id.to_string(),
            status: "completed".to_string(),
            related_task_id,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deterministic() -> InMemoryMemoryBackend {
        InMemoryMemoryBackend::deterministic()
    }

    #[tokio::test]
    async fn add_then_get_round_trips() {
        let backend = deterministic();
        let added = backend
            .add("hello world".to_string(), vec!["greeting".to_string()])
            .await
            .unwrap();
        assert_eq!(added.id, "mem-001");
        assert_eq!(added.created_ordinal, 1);
        let fetched = backend.get("mem-001").await.unwrap();
        assert_eq!(fetched, added);
    }

    #[tokio::test]
    async fn deterministic_id_seam_yields_stable_ids() {
        let backend = deterministic();
        let a = backend.add("a".to_string(), vec![]).await.unwrap();
        let b = backend.add("b".to_string(), vec![]).await.unwrap();
        let c = backend.add("c".to_string(), vec![]).await.unwrap();
        assert_eq!(
            [a.id.as_str(), b.id.as_str(), c.id.as_str()],
            ["mem-001", "mem-002", "mem-003"]
        );
    }

    #[tokio::test]
    async fn get_missing_is_not_found() {
        let backend = deterministic();
        let err = backend.get("nope").await.unwrap_err();
        assert!(matches!(err, MemError::NotFound(_)));
    }

    #[tokio::test]
    async fn delete_removes_and_is_idempotent() {
        let backend = deterministic();
        backend.add("gone".to_string(), vec![]).await.unwrap();
        assert!(backend.delete("mem-001").await.unwrap());
        assert!(backend.get("mem-001").await.is_err());
        assert!(!backend.delete("mem-001").await.unwrap());
    }

    #[tokio::test]
    async fn list_recent_is_newest_first() {
        let backend = deterministic();
        backend.add("first".to_string(), vec![]).await.unwrap();
        backend.add("second".to_string(), vec![]).await.unwrap();
        backend.add("third".to_string(), vec![]).await.unwrap();
        let recent = backend.list_recent(10).await.unwrap();
        let ids: Vec<&str> = recent.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, ["mem-003", "mem-002", "mem-001"]);
    }

    #[tokio::test]
    async fn search_ranks_by_relevance() {
        let backend = deterministic();
        backend
            .add("the quick brown fox jumps".to_string(), vec![])
            .await
            .unwrap();
        backend
            .add("lorem ipsum dolor sit amet".to_string(), vec![])
            .await
            .unwrap();
        backend
            .add("a quick quick note about foxes".to_string(), vec![])
            .await
            .unwrap();
        let hits = backend.search("quick fox", 10).await.unwrap();
        // Both quick/fox docs match; the unrelated lorem doc is omitted.
        assert_eq!(hits.len(), 2);
        assert!(hits.iter().all(|m| m.id != "mem-002"));
    }

    #[tokio::test]
    async fn search_empty_query_returns_nothing() {
        let backend = deterministic();
        backend.add("some text".to_string(), vec![]).await.unwrap();
        assert!(backend.search("", 10).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn search_tie_break_is_stable_by_ordinal() {
        // Three identical documents → identical scores. Tie-break must be
        // creation ordinal ascending, i.e. mem-001, mem-002, mem-003.
        let backend = deterministic();
        for _ in 0..3 {
            backend
                .add("same same same".to_string(), vec![])
                .await
                .unwrap();
        }
        let hits = backend.search("same", 10).await.unwrap();
        let ids: Vec<&str> = hits.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, ["mem-001", "mem-002", "mem-003"]);
    }

    #[tokio::test]
    async fn add_rejects_empty_text() {
        let backend = deterministic();
        let err = backend.add(String::new(), vec![]).await.unwrap_err();
        assert!(matches!(err, MemError::InvalidArgs(_)));
    }

    #[tokio::test]
    async fn add_enforces_item_and_text_limits() {
        let backend = InMemoryMemoryBackend::deterministic().with_limits(MemLimits {
            max_items: 1,
            max_text_len: 8,
            max_query_len: 8,
            max_result_limit: 10,
        });
        backend.add("ok".to_string(), vec![]).await.unwrap();
        // Second item exceeds max_items == 1.
        let err = backend.add("second".to_string(), vec![]).await.unwrap_err();
        assert!(matches!(err, MemError::LimitExceeded(_)));
        // Over-long text exceeds max_text_len.
        let empty = InMemoryMemoryBackend::deterministic().with_limits(MemLimits {
            max_items: 10,
            max_text_len: 4,
            max_query_len: 4,
            max_result_limit: 10,
        });
        let err = empty
            .add("way too long".to_string(), vec![])
            .await
            .unwrap_err();
        assert!(matches!(err, MemError::LimitExceeded(_)));
    }

    #[tokio::test]
    async fn search_enforces_query_length_limit() {
        let backend = InMemoryMemoryBackend::deterministic().with_limits(MemLimits {
            max_items: 10,
            max_text_len: 100,
            max_query_len: 3,
            max_result_limit: 10,
        });
        let err = backend.search("too long", 10).await.unwrap_err();
        assert!(matches!(err, MemError::LimitExceeded(_)));
    }

    #[tokio::test]
    async fn complete_task_carries_related_id() {
        let backend = deterministic();
        let done = backend
            .complete_task("t1", Some("t2".to_string()))
            .await
            .unwrap();
        assert_eq!(done.task_id, "t1");
        assert_eq!(done.status, "completed");
        assert_eq!(done.related_task_id.as_deref(), Some("t2"));
        assert!(backend.complete_task("", None).await.is_err());
    }
}
