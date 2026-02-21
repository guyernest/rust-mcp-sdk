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

use proptest::prelude::*;

use pmcp_tasks::domain::TaskRecord;
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
