//! TEST-03: Integration tests for TaskContext behavior.
//!
//! Tests exercise the TaskContext ergonomic wrapper against InMemoryTaskStore,
//! covering variable CRUD with typed accessors, null-deletion semantics,
//! status transition convenience methods, atomic complete_with_result,
//! invalid transition errors, and TaskContext identity/clone behavior.

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::json;

use pmcp_tasks::context::TaskContext;
use pmcp_tasks::error::TaskError;
use pmcp_tasks::security::TaskSecurityConfig;
use pmcp_tasks::store::memory::InMemoryTaskStore;
use pmcp_tasks::store::TaskStore;
use pmcp_tasks::TaskStatus;

/// Creates a test store with anonymous access enabled and a TaskContext
/// scoped to a freshly created task.
async fn create_store_and_context() -> (Arc<InMemoryTaskStore>, TaskContext) {
    let store = Arc::new(
        InMemoryTaskStore::new()
            .with_security(TaskSecurityConfig::default().with_allow_anonymous(true)),
    );

    let record = store
        .create("test-owner", "tools/call", None)
        .await
        .unwrap();

    let ctx = TaskContext::new(
        store.clone(),
        record.task.task_id.clone(),
        "test-owner".to_string(),
    );

    (store, ctx)
}

mod variable_tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn test_set_and_get_string_variable() {
        let (_store, ctx) = create_store_and_context().await;

        ctx.set_variable("name", json!("Alice")).await.unwrap();
        let value = ctx.get_string("name").await.unwrap();
        assert_eq!(value, Some("Alice".to_string()));
    }

    #[tokio::test]
    async fn test_set_and_get_i64_variable() {
        let (_store, ctx) = create_store_and_context().await;

        ctx.set_variable("count", json!(42)).await.unwrap();
        let value = ctx.get_i64("count").await.unwrap();
        assert_eq!(value, Some(42));
    }

    #[tokio::test]
    async fn test_set_and_get_f64_variable() {
        let (_store, ctx) = create_store_and_context().await;

        ctx.set_variable("ratio", json!(1.234)).await.unwrap();
        let value = ctx.get_f64("ratio").await.unwrap();
        assert_eq!(value, Some(1.234));
    }

    #[tokio::test]
    async fn test_set_and_get_bool_variable() {
        let (_store, ctx) = create_store_and_context().await;

        ctx.set_variable("enabled", json!(true)).await.unwrap();
        let value = ctx.get_bool("enabled").await.unwrap();
        assert_eq!(value, Some(true));
    }

    #[tokio::test]
    async fn test_get_typed_struct() {
        let (_store, ctx) = create_store_and_context().await;

        #[derive(Debug, PartialEq, serde::Deserialize)]
        struct Point {
            x: i32,
            y: i32,
        }

        ctx.set_variable("point", json!({"x": 10, "y": 20}))
            .await
            .unwrap();
        let point: Option<Point> = ctx.get_typed("point").await.unwrap();
        assert_eq!(point, Some(Point { x: 10, y: 20 }));
    }

    #[tokio::test]
    async fn test_get_variable_returns_none_for_missing_key() {
        let (_store, ctx) = create_store_and_context().await;

        let value = ctx.get_variable("nonexistent").await.unwrap();
        assert_eq!(value, None);
    }

    #[tokio::test]
    async fn test_get_string_returns_none_for_wrong_type() {
        let (_store, ctx) = create_store_and_context().await;

        ctx.set_variable("number", json!(42)).await.unwrap();
        let value = ctx.get_string("number").await.unwrap();
        assert_eq!(value, None);
    }

    #[tokio::test]
    async fn test_get_i64_returns_none_for_wrong_type() {
        let (_store, ctx) = create_store_and_context().await;

        ctx.set_variable("text", json!("hello")).await.unwrap();
        let value = ctx.get_i64("text").await.unwrap();
        assert_eq!(value, None);
    }

    #[tokio::test]
    async fn test_get_bool_returns_none_for_wrong_type() {
        let (_store, ctx) = create_store_and_context().await;

        ctx.set_variable("number", json!(1)).await.unwrap();
        let value = ctx.get_bool("number").await.unwrap();
        assert_eq!(value, None);
    }

    #[tokio::test]
    async fn test_get_typed_returns_none_for_incompatible_type() {
        let (_store, ctx) = create_store_and_context().await;

        #[derive(Debug, PartialEq, serde::Deserialize)]
        struct Point {
            x: i32,
            y: i32,
        }

        ctx.set_variable("not_a_point", json!("just a string"))
            .await
            .unwrap();
        let point: Option<Point> = ctx.get_typed("not_a_point").await.unwrap();
        assert_eq!(point, None);
    }

    #[tokio::test]
    async fn test_set_variables_merges_multiple() {
        let (_store, ctx) = create_store_and_context().await;

        let mut vars = HashMap::new();
        vars.insert("a".to_string(), json!("alpha"));
        vars.insert("b".to_string(), json!("beta"));
        vars.insert("c".to_string(), json!("gamma"));
        ctx.set_variables(vars).await.unwrap();

        let all = ctx.variables().await.unwrap();
        assert_eq!(all.len(), 3);
        assert_eq!(all.get("a").unwrap(), &json!("alpha"));
        assert_eq!(all.get("b").unwrap(), &json!("beta"));
        assert_eq!(all.get("c").unwrap(), &json!("gamma"));
    }

    #[tokio::test]
    async fn test_delete_variable_via_null() {
        let (_store, ctx) = create_store_and_context().await;

        // Set a variable first
        ctx.set_variable("to_delete", json!("value"))
            .await
            .unwrap();
        assert!(ctx.get_variable("to_delete").await.unwrap().is_some());

        // Delete via null-deletion semantics
        ctx.delete_variable("to_delete").await.unwrap();
        let value = ctx.get_variable("to_delete").await.unwrap();
        assert_eq!(value, None);
    }

    #[tokio::test]
    async fn test_variables_returns_all() {
        let (_store, ctx) = create_store_and_context().await;

        ctx.set_variable("x", json!(1)).await.unwrap();
        ctx.set_variable("y", json!(2)).await.unwrap();
        ctx.set_variable("z", json!(3)).await.unwrap();

        let all = ctx.variables().await.unwrap();
        assert_eq!(all.len(), 3);
        assert_eq!(all.get("x").unwrap(), &json!(1));
        assert_eq!(all.get("y").unwrap(), &json!(2));
        assert_eq!(all.get("z").unwrap(), &json!(3));
    }

    #[tokio::test]
    async fn test_set_variable_overwrites_existing() {
        let (_store, ctx) = create_store_and_context().await;

        ctx.set_variable("key", json!("old")).await.unwrap();
        ctx.set_variable("key", json!("new")).await.unwrap();

        let value = ctx.get_string("key").await.unwrap();
        assert_eq!(value, Some("new".to_string()));
    }

    #[tokio::test]
    async fn test_get_raw_variable_returns_json_value() {
        let (_store, ctx) = create_store_and_context().await;

        let complex = json!({"nested": {"deep": [1, 2, 3]}});
        ctx.set_variable("data", complex.clone()).await.unwrap();

        let value = ctx.get_variable("data").await.unwrap();
        assert_eq!(value, Some(complex));
    }
}

mod transition_tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn test_complete_transitions_to_completed_with_result() {
        let (_store, ctx) = create_store_and_context().await;

        let result_value = json!({"output": "computation result"});
        let record = ctx.complete(result_value.clone()).await.unwrap();

        assert_eq!(record.task.status, TaskStatus::Completed);
        assert_eq!(record.result, Some(result_value));
    }

    #[tokio::test]
    async fn test_fail_transitions_to_failed_with_message() {
        let (_store, ctx) = create_store_and_context().await;

        let record = ctx.fail("connection timeout").await.unwrap();

        assert_eq!(record.task.status, TaskStatus::Failed);
        assert_eq!(
            record.task.status_message.as_deref(),
            Some("connection timeout")
        );
    }

    #[tokio::test]
    async fn test_require_input_transitions_to_input_required() {
        let (_store, ctx) = create_store_and_context().await;

        let record = ctx
            .require_input("Please provide your API key")
            .await
            .unwrap();

        assert_eq!(record.task.status, TaskStatus::InputRequired);
        assert_eq!(
            record.task.status_message.as_deref(),
            Some("Please provide your API key")
        );
    }

    #[tokio::test]
    async fn test_resume_transitions_back_to_working() {
        let (_store, ctx) = create_store_and_context().await;

        // First transition to input_required
        ctx.require_input("Need input").await.unwrap();

        // Resume back to working
        let record = ctx.resume().await.unwrap();
        assert_eq!(record.task.status, TaskStatus::Working);
    }

    #[tokio::test]
    async fn test_cancel_transitions_to_cancelled() {
        let (_store, ctx) = create_store_and_context().await;

        let record = ctx.cancel().await.unwrap();
        assert_eq!(record.task.status, TaskStatus::Cancelled);
    }

    #[tokio::test]
    async fn test_complete_is_atomic() {
        let (_store, ctx) = create_store_and_context().await;

        let result_value = json!({"data": [1, 2, 3]});
        ctx.complete(result_value.clone()).await.unwrap();

        // Verify via get() that both status and result are set
        let record = ctx.get().await.unwrap();
        assert_eq!(record.task.status, TaskStatus::Completed);
        assert_eq!(record.result, Some(result_value));
    }

    #[tokio::test]
    async fn test_invalid_transition_returns_error() {
        let (_store, ctx) = create_store_and_context().await;

        // Complete the task first
        ctx.complete(json!("done")).await.unwrap();

        // Attempting to fail a completed task should return InvalidTransition
        let result = ctx.fail("too late").await;
        assert!(result.is_err());
        match result.unwrap_err() {
            TaskError::InvalidTransition {
                from, to, task_id, ..
            } => {
                assert_eq!(from, TaskStatus::Completed);
                assert_eq!(to, TaskStatus::Failed);
                assert_eq!(task_id, ctx.task_id());
            }
            other => panic!("expected InvalidTransition, got: {other}"),
        }
    }

    #[tokio::test]
    async fn test_complete_then_complete_returns_error() {
        let (_store, ctx) = create_store_and_context().await;

        ctx.complete(json!("first")).await.unwrap();

        let result = ctx.complete(json!("second")).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TaskError::InvalidTransition { .. }
        ));
    }

    #[tokio::test]
    async fn test_cancel_then_resume_returns_error() {
        let (_store, ctx) = create_store_and_context().await;

        ctx.cancel().await.unwrap();

        let result = ctx.resume().await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TaskError::InvalidTransition { .. }
        ));
    }

    #[tokio::test]
    async fn test_fail_with_string_reference() {
        let (_store, ctx) = create_store_and_context().await;

        // Test that fail() works with both &str and String
        let msg = String::from("dynamic error");
        let record = ctx.fail(msg).await.unwrap();
        assert_eq!(record.task.status, TaskStatus::Failed);
        assert_eq!(
            record.task.status_message.as_deref(),
            Some("dynamic error")
        );
    }

    #[tokio::test]
    async fn test_require_input_then_complete() {
        let (_store, ctx) = create_store_and_context().await;

        // Transition: working -> input_required -> completed
        ctx.require_input("Need API key").await.unwrap();
        let record = ctx.complete(json!({"key_used": true})).await.unwrap();

        assert_eq!(record.task.status, TaskStatus::Completed);
        assert_eq!(record.result, Some(json!({"key_used": true})));
    }
}

mod identity_tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn test_task_id_returns_correct_id() {
        let (store, _) = create_store_and_context().await;

        let record = store
            .create("owner-check", "tools/call", None)
            .await
            .unwrap();
        let ctx = TaskContext::new(
            store,
            record.task.task_id.clone(),
            "owner-check".to_string(),
        );

        assert_eq!(ctx.task_id(), record.task.task_id);
    }

    #[tokio::test]
    async fn test_owner_id_returns_correct_owner() {
        let (_store, ctx) = create_store_and_context().await;
        assert_eq!(ctx.owner_id(), "test-owner");
    }

    #[tokio::test]
    async fn test_clone_shares_store() {
        let (_store, ctx) = create_store_and_context().await;

        // Set a variable via original context
        ctx.set_variable("shared", json!("hello")).await.unwrap();

        // Read via cloned context
        let ctx_clone = ctx.clone();
        let value = ctx_clone.get_string("shared").await.unwrap();
        assert_eq!(value, Some("hello".to_string()));

        // Mutate via clone, read via original
        ctx_clone
            .set_variable("from_clone", json!(true))
            .await
            .unwrap();
        let from_clone = ctx.get_bool("from_clone").await.unwrap();
        assert_eq!(from_clone, Some(true));
    }

    #[tokio::test]
    async fn test_clone_has_same_ids() {
        let (_store, ctx) = create_store_and_context().await;
        let cloned = ctx.clone();

        assert_eq!(ctx.task_id(), cloned.task_id());
        assert_eq!(ctx.owner_id(), cloned.owner_id());
    }

    #[tokio::test]
    async fn test_debug_format() {
        let (_store, ctx) = create_store_and_context().await;
        let debug = format!("{ctx:?}");
        assert!(debug.contains("TaskContext"));
        assert!(debug.contains(ctx.task_id()));
        assert!(debug.contains("test-owner"));
    }

    #[tokio::test]
    async fn test_context_with_wrong_owner_returns_not_found() {
        let (store, _) = create_store_and_context().await;

        let record = store
            .create("real-owner", "tools/call", None)
            .await
            .unwrap();

        // Create context with wrong owner
        let ctx = TaskContext::new(
            store,
            record.task.task_id.clone(),
            "wrong-owner".to_string(),
        );

        let result = ctx.get().await;
        assert!(matches!(result.unwrap_err(), TaskError::NotFound { .. }));
    }
}
