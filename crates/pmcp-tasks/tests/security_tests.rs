//! Security tests for InMemoryTaskStore.
//!
//! Tests verify owner isolation, anonymous access control, resource limits,
//! and UUID entropy. These tests prove the security boundary of the task
//! system: no cross-owner data leakage, proper access rejection, and
//! resource exhaustion protection.

use std::collections::{HashMap, HashSet};

use pmcp_tasks::security::{TaskSecurityConfig, DEFAULT_LOCAL_OWNER};
use pmcp_tasks::store::memory::InMemoryTaskStore;
use pmcp_tasks::store::{ListTasksOptions, StoreConfig, TaskStore};
use pmcp_tasks::types::task::TaskStatus;
use pmcp_tasks::TaskError;
use serde_json::json;

/// Creates a store with anonymous access enabled for test convenience.
fn test_store() -> InMemoryTaskStore {
    InMemoryTaskStore::new().with_security(TaskSecurityConfig::default().with_allow_anonymous(true))
}

/// Creates a store with a specific max tasks limit and anonymous access enabled.
fn store_with_max_tasks(max: usize) -> InMemoryTaskStore {
    InMemoryTaskStore::new().with_security(
        TaskSecurityConfig::default()
            .with_max_tasks_per_owner(max)
            .with_allow_anonymous(true),
    )
}

// ─── Owner Isolation Tests ──────────────────────────────────────────────────

mod owner_isolation_tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn test_get_with_wrong_owner_returns_not_found() {
        let store = test_store();
        let created = store.create("owner-a", "tools/call", None).await.unwrap();
        let result = store.get(&created.task.task_id, "owner-b").await;
        assert!(
            matches!(result, Err(TaskError::NotFound { .. })),
            "expected NotFound, got: {result:?}"
        );
    }

    #[tokio::test]
    async fn test_update_status_wrong_owner_returns_not_found() {
        let store = test_store();
        let created = store.create("owner-a", "tools/call", None).await.unwrap();
        let result = store
            .update_status(
                &created.task.task_id,
                "owner-b",
                TaskStatus::Completed,
                None,
            )
            .await;
        assert!(
            matches!(result, Err(TaskError::NotFound { .. })),
            "expected NotFound, got: {result:?}"
        );
    }

    #[tokio::test]
    async fn test_set_variables_wrong_owner_returns_not_found() {
        let store = test_store();
        let created = store.create("owner-a", "tools/call", None).await.unwrap();
        let mut vars = HashMap::new();
        vars.insert("key".to_string(), json!("value"));
        let result = store
            .set_variables(&created.task.task_id, "owner-b", vars)
            .await;
        assert!(
            matches!(result, Err(TaskError::NotFound { .. })),
            "expected NotFound, got: {result:?}"
        );
    }

    #[tokio::test]
    async fn test_set_result_wrong_owner_returns_not_found() {
        let store = test_store();
        let created = store.create("owner-a", "tools/call", None).await.unwrap();
        let result = store
            .set_result(&created.task.task_id, "owner-b", json!("data"))
            .await;
        assert!(
            matches!(result, Err(TaskError::NotFound { .. })),
            "expected NotFound, got: {result:?}"
        );
    }

    #[tokio::test]
    async fn test_get_result_wrong_owner_returns_not_found() {
        let store = test_store();
        let created = store.create("owner-a", "tools/call", None).await.unwrap();
        // Complete the task as owner-a
        store
            .complete_with_result(
                &created.task.task_id,
                "owner-a",
                TaskStatus::Completed,
                None,
                json!({"result": true}),
            )
            .await
            .unwrap();
        // owner-b tries to get result
        let result = store.get_result(&created.task.task_id, "owner-b").await;
        assert!(
            matches!(result, Err(TaskError::NotFound { .. })),
            "expected NotFound, got: {result:?}"
        );
    }

    #[tokio::test]
    async fn test_cancel_wrong_owner_returns_not_found() {
        let store = test_store();
        let created = store.create("owner-a", "tools/call", None).await.unwrap();
        let result = store.cancel(&created.task.task_id, "owner-b").await;
        assert!(
            matches!(result, Err(TaskError::NotFound { .. })),
            "expected NotFound, got: {result:?}"
        );
    }

    #[tokio::test]
    async fn test_complete_with_result_wrong_owner_returns_not_found() {
        let store = test_store();
        let created = store.create("owner-a", "tools/call", None).await.unwrap();
        let result = store
            .complete_with_result(
                &created.task.task_id,
                "owner-b",
                TaskStatus::Completed,
                None,
                json!({"hijacked": true}),
            )
            .await;
        assert!(
            matches!(result, Err(TaskError::NotFound { .. })),
            "expected NotFound, got: {result:?}"
        );
    }

    #[tokio::test]
    async fn test_list_scoped_to_owner() {
        let store = test_store();
        store.create("owner-a", "tools/call", None).await.unwrap();
        store.create("owner-a", "tools/call", None).await.unwrap();
        store.create("owner-b", "tools/call", None).await.unwrap();
        store.create("owner-c", "tools/call", None).await.unwrap();

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
    async fn test_error_does_not_leak_owner_info() {
        let store = test_store();
        let created = store.create("owner-a", "tools/call", None).await.unwrap();
        let result = store.get(&created.task.task_id, "owner-b").await;
        let err = result.unwrap_err();
        let err_msg = err.to_string();

        // The error message must NOT contain any reference to the actual owner.
        // It should be indistinguishable from "task does not exist."
        assert!(
            !err_msg.contains("owner-a"),
            "error leaks actual owner: {err_msg}"
        );
        assert!(
            !err_msg.contains("owner-b"),
            "error leaks requesting owner: {err_msg}"
        );
        assert!(
            !err_msg.contains("mismatch"),
            "error reveals mismatch: {err_msg}"
        );
        // Verify the error is a simple NotFound
        assert!(
            err_msg.contains("not found"),
            "error should be a simple not found: {err_msg}"
        );
    }
}

// ─── Anonymous Access Tests ─────────────────────────────────────────────────

mod anonymous_access_tests {
    use super::*;

    #[tokio::test]
    async fn test_anonymous_rejected_when_not_allowed() {
        // Default config has allow_anonymous = false
        let store = InMemoryTaskStore::new();
        let result = store.create(DEFAULT_LOCAL_OWNER, "tools/call", None).await;
        assert!(result.is_err(), "anonymous access should be rejected");
        assert!(
            result.unwrap_err().to_string().contains("anonymous"),
            "error should mention anonymous access"
        );
    }

    #[tokio::test]
    async fn test_anonymous_rejected_with_empty_owner() {
        let store = InMemoryTaskStore::new();
        let result = store.create("", "tools/call", None).await;
        assert!(result.is_err(), "empty owner should be rejected");
    }

    #[tokio::test]
    async fn test_anonymous_allowed_when_configured() {
        let store = InMemoryTaskStore::new()
            .with_security(TaskSecurityConfig::default().with_allow_anonymous(true));
        let result = store.create(DEFAULT_LOCAL_OWNER, "tools/call", None).await;
        assert!(
            result.is_ok(),
            "anonymous access should be allowed: {result:?}"
        );
    }

    #[tokio::test]
    async fn test_authenticated_owner_always_allowed() {
        // Even with allow_anonymous = false, authenticated owners succeed
        let store = InMemoryTaskStore::new(); // allow_anonymous = false
        let result = store.create("user-123", "tools/call", None).await;
        assert!(
            result.is_ok(),
            "authenticated owner should always succeed: {result:?}"
        );
    }
}

// ─── Resource Limit Tests ───────────────────────────────────────────────────

mod resource_limit_tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn test_max_tasks_per_owner_enforced() {
        let store = store_with_max_tasks(3);
        for i in 0..3 {
            store
                .create("test-owner", &format!("tools/call-{i}"), None)
                .await
                .unwrap();
        }
        let result = store.create("test-owner", "tools/call-extra", None).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            TaskError::ResourceExhausted { suggested_action } => {
                assert!(suggested_action.is_some());
            },
            other => panic!("expected ResourceExhausted, got: {other}"),
        }
    }

    #[tokio::test]
    async fn test_max_tasks_per_owner_scoped_to_owner() {
        let store = store_with_max_tasks(2);
        // Owner A fills their quota
        store.create("owner-a", "tools/call", None).await.unwrap();
        store.create("owner-a", "tools/call", None).await.unwrap();
        // Owner B can still create
        let result = store.create("owner-b", "tools/call", None).await;
        assert!(result.is_ok(), "owner-b should not be limited by owner-a");
    }

    #[tokio::test]
    async fn test_max_tasks_per_owner_freed_after_cleanup() {
        let store = store_with_max_tasks(2);
        // Fill quota with 1ms TTL tasks
        store
            .create("test-owner", "tools/call", Some(1))
            .await
            .unwrap();
        store
            .create("test-owner", "tools/call", Some(1))
            .await
            .unwrap();

        // At limit
        let result = store.create("test-owner", "tools/call", None).await;
        assert!(result.is_err(), "should be at limit");

        // Expire and cleanup
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        let removed = store.cleanup_expired().await.unwrap();
        assert_eq!(removed, 2);

        // Should be able to create again
        let result = store.create("test-owner", "tools/call", None).await;
        assert!(result.is_ok(), "should create after cleanup: {result:?}");
    }

    #[tokio::test]
    async fn test_variable_size_limit_enforced() {
        let store = InMemoryTaskStore::new()
            .with_config(StoreConfig {
                max_variable_size_bytes: 100,
                ..StoreConfig::default()
            })
            .with_security(TaskSecurityConfig::default().with_allow_anonymous(true));

        let created = store
            .create("test-owner", "tools/call", None)
            .await
            .unwrap();

        // Set a variable that exceeds the limit
        let mut vars = HashMap::new();
        vars.insert("big".to_string(), json!("x".repeat(200)));
        let result = store
            .set_variables(&created.task.task_id, "test-owner", vars)
            .await;
        assert!(
            matches!(result, Err(TaskError::VariableSizeExceeded { .. })),
            "expected VariableSizeExceeded, got: {result:?}"
        );
    }
}

// ─── UUID Entropy Tests ─────────────────────────────────────────────────────

mod uuid_entropy_tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn test_task_ids_unique_across_1000_creations() {
        let store = InMemoryTaskStore::new().with_security(
            TaskSecurityConfig::default()
                .with_max_tasks_per_owner(1100)
                .with_allow_anonymous(true),
        );

        let mut ids = HashSet::new();
        for _ in 0..1000 {
            let record = store
                .create("test-owner", "tools/call", None)
                .await
                .unwrap();
            ids.insert(record.task.task_id);
        }
        assert_eq!(ids.len(), 1000, "all 1000 task IDs must be unique");
    }

    #[tokio::test]
    async fn test_task_id_is_valid_uuid_v4() {
        let store = test_store();
        let record = store
            .create("test-owner", "tools/call", None)
            .await
            .unwrap();
        let parsed =
            uuid::Uuid::parse_str(&record.task.task_id).expect("task_id should be a valid UUID");
        assert_eq!(parsed.get_version_num(), 4, "task_id should be UUID v4");
    }
}
