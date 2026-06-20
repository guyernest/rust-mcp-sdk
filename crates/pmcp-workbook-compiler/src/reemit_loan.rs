//! WBEX-01 generalization gate: compiling a SECOND, non-lighthouse loan/mortgage
//! workbook through the GENERIC `compile_workbook` driver produces a bundle that
//! serves ITS OWN `get_manifest`/`tools/list` schema — behind the SAME five
//! generic tool names `tax-calc` uses, with ZERO per-workbook served Rust.
//!
//! # What this proves (the literal generalization gate)
//!
//! `tax-calc` (the lighthouse) is one workbook. A serve path that "works" only
//! because it was shaped to one workbook is not a platform. This module compiles
//! a deliberately-divergent loan workbook (a rate-tier DAG — VLOOKUP / INDEX-MATCH
//! against a constant rate table, IFERROR guards, nested-IF tiering, ROUND/CEILING
//! to currency; NO PMT/POWER/exponentiation) and asserts that the GENERIC toolkit
//! serve path projects the LOAN's own inputs/outputs:
//!
//!   1. The FIVE generic tool NAMES are UNCHANGED vs tax-calc
//!      (`calculate`/`explain`/`get_manifest`/`diff_version`/`render_workbook`) —
//!      the names are generic; only the manifest/schema PAYLOAD behind them differs.
//!   2. The SERVED input schema (`schema::input_schema_for_manifest`) contains the
//!      LOAN's input keys and NOT tax-calc's input keys.
//!   3. The SERVED output schema (`schema::output_schema_for_manifest`) carries the
//!      LOAN's `out_*` keys and NOT tax-calc's output keys, with MULTIPLE outputs
//!      and no privileged single headline (S-1).
//!   4. The SERVED `get_manifest` projection (`GetManifestHandler`/`curated_manifest`)
//!      reflects the loan's own inputs/outputs.
//!   5. The loan and tax-calc served key SETS are DISJOINT — that disjointness,
//!      read off the GENERIC-driver output (no loan-specific Rust builds the
//!      manifest), IS the generalization proof (T-96-11).
//!
//! # Why this lives in `src/` under `#[cfg(test)]` (CR-01)
//!
//! Identical to [`crate::reemit_golden`]: the trusted-fixture override
//! (`compile_workbook_with_fixture_override`) is `#[cfg(test)]`-only, so the proof
//! MUST live INSIDE the crate to reach it. No loan GOLDEN exists, so — unlike
//! `reemit_golden` — this asserts the loan's INTRINSIC structure + ITS OWN SERVED
//! schema, never equality to a second file. The production-refusal counter-test
//! (T-96-10) confirms bare `compile_workbook` (Enforce) still refuses the bytes.

#![cfg(test)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::Value;

use crate::{build_bundle_lock, compile_workbook, compile_workbook_with_fixture_override};

use pmcp_server_toolkit::workbook::handler::{
    CalculateHandler, DiffVersionHandler, ExplainHandler, GetManifestHandler, RenderWorkbookHandler,
};
use pmcp_server_toolkit::workbook::schema::{
    input_schema_for_manifest, output_schema_for_manifest,
};
use pmcp_server_toolkit::workbook::{load_bundle, LocalDirSource, WorkbookBundle};
use pmcp_workbook_runtime::Role;

/// The committed loan fixture (`tests/fixtures/loan-calc.xlsx`, Task 1).
fn committed_fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/loan-calc.xlsx")
}

/// Copy the COMMITTED loan fixture into `dir/loan-calc.xlsx`. Returns the path.
fn place_fixture(dir: &Path) -> PathBuf {
    let xlsx = dir.join("loan-calc.xlsx");
    std::fs::copy(committed_fixture(), &xlsx).expect("copy committed loan fixture");
    xlsx
}

/// Place the loan fixture into a fresh scratch dir, compile it via the
/// `#[cfg(test)]` trusted-fixture override, and return the emitted bundle dir.
fn compile_fixture() -> (tempfile::TempDir, PathBuf) {
    let scratch = tempfile::TempDir::new().expect("scratch dir");
    let xlsx = place_fixture(scratch.path());

    let out_root = scratch.path().join("out");
    std::fs::create_dir_all(&out_root).expect("out root");
    compile_workbook_with_fixture_override(
        &xlsx,
        &out_root,
        "loan-calc",
        "1.0.0",
        "proof-approver",
    )
    .expect("compile the loan fixture via the trusted-fixture override");
    let bundle = out_root.join("loan-calc@1.0.0");
    (scratch, bundle)
}

/// Load the emitted loan bundle via the GENERIC fail-closed toolkit loader — the
/// SAME entry point the served binary uses (no loan-specific Rust).
fn load_loan_bundle() -> (tempfile::TempDir, WorkbookBundle) {
    let (scratch, bundle_dir) = compile_fixture();
    let loaded =
        load_bundle(&LocalDirSource::new(&bundle_dir)).expect("the loan bundle loads via toolkit");
    (scratch, loaded)
}

// ---- The loan's own served key sets vs tax-calc's (the disjointness oracle) ----

/// The loan's served INPUT keys (the `cell_map.inputs` json_keys), authored in
/// Task 1's `loan_calc_spec` via the `in_*` named-range convention. F3: the served
/// key STRIPS the `in_` governance prefix (`in_loan_amount` → `loan_amount`); the
/// underlying `role.name` stays prefixed for named-range matching.
fn loan_input_keys() -> BTreeSet<String> {
    ["loan_amount", "term_months", "credit_score"]
        .into_iter()
        .map(String::from)
        .collect()
}

/// The loan's served OUTPUT keys (the `out_*` named ranges), authored in Task 1.
/// F3: the served key STRIPS the `out_` governance prefix.
fn loan_output_keys() -> BTreeSet<String> {
    [
        "credit_tier",
        "applied_rate",
        "monthly_interest",
        "total_interest",
        "tier_rate",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

/// Tax-calc's served INPUT keys (the CONTRAST case — must be ABSENT from the loan
/// schema). Sourced from `reemit_golden`'s known 3-input contract.
fn tax_input_keys() -> BTreeSet<String> {
    ["gross_income", "filing_status", "deductions"]
        .into_iter()
        .map(String::from)
        .collect()
}

/// Tax-calc's served OUTPUT keys (the CONTRAST case — must be ABSENT from the loan
/// schema). Sourced from the tax-calc golden's 4 named outputs.
fn tax_output_keys() -> BTreeSet<String> {
    [
        "taxable_income",
        "tax_owed",
        "effective_rate",
        "marginal_rate",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

/// Collect the property keys under a served schema's `properties.<group>.properties`.
fn schema_property_keys(schema: &Value, group: &str) -> BTreeSet<String> {
    schema["properties"][group]["properties"]
        .as_object()
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default()
}

// ---- The proof ----------------------------------------------------------------

/// (Intrinsic #1) The loan workbook compiles end-to-end through the generic driver
/// and the emitted bundle carries the seven-member contract — proving a SECOND,
/// non-lighthouse workbook rides the same producer path (no per-workbook compiler).
#[test]
fn loan_bundle_carries_seven_member_contract() {
    let (_scratch, bundle) = compile_fixture();
    for member in [
        "manifest.json",
        "executable.ir.json",
        "cell_map.json",
        "layout.json",
        "BUNDLE.lock",
        "evidence/changelog.json",
        "evidence/parser_equivalence.json",
    ] {
        assert!(
            bundle.join(member).exists(),
            "the loan bundle carries the seven-member contract: missing {member}"
        );
    }
}

/// (Intrinsic #2) The loan `BUNDLE.lock` combined hash recomputes via the runtime
/// helper, and `bundle_id` is `loan-calc` (D-17) — the same lock integrity the
/// golden gate asserts, on a workbook with no golden.
#[test]
fn loan_bundle_lock_recomputes() {
    let (_scratch, bundle) = compile_fixture();
    let lock: Value = {
        let bytes = std::fs::read(bundle.join("BUNDLE.lock")).expect("read BUNDLE.lock");
        serde_json::from_slice(&bytes).expect("parse BUNDLE.lock")
    };
    assert_eq!(
        lock["bundle_id"].as_str().expect("bundle_id"),
        "loan-calc",
        "the loan bundle is stamped with its own bundle_id"
    );

    let ir_json = std::fs::read_to_string(bundle.join("executable.ir.json")).expect("ir");
    let manifest_json = std::fs::read_to_string(bundle.join("manifest.json")).expect("manifest");
    let recomputed = build_bundle_lock(
        lock["bundle_id"].as_str().expect("bundle_id"),
        lock["version"].as_str().expect("version"),
        lock["workbook_hash"]
            .as_str()
            .expect("workbook_hash")
            .to_string(),
        &ir_json,
        &manifest_json,
        lock["artifacts"]["evidence"]
            .as_str()
            .expect("evidence hash"),
    );
    assert_eq!(
        recomputed.combined,
        lock["combined"].as_str().expect("combined"),
        "the loan BUNDLE.lock combined hash recomputes via the runtime helper"
    );
}

/// (Generalization #3) The FIVE served tool NAMES are UNCHANGED vs tax-calc — the
/// names are generic; only the payload behind them differs. Read off the handler
/// `NAME` consts (the single registration source).
#[test]
fn five_generic_tool_names_unchanged() {
    let served: BTreeSet<&str> = [
        CalculateHandler::NAME,
        ExplainHandler::NAME,
        GetManifestHandler::NAME,
        DiffVersionHandler::NAME,
        RenderWorkbookHandler::NAME,
    ]
    .into_iter()
    .collect();
    let expected: BTreeSet<&str> = [
        "calculate",
        "explain",
        "get_manifest",
        "diff_version",
        "render_workbook",
    ]
    .into_iter()
    .collect();
    assert_eq!(
        served, expected,
        "the loan serves the SAME five generic tool names as tax-calc — only the \
         manifest/schema payload behind them differs (the WBEX-01 invariant)"
    );
}

/// (Generalization #4) The served INPUT schema reflects the LOAN's own inputs:
/// `schema::input_schema_for_manifest` over the loaded loan bundle CONTAINS the
/// loan input keys and NONE of tax-calc's input keys.
#[test]
fn served_input_schema_reflects_loan_inputs() {
    let (_scratch, bundle) = load_loan_bundle();
    let schema = input_schema_for_manifest(&bundle.manifest, &bundle.cell_map);
    let served = schema_property_keys(&schema, "inputs");

    for key in loan_input_keys() {
        assert!(
            served.contains(&key),
            "the served input schema must carry the loan input `{key}` (served: {served:?})"
        );
    }
    for tax_key in tax_input_keys() {
        assert!(
            !served.contains(&tax_key),
            "the served loan input schema must NOT carry tax-calc's input `{tax_key}`"
        );
    }
}

/// (Generalization #5) The served OUTPUT schema reflects the LOAN's own outputs:
/// `schema::output_schema_for_manifest` carries the loan `out_*` keys (MULTIPLE,
/// no privileged headline) and NONE of tax-calc's output keys.
#[test]
fn served_output_schema_reflects_loan_outputs() {
    let (_scratch, bundle) = load_loan_bundle();
    let schema = output_schema_for_manifest(&bundle.manifest, &bundle.cell_map);
    let served = schema_property_keys(&schema, "outputs");

    for key in loan_output_keys() {
        assert!(
            served.contains(&key),
            "the served output schema must carry the loan output `{key}` (served: {served:?})"
        );
    }
    assert!(
        served.len() >= 3,
        "the loan serves MULTIPLE named outputs with no privileged single headline \
         (S-1); served {} outputs",
        served.len()
    );
    // Each served output projects the generic { value, unit } nested shape — the
    // served unit-projection path runs uniformly for this non-golden workbook.
    let outputs = &schema["properties"]["outputs"]["properties"];
    for key in loan_output_keys() {
        assert!(
            outputs[&key]["properties"]["value"].is_object(),
            "served loan output `{key}` carries the generic {{ value, unit }} projection"
        );
        assert!(
            outputs[&key]["properties"].get("unit").is_some(),
            "served loan output `{key}` carries the unit projection slot"
        );
    }
}

/// (Generalization #6) Tax-calc's input/output fields are ABSENT from the loan
/// served schema — the contrast assertion stated as a standalone gate.
#[test]
fn tax_specific_fields_absent_from_loan_schema() {
    let (_scratch, bundle) = load_loan_bundle();
    let in_schema = input_schema_for_manifest(&bundle.manifest, &bundle.cell_map);
    let out_schema = output_schema_for_manifest(&bundle.manifest, &bundle.cell_map);
    let served_inputs = schema_property_keys(&in_schema, "inputs");
    let served_outputs = schema_property_keys(&out_schema, "outputs");

    for tax_in in tax_input_keys() {
        assert!(
            !served_inputs.contains(&tax_in),
            "tax input `{tax_in}` absent"
        );
    }
    for tax_out in tax_output_keys() {
        assert!(
            !served_outputs.contains(&tax_out),
            "tax output `{tax_out}` absent"
        );
    }
}

/// (Generalization #7 — THE PROOF) The loan served input/output key SETS are
/// DISJOINT from tax-calc's. This disjointness, read off the GENERIC-driver output
/// (no loan-specific Rust builds the manifest), IS the generalization proof
/// (T-96-11).
#[test]
fn loan_and_tax_served_key_sets_are_disjoint() {
    let (_scratch, bundle) = load_loan_bundle();
    let served_inputs = schema_property_keys(
        &input_schema_for_manifest(&bundle.manifest, &bundle.cell_map),
        "inputs",
    );
    let served_outputs = schema_property_keys(
        &output_schema_for_manifest(&bundle.manifest, &bundle.cell_map),
        "outputs",
    );

    assert!(
        served_inputs.is_disjoint(&tax_input_keys()),
        "the loan served INPUT keys are disjoint from tax-calc's (generalization proof)"
    );
    assert!(
        served_outputs.is_disjoint(&tax_output_keys()),
        "the loan served OUTPUT keys are disjoint from tax-calc's (generalization proof)"
    );
}

/// (Generalization #8) The served `get_manifest` projection reflects the loan's
/// OWN inputs/outputs with MULTIPLE outputs and no privileged single headline.
///
/// `GetManifestHandler` (the GENERIC toolkit type) is CONSTRUCTED over the loaded
/// loan bundle — proving the generic handler accepts the second workbook with no
/// loan-specific Rust. Its `curated_manifest` projection is a private fn that reads
/// `bundle.manifest.cells` (Role::Input → inputs, Role::Output → outputs); since
/// the compiler crate cannot pull `pmcp`/an async runtime as a dev-dep (the purity
/// boundary — Cargo.toml dev-deps carry only the toolkit `workbook` feature), this
/// asserts that SAME projection directly over the loaded bundle's public
/// `manifest` field — the exact input the handler serves.
#[test]
fn served_get_manifest_reflects_loan_cells() {
    let (_scratch, bundle) = load_loan_bundle();

    // The generic handler accepts the loan bundle (constructs over Arc<bundle>) —
    // no loan-specific handler exists. Constructing it proves the generic serve
    // type is what carries the loan's payload.
    let _handler = GetManifestHandler::new(Arc::new(bundle.clone()));

    // The curated get_manifest payload projects from manifest.cells by role — the
    // SAME source `GetManifestHandler::handle` serves.
    let inputs: Vec<_> = bundle
        .manifest
        .cells
        .iter()
        .filter(|c| c.role == Role::Input)
        .collect();
    let outputs: Vec<_> = bundle
        .manifest
        .cells
        .iter()
        .filter(|c| c.role == Role::Output)
        .collect();

    assert_eq!(
        bundle.stamp.bundle_id, "loan-calc",
        "the served manifest carries the loan's own bundle_id"
    );
    assert!(
        outputs.len() >= 3,
        "the served manifest has MULTIPLE outputs (no privileged single headline); \
         got {}",
        outputs.len()
    );
    assert_eq!(
        inputs.len(),
        loan_input_keys().len(),
        "the served manifest reflects the loan's own input cells (got {} inputs)",
        inputs.len()
    );
}

/// (Counter-test — T-96-10) The SAME loan bytes are REFUSED on the PRODUCTION
/// `compile_workbook` (Enforce) path: only the `#[cfg(test)]` override accepts the
/// authored fixture's staleness; production never weakens.
#[test]
fn production_compile_refuses_loan_fixture() {
    let scratch = tempfile::TempDir::new().expect("scratch");
    let xlsx = place_fixture(scratch.path());
    let out_root = scratch.path().join("out");
    std::fs::create_dir_all(&out_root).expect("out root");

    let result = compile_workbook(&xlsx, &out_root, "loan-calc", "1.0.0", "prod-approver");
    assert!(
        result.is_err(),
        "production compile_workbook (Enforce) MUST refuse the authored loan fixture's \
         staleness — the trusted-fixture override is test-only and never weakens \
         production (T-96-10)"
    );
}
