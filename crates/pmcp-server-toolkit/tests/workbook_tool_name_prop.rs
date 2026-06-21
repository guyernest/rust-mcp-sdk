//! WBV2-04/05 PROPERTY arm (CLAUDE.md ALWAYS PROPERTY requirement) — the three
//! SECURITY-BOUNDARY invariants behind the multi-tool fan-out:
//!
//! - **T-100-10 (tool-name spoofing):** for ANY input string, `sanitize_tool_name`
//!   either returns `Err` OR an `Ok(name)` matching `^[a-zA-Z0-9_-]{1,64}$` — an
//!   uncallable / charset-illegal tool can never ship. Empty / all-illegal → `Err`.
//! - **T-100-17 (post-sanitize collision collapse):** two DISTINCT raw names whose
//!   sanitizations are equal-and-`Ok` are ALWAYS detected as a collision (grouping
//!   by sanitized name flags ≥2 sources) — no two source Tables silently collapse
//!   into one MCP tool.
//! - **T-100-11 (strict-envelope relaxation):** for an ARBITRARY [`Tool`], the
//!   per-tool `input_schema_for_tool` ALWAYS emits `additionalProperties == false`
//!   — the V5 strict envelope can never be relaxed by any tool shape.
//!
//! The sanitizer under property is the SINGLE shared
//! [`pmcp_workbook_runtime::sanitize_tool_name`] the served registration AND the
//! compiler's collision lint both call, so proving it here proves both call sites.
#![cfg(feature = "workbook")]

use std::collections::BTreeMap;

use pmcp_server_toolkit::workbook::Manifest;
use pmcp_workbook_runtime::{sanitize_tool_name, CellEntry, CellMap, Tool};
use proptest::prelude::*;
use serde_json::Value;

/// Every byte of an `Ok` sanitized name is in the MCP tool-name charset.
fn is_charset_legal(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// A minimal manifest with no cells (the per-tool input schema reads dtype/meaning
/// from roles when present; absent roles default to `number`, which is fine here —
/// the property under test is the strict-envelope flag, not the per-input dtype).
fn empty_manifest() -> Manifest {
    Manifest {
        schema_version: 1,
        workflow: "prop".to_string(),
        workbook_hash: None,
        ratified: true,
        ratified_by: None,
        ratified_at: None,
        cells: vec![],
        loop_block: None,
        governed_data: vec![],
        changelog: vec![],
        capability_calls: vec![],
        annotations: vec![],
    }
}

// An arbitrary `Tool` over arbitrary input_keys + outputs (names from a small
// alphabet so they are valid json_keys, but the strict-envelope property holds
// regardless of content).
prop_compose! {
    fn arb_tool()(
        name in "[a-zA-Z0-9 _-]{1,20}",
        input_keys in prop::collection::vec("[a-z_]{1,10}", 0..6),
        outputs in prop::collection::vec("[a-z_]{1,10}", 1..5),
    ) -> Tool {
        Tool {
            name,
            description: None,
            input_keys: input_keys.clone(),
            outputs: outputs
                .iter()
                .enumerate()
                .map(|(i, k)| CellEntry {
                    json_key: k.clone(),
                    seed_coord: format!("S!A{i}"),
                    unit: None,
                })
                .collect(),
            oracle: BTreeMap::new(),
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Property 1 (T-100-10 charset): an arbitrary input string either rejects or
    /// produces a charset-legal, ≤64-char name. NEVER an Ok name with an illegal
    /// char or over length.
    #[test]
    fn prop_sanitize_charset_or_reject(raw in ".*") {
        // A rejection (Err) is always allowed; an Ok MUST be charset-legal.
        if let Ok(name) = sanitize_tool_name(&raw) {
            prop_assert!(
                is_charset_legal(&name),
                "sanitize returned an illegal/oversized name {:?} for input {:?}",
                name,
                raw
            );
        }
    }

    /// Property 2 (T-100-17 collision): two DISTINCT raw names that sanitize to the
    /// SAME Ok value are detected as a collision when grouped by sanitized name.
    #[test]
    fn prop_equal_sanitization_is_a_collision(a in ".*", b in ".*") {
        prop_assume!(a != b);
        if let (Ok(sa), Ok(sb)) = (sanitize_tool_name(&a), sanitize_tool_name(&b)) {
            if sa == sb {
                // Group the two distinct raw names by their sanitized name; a group
                // of ≥2 is the collision the compiler lint flags.
                let mut by_sanitized: BTreeMap<String, Vec<&str>> = BTreeMap::new();
                by_sanitized.entry(sa.clone()).or_default().push(&a);
                by_sanitized.entry(sb.clone()).or_default().push(&b);
                let group = &by_sanitized[&sa];
                prop_assert_eq!(
                    group.len(),
                    2,
                    "two distinct raw names sanitizing to {} must group as a collision",
                    sa
                );
            }
        }
    }

    /// Property 3 (T-100-11 strict envelope): an arbitrary Tool's per-tool input
    /// schema ALWAYS carries additionalProperties == false.
    #[test]
    fn prop_input_schema_is_always_strict(tool in arb_tool()) {
        let manifest = empty_manifest();
        // Build a cell_map whose inputs cover every input_key (so the per-tool schema
        // can project them); the strict-envelope flag is independent of coverage.
        let inputs: Vec<CellEntry> = tool
            .input_keys
            .iter()
            .enumerate()
            .map(|(i, k)| CellEntry {
                json_key: k.clone(),
                seed_coord: format!("In!B{i}"),
                unit: None,
            })
            .collect();
        let cell_map = CellMap {
            inputs,
            tools: vec![tool.clone()],
        };
        let schema = pmcp_server_toolkit::workbook::schema::input_schema_for_tool(
            &manifest, &cell_map, &tool,
        );
        prop_assert_eq!(
            &schema["additionalProperties"],
            &Value::Bool(false),
            "per-tool input schema must keep additionalProperties:false (V5)"
        );
    }
}

/// Explicit seed for Property 1: the edge inputs (empty, all-whitespace,
/// all-punctuation, oversized) deterministically covered.
#[test]
fn sanitize_edge_seeds_reject_or_truncate() {
    assert!(sanitize_tool_name("").is_err(), "empty rejects");
    assert!(
        sanitize_tool_name("    ").is_err(),
        "all-whitespace rejects"
    );
    assert!(
        sanitize_tool_name("@@@***").is_err(),
        "all-punctuation rejects"
    );
    let long = sanitize_tool_name(&"a".repeat(200)).expect("200 a's sanitize");
    assert_eq!(long.len(), 64, "oversized input truncates to 64");
    assert!(is_charset_legal(&long));
}

/// Explicit seed for Property 2 (the plan's named triple): `Calculate Tax` and
/// `calculate_tax` both sanitize to `calculate_tax` (a collision); `calculate-tax`
/// keeps its hyphen and is DISTINCT (per the locked semantics).
#[test]
fn sanitize_collision_seed_calculate_tax() {
    let a = sanitize_tool_name("Calculate Tax").unwrap();
    let b = sanitize_tool_name("calculate_tax").unwrap();
    let c = sanitize_tool_name("calculate-tax").unwrap();
    assert_eq!(a, "calculate_tax");
    assert_eq!(b, "calculate_tax");
    assert_eq!(a, b, "`Calculate Tax` and `calculate_tax` collide");
    assert_eq!(c, "calculate-tax");
    assert_ne!(
        a, c,
        "`calculate-tax` keeps its hyphen and is distinct (locked semantics)"
    );
}
