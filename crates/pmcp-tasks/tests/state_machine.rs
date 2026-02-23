//! State machine transition tests (TEST-02).
//!
//! Verifies the TaskStatus state machine correctly validates all transitions:
//! 8 valid, 5 self-transition rejections, 12 terminal-state rejections.
//! Covers the full 5x5 transition matrix exhaustively.

// Imports are in sub-modules to avoid ambiguity with pretty_assertions.

// ─── is_terminal Tests ──────────────────────────────────────────────────────

mod is_terminal {
    use pmcp_tasks::TaskStatus;

    #[test]
    fn working_is_not_terminal() {
        assert!(!TaskStatus::Working.is_terminal());
    }

    #[test]
    fn input_required_is_not_terminal() {
        assert!(!TaskStatus::InputRequired.is_terminal());
    }

    #[test]
    fn completed_is_terminal() {
        assert!(TaskStatus::Completed.is_terminal());
    }

    #[test]
    fn failed_is_terminal() {
        assert!(TaskStatus::Failed.is_terminal());
    }

    #[test]
    fn cancelled_is_terminal() {
        assert!(TaskStatus::Cancelled.is_terminal());
    }
}

// ─── Valid Transitions (8 total) ────────────────────────────────────────────

mod valid_transitions {
    use pmcp_tasks::TaskStatus;

    #[test]
    fn working_to_input_required() {
        assert!(TaskStatus::Working.can_transition_to(&TaskStatus::InputRequired));
        assert!(TaskStatus::Working
            .validate_transition("t1", &TaskStatus::InputRequired)
            .is_ok());
    }

    #[test]
    fn working_to_completed() {
        assert!(TaskStatus::Working.can_transition_to(&TaskStatus::Completed));
        assert!(TaskStatus::Working
            .validate_transition("t1", &TaskStatus::Completed)
            .is_ok());
    }

    #[test]
    fn working_to_failed() {
        assert!(TaskStatus::Working.can_transition_to(&TaskStatus::Failed));
        assert!(TaskStatus::Working
            .validate_transition("t1", &TaskStatus::Failed)
            .is_ok());
    }

    #[test]
    fn working_to_cancelled() {
        assert!(TaskStatus::Working.can_transition_to(&TaskStatus::Cancelled));
        assert!(TaskStatus::Working
            .validate_transition("t1", &TaskStatus::Cancelled)
            .is_ok());
    }

    #[test]
    fn input_required_to_working() {
        assert!(TaskStatus::InputRequired.can_transition_to(&TaskStatus::Working));
        assert!(TaskStatus::InputRequired
            .validate_transition("t1", &TaskStatus::Working)
            .is_ok());
    }

    #[test]
    fn input_required_to_completed() {
        assert!(TaskStatus::InputRequired.can_transition_to(&TaskStatus::Completed));
        assert!(TaskStatus::InputRequired
            .validate_transition("t1", &TaskStatus::Completed)
            .is_ok());
    }

    #[test]
    fn input_required_to_failed() {
        assert!(TaskStatus::InputRequired.can_transition_to(&TaskStatus::Failed));
        assert!(TaskStatus::InputRequired
            .validate_transition("t1", &TaskStatus::Failed)
            .is_ok());
    }

    #[test]
    fn input_required_to_cancelled() {
        assert!(TaskStatus::InputRequired.can_transition_to(&TaskStatus::Cancelled));
        assert!(TaskStatus::InputRequired
            .validate_transition("t1", &TaskStatus::Cancelled)
            .is_ok());
    }
}

// ─── Invalid Transitions: Self-transitions (5 total) ────────────────────────

mod self_transitions {
    use pmcp_tasks::TaskStatus;

    #[test]
    fn working_to_working_rejected() {
        assert!(!TaskStatus::Working.can_transition_to(&TaskStatus::Working));
        assert!(TaskStatus::Working
            .validate_transition("t1", &TaskStatus::Working)
            .is_err());
    }

    #[test]
    fn input_required_to_input_required_rejected() {
        assert!(!TaskStatus::InputRequired.can_transition_to(&TaskStatus::InputRequired));
        assert!(TaskStatus::InputRequired
            .validate_transition("t1", &TaskStatus::InputRequired)
            .is_err());
    }

    #[test]
    fn completed_to_completed_rejected() {
        assert!(!TaskStatus::Completed.can_transition_to(&TaskStatus::Completed));
        assert!(TaskStatus::Completed
            .validate_transition("t1", &TaskStatus::Completed)
            .is_err());
    }

    #[test]
    fn failed_to_failed_rejected() {
        assert!(!TaskStatus::Failed.can_transition_to(&TaskStatus::Failed));
        assert!(TaskStatus::Failed
            .validate_transition("t1", &TaskStatus::Failed)
            .is_err());
    }

    #[test]
    fn cancelled_to_cancelled_rejected() {
        assert!(!TaskStatus::Cancelled.can_transition_to(&TaskStatus::Cancelled));
        assert!(TaskStatus::Cancelled
            .validate_transition("t1", &TaskStatus::Cancelled)
            .is_err());
    }
}

// ─── Invalid Transitions: From Terminal States (12 total) ───────────────────

mod from_terminal_states {
    use pmcp_tasks::TaskStatus;

    // Completed -> {Working, InputRequired, Failed, Cancelled}
    #[test]
    fn completed_to_working() {
        assert!(!TaskStatus::Completed.can_transition_to(&TaskStatus::Working));
    }

    #[test]
    fn completed_to_input_required() {
        assert!(!TaskStatus::Completed.can_transition_to(&TaskStatus::InputRequired));
    }

    #[test]
    fn completed_to_failed() {
        assert!(!TaskStatus::Completed.can_transition_to(&TaskStatus::Failed));
    }

    #[test]
    fn completed_to_cancelled() {
        assert!(!TaskStatus::Completed.can_transition_to(&TaskStatus::Cancelled));
    }

    // Failed -> {Working, InputRequired, Completed, Cancelled}
    #[test]
    fn failed_to_working() {
        assert!(!TaskStatus::Failed.can_transition_to(&TaskStatus::Working));
    }

    #[test]
    fn failed_to_input_required() {
        assert!(!TaskStatus::Failed.can_transition_to(&TaskStatus::InputRequired));
    }

    #[test]
    fn failed_to_completed() {
        assert!(!TaskStatus::Failed.can_transition_to(&TaskStatus::Completed));
    }

    #[test]
    fn failed_to_cancelled() {
        assert!(!TaskStatus::Failed.can_transition_to(&TaskStatus::Cancelled));
    }

    // Cancelled -> {Working, InputRequired, Completed, Failed}
    #[test]
    fn cancelled_to_working() {
        assert!(!TaskStatus::Cancelled.can_transition_to(&TaskStatus::Working));
    }

    #[test]
    fn cancelled_to_input_required() {
        assert!(!TaskStatus::Cancelled.can_transition_to(&TaskStatus::InputRequired));
    }

    #[test]
    fn cancelled_to_completed() {
        assert!(!TaskStatus::Cancelled.can_transition_to(&TaskStatus::Completed));
    }

    #[test]
    fn cancelled_to_failed() {
        assert!(!TaskStatus::Cancelled.can_transition_to(&TaskStatus::Failed));
    }
}

// ─── Exhaustive 5x5 Matrix Test ────────────────────────────────────────────

mod exhaustive_matrix {
    use pmcp_tasks::TaskStatus;

    /// Build the expected transition table as a 5x5 bool matrix.
    /// Rows: from status, Columns: to status.
    /// Order: Working, InputRequired, Completed, Failed, Cancelled
    fn expected_transitions() -> [[bool; 5]; 5] {
        [
            // Working ->
            [false, true, true, true, true],
            // InputRequired ->
            [true, false, true, true, true],
            // Completed ->
            [false, false, false, false, false],
            // Failed ->
            [false, false, false, false, false],
            // Cancelled ->
            [false, false, false, false, false],
        ]
    }

    fn all_statuses() -> [TaskStatus; 5] {
        [
            TaskStatus::Working,
            TaskStatus::InputRequired,
            TaskStatus::Completed,
            TaskStatus::Failed,
            TaskStatus::Cancelled,
        ]
    }

    #[test]
    fn full_5x5_matrix() {
        let expected = expected_transitions();
        let statuses = all_statuses();

        for (from_idx, from) in statuses.iter().enumerate() {
            for (to_idx, to) in statuses.iter().enumerate() {
                let can = from.can_transition_to(to);
                assert_eq!(
                    can, expected[from_idx][to_idx],
                    "Transition {from} -> {to}: expected {}, got {can}",
                    expected[from_idx][to_idx]
                );
            }
        }
    }

    #[test]
    fn matrix_covers_all_25_cells() {
        let statuses = all_statuses();
        let mut checked = 0;
        for from in &statuses {
            for to in &statuses {
                // Just calling can_transition_to verifies it doesn't panic
                let _ = from.can_transition_to(to);
                checked += 1;
            }
        }
        assert_eq!(checked, 25, "Must check all 25 cells");
    }
}

// ─── validate_transition Error Quality ──────────────────────────────────────

mod error_quality {
    use pmcp_tasks::{TaskError, TaskStatus};

    #[test]
    fn error_contains_correct_task_id() {
        let err = TaskStatus::Completed
            .validate_transition("task-abc-123", &TaskStatus::Working)
            .unwrap_err();

        let msg = err.to_string();
        assert!(
            msg.contains("task-abc-123"),
            "error should contain task_id, got: {msg}"
        );
    }

    #[test]
    fn error_contains_from_and_to_status() {
        let err = TaskStatus::Working
            .validate_transition("t1", &TaskStatus::Working)
            .unwrap_err();

        let msg = err.to_string();
        assert!(
            msg.contains("working"),
            "error should contain from status, got: {msg}"
        );
    }

    #[test]
    fn error_display_is_readable() {
        let err = TaskStatus::Completed
            .validate_transition("my-task", &TaskStatus::Working)
            .unwrap_err();

        let msg = err.to_string();
        // Should contain: "invalid transition from completed to working for task my-task"
        assert!(msg.contains("invalid transition"), "got: {msg}");
        assert!(msg.contains("completed"), "got: {msg}");
        assert!(msg.contains("working"), "got: {msg}");
        assert!(msg.contains("my-task"), "got: {msg}");
    }

    #[test]
    fn terminal_state_error_has_suggested_action() {
        let err = TaskStatus::Completed
            .validate_transition("t1", &TaskStatus::Working)
            .unwrap_err();

        // The error should be TaskError::InvalidTransition with suggested_action
        match err {
            TaskError::InvalidTransition {
                task_id,
                from,
                to,
                suggested_action,
            } => {
                assert_eq!(task_id, "t1");
                assert_eq!(from, TaskStatus::Completed);
                assert_eq!(to, TaskStatus::Working);
                assert!(
                    suggested_action.is_some(),
                    "terminal state transition should have suggested_action"
                );
                let action = suggested_action.unwrap();
                assert!(
                    action.contains("terminal"),
                    "suggested_action should mention terminal, got: {action}"
                );
            },
            other => panic!("expected InvalidTransition, got: {other:?}"),
        }
    }

    #[test]
    fn self_transition_error_has_suggested_action() {
        let err = TaskStatus::Working
            .validate_transition("t2", &TaskStatus::Working)
            .unwrap_err();

        match err {
            TaskError::InvalidTransition {
                task_id,
                from,
                to,
                suggested_action,
            } => {
                assert_eq!(task_id, "t2");
                assert_eq!(from, TaskStatus::Working);
                assert_eq!(to, TaskStatus::Working);
                assert!(
                    suggested_action.is_some(),
                    "self-transition should have suggested_action"
                );
                let action = suggested_action.unwrap();
                assert!(
                    action.contains("already"),
                    "suggested_action should mention 'already', got: {action}"
                );
            },
            other => panic!("expected InvalidTransition, got: {other:?}"),
        }
    }
}

// ─── TaskRecord Constructor Tests ───────────────────────────────────────────

mod task_record {
    use pmcp_tasks::domain::TaskRecord;
    use pmcp_tasks::TaskStatus;

    #[test]
    fn new_creates_working_state() {
        let record = TaskRecord::new("owner".to_string(), "tools/call".to_string(), None);
        assert_eq!(record.task.status, TaskStatus::Working);
    }

    #[test]
    fn new_generates_valid_uuid_v4() {
        let record = TaskRecord::new("owner".to_string(), "tools/call".to_string(), None);
        let id = &record.task.task_id;

        // UUID v4 format: 8-4-4-4-12 hex chars = 36 total with hyphens
        assert_eq!(id.len(), 36, "UUID should be 36 chars, got: {id}");

        // Parse as UUID to verify validity
        let parsed = uuid::Uuid::parse_str(id);
        assert!(parsed.is_ok(), "should parse as valid UUID: {id}");
        assert_eq!(parsed.unwrap().get_version_num(), 4, "should be UUID v4");
    }

    #[test]
    fn new_sets_equal_timestamps() {
        let record = TaskRecord::new("owner".to_string(), "tools/call".to_string(), None);
        assert_eq!(
            record.task.created_at, record.task.last_updated_at,
            "created_at and last_updated_at should be the same initially"
        );
    }

    #[test]
    fn is_expired_false_for_fresh_task_with_ttl() {
        let record = TaskRecord::new("owner".to_string(), "tools/call".to_string(), Some(60_000));
        assert!(!record.is_expired());
    }

    #[test]
    fn is_expired_false_for_none_ttl() {
        let record = TaskRecord::new("owner".to_string(), "tools/call".to_string(), None);
        assert!(
            !record.is_expired(),
            "task with None TTL should never expire"
        );
    }
}

// ─── TaskWithVariables _meta Injection Tests ────────────────────────────────

mod task_with_variables {
    use pmcp_tasks::domain::{TaskRecord, TaskWithVariables};
    use serde_json::json;

    #[test]
    fn to_wire_task_empty_variables_no_meta_change() {
        let record = TaskRecord::new("owner".to_string(), "tools/call".to_string(), None);
        let twv = TaskWithVariables::from_record(&record);
        let wire = twv.to_wire_task();

        // No variables, so _meta should remain None
        assert!(
            wire._meta.is_none(),
            "_meta should be None when no variables"
        );
    }

    #[test]
    fn to_wire_task_with_variables_produces_meta() {
        let mut record = TaskRecord::new("owner".to_string(), "tools/call".to_string(), None);
        record.variables.insert("progress".to_string(), json!(42));
        record
            .variables
            .insert("stage".to_string(), json!("building"));

        let twv = TaskWithVariables::from_record(&record);
        let wire = twv.to_wire_task();

        let meta = wire._meta.expect("_meta should be present with variables");
        assert_eq!(meta["progress"], json!(42));
        assert_eq!(meta["stage"], json!("building"));
    }

    #[test]
    fn variables_appear_in_meta_not_as_separate_field() {
        let mut record = TaskRecord::new("owner".to_string(), "tools/call".to_string(), None);
        record
            .variables
            .insert("my_var".to_string(), json!("value"));

        let twv = TaskWithVariables::from_record(&record);
        let wire = twv.to_wire_task();

        // Serialize and verify variables are ONLY in _meta
        let json = serde_json::to_value(&wire).unwrap();
        assert!(
            json.get("variables").is_none(),
            "variables must not appear as a separate field on wire task"
        );
        assert!(json.get("_meta").is_some(), "_meta should be present");
        assert_eq!(json["_meta"]["my_var"], "value");
    }

    #[test]
    fn null_value_variables_preserved_in_meta() {
        let mut record = TaskRecord::new("owner".to_string(), "tools/call".to_string(), None);
        record
            .variables
            .insert("to_delete".to_string(), serde_json::Value::Null);

        let twv = TaskWithVariables::from_record(&record);
        let wire = twv.to_wire_task();

        let meta = wire._meta.expect("_meta should be present");
        assert!(
            meta["to_delete"].is_null(),
            "null-value variables should be preserved in _meta"
        );
    }
}
