//! WBEX-02 Excel-quirk reconcile corpus — layer 2 (penny-reconcile fixtures).
//!
//! This module is the D-08 layer-2 partner of the `scalar_eval` quirk unit tests
//! (in `pmcp-workbook-runtime`): each quirk that is numerically expressible as a
//! whitelisted formula DAG is encoded as a tiny `.xlsx` fixture with a cached
//! `<v>` oracle, compiled through the REAL trusted-fixture override, and graded by
//! RETRIEVING the executor's recomputed value AND the cached oracle and comparing
//! them through the SAME penny-reconcile mechanism the golden gate uses
//! ([`crate::reconcile::within_tol`], `TOL` = 0.01). It verifies reconcile
//! DETERMINISM across a corpus of Excel edge cases, not just the single golden.
//!
//! # Why this lives in `src/` under `#[cfg(test)]` (CR-01)
//!
//! Identical to [`crate::reemit_golden`] / [`crate::reemit_loan`]: the
//! trusted-fixture override (`compile_workbook_with_fixture_override`) is
//! `#[cfg(test)]`-only, so the harness MUST live INSIDE the crate to reach it.
//!
//! # The harness CANNOT pass on compile-success alone (T-96-14b)
//!
//! Each test does NOT merely assert the fixture compiled. It loads the emitted
//! bundle via the GENERIC toolkit loader, seeds the LOAN's authored inputs, runs
//! the runtime executor, RETRIEVES the computed `CellValue` at the reconcile cell
//! key AND the cached `<v>` oracle, and asserts they reconcile within `TOL`
//! through the real `within_tol` penny path. A compile that produced a WRONG value
//! would fail the assertion — the value is the witness, not the clean compile.
//!
//! # The forbidden exact-float `==` (T-96-14)
//!
//! Every money/numeric compare goes through `within_tol` (±0.01). An exact-float
//! `==` on money is forbidden repo-wide and absent from this harness — the
//! float-boundary quirk (`0.1 + 0.2`) is the in-corpus proof of WHY.
//!
//! # Quirk -> WBEX-02 traceability map
//!
//! The corpus is ~8 quirks (D-09 cap: the four roadmap-NAMED quirks + a curated
//! set). The two layers (scalar_eval / penny-reconcile) cover them as follows.
//! WBEX-02 = "an Excel-quirk fixture corpus verifies reconcile determinism beyond
//! the single golden case."
//!
//! | # | Quirk | Class | scalar_eval (layer 1) | penny-reconcile (layer 2) |
//! |---|-------|-------|-----------------------|---------------------------|
//! | 1 | 1900 leap-year | NAMED | `quirk_1900_leap_serial_offset_components` | `leap1900-probe.xlsx` (SPIKE disposition A — the load-bearing artifact; see [`crate::fixture_author`]) |
//! | 2 | empty-cell coercion | NAMED | `quirk_empty_cell_coerces_to_zero_in_additive_context` | `quirk-empty-coercion.xlsx` (`A2 + A1` where `A1=IF(A2>9999,1)` is empty -> 5) |
//! | 3 | error propagation | NAMED | `quirk_error_propagates_through_arithmetic` | scalar_eval-only + this note (see "Named quirks without a reconcile fixture" below) |
//! | 4 | half-rounding boundaries | NAMED | `quirk_half_rounding_uses_excel_round_source_of_truth` | `quirk-half-rounding.xlsx` (`ROUND(1594.925,2)` -> 1594.93) |
//! | 5 | negative-value rounding sign | curated | `quirk_negative_rounding_sign_away_from_zero` | `quirk-negative-rounding.xlsx` (`ROUND(-2.5,0)` -> -3) |
//! | 6 | text->number coercion | curated | `quirk_text_to_number_coercion_is_context_specific` | `quirk-text-coercion.xlsx` (`"5.5"*2` -> 11) |
//! | 7 | explicit `#DIV/0!` propagation | curated | `quirk_explicit_div_zero_error_propagates` | scalar_eval-only + this note (runtime parity limitation below) |
//! | 8 | float boundary (`0.1+0.2`) | curated | `quirk_float_boundary_compares_within_tol_not_exact` | `quirk-float-boundary.xlsx` (`0.1+0.2` ~= 0.3 within TOL) |
//!
//! ## Named quirks without a reconcile fixture (the documented stand-in)
//!
//! The plan requires >= 1 reconcile assertion per NAMED roadmap quirk UNLESS the
//! quirk is impossible to express numerically through the real penny path; then
//! the scalar_eval assertion + this note stands in. ONE named quirk takes the
//! stand-in: **error propagation** (#3). The runtime's `Div` binop CLAMPS a
//! zero-divisor `NaN` to `0.0` for byte-parity with the locked JS kernel
//! (`scalar_eval.rs` WR-02 / IN-03), so a division-by-zero never surfaces a
//! reconcilable `#DIV/0!` oracle on the served path, and an Excel error
//! short-circuits at the `preflight_error` boundary (it never reaches a numeric
//! reconcile cell with a numeric oracle). Error propagation is therefore proven
//! at the scalar_eval layer (where `preflight_error` IS the mechanism under test:
//! `quirk_error_propagates_through_arithmetic` + `quirk_explicit_div_zero_error_propagates`),
//! and the curated `#DIV/0!` quirk (#7) is the same class. This is the
//! plan-sanctioned documented stand-in, not a gap. The OTHER three named quirks
//! (1900-leap, empty-cell coercion, half-rounding) each have a real reconcile
//! fixture above.

#![cfg(test)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::compile_workbook;
use crate::compile_workbook_with_fixture_override;
use crate::reconcile::{within_tol, TOL};

use pmcp_server_toolkit::workbook::{load_bundle, LocalDirSource};
use pmcp_workbook_runtime::{run_executor, CellEnv, CellValue};

/// One quirk reconcile case: the committed fixture stem, the compiled bundle id,
/// the authored INPUT seeds (`seed_coord -> value`), the reconcile OUTPUT cell key
/// (`seed_coord`), and the cached numeric oracle the recomputation must reconcile
/// to within [`TOL`].
struct QuirkCase {
    /// The committed fixture file stem under `tests/fixtures/quirks/`.
    stem: &'static str,
    /// The bundle id the override compiles the fixture under.
    bundle_id: &'static str,
    /// Authored numeric input seeds: the fully-qualified `sheet!addr` key -> value.
    inputs: &'static [(&'static str, f64)],
    /// Authored TEXT input seeds: `sheet!addr` key -> text value (for the
    /// text->number coercion quirk, whose operand is genuinely a string cell that
    /// the served path seeds as text).
    text_inputs: &'static [(&'static str, &'static str)],
    /// The reconcile OUTPUT cell key (`sheet!addr`) whose recomputation is graded.
    output_key: &'static str,
    /// The cached Excel oracle the recomputed output must reconcile to.
    oracle: f64,
}

/// The committed quirk fixtures dir (relative to THIS crate).
fn quirks_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/quirks")
}

/// Copy a committed quirk fixture into `dir` and return the copied path.
fn place_quirk(dir: &Path, stem: &str) -> PathBuf {
    let xlsx = dir.join(format!("{stem}.xlsx"));
    std::fs::copy(quirks_dir().join(format!("{stem}.xlsx")), &xlsx).expect("copy quirk fixture");
    xlsx
}

/// Compile a quirk fixture via the `#[cfg(test)]` trusted-fixture override and
/// return the emitted bundle dir (plus the scratch guard).
fn compile_quirk(stem: &str, bundle_id: &str) -> (tempfile::TempDir, PathBuf) {
    let scratch = tempfile::TempDir::new().expect("scratch dir");
    let xlsx = place_quirk(scratch.path(), stem);
    let out_root = scratch.path().join("out");
    std::fs::create_dir_all(&out_root).expect("out root");
    compile_workbook_with_fixture_override(&xlsx, &out_root, bundle_id, "1.0.0", "quirk-proof")
        .expect("the quirk fixture compiles via the trusted-fixture override");
    let bundle = out_root.join(format!("{bundle_id}@1.0.0"));
    (scratch, bundle)
}

/// The full corpus of numerically-expressible quirk reconcile cases (the rows of
/// the traceability map that HAVE a reconcile fixture). The 1900-leap reconcile
/// fixture is the committed `leap1900-probe.xlsx`, proven by
/// `crate::fixture_author::tests::leap1900_probe_compiles_and_reconciles` and
/// re-graded here through the same retrieve-and-compare harness.
fn quirk_cases() -> Vec<QuirkCase> {
    vec![
        QuirkCase {
            stem: "quirk-half-rounding",
            bundle_id: "quirk-half-rounding",
            inputs: &[("Quirk!A1", 1594.925)],
            text_inputs: &[],
            output_key: "Quirk!B1",
            oracle: 1594.93,
        },
        QuirkCase {
            stem: "quirk-negative-rounding",
            bundle_id: "quirk-negative-rounding",
            inputs: &[("Quirk!A1", -2.5)],
            text_inputs: &[],
            output_key: "Quirk!B1",
            oracle: -3.0,
        },
        QuirkCase {
            stem: "quirk-empty-coercion",
            bundle_id: "quirk-empty-coercion",
            inputs: &[("Quirk!A2", 5.0)],
            text_inputs: &[],
            output_key: "Quirk!B1",
            oracle: 5.0,
        },
        QuirkCase {
            stem: "quirk-float-boundary",
            bundle_id: "quirk-float-boundary",
            inputs: &[("Quirk!A1", 0.1), ("Quirk!A2", 0.2)],
            text_inputs: &[],
            output_key: "Quirk!B1",
            oracle: 0.3,
        },
        QuirkCase {
            stem: "quirk-text-coercion",
            bundle_id: "quirk-text-coercion",
            inputs: &[("Quirk!A2", 2.0)],
            // A1 is the numeric-TEXT operand the quirk coerces (`"5.5" * 2`).
            text_inputs: &[("Quirk!A1", "5.5")],
            output_key: "Quirk!B1",
            oracle: 11.0,
        },
    ]
}

/// Compile `case`, load its bundle, seed the authored inputs, run the executor,
/// and RETURN `(computed, oracle)` at the reconcile cell key — so the caller can
/// grade them through the real `within_tol` penny path. This RETRIEVES both the
/// real recomputed value and the cached oracle (T-96-14b): a wrong computation
/// fails, not just a failed compile.
fn recompute_at_reconcile_key(case: &QuirkCase) -> (CellValue, CellValue) {
    let (_scratch, bundle_dir) = compile_quirk(case.stem, case.bundle_id);
    let bundle = load_bundle(&LocalDirSource::new(&bundle_dir))
        .expect("the emitted quirk bundle loads via the generic toolkit loader");

    // Seed the authored inputs (Role::Input cells are ABSENT from `ir`; the
    // executor's seed contract requires the caller to pre-load them).
    let mut seed = CellEnv::new();
    for (key, value) in case.inputs {
        seed = seed.seed_cell(*key, &CellValue::Number(*value));
    }
    for (key, text) in case.text_inputs {
        seed = seed.seed_cell(*key, &CellValue::Text((*text).to_string()));
    }

    let result = run_executor(&bundle.ir, &bundle.dag, &seed)
        .expect("the quirk DAG is acyclic and runs to completion");
    let computed = result
        .computed
        .get(case.output_key)
        .cloned()
        .unwrap_or(CellValue::Empty);
    (computed, CellValue::Number(case.oracle))
}

/// Every numerically-expressible quirk's RECOMPUTED output reconciles to its
/// cached oracle through the REAL penny path (`within_tol`, `TOL` = 0.01). This is
/// the load-bearing WBEX-02 layer-2 assertion: it grades the recomputed value, so
/// it cannot pass on compile-success alone (T-96-14b), and it never uses
/// exact-float `==` on money (T-96-14).
#[test]
fn every_quirk_reconciles_recomputed_value_to_oracle_within_tol() {
    for case in quirk_cases() {
        let (computed, oracle) = recompute_at_reconcile_key(&case);
        assert!(
            within_tol(&computed, &oracle),
            "quirk `{}`: recomputed {:?} at {} must reconcile to oracle {:?} within TOL={}",
            case.stem,
            computed,
            case.output_key,
            oracle,
            TOL
        );
    }
}

/// The harness genuinely distinguishes right from wrong values (a guard against
/// the T-96-14b "degrades to compile-success only" failure mode): a deliberately
/// WRONG oracle does NOT reconcile through `within_tol`, proving the recomputed
/// value — not the clean compile — is the witness.
#[test]
fn a_wrong_oracle_does_not_reconcile_proving_the_value_is_graded() {
    let case = &quirk_cases()[0]; // half-rounding, oracle 1594.93
    let (computed, _correct) = recompute_at_reconcile_key(case);
    let wrong = CellValue::Number(case.oracle + 100.0); // far outside TOL
    assert!(
        !within_tol(&computed, &wrong),
        "a wrong oracle must NOT reconcile — the harness grades the recomputed \
         value, it does not pass on compile-success alone (T-96-14b)"
    );
}

/// PRODUCTION-REFUSAL SPOT CHECK (T-96-13): a quirk fixture is REFUSED by the bare
/// production `compile_workbook` (Enforce) — only the `#[cfg(test)]` trusted
/// override accepts the authored fixture's `fullCalcOnLoad` staleness. This proves
/// the quirk corpus rides the test-only override and never weakens the production
/// freshness/provenance gate.
#[test]
fn production_compile_refuses_a_quirk_fixture() {
    let scratch = tempfile::TempDir::new().expect("scratch dir");
    let xlsx = place_quirk(scratch.path(), "quirk-half-rounding");
    let out_root = scratch.path().join("out");
    std::fs::create_dir_all(&out_root).expect("out root");

    let result = compile_workbook(
        &xlsx,
        &out_root,
        "quirk-half-rounding",
        "1.0.0",
        "prod-approver",
    );
    assert!(
        result.is_err(),
        "production compile_workbook (Enforce) MUST refuse a quirk fixture's staleness — \
         the trusted-fixture override is test-only and never weakens production (T-96-13)"
    );
}

/// Each named quirk that HAS a reconcile fixture (1900-leap, empty-cell coercion,
/// half-rounding) is present in the corpus and reconciles — the explicit
/// per-named-quirk assertion the WBEX-02 traceability map promises. (Error
/// propagation, the fourth named quirk, is the documented scalar_eval-only
/// stand-in per the module header.)
#[test]
fn each_named_reconcilable_quirk_has_a_reconcile_assertion() {
    // Look the named cases up from the single-source `quirk_cases()` table rather
    // than re-declaring their literals here (which could silently drift from it).
    let cases: Vec<QuirkCase> = quirk_cases();
    let case_by_stem = |stem: &str| -> &QuirkCase {
        cases
            .iter()
            .find(|c| c.stem == stem)
            .unwrap_or_else(|| panic!("named quirk `{stem}` is present in quirk_cases()"))
    };

    // empty-cell coercion (NAMED).
    let (c, o) = recompute_at_reconcile_key(case_by_stem("quirk-empty-coercion"));
    assert!(within_tol(&c, &o), "empty-cell coercion reconciles: {c:?}");

    // half-rounding boundaries (NAMED).
    let (c, o) = recompute_at_reconcile_key(case_by_stem("quirk-half-rounding"));
    assert!(within_tol(&c, &o), "half-rounding reconciles: {c:?}");

    // 1900 leap-year (NAMED) — the committed probe, re-graded here through the
    // same retrieve-and-compare harness against its cached `<v>` oracle (62).
    let scratch = tempfile::TempDir::new().expect("scratch dir");
    let leap_src = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/leap1900-probe.xlsx");
    let xlsx = scratch.path().join("leap1900-probe.xlsx");
    std::fs::copy(&leap_src, &xlsx).expect("copy committed leap probe");
    let out_root = scratch.path().join("out");
    std::fs::create_dir_all(&out_root).expect("out root");
    compile_workbook_with_fixture_override(
        &xlsx,
        &out_root,
        "leap1900-probe",
        "1.0.0",
        "leap-proof",
    )
    .expect("the committed leap probe compiles via the override");
    let leap_bundle_dir = out_root.join("leap1900-probe@1.0.0");
    let bundle =
        load_bundle(&LocalDirSource::new(&leap_bundle_dir)).expect("the leap bundle loads");
    let seed = CellEnv::new().seed_cell("Serial!A1", &CellValue::Number(61.0));
    let result = run_executor(&bundle.ir, &bundle.dag, &seed).expect("leap DAG runs");
    let computed = result
        .computed
        .get("Serial!B1")
        .cloned()
        .unwrap_or(CellValue::Empty);
    assert!(
        within_tol(&computed, &CellValue::Number(62.0)),
        "1900-leap serial offset reconciles to 62: {computed:?}"
    );

    // Pin the reconcile-fixture count to its single source of truth (`quirk_cases`)
    // so adding/removing a fixture forces a DELIBERATE edit here and a re-check
    // against the D-09 ~7-9 cap. `quirk_cases()` returns a hardcoded 5-element vec
    // (5 reconcile fixtures + the leap probe = 6 reconcile fixtures; 8 quirks across
    // both layers via the traceability map), so an exact equality is meaningful
    // where the old `(5..=9).contains(len+1)` range was tautological (len is
    // compile-time-fixed, so it could only ever read 6). Reuses the `cases`
    // fetched at the top of this test.
    assert_eq!(
        cases.len(),
        5,
        "the reconcile corpus is the 5 fixtures in `quirk_cases` (+ the leap probe = \
         6); changing it must be a deliberate edit re-checked against the D-09 cap"
    );
}

/// Build a key map from a quirk's authored inputs (a tiny sanity check that the
/// case table is internally consistent: no duplicate input keys, and the output
/// key is not also an input key — the output is a FORMULA cell, never seeded).
#[test]
fn quirk_case_tables_are_internally_consistent() {
    for case in quirk_cases() {
        let mut seen: HashMap<&str, f64> = HashMap::new();
        for (k, v) in case.inputs {
            assert!(
                seen.insert(k, *v).is_none(),
                "quirk `{}` has a duplicate input key {k}",
                case.stem
            );
            assert_ne!(
                *k, case.output_key,
                "quirk `{}` output key {} must not also be a seeded input",
                case.stem, case.output_key
            );
        }
    }
}
