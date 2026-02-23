//! Protocol type serialization round-trip tests (TEST-01).
//!
//! Verifies all wire types round-trip through serde_json and produce JSON
//! matching the MCP 2025-11-25 spec exactly.

use pretty_assertions::assert_eq;
use serde_json::json;

use pmcp_tasks::constants::{
    METHOD_TASKS_CANCEL, METHOD_TASKS_GET, METHOD_TASKS_LIST, METHOD_TASKS_RESULT,
    METHOD_TASKS_STATUS_NOTIFICATION,
};
use pmcp_tasks::{related_task_meta, MODEL_IMMEDIATE_RESPONSE_META_KEY, RELATED_TASK_META_KEY};
use pmcp_tasks::{
    ClientTaskCapabilities, CreateTaskResult, EmptyObject, ServerTaskCapabilities, Task,
    TaskCancelParams, TaskGetParams, TaskListParams, TaskParams, TaskResultParams, TaskStatus,
    TaskStatusNotification, TaskSupport, ToolExecution,
};

// ─── TaskStatus Serialization ───────────────────────────────────────────────

#[test]
fn test_task_status_serializes_snake_case() {
    assert_eq!(
        serde_json::to_value(TaskStatus::Working).unwrap(),
        "working"
    );
    assert_eq!(
        serde_json::to_value(TaskStatus::InputRequired).unwrap(),
        "input_required"
    );
    assert_eq!(
        serde_json::to_value(TaskStatus::Completed).unwrap(),
        "completed"
    );
    assert_eq!(serde_json::to_value(TaskStatus::Failed).unwrap(), "failed");
    assert_eq!(
        serde_json::to_value(TaskStatus::Cancelled).unwrap(),
        "cancelled"
    );
}

#[test]
fn test_task_status_round_trip() {
    for status in [
        TaskStatus::Working,
        TaskStatus::InputRequired,
        TaskStatus::Completed,
        TaskStatus::Failed,
        TaskStatus::Cancelled,
    ] {
        let json = serde_json::to_value(status).unwrap();
        let back: TaskStatus = serde_json::from_value(json).unwrap();
        assert_eq!(status, back, "round-trip failed for {status}");
    }
}

#[test]
fn test_task_status_unknown_string_errors() {
    let result = serde_json::from_value::<TaskStatus>(json!("unknown_status"));
    assert!(
        result.is_err(),
        "unknown status string should produce error"
    );
}

// ─── Task Serialization (critical spec compliance) ──────────────────────────

#[test]
fn test_task_full_serialization_camel_case() {
    let task = Task {
        task_id: "786512e2-9e0d-44bd-8f29-789f320fe840".to_string(),
        status: TaskStatus::Working,
        status_message: Some("The operation is now in progress.".to_string()),
        created_at: "2025-11-25T10:30:00Z".to_string(),
        last_updated_at: "2025-11-25T10:40:00Z".to_string(),
        ttl: Some(60000),
        poll_interval: Some(5000),
        _meta: None,
    };

    let json = serde_json::to_value(&task).unwrap();
    assert_eq!(json["taskId"], "786512e2-9e0d-44bd-8f29-789f320fe840");
    assert_eq!(json["status"], "working");
    assert_eq!(json["statusMessage"], "The operation is now in progress.");
    assert_eq!(json["createdAt"], "2025-11-25T10:30:00Z");
    assert_eq!(json["lastUpdatedAt"], "2025-11-25T10:40:00Z");
    assert_eq!(json["ttl"], 60000);
    assert_eq!(json["pollInterval"], 5000);
    assert!(json.get("_meta").is_none());
}

#[test]
fn test_task_ttl_null_not_omitted() {
    // THE #1 PITFALL: ttl: None MUST serialize as "ttl": null, NOT be omitted.
    let task = Task {
        task_id: "ttl-test".to_string(),
        status: TaskStatus::Working,
        status_message: None,
        created_at: "2025-11-25T10:30:00Z".to_string(),
        last_updated_at: "2025-11-25T10:30:00Z".to_string(),
        ttl: None,
        poll_interval: None,
        _meta: None,
    };

    let json = serde_json::to_value(&task).unwrap();

    // ttl MUST be present as null
    assert!(json.get("ttl").is_some(), "ttl must be present in JSON");
    assert!(json["ttl"].is_null(), "ttl must be null when None");
}

#[test]
fn test_task_optional_fields_omitted_when_none() {
    let task = Task {
        task_id: "omit-test".to_string(),
        status: TaskStatus::Working,
        status_message: None,
        created_at: "2025-11-25T10:30:00Z".to_string(),
        last_updated_at: "2025-11-25T10:30:00Z".to_string(),
        ttl: Some(60000),
        poll_interval: None,
        _meta: None,
    };

    let json = serde_json::to_value(&task).unwrap();

    // poll_interval should be omitted when None
    assert!(
        json.get("pollInterval").is_none(),
        "pollInterval should be omitted when None"
    );
    // status_message should be omitted when None
    assert!(
        json.get("statusMessage").is_none(),
        "statusMessage should be omitted when None"
    );
    // _meta should be omitted when None
    assert!(
        json.get("_meta").is_none(),
        "_meta should be omitted when None"
    );
}

#[test]
fn test_task_meta_included_when_present() {
    let mut meta = serde_json::Map::new();
    meta.insert("custom_key".to_string(), json!("custom_value"));

    let task = Task {
        task_id: "meta-test".to_string(),
        status: TaskStatus::Working,
        status_message: None,
        created_at: "2025-11-25T10:30:00Z".to_string(),
        last_updated_at: "2025-11-25T10:30:00Z".to_string(),
        ttl: Some(60000),
        poll_interval: None,
        _meta: Some(meta),
    };

    let json = serde_json::to_value(&task).unwrap();
    assert!(
        json.get("_meta").is_some(),
        "_meta should be included when Some"
    );
    assert_eq!(json["_meta"]["custom_key"], "custom_value");
}

#[test]
fn test_task_round_trip() {
    let task = Task {
        task_id: "round-trip-1".to_string(),
        status: TaskStatus::InputRequired,
        status_message: Some("Need more info".to_string()),
        created_at: "2025-11-25T10:30:00Z".to_string(),
        last_updated_at: "2025-11-25T10:35:00Z".to_string(),
        ttl: None,
        poll_interval: Some(3000),
        _meta: None,
    };

    let json_str = serde_json::to_string(&task).unwrap();
    let back: Task = serde_json::from_str(&json_str).unwrap();

    assert_eq!(back.task_id, "round-trip-1");
    assert_eq!(back.status, TaskStatus::InputRequired);
    assert_eq!(back.status_message.as_deref(), Some("Need more info"));
    assert!(back.ttl.is_none());
    assert_eq!(back.poll_interval, Some(3000));
}

#[test]
fn test_task_deserialize_from_spec_json() {
    // Matches the spec example from the MCP Tasks documentation
    let json_str = r#"{
        "taskId": "786512e2-9e0d-44bd-8f29-789f320fe840",
        "status": "working",
        "statusMessage": "The operation is now in progress.",
        "createdAt": "2025-11-25T10:30:00Z",
        "lastUpdatedAt": "2025-11-25T10:40:00Z",
        "ttl": 60000,
        "pollInterval": 5000
    }"#;

    let task: Task = serde_json::from_str(json_str).unwrap();
    assert_eq!(task.task_id, "786512e2-9e0d-44bd-8f29-789f320fe840");
    assert_eq!(task.status, TaskStatus::Working);
    assert_eq!(
        task.status_message.as_deref(),
        Some("The operation is now in progress.")
    );
    assert_eq!(task.ttl, Some(60000));
    assert_eq!(task.poll_interval, Some(5000));
}

// ─── CreateTaskResult Serialization ─────────────────────────────────────────

#[test]
fn test_create_task_result_wraps_in_task_field() {
    let result = CreateTaskResult {
        task: Task {
            task_id: "task-abc".to_string(),
            status: TaskStatus::Working,
            status_message: None,
            created_at: "2025-11-25T10:30:00Z".to_string(),
            last_updated_at: "2025-11-25T10:30:00Z".to_string(),
            ttl: Some(60000),
            poll_interval: None,
            _meta: None,
        },
        _meta: None,
    };

    let json = serde_json::to_value(&result).unwrap();
    // CreateTaskResult wraps task in a "task" field
    assert!(json.get("task").is_some(), "must have task field");
    assert_eq!(json["task"]["taskId"], "task-abc");
    assert_eq!(json["task"]["status"], "working");
    assert_eq!(json["task"]["ttl"], 60000);
}

#[test]
fn test_create_task_result_meta_included_when_present() {
    let mut meta = serde_json::Map::new();
    meta.insert("result_key".to_string(), json!("result_value"));

    let result = CreateTaskResult {
        task: Task {
            task_id: "task-meta".to_string(),
            status: TaskStatus::Working,
            status_message: None,
            created_at: "2025-11-25T10:30:00Z".to_string(),
            last_updated_at: "2025-11-25T10:30:00Z".to_string(),
            ttl: Some(30000),
            poll_interval: None,
            _meta: None,
        },
        _meta: Some(meta),
    };

    let json = serde_json::to_value(&result).unwrap();
    assert!(json.get("_meta").is_some());
    assert_eq!(json["_meta"]["result_key"], "result_value");
}

#[test]
fn test_create_task_result_meta_omitted_when_none() {
    let result = CreateTaskResult {
        task: Task {
            task_id: "task-no-meta".to_string(),
            status: TaskStatus::Working,
            status_message: None,
            created_at: "2025-11-25T10:30:00Z".to_string(),
            last_updated_at: "2025-11-25T10:30:00Z".to_string(),
            ttl: Some(30000),
            poll_interval: None,
            _meta: None,
        },
        _meta: None,
    };

    let json = serde_json::to_value(&result).unwrap();
    assert!(
        json.get("_meta").is_none(),
        "_meta should be omitted when None"
    );
}

#[test]
fn test_create_task_result_round_trip_spec_example() {
    let result = CreateTaskResult {
        task: Task {
            task_id: "786512e2-9e0d-44bd-8f29-789f320fe840".to_string(),
            status: TaskStatus::Working,
            status_message: Some("The operation is now in progress.".to_string()),
            created_at: "2025-11-25T10:30:00Z".to_string(),
            last_updated_at: "2025-11-25T10:40:00Z".to_string(),
            ttl: Some(60000),
            poll_interval: Some(5000),
            _meta: None,
        },
        _meta: None,
    };

    let json_str = serde_json::to_string(&result).unwrap();
    let back: CreateTaskResult = serde_json::from_str(&json_str).unwrap();
    assert_eq!(back.task.task_id, "786512e2-9e0d-44bd-8f29-789f320fe840");
    assert_eq!(back.task.status, TaskStatus::Working);
    assert_eq!(back.task.ttl, Some(60000));
    assert_eq!(back.task.poll_interval, Some(5000));
}

// ─── GetTaskResult / CancelTaskResult (flat, no wrapper) ────────────────────

#[test]
fn test_get_task_result_is_flat() {
    // GetTaskResult is a type alias for Task, so it serializes flat
    let result: pmcp_tasks::GetTaskResult = Task {
        task_id: "task-def".to_string(),
        status: TaskStatus::Completed,
        status_message: Some("Done".to_string()),
        created_at: "2025-11-25T10:30:00Z".to_string(),
        last_updated_at: "2025-11-25T10:35:00Z".to_string(),
        ttl: None,
        poll_interval: None,
        _meta: None,
    };

    let json = serde_json::to_value(&result).unwrap();
    // GetTaskResult is flat -- no "task" wrapper
    assert!(json.get("task").is_none(), "must NOT have task wrapper");
    assert_eq!(json["taskId"], "task-def");
    assert_eq!(json["status"], "completed");
}

#[test]
fn test_cancel_task_result_is_flat() {
    // CancelTaskResult is also a type alias for Task
    let result: pmcp_tasks::CancelTaskResult = Task {
        task_id: "task-cancel".to_string(),
        status: TaskStatus::Cancelled,
        status_message: Some("User requested cancellation".to_string()),
        created_at: "2025-11-25T10:30:00Z".to_string(),
        last_updated_at: "2025-11-25T10:36:00Z".to_string(),
        ttl: Some(60000),
        poll_interval: None,
        _meta: None,
    };

    let json = serde_json::to_value(&result).unwrap();
    assert!(json.get("task").is_none(), "must NOT have task wrapper");
    assert_eq!(json["taskId"], "task-cancel");
    assert_eq!(json["status"], "cancelled");
}

// ─── TaskParams Serialization ───────────────────────────────────────────────

#[test]
fn test_task_params_all_optional_empty() {
    let params = TaskParams {
        task_id: None,
        ttl: None,
        poll_interval: None,
    };
    let json = serde_json::to_value(&params).unwrap();
    assert_eq!(json, json!({}));
}

#[test]
fn test_task_params_camel_case() {
    let params = TaskParams {
        task_id: Some("abc-123".to_string()),
        ttl: Some(60000),
        poll_interval: Some(5000),
    };
    let json = serde_json::to_value(&params).unwrap();
    assert_eq!(json["taskId"], "abc-123");
    assert_eq!(json["ttl"], 60000);
    assert_eq!(json["pollInterval"], 5000);
}

// ─── Request Param Types ────────────────────────────────────────────────────

#[test]
fn test_task_get_params_round_trip() {
    let params = TaskGetParams {
        task_id: "abc".to_string(),
    };
    let json = serde_json::to_value(&params).unwrap();
    assert_eq!(json, json!({"taskId": "abc"}));

    let back: TaskGetParams = serde_json::from_value(json).unwrap();
    assert_eq!(back.task_id, "abc");
}

#[test]
fn test_task_result_params_round_trip() {
    let params = TaskResultParams {
        task_id: "result-123".to_string(),
    };
    let json = serde_json::to_value(&params).unwrap();
    assert_eq!(json, json!({"taskId": "result-123"}));

    let back: TaskResultParams = serde_json::from_value(json).unwrap();
    assert_eq!(back.task_id, "result-123");
}

#[test]
fn test_task_list_params_cursor_omitted_when_none() {
    let params = TaskListParams { cursor: None };
    let json = serde_json::to_value(&params).unwrap();
    assert!(json.get("cursor").is_none());
}

#[test]
fn test_task_list_params_cursor_included_when_some() {
    let params = TaskListParams {
        cursor: Some("page-2-token".to_string()),
    };
    let json = serde_json::to_value(&params).unwrap();
    assert_eq!(json["cursor"], "page-2-token");

    let back: TaskListParams = serde_json::from_value(json).unwrap();
    assert_eq!(back.cursor.as_deref(), Some("page-2-token"));
}

#[test]
fn test_task_cancel_params_round_trip() {
    let params = TaskCancelParams {
        task_id: "cancel-me".to_string(),
        result: None,
    };
    let json = serde_json::to_value(&params).unwrap();
    assert_eq!(json, json!({"taskId": "cancel-me"}));

    let back: TaskCancelParams = serde_json::from_value(json).unwrap();
    assert_eq!(back.task_id, "cancel-me");
    assert!(back.result.is_none());
}

// ─── Capability Types ───────────────────────────────────────────────────────

#[test]
fn test_server_task_capabilities_full() {
    let caps = ServerTaskCapabilities::full();
    let json = serde_json::to_value(&caps).unwrap();

    assert!(json.get("list").is_some());
    assert!(json["list"].is_object());
    assert_eq!(json["list"].as_object().unwrap().len(), 0);

    assert!(json.get("cancel").is_some());
    assert!(json["cancel"].is_object());
    assert_eq!(json["cancel"].as_object().unwrap().len(), 0);

    assert!(json["requests"]["tools"]["call"].is_object());
}

#[test]
fn test_server_task_capabilities_tools_only() {
    let caps = ServerTaskCapabilities::tools_only();
    let json = serde_json::to_value(&caps).unwrap();

    assert!(json.get("list").is_none());
    assert!(json.get("cancel").is_none());
    assert!(json["requests"]["tools"]["call"].is_object());
}

#[test]
fn test_client_task_capabilities_serialization() {
    let caps = ClientTaskCapabilities { supported: true };
    let json = serde_json::to_value(&caps).unwrap();
    assert_eq!(json, json!({"supported": true}));

    let caps = ClientTaskCapabilities { supported: false };
    let json = serde_json::to_value(&caps).unwrap();
    assert_eq!(json, json!({"supported": false}));
}

#[test]
fn test_capability_types_round_trip() {
    // Server full
    let original = ServerTaskCapabilities::full();
    let json_str = serde_json::to_string(&original).unwrap();
    let back: ServerTaskCapabilities = serde_json::from_str(&json_str).unwrap();
    assert!(back.list.is_some());
    assert!(back.cancel.is_some());
    assert!(back.requests.is_some());

    // Client
    let original = ClientTaskCapabilities { supported: true };
    let json_str = serde_json::to_string(&original).unwrap();
    let back: ClientTaskCapabilities = serde_json::from_str(&json_str).unwrap();
    assert!(back.supported);
}

// ─── TaskStatusNotification Serialization ───────────────────────────────────

#[test]
fn test_notification_camel_case_keys() {
    let notification = TaskStatusNotification {
        task_id: "task-42".to_string(),
        status: TaskStatus::Completed,
        status_message: Some("All done".to_string()),
        created_at: "2025-11-25T10:30:00Z".to_string(),
        last_updated_at: "2025-11-25T10:35:00Z".to_string(),
        ttl: Some(60000),
        poll_interval: Some(3000),
    };

    let json = serde_json::to_value(&notification).unwrap();
    assert_eq!(json["taskId"], "task-42");
    assert_eq!(json["status"], "completed");
    assert_eq!(json["statusMessage"], "All done");
    assert_eq!(json["createdAt"], "2025-11-25T10:30:00Z");
    assert_eq!(json["lastUpdatedAt"], "2025-11-25T10:35:00Z");
    assert_eq!(json["ttl"], 60000);
    assert_eq!(json["pollInterval"], 3000);
}

#[test]
fn test_notification_ttl_null_not_omitted() {
    let notification = TaskStatusNotification {
        task_id: "task-99".to_string(),
        status: TaskStatus::Working,
        status_message: None,
        created_at: "2025-11-25T10:30:00Z".to_string(),
        last_updated_at: "2025-11-25T10:30:00Z".to_string(),
        ttl: None,
        poll_interval: None,
    };

    let json = serde_json::to_value(&notification).unwrap();
    assert!(json.get("ttl").is_some(), "ttl must be present");
    assert!(json["ttl"].is_null(), "ttl must be null when None");
    assert!(
        json.get("pollInterval").is_none(),
        "pollInterval should be omitted when None"
    );
}

// ─── TaskSupport / ToolExecution Serialization ──────────────────────────────

#[test]
fn test_task_support_serialization() {
    assert_eq!(
        serde_json::to_value(TaskSupport::Forbidden).unwrap(),
        "forbidden"
    );
    assert_eq!(
        serde_json::to_value(TaskSupport::Optional).unwrap(),
        "optional"
    );
    assert_eq!(
        serde_json::to_value(TaskSupport::Required).unwrap(),
        "required"
    );
}

#[test]
fn test_task_support_round_trip() {
    for support in [
        TaskSupport::Forbidden,
        TaskSupport::Optional,
        TaskSupport::Required,
    ] {
        let json = serde_json::to_value(support).unwrap();
        let back: TaskSupport = serde_json::from_value(json).unwrap();
        assert_eq!(support, back);
    }
}

#[test]
fn test_tool_execution_with_default_support() {
    let execution = ToolExecution::default();
    let json = serde_json::to_value(&execution).unwrap();
    assert_eq!(json["taskSupport"], "forbidden");
}

#[test]
fn test_tool_execution_round_trip() {
    let execution = ToolExecution {
        task_support: TaskSupport::Required,
    };
    let json_str = serde_json::to_string(&execution).unwrap();
    let back: ToolExecution = serde_json::from_str(&json_str).unwrap();
    assert_eq!(back.task_support, TaskSupport::Required);
}

// ─── Related-task Metadata Helper ───────────────────────────────────────────

#[test]
fn test_related_task_meta_structure() {
    let meta = related_task_meta("some-id");
    let json = serde_json::to_value(&meta).unwrap();
    assert_eq!(
        json["io.modelcontextprotocol/related-task"]["taskId"],
        "some-id"
    );
}

// ─── Constants Verification ─────────────────────────────────────────────────

#[test]
fn test_meta_key_constants() {
    assert_eq!(
        RELATED_TASK_META_KEY,
        "io.modelcontextprotocol/related-task"
    );
    assert_eq!(
        MODEL_IMMEDIATE_RESPONSE_META_KEY,
        "io.modelcontextprotocol/model-immediate-response"
    );
}

#[test]
fn test_method_name_constants() {
    assert_eq!(METHOD_TASKS_GET, "tasks/get");
    assert_eq!(METHOD_TASKS_RESULT, "tasks/result");
    assert_eq!(METHOD_TASKS_LIST, "tasks/list");
    assert_eq!(METHOD_TASKS_CANCEL, "tasks/cancel");
    assert_eq!(
        METHOD_TASKS_STATUS_NOTIFICATION,
        "notifications/tasks/status"
    );
}

// ─── EmptyObject ────────────────────────────────────────────────────────────

#[test]
fn test_empty_object_serializes_to_empty_json_object() {
    let obj = EmptyObject {};
    let json = serde_json::to_value(&obj).unwrap();
    assert!(json.is_object());
    assert_eq!(json.as_object().unwrap().len(), 0);
}
