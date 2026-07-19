//! Fuzz-hardening for the tool-result digestion + raw-parse paths (T-108-03-02).
//!
//! TWO fuzz targets, per the plan (Codex MEDIUM — also fuzz the parser entry for
//! the malformed-JSON claim, not just already-parsed Values):
//!
//! - STRUCTURAL: arbitrary `serde_json::Value` payloads flow through
//!   `digest_tool_results` / `extract_tool_calls`. Values are ALREADY parsed, so
//!   this fuzzes unusual STRUCTURE (deep nesting, odd keys) — asserting no panic
//!   and a well-formed result.
//! - RAW-BYTES: arbitrary byte strings hit the `serde_json::from_slice` boundary
//!   in `parse_completion` / `parse_tool_result` — asserting malformed bytes
//!   yield an `Err`, never a panic.

use pmcp::types::content::Role;
use pmcp::types::sampling::{CreateMessageResultWithTools, SamplingMessageContent};
use pmcp_agent::iteration::{
    digest_tool_results, extract_tool_calls, parse_completion, parse_tool_result,
};
use pmcp_agent::ToolCallResult;
use proptest::prelude::*;
use serde_json::Value;

/// A bounded, recursive arbitrary-JSON strategy (no floats — replay discipline).
fn arb_json() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        any::<i64>().prop_map(Value::from),
        ".*".prop_map(Value::String),
    ];
    leaf.prop_recursive(3, 24, 6, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..5).prop_map(Value::Array),
            prop::collection::vec(("[a-z]{1,6}", inner), 0..5)
                .prop_map(|kvs| Value::Object(kvs.into_iter().collect())),
        ]
    })
}

proptest! {
    // ---- STRUCTURAL: arbitrary already-parsed Values ----

    /// Arbitrary tool-result content must digest without panicking, preserving
    /// one output block per input and the id correlation.
    #[test]
    fn digest_survives_arbitrary_structure(ok in arb_json(), err in ".*") {
        let results = vec![
            ToolCallResult::ok("id-ok", ok),
            ToolCallResult::error("id-err", err),
        ];
        let turn = digest_tool_results(results);
        prop_assert_eq!(turn.content.len(), 2);
        // Id correlation survives digestion.
        match &turn.content[0] {
            SamplingMessageContent::ToolResult { tool_use_id, .. } => {
                prop_assert_eq!(tool_use_id, "id-ok");
            },
            other => prop_assert!(false, "expected tool_result, got {:?}", other),
        }
    }

    /// Arbitrary tool-use input must extract without panicking, preserving id.
    #[test]
    fn extract_survives_arbitrary_tool_input(input in arb_json()) {
        let result = CreateMessageResultWithTools::new(
            "m",
            Role::Assistant,
            vec![SamplingMessageContent::ToolUse {
                name: "t".into(),
                id: "corr-1".into(),
                input,
                meta: None,
            }],
        );
        let calls = extract_tool_calls(&result);
        prop_assert_eq!(calls.len(), 1);
        prop_assert_eq!(&calls[0].id, "corr-1");
    }

    // ---- RAW-BYTES: the serde_json::from_slice parser boundary ----

    /// Arbitrary bytes into the completion parser: never a panic (Err or Ok).
    #[test]
    fn parse_completion_never_panics_on_raw_bytes(bytes in prop::collection::vec(any::<u8>(), 0..256)) {
        // Reaching this line without unwinding = no panic. A well-formed random
        // byte string is astronomically unlikely to be a valid completion.
        let _ = parse_completion(&bytes);
    }

    /// Arbitrary bytes into the tool-result parser: never a panic.
    #[test]
    fn parse_tool_result_never_panics_on_raw_bytes(bytes in prop::collection::vec(any::<u8>(), 0..256)) {
        let _ = parse_tool_result(&bytes);
    }

    /// Arbitrary JSON that lacks the required completion fields must ERROR (shape
    /// mismatch), never panic.
    #[test]
    fn parse_completion_rejects_non_completion_json(v in arb_json()) {
        let bytes = serde_json::to_vec(&v).expect("serialize arb json");
        // arb_json never emits the required model/role/content triple, so this is
        // always a shape mismatch → Err.
        prop_assert!(parse_completion(&bytes).is_err());
    }
}

#[test]
fn parse_completion_errors_on_obvious_garbage() {
    assert!(parse_completion(b"\x00\x01\x02 not json").is_err());
    assert!(parse_completion(b"").is_err());
    assert!(parse_tool_result(b"<<<garbage>>>").is_err());
}
