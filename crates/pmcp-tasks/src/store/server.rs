//! The store a pmcp server's `task_store(...)` takes, over any [`StorageBackend`].
//!
//! [`ServerTaskStore`] implements `pmcp::server::task_store::TaskStore`, the
//! trait `ServerBuilder::task_store(...)` and `ServerCoreBuilder::task_store(...)`
//! accept. It wraps a [`GenericTaskStore`], so every backend in this crate
//! (in-memory, `DynamoDbBackend`, `RedisBackend`) plugs into the recommended
//! task path, and task state survives a restart and is shared between
//! instances when the backend is.
//!
//! ```rust,no_run
//! use std::sync::Arc;
//! use pmcp::server::task_store::TaskStore;
//! use pmcp_tasks::{GenericTaskStore, InMemoryBackend, ServerTaskStore};
//!
//! # fn build() -> pmcp::Result<()> {
//! // Swap `InMemoryBackend` for `DynamoDbBackend` or `RedisBackend` in production.
//! let store = ServerTaskStore::new(GenericTaskStore::new(InMemoryBackend::new()));
//! let server = pmcp::Server::builder()
//!     .name("tasks")
//!     .version("1.0.0")
//!     .task_store(Arc::new(store) as Arc<dyn TaskStore>)
//!     .build()?;
//! # Ok(())
//! # }
//! ```
//!
//! # Anonymous owners are refused by default
//!
//! An unauthenticated request reaches the store with the owner `"local"` (v1)
//! or `""` (v2), and every such client shares that owner. The wrapped store's
//! [`TaskSecurityConfig`](crate::security::TaskSecurityConfig) refuses task
//! creation for those owners unless `allow_anonymous` is set. Configure an auth
//! provider so each caller has its own owner, or opt in explicitly:
//!
//! ```rust
//! use pmcp_tasks::{GenericTaskStore, InMemoryBackend, ServerTaskStore, TaskSecurityConfig};
//!
//! let store = ServerTaskStore::new(
//!     GenericTaskStore::new(InMemoryBackend::new())
//!         .with_security(TaskSecurityConfig::default().with_allow_anonymous(true)),
//! );
//! # let _ = store;
//! ```
//!
//! # Matching pmcp's in-memory store
//!
//! The adapter gives the semantics of `pmcp::server::task_store::InMemoryTaskStore`
//! wherever this crate's store differs:
//!
//! - A requested TTL above `max_ttl_ms` is clamped to it, not refused.
//! - Reading an expired task returns `Expired`, and `list` leaves expired tasks
//!   out.
//! - `get_result` returns a stored result whatever the task's status.
//! - `list` pages hold 20 tasks.
//!
//! Every mutation is a single compare-and-set on the backend. Two writes that
//! race on one task (a worker's progress update against a client's cancel, say)
//! leave one of them failing with an `Internal` error that names the conflict;
//! nothing is lost silently.

use async_trait::async_trait;
use pmcp::server::task_store::{
    StoreConfig as SdkStoreConfig, TaskInputDelivery, TaskInputSnapshot, TaskStore as SdkTaskStore,
    TaskStoreError,
};
use pmcp::types::mrtr::{InputRequests, InputResponses};
use pmcp::types::tasks::{Task as SdkTask, TaskStatus as SdkTaskStatus};
use pmcp::types::CallToolResult;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::json;

use crate::domain::TaskRecord;
use crate::error::TaskError;
use crate::store::backend::StorageBackend;
use crate::store::generic::GenericTaskStore;
use crate::store::ListTasksOptions;
use crate::types::task::TaskStatus;

/// The request method recorded on tasks this adapter creates: on the
/// `task_store(...)` path only `tools/call` creates tasks.
const CREATING_METHOD: &str = "tools/call";

/// Tasks per `list` page, matching pmcp's in-memory store.
const PAGE_SIZE: usize = 20;

/// A [`GenericTaskStore`] that a pmcp server's `task_store(...)` accepts.
///
/// See the [module documentation](self) for how it maps onto pmcp's trait.
#[derive(Debug)]
pub struct ServerTaskStore<B: StorageBackend> {
    inner: GenericTaskStore<B>,
    config: SdkStoreConfig,
}

impl<B: StorageBackend> ServerTaskStore<B> {
    /// Wraps `inner`, taking the TTL, poll-interval and per-owner limits it
    /// enforces as the configuration pmcp reads.
    pub fn new(inner: GenericTaskStore<B>) -> Self {
        let config = SdkStoreConfig {
            default_ttl_ms: inner.config().default_ttl_ms,
            max_ttl_ms: inner.config().max_ttl_ms,
            default_poll_interval_ms: inner.poll_interval_ms(),
            max_tasks_per_owner: inner.security().max_tasks_per_owner,
        };
        Self { inner, config }
    }

    /// Returns the wrapped store, for the operations pmcp's trait doesn't
    /// have, such as task variables.
    pub fn inner(&self) -> &GenericTaskStore<B> {
        &self.inner
    }

    /// Reads a task that exists, belongs to `owner_id` and has not expired.
    async fn live_record(
        &self,
        task_id: &str,
        owner_id: &str,
    ) -> Result<TaskRecord, TaskStoreError> {
        let record = self
            .inner
            .get(task_id, owner_id)
            .await
            .map_err(store_error)?;
        if record.is_expired() {
            return Err(TaskStoreError::Expired {
                task_id: task_id.to_string(),
            });
        }
        Ok(record)
    }
}

impl<B: StorageBackend> From<GenericTaskStore<B>> for ServerTaskStore<B> {
    fn from(inner: GenericTaskStore<B>) -> Self {
        Self::new(inner)
    }
}

#[async_trait]
impl<B: StorageBackend + 'static> SdkTaskStore for ServerTaskStore<B> {
    async fn create(&self, owner_id: &str, ttl: Option<u64>) -> Result<SdkTask, TaskStoreError> {
        let ttl = match (ttl, self.config.max_ttl_ms) {
            (Some(requested), Some(max)) if requested > max => Some(max),
            (requested, _) => requested,
        };
        let record = self
            .inner
            .create(owner_id, CREATING_METHOD, ttl)
            .await
            .map_err(store_error)?;
        wire_task(&record)
    }

    async fn get(&self, task_id: &str, owner_id: &str) -> Result<SdkTask, TaskStoreError> {
        wire_task(&self.live_record(task_id, owner_id).await?)
    }

    async fn update_status(
        &self,
        task_id: &str,
        owner_id: &str,
        status: SdkTaskStatus,
        message: Option<String>,
    ) -> Result<SdkTask, TaskStoreError> {
        let record = self
            .inner
            .update_status(task_id, owner_id, crate_status(status), message)
            .await
            .map_err(store_error)?;
        wire_task(&record)
    }

    async fn list(
        &self,
        owner_id: &str,
        cursor: Option<&str>,
    ) -> Result<(Vec<SdkTask>, Option<String>), TaskStoreError> {
        let page = self
            .inner
            .list(ListTasksOptions {
                owner_id: owner_id.to_string(),
                cursor: cursor.map(str::to_string),
                limit: Some(PAGE_SIZE),
            })
            .await
            .map_err(store_error)?;
        let tasks = page
            .tasks
            .iter()
            .filter(|record| !record.is_expired())
            .map(wire_task)
            .collect::<Result<_, _>>()?;
        Ok((tasks, page.next_cursor))
    }

    async fn cancel(&self, task_id: &str, owner_id: &str) -> Result<SdkTask, TaskStoreError> {
        let record = self
            .inner
            .cancel(task_id, owner_id)
            .await
            .map_err(store_error)?;
        wire_task(&record)
    }

    async fn cleanup_expired(&self) -> Result<usize, TaskStoreError> {
        self.inner.cleanup_expired().await.map_err(store_error)
    }

    fn config(&self) -> &SdkStoreConfig {
        &self.config
    }

    async fn set_result(
        &self,
        task_id: &str,
        owner_id: &str,
        result: CallToolResult,
    ) -> Result<(), TaskStoreError> {
        let result = convert(&result, "tool result")?;
        self.inner
            .set_result(task_id, owner_id, result)
            .await
            .map_err(store_error)
    }

    async fn get_result(
        &self,
        task_id: &str,
        owner_id: &str,
    ) -> Result<CallToolResult, TaskStoreError> {
        let record = self.live_record(task_id, owner_id).await?;
        let result = record.result.ok_or_else(|| TaskStoreError::NotFound {
            task_id: task_id.to_string(),
        })?;
        convert(&result, "tool result")
    }

    fn supports_results(&self) -> bool {
        true
    }

    async fn deliver_task_inputs(
        &self,
        task_id: &str,
        owner_id: &str,
        responses: InputResponses,
    ) -> Result<TaskInputDelivery, TaskStoreError> {
        let responses = convert(&responses, "input responses")?;
        let delivery = self
            .inner
            .deliver_inputs(task_id, owner_id, responses)
            .await
            .map_err(store_error)?;
        convert(&delivery, "input delivery")
    }

    async fn task_input_snapshot(
        &self,
        task_id: &str,
        owner_id: &str,
    ) -> Result<TaskInputSnapshot, TaskStoreError> {
        let record = self.live_record(task_id, owner_id).await?;
        // No recorded requests is "the server never asked", which is not an
        // empty snapshot.
        let requests = record
            .input_requests
            .ok_or_else(|| TaskStoreError::NotFound {
                task_id: task_id.to_string(),
            })?;
        let snapshot = json!({
            "inputRequests": requests,
            "inputResponses": record.input_responses.unwrap_or_default(),
            "status": record.task.status,
        });
        convert(&snapshot, "input snapshot")
    }

    async fn record_input_requests(
        &self,
        task_id: &str,
        owner_id: &str,
        requests: InputRequests,
    ) -> Result<SdkTask, TaskStoreError> {
        let requests = convert(&requests, "input requests")?;
        let task = self
            .inner
            .record_input_requests(task_id, owner_id, requests)
            .await
            .map_err(store_error)?;
        convert(&task, "task")
    }

    async fn set_error(
        &self,
        task_id: &str,
        owner_id: &str,
        error: serde_json::Value,
    ) -> Result<(), TaskStoreError> {
        self.inner
            .set_error(task_id, owner_id, error)
            .await
            .map_err(store_error)
    }

    async fn get_error(
        &self,
        task_id: &str,
        owner_id: &str,
    ) -> Result<serde_json::Value, TaskStoreError> {
        let record = self.live_record(task_id, owner_id).await?;
        record.error.ok_or_else(|| TaskStoreError::NotFound {
            task_id: task_id.to_string(),
        })
    }

    fn supports_inputs(&self) -> bool {
        true
    }

    async fn set_status_message(
        &self,
        task_id: &str,
        owner_id: &str,
        message: Option<String>,
    ) -> Result<SdkTask, TaskStoreError> {
        let record = self
            .inner
            .set_status_message(task_id, owner_id, message)
            .await
            .map_err(store_error)?;
        wire_task(&record)
    }
}

/// Re-reads a value as another type with the same wire shape. The two crates
/// define their task, result and input types separately but serialize them
/// identically, so a mismatch here is a bug, reported as `Internal`.
fn convert<T: Serialize, U: DeserializeOwned>(value: &T, what: &str) -> Result<U, TaskStoreError> {
    serde_json::to_value(value)
        .and_then(serde_json::from_value)
        .map_err(|e| TaskStoreError::Internal {
            message: format!("failed to convert {what}: {e}"),
        })
}

fn wire_task(record: &TaskRecord) -> Result<SdkTask, TaskStoreError> {
    convert(&record.to_wire_task(), "task")
}

fn crate_status(status: SdkTaskStatus) -> TaskStatus {
    match status {
        SdkTaskStatus::Working => TaskStatus::Working,
        SdkTaskStatus::InputRequired => TaskStatus::InputRequired,
        SdkTaskStatus::Completed => TaskStatus::Completed,
        SdkTaskStatus::Failed => TaskStatus::Failed,
        SdkTaskStatus::Cancelled => TaskStatus::Cancelled,
    }
}

fn sdk_status(status: TaskStatus) -> SdkTaskStatus {
    match status {
        TaskStatus::Working => SdkTaskStatus::Working,
        TaskStatus::InputRequired => SdkTaskStatus::InputRequired,
        TaskStatus::Completed => SdkTaskStatus::Completed,
        TaskStatus::Failed => SdkTaskStatus::Failed,
        TaskStatus::Cancelled => SdkTaskStatus::Cancelled,
    }
}

/// Maps this crate's errors onto pmcp's four. The ones pmcp's trait has no
/// variant for (limits, conflicts, backend failures) become `Internal` with
/// the original message.
fn store_error(err: TaskError) -> TaskStoreError {
    match err {
        TaskError::NotFound { task_id } | TaskError::OwnerMismatch { task_id } => {
            TaskStoreError::NotFound { task_id }
        },
        TaskError::Expired { task_id, .. } => TaskStoreError::Expired { task_id },
        TaskError::InvalidTransition {
            task_id, from, to, ..
        } => TaskStoreError::InvalidTransition {
            task_id,
            from: sdk_status(from),
            to: sdk_status(to),
        },
        other => TaskStoreError::Internal {
            message: other.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [TaskStatus; 5] = [
        TaskStatus::Working,
        TaskStatus::InputRequired,
        TaskStatus::Completed,
        TaskStatus::Failed,
        TaskStatus::Cancelled,
    ];

    #[test]
    fn status_mapping_round_trips_and_keeps_the_wire_name() {
        for status in ALL {
            let sdk = sdk_status(status);
            assert_eq!(crate_status(sdk), status);
            assert_eq!(
                serde_json::to_value(status).unwrap(),
                serde_json::to_value(sdk).unwrap()
            );
        }
    }

    #[test]
    fn transition_rules_agree_with_pmcp() {
        for from in ALL {
            for to in ALL {
                assert_eq!(
                    from.can_transition_to(&to),
                    sdk_status(from).can_transition_to(&sdk_status(to)),
                    "{from} -> {to}"
                );
            }
        }
    }

    #[test]
    fn errors_map_to_the_matching_variant() {
        let not_found = store_error(TaskError::OwnerMismatch {
            task_id: "t".into(),
        });
        assert!(matches!(not_found, TaskStoreError::NotFound { task_id } if task_id == "t"));

        let expired = store_error(TaskError::Expired {
            task_id: "t".into(),
            expired_at: None,
        });
        assert!(matches!(expired, TaskStoreError::Expired { .. }));

        let transition = store_error(TaskError::InvalidTransition {
            task_id: "t".into(),
            from: TaskStatus::Completed,
            to: TaskStatus::Working,
            suggested_action: None,
        });
        assert!(matches!(
            transition,
            TaskStoreError::InvalidTransition {
                from: SdkTaskStatus::Completed,
                to: SdkTaskStatus::Working,
                ..
            }
        ));

        let conflict = store_error(TaskError::ConcurrentModification {
            task_id: "t".into(),
            expected_version: 1,
            actual_version: 2,
        });
        assert!(
            matches!(&conflict, TaskStoreError::Internal { message } if message.contains('t')),
            "{conflict}"
        );
    }
}
