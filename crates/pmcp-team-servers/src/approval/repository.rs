//! The approval-record repository — the approval-DOMAIN state a `TaskStore`
//! cannot hold (question, option set, target role, verdict, and the optional
//! `subject_task_id`/`subject_ref` link, D-12).
//!
//! The task store observes only the pending→resolved LIFECYCLE (a `Working`
//! task that transitions to `Completed`); the answer to *what was asked* and
//! *what was decided* lives here.
//!
//! # Owner policy (D-10)
//!
//! Dev servers have no auth, so approvals are **SERVICE-OWNED**: the repository
//! is a single shared instance ([`Arc`](std::sync::Arc)), never scoped to the
//! client that created an approval. Any connected client may therefore resolve
//! any pending approval — there is one resolution path ([`resolve`](ApprovalRepository::resolve))
//! for both notification channels. The paired [`SERVICE_OWNER`] constant is the
//! fixed task-store owner the server uses so the observable task is likewise
//! not client-scoped.
//!
//! # Lifecycle
//!
//! `Pending → Resolved`. `Resolved` is terminal (no cancellation in dev).
//! [`resolve`](ApprovalRepository::resolve) is an ATOMIC FIRST-WRITER under a
//! mutex: the first valid decision wins and a second resolution is rejected
//! ([`ApprovalError::AlreadyResolved`]), never double-applied. A decision
//! outside the record's original option set is rejected
//! ([`ApprovalError::InvalidDecision`]).

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// The fixed task-store owner for every approval (D-10).
///
/// Because dev servers have no auth, every approval's observable task is minted
/// under this single service owner, so resolution is not scoped to the creating
/// client — any connected client may resolve.
pub const SERVICE_OWNER: &str = "approval-mcp-service";

/// The lifecycle state of an approval.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalStatus {
    /// Awaiting a human decision.
    Pending,
    /// A decision has been recorded; terminal.
    Resolved,
}

/// One approval record: the domain state of a single ask/resolve.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalRecord {
    /// The deterministic approval id (what `resolve_approval`/`get_approval` take).
    pub id: String,
    /// The human-facing question.
    pub question: String,
    /// The closed set of acceptable decisions.
    pub options: Vec<String>,
    /// The human role this approval targets.
    pub target_role: String,
    /// Optional linked task id (D-12), echoed verbatim.
    pub subject_task_id: Option<String>,
    /// Optional linked component/ref (D-12), echoed verbatim.
    pub subject_ref: Option<String>,
    /// The observable lifecycle handle on the `InMemoryTaskStore`.
    pub task_id: Option<String>,
    /// Current lifecycle state.
    pub status: ApprovalStatus,
    /// The recorded decision once resolved (always one of `options`).
    pub verdict: Option<String>,
}

/// The fields required to open a new approval (the id is minted by the repository).
#[derive(Debug, Clone)]
pub struct NewApproval {
    /// The human-facing question.
    pub question: String,
    /// The closed set of acceptable decisions.
    pub options: Vec<String>,
    /// The human role this approval targets.
    pub target_role: String,
    /// Optional linked task id (D-12).
    pub subject_task_id: Option<String>,
    /// Optional linked component/ref (D-12).
    pub subject_ref: Option<String>,
    /// The observable lifecycle task id minted on the task store.
    pub task_id: Option<String>,
}

/// Errors returned by [`ApprovalRepository`] operations.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ApprovalError {
    /// No approval with the given id exists.
    #[error("approval not found: {0}")]
    NotFound(String),
    /// The approval was already resolved; a second resolution is rejected.
    #[error("approval {id} already resolved with verdict '{verdict}'")]
    AlreadyResolved {
        /// The approval id.
        id: String,
        /// The verdict recorded by the first writer.
        verdict: String,
    },
    /// The decision is not a member of the approval's original option set.
    #[error("decision '{decision}' is not in the option set {options:?}")]
    InvalidDecision {
        /// The rejected decision.
        decision: String,
        /// The valid option set.
        options: Vec<String>,
    },
    /// The targeted human role is not part of the configured roster.
    #[error("unknown human role: {0}")]
    UnknownRole(String),
}

/// A source of approval ids. Object-safe so a deterministic sequence can be
/// injected for conformance while production uses random UUIDs.
pub trait ApprovalIdSource: Send + Sync {
    /// Mint the next approval id.
    fn next_id(&self) -> String;
}

/// Production id source: `appr-<uuid>` (globally unique, non-deterministic).
#[derive(Debug, Default)]
pub struct UuidApprovalIdSource;

impl ApprovalIdSource for UuidApprovalIdSource {
    fn next_id(&self) -> String {
        format!("appr-{}", uuid::Uuid::new_v4())
    }
}

/// Conformance/example id source: `appr-001`, `appr-002`, … (deterministic).
#[derive(Debug)]
pub struct SequentialApprovalIdSource {
    counter: AtomicU64,
}

impl Default for SequentialApprovalIdSource {
    fn default() -> Self {
        Self {
            counter: AtomicU64::new(1),
        }
    }
}

impl ApprovalIdSource for SequentialApprovalIdSource {
    fn next_id(&self) -> String {
        let n = self.counter.fetch_add(1, Ordering::Relaxed);
        format!("appr-{n:03}")
    }
}

/// The shared, service-owned approval-domain store.
///
/// Backed by a [`parking_lot::Mutex`] over an id→record map plus an injectable
/// [`ApprovalIdSource`]. The mutex is what makes [`resolve`](Self::resolve) an
/// atomic first-writer.
pub struct ApprovalRepository {
    inner: Mutex<HashMap<String, ApprovalRecord>>,
    id_source: Arc<dyn ApprovalIdSource>,
}

impl std::fmt::Debug for ApprovalRepository {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApprovalRepository")
            .field("len", &self.inner.lock().len())
            .finish_non_exhaustive()
    }
}

impl Default for ApprovalRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl ApprovalRepository {
    /// Production repository using the random-UUID id seam.
    #[must_use]
    pub fn new() -> Self {
        Self::with_id_source(Arc::new(UuidApprovalIdSource))
    }

    /// Conformance repository using the deterministic `appr-001…` id seam.
    #[must_use]
    pub fn deterministic() -> Self {
        Self::with_id_source(Arc::new(SequentialApprovalIdSource::default()))
    }

    /// Repository with a caller-supplied id seam.
    #[must_use]
    pub fn with_id_source(id_source: Arc<dyn ApprovalIdSource>) -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
            id_source,
        }
    }

    /// Open a new `Pending` approval and return the stored record.
    pub fn create(&self, new: NewApproval) -> ApprovalRecord {
        let id = self.id_source.next_id();
        let record = ApprovalRecord {
            id: id.clone(),
            question: new.question,
            options: new.options,
            target_role: new.target_role,
            subject_task_id: new.subject_task_id,
            subject_ref: new.subject_ref,
            task_id: new.task_id,
            status: ApprovalStatus::Pending,
            verdict: None,
        };
        self.inner.lock().insert(id, record.clone());
        record
    }

    /// Fetch an approval record by id.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<ApprovalRecord> {
        self.inner.lock().get(id).cloned()
    }

    /// Atomically resolve an approval with `decision`.
    ///
    /// First-writer wins: the whole check-and-set runs under the mutex, so a
    /// concurrent second call observes `Resolved` and is rejected.
    ///
    /// # Errors
    ///
    /// - [`ApprovalError::NotFound`] if no approval has this id.
    /// - [`ApprovalError::AlreadyResolved`] if it was already resolved (the
    ///   first writer's verdict is returned in the error, never overwritten).
    /// - [`ApprovalError::InvalidDecision`] if `decision` is outside the record's
    ///   original option set.
    pub fn resolve(&self, id: &str, decision: &str) -> Result<ApprovalRecord, ApprovalError> {
        let mut map = self.inner.lock();
        let record = map
            .get_mut(id)
            .ok_or_else(|| ApprovalError::NotFound(id.to_string()))?;

        if record.status == ApprovalStatus::Resolved {
            return Err(ApprovalError::AlreadyResolved {
                id: id.to_string(),
                verdict: record.verdict.clone().unwrap_or_default(),
            });
        }
        if !record.options.iter().any(|o| o == decision) {
            return Err(ApprovalError::InvalidDecision {
                decision: decision.to_string(),
                options: record.options.clone(),
            });
        }

        record.status = ApprovalStatus::Resolved;
        record.verdict = Some(decision.to_string());
        Ok(record.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_approval() -> NewApproval {
        NewApproval {
            question: "Ship v2.4?".to_string(),
            options: vec!["approve".to_string(), "reject".to_string()],
            target_role: "release-manager".to_string(),
            subject_task_id: Some("task-42".to_string()),
            subject_ref: Some("agent://triage@1".to_string()),
            task_id: Some("store-task-1".to_string()),
        }
    }

    #[test]
    fn deterministic_ids_start_at_001() {
        let repo = ApprovalRepository::deterministic();
        let a = repo.create(new_approval());
        let b = repo.create(new_approval());
        assert_eq!(a.id, "appr-001");
        assert_eq!(b.id, "appr-002");
    }

    #[test]
    fn create_is_pending_and_echoes_subject_refs() {
        let repo = ApprovalRepository::deterministic();
        let rec = repo.create(new_approval());
        assert_eq!(rec.status, ApprovalStatus::Pending);
        assert_eq!(rec.verdict, None);
        assert_eq!(rec.subject_task_id.as_deref(), Some("task-42"));
        assert_eq!(rec.subject_ref.as_deref(), Some("agent://triage@1"));
        let got = repo.get(&rec.id).expect("stored");
        assert_eq!(got, rec);
    }

    #[test]
    fn resolve_sets_verdict_and_marks_resolved() {
        let repo = ApprovalRepository::deterministic();
        let rec = repo.create(new_approval());
        let resolved = repo.resolve(&rec.id, "approve").expect("resolve");
        assert_eq!(resolved.status, ApprovalStatus::Resolved);
        assert_eq!(resolved.verdict.as_deref(), Some("approve"));
        // Subject refs still echoed after resolution (D-12).
        assert_eq!(resolved.subject_task_id.as_deref(), Some("task-42"));
    }

    #[test]
    fn double_resolve_is_rejected_first_writer_wins() {
        let repo = ApprovalRepository::deterministic();
        let rec = repo.create(new_approval());
        repo.resolve(&rec.id, "approve").expect("first resolve");
        let err = repo
            .resolve(&rec.id, "reject")
            .expect_err("second must reject");
        match err {
            ApprovalError::AlreadyResolved { verdict, .. } => assert_eq!(verdict, "approve"),
            other => panic!("expected AlreadyResolved, got {other:?}"),
        }
        // The first writer's verdict is intact.
        assert_eq!(
            repo.get(&rec.id).unwrap().verdict.as_deref(),
            Some("approve")
        );
    }

    #[test]
    fn out_of_set_decision_errors() {
        let repo = ApprovalRepository::deterministic();
        let rec = repo.create(new_approval());
        let err = repo.resolve(&rec.id, "maybe").expect_err("out-of-set");
        assert!(matches!(err, ApprovalError::InvalidDecision { .. }));
        // Still pending — an invalid decision never resolves.
        assert_eq!(repo.get(&rec.id).unwrap().status, ApprovalStatus::Pending);
    }

    #[test]
    fn resolve_unknown_id_is_not_found() {
        let repo = ApprovalRepository::deterministic();
        let err = repo.resolve("appr-999", "approve").expect_err("missing");
        assert!(matches!(err, ApprovalError::NotFound(_)));
    }
}
