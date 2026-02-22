//! Property-based tests and fuzz deserialization tests using proptest.
//!
//! Satisfies the CLAUDE.md ALWAYS requirements for:
//! - **PROPERTY testing**: Invariant verification with proptest
//! - **FUZZ testing**: Fuzz-style deserialization via proptest (preferred over cargo-fuzz
//!   because it does not require nightly Rust, integrates with the standard test harness,
//!   and provides shrinking for minimal failure cases)
//!
//! Property tests verify state machine invariants, serde round-trip stability,
//! and TTL correctness under arbitrary inputs. Fuzz tests verify that Task and
//! TaskStatus handle arbitrary JSON inputs without panicking.

use std::collections::{HashMap, HashSet};

use proptest::prelude::*;
use serde_json::Value;

use pmcp_tasks::domain::TaskRecord;
use pmcp_tasks::security::TaskSecurityConfig;
use pmcp_tasks::store::memory::InMemoryTaskStore;
use pmcp_tasks::store::TaskStore;
use pmcp_tasks::TaskError;
use pmcp_tasks::{Task, TaskStatus};

// ─── Arbitrary Strategies ───────────────────────────────────────────────────

fn arb_task_status() -> impl Strategy<Value = TaskStatus> {
    prop::sample::select(vec![
        TaskStatus::Working,
        TaskStatus::InputRequired,
        TaskStatus::Completed,
        TaskStatus::Failed,
        TaskStatus::Cancelled,
    ])
}

fn arb_task() -> impl Strategy<Value = Task> {
    (
        "[a-f0-9]{8}-[a-f0-9]{4}-4[a-f0-9]{3}-[89ab][a-f0-9]{3}-[a-f0-9]{12}", // uuid-like
        arb_task_status(),
        proptest::option::of("[a-zA-Z0-9 ]{0,100}"),    // status_message
        "2025-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z", // created_at
        "2025-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z", // last_updated_at
        proptest::option::of(0u64..=86_400_000u64),      // ttl
        proptest::option::of(1000u64..=60_000u64),       // poll_interval
    )
        .prop_map(
            |(task_id, status, status_message, created_at, last_updated_at, ttl, poll_interval)| {
                Task {
                    task_id,
                    status,
                    status_message,
                    created_at,
                    last_updated_at,
                    ttl,
                    poll_interval,
                    _meta: None,
                }
            },
        )
}

// ─── Property Tests: State Machine Invariants ───────────────────────────────

proptest! {
    /// Terminal states (Completed, Failed, Cancelled) reject ALL transitions.
    #[test]
    fn terminal_states_reject_all_transitions(
        from in prop::sample::select(vec![
            TaskStatus::Completed,
            TaskStatus::Failed,
            TaskStatus::Cancelled,
        ]),
        to in arb_task_status(),
    ) {
        prop_assert!(!from.can_transition_to(&to));
    }

    /// No status can transition to itself (self-transitions rejected).
    #[test]
    fn no_self_transitions(status in arb_task_status()) {
        prop_assert!(!status.can_transition_to(&status));
    }

    /// is_terminal() returns true if and only if can_transition_to returns
    /// false for ALL possible target statuses.
    #[test]
    fn is_terminal_iff_no_valid_transitions(status in arb_task_status()) {
        let all_statuses = [
            TaskStatus::Working,
            TaskStatus::InputRequired,
            TaskStatus::Completed,
            TaskStatus::Failed,
            TaskStatus::Cancelled,
        ];
        let has_any_transition = all_statuses.iter().any(|t| status.can_transition_to(t));
        prop_assert_eq!(status.is_terminal(), !has_any_transition);
    }
}

// ─── Property Tests: Serde Round-trip ───────────────────────────────────────

proptest! {
    /// Arbitrary TaskStatus round-trips through serde_json without data loss.
    #[test]
    fn task_status_serde_round_trip(status in arb_task_status()) {
        let json = serde_json::to_value(status).unwrap();
        let back: TaskStatus = serde_json::from_value(json).unwrap();
        prop_assert_eq!(status, back);
    }

    /// Arbitrary Task round-trips through serde_json without data loss.
    /// Verifies that None ttl round-trips through null correctly.
    #[test]
    fn task_serde_round_trip(task in arb_task()) {
        let json = serde_json::to_value(&task).unwrap();
        let back: Task = serde_json::from_value(json).unwrap();

        prop_assert_eq!(&task.task_id, &back.task_id);
        prop_assert_eq!(task.status, back.status);
        prop_assert_eq!(&task.status_message, &back.status_message);
        prop_assert_eq!(&task.created_at, &back.created_at);
        prop_assert_eq!(&task.last_updated_at, &back.last_updated_at);
        prop_assert_eq!(task.ttl, back.ttl);
        prop_assert_eq!(task.poll_interval, back.poll_interval);
    }
}

// ─── Property Tests: TTL / TaskRecord ───────────────────────────────────────

proptest! {
    /// A freshly created TaskRecord with any valid TTL (including None)
    /// is never expired. Extremely large TTL values that overflow DateTime
    /// arithmetic are treated as "never expires" (expires_at = None).
    #[test]
    fn fresh_task_record_is_not_expired(ttl in proptest::option::of(0u64..=u64::MAX)) {
        let record = TaskRecord::new(
            "owner".to_string(),
            "tools/call".to_string(),
            ttl,
        );
        prop_assert!(!record.is_expired());
    }
}

// ─── Fuzz Deserialization: TaskStatus from Arbitrary Strings ────────────────

proptest! {
    /// Deserializing arbitrary strings as TaskStatus either succeeds with
    /// a valid variant or fails without panicking (no unwinding panics).
    #[test]
    fn fuzz_task_status_deserialization(s in "\\PC*") {
        let json_str = format!(
            "\"{}\"",
            s.replace('\\', "\\\\").replace('"', "\\\"")
        );
        // Must not panic -- Ok or Err are both fine
        let _ = serde_json::from_str::<TaskStatus>(&json_str);
    }
}

// ─── Fuzz Deserialization: Task from Arbitrary Bytes ────────────────────────

proptest! {
    /// Deserializing arbitrary bytes as Task must not panic.
    #[test]
    fn fuzz_task_deserialization_from_bytes(
        bytes in proptest::collection::vec(any::<u8>(), 0..1024)
    ) {
        // Must not panic -- Ok or Err are both fine
        let _ = serde_json::from_slice::<Task>(&bytes);
    }
}

// ─── Fuzz Deserialization: Task from Arbitrary Strings ──────────────────────

proptest! {
    /// Deserializing arbitrary strings as Task must not panic.
    #[test]
    fn fuzz_task_deserialization_from_json_string(s in "\\PC{0,512}") {
        // Must not panic -- Ok or Err are both fine
        let _ = serde_json::from_str::<Task>(&s);
    }
}

// === Phase 2: Store-level property tests ===

// ─── Phase 2 Arbitrary Strategies ───────────────────────────────────────────

/// Generates non-empty alphanumeric owner IDs (1..20 chars), excluding
/// DEFAULT_LOCAL_OWNER ("local") to avoid anonymous access confusion.
fn arb_owner() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9]{1,20}".prop_filter("must not be DEFAULT_LOCAL_OWNER", |s| s != "local")
}

/// Generates a variable map with 0..10 entries. Values are strings, numbers,
/// bools, or null.
fn arb_variable_map() -> impl Strategy<Value = HashMap<String, Value>> {
    proptest::collection::hash_map(
        "[a-zA-Z_][a-zA-Z0-9_]{0,15}",
        prop_oneof![
            "[a-zA-Z0-9 ]{0,20}".prop_map(Value::String),
            (0i64..1000).prop_map(|n| Value::Number(n.into())),
            any::<bool>().prop_map(Value::Bool),
            Just(Value::Null),
        ],
        0..10,
    )
}

/// Generates a sequence of valid state machine transitions starting from Working.
fn arb_valid_transition_sequence() -> impl Strategy<Value = Vec<TaskStatus>> {
    // Valid targets from non-terminal states
    let non_terminal_targets = vec![
        TaskStatus::InputRequired,
        TaskStatus::Completed,
        TaskStatus::Failed,
        TaskStatus::Cancelled,
    ];

    // Generate 0..5 transitions, each valid from the current state
    proptest::collection::vec(
        prop::sample::select(non_terminal_targets),
        0..5,
    )
    .prop_map(|targets| {
        let mut sequence = Vec::new();
        let mut current = TaskStatus::Working;

        for target in targets {
            if current.can_transition_to(&target) {
                sequence.push(target);
                current = target;
            }
            // If terminal, stop adding transitions
            if current.is_terminal() {
                break;
            }
        }
        sequence
    })
}

/// Helper: creates a store with anonymous access enabled and high task limit.
fn prop_test_store() -> InMemoryTaskStore {
    InMemoryTaskStore::new().with_security(
        TaskSecurityConfig::default()
            .with_max_tasks_per_owner(1000)
            .with_allow_anonymous(true),
    )
}

// ─── Phase 2 Property Tests: Store-level Invariants ─────────────────────────

proptest! {
    /// For arbitrary valid transition sequences, InMemoryTaskStore accepts
    /// them all and the final status matches the last transition applied.
    #[test]
    fn prop_state_machine_transitions_consistent_through_store(
        transitions in arb_valid_transition_sequence()
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let store = prop_test_store();
            let record = store.create("prop-owner", "tools/call", None).await.unwrap();
            let task_id = record.task.task_id.clone();

            let mut expected_status = TaskStatus::Working;
            for target in &transitions {
                let result = store
                    .update_status(&task_id, "prop-owner", *target, None)
                    .await;
                prop_assert!(result.is_ok(), "transition to {target} should succeed from {expected_status}");
                expected_status = *target;
            }

            // Verify final state matches
            let final_record = store.get(&task_id, "prop-owner").await.unwrap();
            prop_assert_eq!(final_record.task.status, expected_status);

            Ok(())
        })?;
    }

    /// For arbitrary variable maps with some null values, after set_variables
    /// the null keys are absent and non-null keys are present with correct values.
    #[test]
    fn prop_variable_merge_null_deletes_and_preserves_others(
        vars in arb_variable_map()
    ) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let store = prop_test_store();
            let record = store.create("prop-owner", "tools/call", None).await.unwrap();
            let task_id = record.task.task_id.clone();

            let result = store.set_variables(&task_id, "prop-owner", vars.clone()).await;

            // Variable size check might fail for very large maps, which is expected
            if let Ok(updated) = result {
                for (key, value) in &vars {
                    if value.is_null() {
                        prop_assert!(
                            !updated.variables.contains_key(key),
                            "null key '{key}' should be deleted"
                        );
                    } else {
                        prop_assert_eq!(
                            updated.variables.get(key),
                            Some(value),
                            "non-null key should be present"
                        );
                    }
                }
            }

            Ok(())
        })?;
    }

    /// Creating N tasks (N in 1..50) always produces unique task IDs.
    #[test]
    fn prop_task_ids_always_unique(n in 1usize..50) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let store = prop_test_store();
            let mut ids = HashSet::new();

            for _ in 0..n {
                let record = store.create("prop-owner", "tools/call", None).await.unwrap();
                ids.insert(record.task.task_id);
            }

            prop_assert_eq!(ids.len(), n, "all task IDs must be unique");

            Ok(())
        })?;
    }

    /// For arbitrary (owner_a, owner_b) where a != b, a task created by
    /// owner_a returns NotFound when accessed by owner_b.
    #[test]
    fn prop_owner_isolation_holds_for_arbitrary_owners(
        owner_a in arb_owner(),
        owner_b in arb_owner(),
    ) {
        // Skip if owners happen to be the same
        prop_assume!(owner_a != owner_b);

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let store = prop_test_store();
            let record = store.create(&owner_a, "tools/call", None).await.unwrap();
            let task_id = record.task.task_id.clone();

            // owner_b should get NotFound for all operations
            let get_result = store.get(&task_id, &owner_b).await;
            prop_assert!(
                matches!(get_result, Err(TaskError::NotFound { .. })),
                "get by wrong owner should return NotFound, got: {get_result:?}"
            );

            let cancel_result = store.cancel(&task_id, &owner_b).await;
            prop_assert!(
                matches!(cancel_result, Err(TaskError::NotFound { .. })),
                "cancel by wrong owner should return NotFound, got: {cancel_result:?}"
            );

            let update_result = store
                .update_status(&task_id, &owner_b, TaskStatus::Completed, None)
                .await;
            prop_assert!(
                matches!(update_result, Err(TaskError::NotFound { .. })),
                "update_status by wrong owner should return NotFound, got: {update_result:?}"
            );

            Ok(())
        })?;
    }
}
