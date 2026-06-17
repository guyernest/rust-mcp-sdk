//! Promote-time governance gate — the build-time approval boundary.
//!
//! The build-time governance dir (reviewable on disk, never served): a
//! candidate fingerprint binds prev-hash + candidate-hash + region deltas
//! ([`corpus::candidate_fingerprint`]), an [`corpus::ApprovalRecord`] /
//! [`corpus::ApprovalCase`] corpus replays both versions over an AUTO-DERIVED case
//! grid (D-09: manifest defaults + enum domains + numeric boundaries, capped at
//! [`corpus::MAX_CORPUS_CASES`] — replaces the lighthouse's BA-curated
//! checked-in case file), and [`accept::accept`] re-baselines + records a fingerprint-bound
//! approval, and the CR-02 promote ([`promote`]) writes into a NEW
//! `{bundle_id}@{version}/` dir without overwriting the baseline. First version is
//! a no-op baseline (D-12).

pub mod accept;
pub mod corpus;
pub mod governed_artifact;

use std::collections::BTreeMap;

use pmcp_workbook_runtime::ChangeClass;

use corpus::{
    approval_matches, candidate_fingerprint, region_deltas, ApprovalCase, ApprovalRecord,
    RegionDelta,
};

/// The one-penny named-output tolerance (£0.01) the gate grades a corpus case's
/// computed value against — the numeric axis of the recompiled-workbook → served-
/// truth trust boundary.
pub const TOL: f64 = 0.01;

/// One over-tolerance (or missing) named-output delta the gate surfaces in a block
/// decision — the BA-actionable detail the CLI (Phase 94) renders.
#[derive(Debug, Clone, PartialEq)]
pub struct GateDelta {
    /// The named output region (`cell_key`).
    pub region: String,
    /// The corpus golden (prior accepted) value.
    pub expected: f64,
    /// The candidate computed value (`None` = the candidate produced no finite
    /// value — a HARD block regardless of approval).
    pub computed: Option<f64>,
}

/// The structured decision of one promote-gate evaluation (the library-level
/// result the Phase-94 CLI renders). A [`GateDecision::Blocked`] carries the deltas,
/// the change class, and the exact copy-pasteable `--accept` command (D-10).
#[derive(Debug, Clone, PartialEq)]
pub enum GateDecision {
    /// Every named output reconciled within [`TOL`], OR a covering fingerprint-bound
    /// approval certified the over-tolerance deltas. Carries the derived
    /// [`corpus::candidate_fingerprint`] (for audit + `--accept`).
    Pass {
        /// The fingerprint binding this exact candidate transition.
        fingerprint: String,
    },
    /// One or more named outputs moved beyond [`TOL`] with no covering approval
    /// (or produced no finite value). Promotion is BLOCKED.
    Blocked(Box<GateBlock>),
}

/// The payload of a [`GateDecision::Blocked`]: the over-tolerance deltas, the
/// auto-derived change classes, the fingerprint, and the exact `--accept` command
/// a reviewer must run to re-baseline (D-10).
#[derive(Debug, Clone, PartialEq)]
pub struct GateBlock {
    /// The case that blocked.
    pub case_id: String,
    /// The over-tolerance / missing named-output deltas.
    pub deltas: Vec<GateDelta>,
    /// The auto-derived change classes for this transition.
    pub change_classes: Vec<ChangeClass>,
    /// The derived candidate fingerprint (an `--accept` binds to THIS).
    pub fingerprint: String,
    /// The exact copy-pasteable approval command (D-10).
    pub accept_command: String,
}

impl GateBlock {
    /// Render the BA-actionable block message (the deltas + change class + the
    /// exact `--accept` command). The Phase-94 CLI prints this.
    #[must_use]
    pub fn render(&self) -> String {
        let mut lines = vec![format!(
            "BLOCKED: case `{}` has named-output changes beyond £{TOL:.2} with no covering approval:",
            self.case_id
        )];
        for d in &self.deltas {
            match d.computed {
                Some(got) => lines.push(format!(
                    "  - {} = {got} (golden {}, Δ £{:.2})",
                    d.region,
                    d.expected,
                    (got - d.expected).abs()
                )),
                None => lines.push(format!(
                    "  - {} produced NO finite value (golden {})",
                    d.region, d.expected
                )),
            }
        }
        lines.push(format!("  change classes: {:?}", self.change_classes));
        lines.push(format!("  to approve, run: {}", self.accept_command));
        lines.join("\n")
    }
}

/// Build the exact copy-pasteable `--accept` command a reviewer runs to re-baseline
/// a blocked case (D-10). The Phase-94 CLI consumes this string verbatim.
#[must_use]
pub fn accept_command(case_id: &str) -> String {
    format!(
        "compile-workbook --accept --case {case_id} --approver <YOU> --effective-date <YYYY-MM-DD>"
    )
}

/// The PROMOTE-TIME numeric gate (WBGV-04). Compare every named output region in
/// `case.expected_outputs` to the candidate's `computed` value at ±[`TOL`] and PASS
/// when every region reconciles OR a fingerprint-bound [`ApprovalRecord`] covers
/// THIS candidate. Otherwise return a [`GateDecision::Blocked`] carrying the deltas,
/// the change classes, the fingerprint, and the exact `--accept` command (D-10).
///
/// COLLECT-ALL: every blocking region is surfaced in one decision (never fail-fast).
///
/// # The "no inherited approval" property (T-93-06-INHERIT)
///
/// The candidate fingerprint folds the `prev_bundle_hash`, the
/// `candidate_workbook_hash`, AND the per-region deltas. A LATER UNRELATED change
/// produces a different fingerprint → [`approval_matches`] returns false → the gate
/// blocks again. A prior approval can NEVER be inherited by a change it did not
/// approve.
#[must_use]
pub fn gate(
    case: &ApprovalCase,
    computed: &BTreeMap<String, f64>,
    candidate_workbook_hash: &str,
    prev_bundle_hash: &str,
    change_classes: &[ChangeClass],
    approvals: &[ApprovalRecord],
) -> GateDecision {
    let deltas: BTreeMap<String, RegionDelta> = region_deltas(case, computed);
    let fingerprint = candidate_fingerprint(prev_bundle_hash, candidate_workbook_hash, &deltas);
    let approved = approval_matches(approvals, &case.case_id, &fingerprint);

    let mut blocking: Vec<GateDelta> = Vec::new();
    for (region, &expected) in &case.expected_outputs {
        match computed.get(region).copied() {
            Some(got) if got.is_finite() => {
                let delta = (got - expected).abs();
                if delta > TOL && !approved {
                    blocking.push(GateDelta {
                        region: region.clone(),
                        expected,
                        computed: Some(got),
                    });
                }
            },
            // A missing / non-finite output is a HARD block regardless of approval
            // (an approval cannot certify a value the candidate did not compute).
            _ => blocking.push(GateDelta {
                region: region.clone(),
                expected,
                computed: None,
            }),
        }
    }

    if blocking.is_empty() {
        GateDecision::Pass { fingerprint }
    } else {
        GateDecision::Blocked(Box::new(GateBlock {
            case_id: case.case_id.clone(),
            deltas: blocking,
            change_classes: change_classes.to_vec(),
            fingerprint,
            accept_command: accept_command(&case.case_id),
        }))
    }
}
