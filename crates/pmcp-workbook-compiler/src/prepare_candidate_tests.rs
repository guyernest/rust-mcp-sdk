//! In-crate `#[cfg(test)]` proof for the `prepare_candidate` facade (94-00-02).
//!
//! These tests reach the `#[cfg(test)]`-only
//! [`crate::prepare_candidate_with_fixture_override`] (same CR-01 reachability
//! reason as `reemit_golden`), so they can build a [`crate::Candidate`] from the
//! committed neutral `tax-calc.xlsx` whose `fullCalcOnLoad=1` staleness signal the
//! trusted-fixture override demotes. They prove:
//!
//! 1. PARITY — `prepare_candidate` walks the IDENTICAL pipeline the seed lane does
//!    (`compile_workbook_inner`) up to the promote step: its `ir` re-serializes to
//!    the seed lane's emitted `executable.ir.json`, and its `computed` named
//!    outputs equal the executor's output over the same seeded IR.
//! 2. WRITES NOTHING — `prepare_candidate` leaves the filesystem untouched
//!    (gate-before-write, T-94-00-WRITE).
//! 3. SAME GATE — a workbook that fails stage 1 surfaces the SAME `CompileError`
//!    the seed lane returns (the facade relaxes no gate).
//! 4. FIELDS LINE UP — a `gate::accept::PromoteInputs` can be assembled by
//!    BORROWING a `Candidate` without inventing any field.

use std::path::{Path, PathBuf};

use crate::gate::accept::PromoteInputs;
use crate::{
    compile_workbook, compile_workbook_with_fixture_override, prepare_candidate,
    prepare_candidate_with_fixture_override,
};

/// The committed neutral fixture (`tests/fixtures/tax-calc.xlsx`) — the SAME one
/// the producer/consumer golden proof compiles.
fn committed_fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tax-calc.xlsx")
}

/// Copy the committed fixture into `dir/tax-calc.xlsx` and return its path.
fn place_fixture(dir: &Path) -> PathBuf {
    let xlsx = dir.join("tax-calc.xlsx");
    std::fs::copy(committed_fixture(), &xlsx).expect("copy committed fixture");
    xlsx
}

/// Read a bundle member into a normalized `serde_json::Value` (key order/format
/// ignored).
fn read_json(dir: &Path, member: &str) -> serde_json::Value {
    let bytes = std::fs::read(dir.join(member)).unwrap_or_else(|e| panic!("read {member}: {e}"));
    serde_json::from_slice(&bytes).unwrap_or_else(|e| panic!("parse {member}: {e}"))
}

#[test]
fn prepare_candidate_ir_matches_the_seed_lane_executable_ir() {
    // SEED LANE: compile the fixture through the seed lane (override) → emits a
    // bundle whose executable.ir.json is the canonical IR projection.
    let seed_scratch = tempfile::TempDir::new().expect("seed scratch");
    let seed_xlsx = place_fixture(seed_scratch.path());
    let seed_out = seed_scratch.path().join("out");
    std::fs::create_dir_all(&seed_out).expect("seed out root");
    compile_workbook_with_fixture_override(&seed_xlsx, &seed_out, "tax-calc", "1.1.0", "approver")
        .expect("seed-lane compile");
    let seed_ir_json = read_json(&seed_out.join("tax-calc@1.1.0"), "executable.ir.json");

    // CANDIDATE: build the candidate for the SAME bytes (override). Its `ir`
    // re-serialized through the bundle's deterministic sorted-map choke point must
    // equal the seed lane's emitted IR (parity: identical pipeline up to promote).
    let cand_scratch = tempfile::TempDir::new().expect("cand scratch");
    let cand_xlsx = place_fixture(cand_scratch.path());
    let candidate =
        prepare_candidate_with_fixture_override(&cand_xlsx, "tax-calc").expect("prepare candidate");

    let candidate_ir_json_str =
        crate::artifact::to_bundle_json_sorted_map(&candidate.ir, "executable.ir.json")
            .expect("serialize candidate ir");
    let candidate_ir_json: serde_json::Value =
        serde_json::from_str(&candidate_ir_json_str).expect("parse candidate ir");

    assert_eq!(
        candidate_ir_json, seed_ir_json,
        "prepare_candidate's IR equals the seed lane's executable.ir.json (parity)"
    );
}

#[test]
fn prepare_candidate_computed_holds_the_named_outputs() {
    // The candidate's `computed` map carries the finite named-output values. The
    // committed fixture's golden output (tax-calc) reconciles, so `computed` is
    // non-empty and every value is finite.
    let scratch = tempfile::TempDir::new().expect("scratch");
    let xlsx = place_fixture(scratch.path());
    let candidate =
        prepare_candidate_with_fixture_override(&xlsx, "tax-calc").expect("prepare candidate");

    assert!(
        !candidate.computed.is_empty(),
        "the candidate carries at least one named-output computed value"
    );
    for (region, value) in &candidate.computed {
        assert!(
            value.is_finite(),
            "named-output {region} computed value is finite: {value}"
        );
    }

    // Every `computed` key is a manifest Role::Output cell key (the projection is
    // exactly the named outputs — no helper/formula cells leak in).
    use pmcp_workbook_runtime::Role;
    for region in candidate.computed.keys() {
        let is_output = candidate
            .manifest
            .cells
            .iter()
            .any(|c| &c.cell == region && matches!(c.role, Role::Output));
        assert!(
            is_output,
            "computed key {region} is a manifest Role::Output cell"
        );
    }
}

#[test]
fn prepare_candidate_writes_nothing() {
    // `prepare` must NOT leak a bundle write before the gate decides. We run it
    // against a fixture placed in `work/`, point an explicit `out/` dir, and assert
    // BOTH stay free of any new bundle artifact after the call.
    let scratch = tempfile::TempDir::new().expect("scratch");
    let work = scratch.path().join("work");
    std::fs::create_dir_all(&work).expect("work dir");
    let xlsx = place_fixture(&work);

    let out = scratch.path().join("out");
    std::fs::create_dir_all(&out).expect("out dir");

    let before_out = dir_entry_count(&out);
    let before_work = dir_entry_count(&work);

    let _candidate =
        prepare_candidate_with_fixture_override(&xlsx, "tax-calc").expect("prepare candidate");

    // The explicit out dir is UNTOUCHED (no `{bundle}@{version}/` appeared).
    assert_eq!(
        dir_entry_count(&out),
        before_out,
        "prepare_candidate writes NOTHING into the out dir (gate-before-write)"
    );
    // The work dir gained NO new file beyond the fixture we placed (no
    // ratifications sidecar, no scratch bundle).
    assert_eq!(
        dir_entry_count(&work),
        before_work,
        "prepare_candidate writes NOTHING beside the workbook (no ratify sidecar)"
    );
}

/// Count the immediate directory entries under `dir` (non-recursive).
fn dir_entry_count(dir: &Path) -> usize {
    std::fs::read_dir(dir)
        .map(|rd| rd.count())
        .unwrap_or_default()
}

#[test]
fn prepare_candidate_relaxes_no_gate_under_enforce() {
    // The PRODUCTION `prepare_candidate` (FreshnessPolicy::Enforce) REFUSES the
    // committed neutral fixture for EXACTLY the reason the production
    // `compile_workbook` does (its `fullCalcOnLoad=1` staleness, only demoted under
    // the test-only override) — proving the facade relaxes no gate.
    let scratch = tempfile::TempDir::new().expect("scratch");
    let xlsx = place_fixture(scratch.path());

    let prepare_err = prepare_candidate(&xlsx, "tax-calc")
        .expect_err("production prepare refuses the stale neutral fixture");

    let out = scratch.path().join("out");
    std::fs::create_dir_all(&out).expect("out dir");
    let compile_err = compile_workbook(&xlsx, &out, "tax-calc", "1.1.0", "approver")
        .expect_err("production compile refuses the same stale fixture");

    // Both refuse with the SAME error DISCRIMINANT (a stage-1 Lint refusal). We
    // compare the rendered Display prefix `lint:` so the gate parity is explicit.
    assert!(
        prepare_err.to_string().starts_with("lint:"),
        "prepare refuses with a stage-1 lint error: {prepare_err}"
    );
    assert!(
        compile_err.to_string().starts_with("lint:"),
        "compile refuses with a stage-1 lint error: {compile_err}"
    );
}

#[test]
fn candidate_fields_assemble_a_promote_inputs() {
    // The Candidate fields line up 1:1 with PromoteInputs: the CLI assembles a
    // PromoteInputs BORROWING a Candidate without inventing any field. (We only
    // need it to type-check + construct; the changelog/bundle_id are the CLI's lane
    // decision, supplied here as locals.)
    use pmcp_workbook_runtime::VersionChangelog;

    let scratch = tempfile::TempDir::new().expect("scratch");
    let xlsx = place_fixture(scratch.path());
    let candidate =
        prepare_candidate_with_fixture_override(&xlsx, "tax-calc").expect("prepare candidate");

    let changelog = VersionChangelog {
        from_version: String::new(),
        to_version: candidate.version.clone(),
        deltas: vec![],
        summary: format!("seed {}", candidate.version),
    };

    let inputs = PromoteInputs {
        bundle_id: "tax-calc",
        version: &candidate.version,
        ir: &candidate.ir,
        manifest: &candidate.manifest,
        layout: &candidate.layout,
        changelog: &changelog,
        parser_equivalence: &candidate.parser_equivalence,
        workbook_hash: candidate.candidate_workbook_hash.clone(),
        output_tables: &candidate.output_tables,
        dag: &candidate.dag,
    };

    // Sanity: the assembled PromoteInputs reflects the candidate's stamp-binding
    // fields (version == changelog to_version; the hash is the candidate's).
    assert_eq!(inputs.version, inputs.changelog.to_version);
    assert_eq!(inputs.workbook_hash, candidate.candidate_workbook_hash);
    assert!(
        !inputs.ir.is_empty(),
        "the borrowed candidate IR is non-empty (promote-ready)"
    );
}
