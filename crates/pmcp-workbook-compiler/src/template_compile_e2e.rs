//! WBV2-04 ACCEPTANCE proof (Plan 100-07): a REAL override-compile of the
//! Table-authored `template.xlsx` emits exactly TWO tools.
//!
//! # Why this lives in `src/` under `#[cfg(test)]` (CR-01)
//!
//! The fixture override (`compile_workbook_with_fixture_override`) is
//! `#[cfg(test)]`-only — there is NO publishable Cargo feature that arms it, so an
//! external `tests/` integration crate (which compiles as a SEPARATE crate where
//! `#[cfg(test)]` items are invisible) cannot reach it. The proof therefore lives
//! INSIDE the crate as a `#[cfg(test)]` module — the SAME reachability reason that
//! places [`crate::reemit_golden`] in `src/`. (Plan 100-07 frontmatter names an
//! external `tests/template_compile_e2e.rs`; this in-`src` placement supersedes it.)
//!
//! This is the authoritative WBV2-04 proof and the one that FAILS if `emit_bundle`
//! ever reverts to the transitional single-tool `build_cell_map` (one tool / empty
//! input_keys): it is a FULL ingest→harvest→synth→DAG→emit compile (NOT a golden
//! load) of the committed `template.xlsx`, asserting:
//!
//!   - `cell_map.tools.len() == 2`;
//!   - the sanitized tool names are exactly {`calculate_tax`, `estimate_refund`};
//!   - each tool's `input_keys` is NON-EMPTY (DAG-derived, populated);
//!   - `estimate_refund.input_keys` contains `withheld` while `calculate_tax`'s
//!     does NOT (the two key sets are disjoint on `withheld`);
//!   - the served `input_schema_for_tool` for each tool advertises a NON-EMPTY
//!     `inputs.properties` (CR-02: the served schema is never empty for a
//!     production-compiled tool).
//!
//! `template.xlsx` is RAW `ExcelTrusted` but carries `fullCalcOnLoad=1` (the same
//! staleness signal `tax-calc.xlsx` carries); the `#[cfg(test)]` override demotes
//! it to a Warning. The override CANNOT weaken provenance refusal — see
//! [`crate::reemit_golden`].

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use pmcp_server_toolkit::workbook::schema::input_schema_for_tool;
use pmcp_server_toolkit::workbook::{load_bundle, LocalDirSource};
use pmcp_workbook_runtime::{sanitize_tool_name, WorkbookBundle};

use crate::compile_workbook_with_fixture_override;

/// The committed Table-authored template (`tests/fixtures/template.xlsx`).
fn committed_template() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/template.xlsx")
}

/// Place the committed `template.xlsx` into a fresh scratch dir, compile it via the
/// `#[cfg(test)]` trusted-fixture override, and load the emitted bundle through the
/// Phase 92 fail-closed toolkit loader.
fn compile_template() -> (tempfile::TempDir, WorkbookBundle) {
    let scratch = tempfile::TempDir::new().expect("scratch dir");
    let xlsx = scratch.path().join("template.xlsx");
    std::fs::copy(committed_template(), &xlsx).expect("copy committed template");

    let out_root = scratch.path().join("out");
    std::fs::create_dir_all(&out_root).expect("out root");
    compile_workbook_with_fixture_override(
        &xlsx,
        &out_root,
        "tax-suite",
        "1.0.0",
        "proof-approver",
    )
    .expect("compile template.xlsx via the trusted-fixture override (WBV2-04)");

    let bundle_dir = out_root.join("tax-suite@1.0.0");
    let source = LocalDirSource::new(&bundle_dir);
    let bundle = load_bundle(&source).expect("the emitted template bundle loads via the toolkit");
    (scratch, bundle)
}

#[test]
fn template_compile_emits_two_tools() {
    let (_scratch, bundle) = compile_template();
    assert_eq!(
        bundle.cell_map.tools.len(),
        2,
        "a Table-authored compile emits ONE tool per output Table (Calculate_Tax + \
         Estimate_Refund) — NOT one workflow-named tool (WBV2-04 / CR-01)"
    );
}

#[test]
fn template_compile_tool_names_are_calculate_tax_and_estimate_refund() {
    let (_scratch, bundle) = compile_template();
    let names: BTreeSet<String> = bundle
        .cell_map
        .tools
        .iter()
        .map(|t| sanitize_tool_name(&t.name).expect("each tool name sanitizes"))
        .collect();
    let expected: BTreeSet<String> = ["calculate_tax".to_string(), "estimate_refund".to_string()]
        .into_iter()
        .collect();
    assert_eq!(
        names, expected,
        "the two output Tables sanitize to exactly {{calculate_tax, estimate_refund}}"
    );
}

#[test]
fn template_compile_input_keys_are_populated_and_disjoint_on_withheld() {
    let (_scratch, bundle) = compile_template();

    let tool_by_name = |sanitized: &str| {
        bundle
            .cell_map
            .tools
            .iter()
            .find(|t| sanitize_tool_name(&t.name).as_deref() == Ok(sanitized))
            .unwrap_or_else(|| panic!("tool {sanitized} present"))
    };

    let calc = tool_by_name("calculate_tax");
    let refund = tool_by_name("estimate_refund");

    // Each tool carries a NON-EMPTY, DAG-derived input_keys (the gap CR-01 left as
    // always-empty on the production path).
    assert!(
        !calc.input_keys.is_empty(),
        "calculate_tax.input_keys is populated: {:?}",
        calc.input_keys
    );
    assert!(
        !refund.input_keys.is_empty(),
        "estimate_refund.input_keys is populated: {:?}",
        refund.input_keys
    );

    // Disjoint on `withheld`: estimate_refund HAS it, calculate_tax does NOT.
    assert!(
        refund.input_keys.iter().any(|k| k == "withheld"),
        "estimate_refund consumes withheld: {:?}",
        refund.input_keys
    );
    assert!(
        !calc.input_keys.iter().any(|k| k == "withheld"),
        "calculate_tax does NOT consume withheld (disjoint): {:?}",
        calc.input_keys
    );
}

#[test]
fn template_compile_served_schema_is_non_empty_per_tool() {
    let (_scratch, bundle) = compile_template();
    for tool in &bundle.cell_map.tools {
        let schema = input_schema_for_tool(&bundle.manifest, &bundle.cell_map, tool);
        let props = schema["properties"]["inputs"]["properties"]
            .as_object()
            .expect("inputs.properties is an object");
        assert!(
            !props.is_empty(),
            "the served per-tool input schema for {} advertises a NON-EMPTY \
             inputs.properties (CR-02 closed on the production path)",
            tool.name
        );
        // The strict envelope (V5) survives on the production tool shape.
        assert_eq!(
            schema["properties"]["inputs"]["additionalProperties"],
            serde_json::json!(false),
            "the strict per-tool envelope is preserved for {}",
            tool.name
        );
    }
}
