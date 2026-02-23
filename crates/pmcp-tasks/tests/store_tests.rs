//! Integration tests for InMemoryTaskStore.
//!
//! Tests cover CRUD operations, cursor-based pagination, TTL enforcement,
//! and concurrent access patterns. Organized into module blocks per concern.

use std::collections::HashMap;
use std::sync::Arc;

use pmcp_tasks::security::TaskSecurityConfig;
use pmcp_tasks::store::memory::InMemoryTaskStore;
use pmcp_tasks::store::{ListTasksOptions, StoreConfig, TaskStore};
use pmcp_tasks::types::task::TaskStatus;
use pmcp_tasks::TaskError;
use serde_json::{json, Value};

/// Creates a store with anonymous access enabled for test convenience.
fn test_store() -> InMemoryTaskStore {
    InMemoryTaskStore::new().with_security(TaskSecurityConfig::default().with_allow_anonymous(true))
}

// ─── CRUD Tests ─────────────────────────────────────────────────────────────

mod crud_tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn test_create_returns_working_task() {
        let store = test_store();
        let record = store
            .create("test-owner", "tools/call", None)
            .await
            .unwrap();
        assert_eq!(record.task.status, TaskStatus::Working);
        assert_eq!(record.owner_id, "test-owner");
        assert!(!record.task.task_id.is_empty());
    }

    #[tokio::test]
    async fn test_create_assigns_uuid_v4_task_id() {
        let store = test_store();
        let record = store
            .create("test-owner", "tools/call", None)
            .await
            .unwrap();
        // UUID v4 format: 8-4-4-4-12 hex chars = 36 total with hyphens
        assert_eq!(record.task.task_id.len(), 36);
        assert!(record.task.task_id.contains('-'));
        // Verify it parses as a valid UUID
        let parsed = uuid::Uuid::parse_str(&record.task.task_id).unwrap();
        assert_eq!(parsed.get_version_num(), 4);
    }

    #[tokio::test]
    async fn test_create_sets_poll_interval() {
        let store = test_store();
        let record = store
            .create("test-owner", "tools/call", None)
            .await
            .unwrap();
        // Default poll interval is 5000ms
        assert_eq!(record.task.poll_interval, Some(5000));

        // Custom poll interval
        let store = test_store().with_poll_interval(3000);
        let record = store
            .create("test-owner", "tools/call", None)
            .await
            .unwrap();
        assert_eq!(record.task.poll_interval, Some(3000));
    }

    #[tokio::test]
    async fn test_get_returns_created_task() {
        let store = test_store();
        let created = store
            .create("test-owner", "tools/call", None)
            .await
            .unwrap();
        let fetched = store
            .get(&created.task.task_id, "test-owner")
            .await
            .unwrap();
        assert_eq!(fetched.task.task_id, created.task.task_id);
        assert_eq!(fetched.owner_id, "test-owner");
        assert_eq!(fetched.task.status, TaskStatus::Working);
    }

    #[tokio::test]
    async fn test_get_nonexistent_returns_not_found() {
        let store = test_store();
        let result = store.get("nonexistent-task-id", "test-owner").await;
        assert!(matches!(result, Err(TaskError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_update_status_working_to_completed() {
        let store = test_store();
        let created = store
            .create("test-owner", "tools/call", None)
            .await
            .unwrap();
        let original_updated_at = created.task.last_updated_at.clone();

        // Small delay to ensure timestamp changes
        tokio::time::sleep(std::time::Duration::from_millis(2)).await;

        let updated = store
            .update_status(
                &created.task.task_id,
                "test-owner",
                TaskStatus::Completed,
                None,
            )
            .await
            .unwrap();
        assert_eq!(updated.task.status, TaskStatus::Completed);
        assert_ne!(updated.task.last_updated_at, original_updated_at);
    }

    #[tokio::test]
    async fn test_update_status_with_message() {
        let store = test_store();
        let created = store
            .create("test-owner", "tools/call", None)
            .await
            .unwrap();
        let updated = store
            .update_status(
                &created.task.task_id,
                "test-owner",
                TaskStatus::Completed,
                Some("Processing complete".to_string()),
            )
            .await
            .unwrap();
        assert_eq!(
            updated.task.status_message.as_deref(),
            Some("Processing complete")
        );
    }

    #[tokio::test]
    async fn test_update_status_invalid_transition_rejected() {
        let store = test_store();
        let created = store
            .create("test-owner", "tools/call", None)
            .await
            .unwrap();
        // Complete the task
        store
            .update_status(
                &created.task.task_id,
                "test-owner",
                TaskStatus::Completed,
                None,
            )
            .await
            .unwrap();
        // Try to transition from terminal state back to Working
        let result = store
            .update_status(
                &created.task.task_id,
                "test-owner",
                TaskStatus::Working,
                None,
            )
            .await;
        assert!(matches!(result, Err(TaskError::InvalidTransition { .. })));
    }

    #[tokio::test]
    async fn test_set_variables_stores_values() {
        let store = test_store();
        let created = store
            .create("test-owner", "tools/call", None)
            .await
            .unwrap();

        let mut vars = HashMap::new();
        vars.insert("key1".to_string(), json!("value1"));
        vars.insert("key2".to_string(), json!(42));

        let updated = store
            .set_variables(&created.task.task_id, "test-owner", vars)
            .await
            .unwrap();
        assert_eq!(updated.variables.get("key1").unwrap(), &json!("value1"));
        assert_eq!(updated.variables.get("key2").unwrap(), &json!(42));
    }

    #[tokio::test]
    async fn test_set_variables_null_deletes_key() {
        let store = test_store();
        let created = store
            .create("test-owner", "tools/call", None)
            .await
            .unwrap();

        // Set initial variable
        let mut vars = HashMap::new();
        vars.insert("to_delete".to_string(), json!("value"));
        store
            .set_variables(&created.task.task_id, "test-owner", vars)
            .await
            .unwrap();

        // Delete via null
        let mut vars = HashMap::new();
        vars.insert("to_delete".to_string(), Value::Null);
        let updated = store
            .set_variables(&created.task.task_id, "test-owner", vars)
            .await
            .unwrap();
        assert!(!updated.variables.contains_key("to_delete"));
    }

    #[tokio::test]
    async fn test_set_variables_merges_with_existing() {
        let store = test_store();
        let created = store
            .create("test-owner", "tools/call", None)
            .await
            .unwrap();

        // Set first batch
        let mut vars = HashMap::new();
        vars.insert("a".to_string(), json!(1));
        store
            .set_variables(&created.task.task_id, "test-owner", vars)
            .await
            .unwrap();

        // Set second batch (should merge, not replace)
        let mut vars = HashMap::new();
        vars.insert("b".to_string(), json!(2));
        let updated = store
            .set_variables(&created.task.task_id, "test-owner", vars)
            .await
            .unwrap();

        assert_eq!(updated.variables.get("a").unwrap(), &json!(1));
        assert_eq!(updated.variables.get("b").unwrap(), &json!(2));
    }

    #[tokio::test]
    async fn test_set_result_and_get_result() {
        let store = test_store();
        let created = store
            .create("test-owner", "tools/call", None)
            .await
            .unwrap();

        // Complete with result atomically
        store
            .complete_with_result(
                &created.task.task_id,
                "test-owner",
                TaskStatus::Completed,
                None,
                json!({"answer": 42}),
            )
            .await
            .unwrap();

        let result = store
            .get_result(&created.task.task_id, "test-owner")
            .await
            .unwrap();
        assert_eq!(result, json!({"answer": 42}));
    }

    #[tokio::test]
    async fn test_get_result_not_ready_for_working_task() {
        let store = test_store();
        let created = store
            .create("test-owner", "tools/call", None)
            .await
            .unwrap();
        let result = store.get_result(&created.task.task_id, "test-owner").await;
        assert!(matches!(result, Err(TaskError::NotReady { .. })));
    }

    #[tokio::test]
    async fn test_complete_with_result_atomic() {
        let store = test_store();
        let created = store
            .create("test-owner", "tools/call", None)
            .await
            .unwrap();

        let completed = store
            .complete_with_result(
                &created.task.task_id,
                "test-owner",
                TaskStatus::Completed,
                Some("All done".to_string()),
                json!({"data": true}),
            )
            .await
            .unwrap();

        // Verify status and result are both set
        assert_eq!(completed.task.status, TaskStatus::Completed);
        assert_eq!(completed.task.status_message.as_deref(), Some("All done"));
        assert_eq!(completed.result, Some(json!({"data": true})));

        // Also verify through get + get_result
        let fetched = store
            .get(&created.task.task_id, "test-owner")
            .await
            .unwrap();
        assert_eq!(fetched.task.status, TaskStatus::Completed);

        let result = store
            .get_result(&created.task.task_id, "test-owner")
            .await
            .unwrap();
        assert_eq!(result, json!({"data": true}));
    }

    #[tokio::test]
    async fn test_cancel_transitions_to_cancelled() {
        let store = test_store();
        let created = store
            .create("test-owner", "tools/call", None)
            .await
            .unwrap();
        let cancelled = store
            .cancel(&created.task.task_id, "test-owner")
            .await
            .unwrap();
        assert_eq!(cancelled.task.status, TaskStatus::Cancelled);
    }

    #[tokio::test]
    async fn test_cancel_terminal_task_rejected() {
        let store = test_store();
        let created = store
            .create("test-owner", "tools/call", None)
            .await
            .unwrap();
        // Complete first
        store
            .update_status(
                &created.task.task_id,
                "test-owner",
                TaskStatus::Completed,
                None,
            )
            .await
            .unwrap();
        // Cancel should be rejected (Completed is terminal)
        let result = store.cancel(&created.task.task_id, "test-owner").await;
        assert!(matches!(result, Err(TaskError::InvalidTransition { .. })));
    }
}

// ─── Pagination Tests ───────────────────────────────────────────────────────

mod pagination_tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn test_list_returns_owner_tasks_only() {
        let store = test_store();
        store.create("owner-a", "tools/call", None).await.unwrap();
        store.create("owner-a", "tools/call", None).await.unwrap();
        store.create("owner-b", "tools/call", None).await.unwrap();

        let page = store
            .list(ListTasksOptions {
                owner_id: "owner-a".to_string(),
                cursor: None,
                limit: None,
            })
            .await
            .unwrap();
        assert_eq!(page.tasks.len(), 2);
        assert!(page.tasks.iter().all(|t| t.owner_id == "owner-a"));
    }

    #[tokio::test]
    async fn test_list_empty_for_new_owner() {
        let store = test_store();
        store.create("owner-a", "tools/call", None).await.unwrap();

        let page = store
            .list(ListTasksOptions {
                owner_id: "nobody".to_string(),
                cursor: None,
                limit: None,
            })
            .await
            .unwrap();
        assert!(page.tasks.is_empty());
        assert!(page.next_cursor.is_none());
    }

    #[tokio::test]
    async fn test_list_with_limit() {
        let store = test_store();
        for _ in 0..5 {
            store
                .create("test-owner", "tools/call", None)
                .await
                .unwrap();
        }

        let page = store
            .list(ListTasksOptions {
                owner_id: "test-owner".to_string(),
                cursor: None,
                limit: Some(2),
            })
            .await
            .unwrap();
        assert_eq!(page.tasks.len(), 2);
        assert!(page.next_cursor.is_some());
    }

    #[tokio::test]
    async fn test_list_pagination_with_cursor() {
        let store = test_store();
        for _ in 0..5 {
            store
                .create("test-owner", "tools/call", None)
                .await
                .unwrap();
            // Small delay for ordering
            tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        }

        // Page 1
        let page1 = store
            .list(ListTasksOptions {
                owner_id: "test-owner".to_string(),
                cursor: None,
                limit: Some(2),
            })
            .await
            .unwrap();
        assert_eq!(page1.tasks.len(), 2);
        assert!(page1.next_cursor.is_some());

        // Page 2
        let page2 = store
            .list(ListTasksOptions {
                owner_id: "test-owner".to_string(),
                cursor: page1.next_cursor.clone(),
                limit: Some(2),
            })
            .await
            .unwrap();
        assert_eq!(page2.tasks.len(), 2);
        assert!(page2.next_cursor.is_some());

        // No overlap between pages
        let page1_ids: Vec<&str> = page1
            .tasks
            .iter()
            .map(|t| t.task.task_id.as_str())
            .collect();
        let page2_ids: Vec<&str> = page2
            .tasks
            .iter()
            .map(|t| t.task.task_id.as_str())
            .collect();
        for id in &page1_ids {
            assert!(
                !page2_ids.contains(id),
                "task {id} appears in both page 1 and page 2"
            );
        }

        // Page 3 (last)
        let page3 = store
            .list(ListTasksOptions {
                owner_id: "test-owner".to_string(),
                cursor: page2.next_cursor.clone(),
                limit: Some(2),
            })
            .await
            .unwrap();
        assert_eq!(page3.tasks.len(), 1);
        assert!(page3.next_cursor.is_none());
    }

    #[tokio::test]
    async fn test_list_sorted_newest_first() {
        let store = test_store();
        let first = store
            .create("test-owner", "tools/call", None)
            .await
            .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        let second = store
            .create("test-owner", "tools/call", None)
            .await
            .unwrap();

        let page = store
            .list(ListTasksOptions {
                owner_id: "test-owner".to_string(),
                cursor: None,
                limit: None,
            })
            .await
            .unwrap();

        assert_eq!(page.tasks.len(), 2);
        // Newest first: second task should be first in list
        assert_eq!(page.tasks[0].task.task_id, second.task.task_id);
        assert_eq!(page.tasks[1].task.task_id, first.task.task_id);
    }

    #[tokio::test]
    async fn test_list_default_limit() {
        let store = test_store();
        // Create 3 tasks (less than default limit of 50)
        for _ in 0..3 {
            store
                .create("test-owner", "tools/call", None)
                .await
                .unwrap();
        }

        let page = store
            .list(ListTasksOptions {
                owner_id: "test-owner".to_string(),
                cursor: None,
                limit: None, // No explicit limit -- should use default (50)
            })
            .await
            .unwrap();
        // All 3 should be returned (well under the default limit)
        assert_eq!(page.tasks.len(), 3);
        assert!(page.next_cursor.is_none());
    }
}

// ─── TTL Tests ──────────────────────────────────────────────────────────────

mod ttl_tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn test_create_with_default_ttl() {
        let store = test_store();
        let record = store
            .create("test-owner", "tools/call", None)
            .await
            .unwrap();
        // Default TTL from StoreConfig is 3_600_000 (1 hour)
        assert_eq!(record.task.ttl, Some(3_600_000));
        assert!(record.expires_at.is_some());
    }

    #[tokio::test]
    async fn test_create_with_explicit_ttl() {
        let store = test_store();
        let record = store
            .create("test-owner", "tools/call", Some(30_000))
            .await
            .unwrap();
        assert_eq!(record.task.ttl, Some(30_000));
        assert!(record.expires_at.is_some());
    }

    #[tokio::test]
    async fn test_create_rejects_ttl_above_max() {
        let store = test_store();
        // Default max TTL is 86_400_000 (24 hours)
        let result = store
            .create("test-owner", "tools/call", Some(100_000_000))
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("TTL"),
            "error should mention TTL: {err}"
        );
    }

    #[tokio::test]
    async fn test_expired_task_readable_via_get() {
        // From integration tests we cannot force expiry by touching private fields,
        // so we use a 1ms TTL and sleep past it, then verify the task is still readable.
        let store = InMemoryTaskStore::new()
            .with_config(StoreConfig::default())
            .with_security(TaskSecurityConfig::default().with_allow_anonymous(true));

        let record = store
            .create("test-owner", "tools/call", Some(1))
            .await
            .unwrap();

        // Sleep past the TTL
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        // Expired task should still be readable via get() (per locked decision)
        let fetched = store.get(&record.task.task_id, "test-owner").await.unwrap();
        assert_eq!(fetched.task.task_id, record.task.task_id);
        assert!(fetched.is_expired());
    }

    #[tokio::test]
    async fn test_expired_task_rejects_update_status() {
        // Use a 1ms TTL and sleep past it
        let store = test_store();
        let created = store
            .create("test-owner", "tools/call", Some(1))
            .await
            .unwrap();

        // Sleep to ensure expiry
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        let result = store
            .update_status(
                &created.task.task_id,
                "test-owner",
                TaskStatus::Completed,
                None,
            )
            .await;
        assert!(matches!(result, Err(TaskError::Expired { .. })));
    }

    #[tokio::test]
    async fn test_expired_task_rejects_set_variables() {
        let store = test_store();
        let created = store
            .create("test-owner", "tools/call", Some(1))
            .await
            .unwrap();

        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        let mut vars = HashMap::new();
        vars.insert("key".to_string(), json!("value"));
        let result = store
            .set_variables(&created.task.task_id, "test-owner", vars)
            .await;
        assert!(matches!(result, Err(TaskError::Expired { .. })));
    }

    #[tokio::test]
    async fn test_cleanup_expired_removes_expired_tasks() {
        let store = test_store();
        let created = store
            .create("test-owner", "tools/call", Some(1))
            .await
            .unwrap();

        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        let removed = store.cleanup_expired().await.unwrap();
        assert_eq!(removed, 1);

        // Task should now be gone
        let result = store.get(&created.task.task_id, "test-owner").await;
        assert!(matches!(result, Err(TaskError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_cleanup_expired_returns_count() {
        let store = test_store();
        // Create 3 tasks with 1ms TTL
        for _ in 0..3 {
            store
                .create("test-owner", "tools/call", Some(1))
                .await
                .unwrap();
        }

        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        let removed = store.cleanup_expired().await.unwrap();
        assert_eq!(removed, 3);
    }

    #[tokio::test]
    async fn test_cleanup_expired_preserves_non_expired() {
        let store = test_store();
        // One with long TTL
        let long_lived = store
            .create("test-owner", "tools/call", Some(3_600_000))
            .await
            .unwrap();
        // One with tiny TTL
        store
            .create("test-owner", "tools/call", Some(1))
            .await
            .unwrap();

        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        let removed = store.cleanup_expired().await.unwrap();
        assert_eq!(removed, 1);

        // Long-lived task should still be accessible
        let fetched = store
            .get(&long_lived.task.task_id, "test-owner")
            .await
            .unwrap();
        assert_eq!(fetched.task.task_id, long_lived.task.task_id);
    }
}

// ─── Concurrency Tests ──────────────────────────────────────────────────────

mod concurrency_tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn test_concurrent_creates_all_succeed() {
        let store = Arc::new(test_store());

        let mut handles = Vec::new();
        for i in 0..10 {
            let store = Arc::clone(&store);
            handles.push(tokio::spawn(async move {
                store
                    .create("test-owner", &format!("tools/call-{i}"), None)
                    .await
                    .unwrap()
            }));
        }

        let results: Vec<pmcp_tasks::domain::TaskRecord> = futures::future::join_all(handles)
            .await
            .into_iter()
            .map(|r| r.unwrap())
            .collect();

        assert_eq!(results.len(), 10);

        // Verify all tasks are accessible
        let page = store
            .list(ListTasksOptions {
                owner_id: "test-owner".to_string(),
                cursor: None,
                limit: None,
            })
            .await
            .unwrap();
        assert_eq!(page.tasks.len(), 10);
    }

    #[tokio::test]
    async fn test_concurrent_updates_no_data_loss() {
        let store = Arc::new(test_store());
        let created = store
            .create("test-owner", "tools/call", None)
            .await
            .unwrap();
        let task_id = created.task.task_id.clone();

        let mut handles = Vec::new();
        for i in 0..5 {
            let store = Arc::clone(&store);
            let task_id = task_id.clone();
            handles.push(tokio::spawn(async move {
                let mut vars = HashMap::new();
                vars.insert(format!("var_{i}"), json!(i));
                store
                    .set_variables(&task_id, "test-owner", vars)
                    .await
                    .unwrap();
            }));
        }

        futures::future::join_all(handles).await;

        // All 5 variables should be present after concurrent writes
        let record = store.get(&task_id, "test-owner").await.unwrap();
        assert_eq!(
            record.variables.len(),
            5,
            "all concurrent variable writes should be preserved"
        );
        for i in 0..5 {
            assert!(
                record.variables.contains_key(&format!("var_{i}")),
                "missing var_{i}"
            );
        }
    }

    #[tokio::test]
    async fn test_concurrent_reads_during_writes() {
        let store = Arc::new(test_store());
        let created = store
            .create("test-owner", "tools/call", None)
            .await
            .unwrap();
        let task_id = created.task.task_id.clone();

        let mut handles = Vec::new();

        // Spawn readers
        for _ in 0..5 {
            let store = Arc::clone(&store);
            let task_id = task_id.clone();
            handles.push(tokio::spawn(async move {
                let result = store.get(&task_id, "test-owner").await;
                assert!(
                    result.is_ok(),
                    "read should not fail during concurrent writes"
                );
            }));
        }

        // Spawn writers
        for i in 0..5 {
            let store = Arc::clone(&store);
            let task_id = task_id.clone();
            handles.push(tokio::spawn(async move {
                let mut vars = HashMap::new();
                vars.insert(format!("concurrent_key_{i}"), json!(i));
                let result = store.set_variables(&task_id, "test-owner", vars).await;
                assert!(
                    result.is_ok(),
                    "write should not fail during concurrent reads"
                );
            }));
        }

        // All operations should complete without panics or errors
        let results = futures::future::join_all(handles).await;
        for result in results {
            let _: () = result.unwrap(); // No panics
        }
    }
}
