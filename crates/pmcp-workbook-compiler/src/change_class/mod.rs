//! The CHANGE-CLASS router (GATE-03) — the SEMANTIC axis of the promote-time
//! trust boundary.
//!
//! Change-class is AUTO-DERIVED (no human labeling, D-08) by diffing the prior vs
//! current [`Manifest`] + compiled IR into the six [`ChangeClass`] variants, then a
//! per-class routing policy (D-09) decides each change's allowed action:
//!
//! | class               | derived from                                            | policy             |
//! |---------------------|---------------------------------------------------------|--------------------|
//! | `GovernedData`      | a `GovernedDatum.value` changed                         | `HotReload`        |
//! | `OutputSchema`      | a `Role::Output` `CellRole` meaning/unit/source changed | `BlockUntilAccept` |
//! | `FormulaLogic`      | a compiled IR (`Cell` AST) changed                      | `BlockUntilAccept` |
//! | `InputSchema`       | a `Role::Input` added / removed / retyped               | `BlockUntilAccept` |
//! | `CapabilityContract`| `capability_calls` changed                              | `BlockUntilAccept` |
//! | `Assumption`        | a `Role::Constant` `source == "yellow-assumption"` change | `NeverAutoPromote` |
//!
//! [`ChangeClass`] is the SHARED enum from [`pmcp_workbook_runtime::changelog`]
//! (re-exported `pmcp_workbook_compiler::ChangeClass`) — NOT a local
//! re-declaration — so the offline classifier and the served `diff_version` tool
//! share ONE definition.
//!
//! # CR-01: the classifier is SYMMETRIC (a security property)
//!
//! Assumption involvement on EITHER side → `Assumption`: a DEMOTION (a yellow
//! assumption re-classified to an ordinary constant) is exactly as review-critical
//! as a promotion — it routes to `Assumption → NeverAutoPromote`, never escaping
//! with zero classes (which would silently HotReload). Role flips AWAY from
//! Input/Output are schema changes too. The `Constant | Formula` arm no longer
//! silently drops a demotion (threat T-93-05-PROMO).
//!
//! When a single compile yields SEVERAL classes at once, [`effective_policy`]
//! collapses them with the strictest-policy reducer
//! `NeverAutoPromote > BlockUntilAccept > HotReload` (WBGV-02): a compile mixing
//! `GovernedData` + `FormulaLogic` resolves to `BlockUntilAccept`, and ANY
//! `Assumption` forces `NeverAutoPromote` regardless of what else changed.

use std::collections::HashMap;

use pmcp_workbook_runtime::manifest_model::{CellRole, GovernedDatum, Manifest, Role};
use pmcp_workbook_runtime::sheet_ir::Cell;
use pmcp_workbook_runtime::ChangeClass;

pub mod ir_identity;
pub mod schema_diff;

pub use ir_identity::ir_subdag_hash;
pub use schema_diff::diff_outputs;

/// The promote-time routing policy a [`ChangeClass`] maps to (D-09). The strictest
/// policy ([`NeverAutoPromote`](GatePolicy::NeverAutoPromote)) wins a multi-class
/// compile (see [`effective_policy`]).
///
/// Variants are declared in strictness order (least → most), so the derived
/// [`Ord`] IS the reducer ordering: `HotReload < BlockUntilAccept < NeverAutoPromote`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum GatePolicy {
    /// Hot-reload-eligible with `--accept` + effective-date + provenance
    /// (governed-data only).
    HotReload,
    /// Blocked until a BA `--accept` re-baselines (schema/formula/capability).
    BlockUntilAccept,
    /// HARD RULE — can NEVER auto-promote, even with `--accept` (assumptions).
    NeverAutoPromote,
}

/// The provenance marker on a `Role::Constant` that distinguishes a yellow
/// "assumption" from an ordinary governed constant.
const YELLOW_ASSUMPTION_SOURCE: &str = "yellow-assumption";

/// AUTO-DERIVE the change classes (D-08) by diffing the prior vs current manifest
/// and the compiled IR. Returns `Vec<(ChangeClass, region)>` in a DETERMINISTIC
/// order (sorted by the `(ChangeClass rank, region)` pair) with duplicate
/// `(class, region)` pairs deduped, so the SAME compile always yields the SAME
/// class list with no HashMap-iteration-order nondeterminism.
#[must_use]
pub fn classify(
    prev: &Manifest,
    current: &Manifest,
    prev_ir: &HashMap<String, Cell>,
    current_ir: &HashMap<String, Cell>,
) -> Vec<(ChangeClass, String)> {
    let mut out: Vec<(ChangeClass, String)> = Vec::new();

    classify_governed_data(prev, current, &mut out);
    classify_cell_roles(prev, current, &mut out);
    classify_formula_logic(prev_ir, current_ir, &mut out);
    classify_capabilities(prev, current, &mut out);

    // Canonical order: by (class rank, region), then dedup so the same compile is
    // byte-for-byte reproducible regardless of HashMap iteration order.
    out.sort_by(|a, b| {
        class_rank(a.0)
            .cmp(&class_rank(b.0))
            .then_with(|| a.1.cmp(&b.1))
    });
    out.dedup();
    out
}

/// Index a manifest's cells by their fully-qualified cell key.
fn index_cells(manifest: &Manifest) -> HashMap<&str, &CellRole> {
    manifest
        .cells
        .iter()
        .map(|c| (c.cell.as_str(), c))
        .collect()
}

/// Index a governed-data table by key.
fn index_governed(manifest: &Manifest) -> HashMap<&str, &GovernedDatum> {
    manifest
        .governed_data
        .iter()
        .map(|g| (g.key.as_str(), g))
        .collect()
}

/// `GovernedDatum.value` diff → `GovernedData` (keyed by the governed `key`). An
/// added / removed / value-changed governed datum is a governed-data change.
fn classify_governed_data(
    prev: &Manifest,
    current: &Manifest,
    out: &mut Vec<(ChangeClass, String)>,
) {
    let prev_g = index_governed(prev);
    let cur_g = index_governed(current);
    for (key, cur) in &cur_g {
        match prev_g.get(key) {
            Some(p) if p.value == cur.value => {},
            _ => out.push((ChangeClass::GovernedData, (*key).to_string())),
        }
    }
    for key in prev_g.keys() {
        if !cur_g.contains_key(key) {
            out.push((ChangeClass::GovernedData, (*key).to_string()));
        }
    }
}

/// Whether a `CellRole` is a yellow assumption (a `Role::Constant` carrying the
/// `yellow-assumption` source) — NOT a 5th `Role` variant.
fn is_assumption(role: &CellRole) -> bool {
    matches!(role.role, Role::Constant) && role.source == YELLOW_ASSUMPTION_SOURCE
}

/// The schema triple a redefinition compares: `(meaning, unit, source)`.
fn schema_triple(role: &CellRole) -> (&Option<String>, &Option<String>, &str) {
    (&role.meaning, &role.unit, role.source.as_str())
}

/// Whether a present-on-both-sides assumption cell actually changed (a triple /
/// dtype edit, or an assumption-status flip). A newly-added assumption (no prior
/// role) is always a change — handled by the `None => true` caller arm.
fn assumption_changed(prev: &CellRole, cur: &CellRole, prev_was_assumption: bool) -> bool {
    is_assumption(cur) != prev_was_assumption
        || schema_triple(prev) != schema_triple(cur)
        || prev.dtype != cur.dtype
}

/// Assumption FIRST (hard rule): assumption involvement on EITHER side is an
/// Assumption change (CR-01). A DEMOTION (yellow assumption re-classified to an
/// ordinary constant / any other role) is exactly as review-critical as a
/// promotion — re-classifying an assumption must route to Assumption →
/// `NeverAutoPromote`, never escape with zero classes (D-09 hard rule).
///
/// Returns `true` when the cell involved an assumption (and was thus fully handled
/// here), so the caller skips the role-specific arms for it.
fn classify_assumption(
    key: &str,
    cur: &CellRole,
    prev_role: Option<&CellRole>,
    out: &mut Vec<(ChangeClass, String)>,
) -> bool {
    let prev_was_assumption = prev_role.is_some_and(is_assumption);
    if !is_assumption(cur) && !prev_was_assumption {
        return false;
    }
    let changed = match prev_role {
        Some(p) => assumption_changed(p, cur, prev_was_assumption),
        None => true,
    };
    if changed {
        out.push((ChangeClass::Assumption, key.to_string()));
    }
    true
}

/// Role flips AWAY from Input/Output are schema changes too (CR-01): an
/// input/output retyped to Constant/Formula silently leaves the served schema
/// unless classified here. The symmetric flips TO Input/Output are caught by
/// [`classify_current_role`]; duplicates dedup in [`classify`].
fn classify_role_flip_away(
    key: &str,
    cur: &CellRole,
    prev_role: Option<&CellRole>,
    out: &mut Vec<(ChangeClass, String)>,
) {
    let Some(p) = prev_role else { return };
    if matches!(p.role, Role::Input) && !matches!(cur.role, Role::Input) {
        out.push((ChangeClass::InputSchema, key.to_string()));
    }
    if matches!(p.role, Role::Output) && !matches!(cur.role, Role::Output) {
        out.push((ChangeClass::OutputSchema, key.to_string()));
    }
}

/// Whether a current-`Output` cell is an output-schema redefinition: a flip TO
/// Output (from any other role) counts even when the `(meaning, unit, source)`
/// triple matches; an added cell (no prior role) is always a redefinition.
fn output_redefined(cur: &CellRole, prev_role: Option<&CellRole>) -> bool {
    match prev_role {
        Some(p) => !matches!(p.role, Role::Output) || schema_triple(p) != schema_triple(cur),
        None => true,
    }
}

/// Whether a current-`Input` cell is retyped: a role flip, a dtype change, OR an
/// enum-domain change (ENUM-07 — an `allowed_values` add/remove/flip on an
/// existing input is a SCHEMA-axis change that must route through InputSchema →
/// `BlockUntilAccept`). An added cell (no prior role) is always retyped.
fn input_retyped(cur: &CellRole, prev_role: Option<&CellRole>) -> bool {
    match prev_role {
        Some(p) => {
            !matches!(p.role, Role::Input)
                || p.dtype != cur.dtype
                || p.allowed_values != cur.allowed_values
        },
        None => true,
    }
}

/// Classify a current cell by its `Role` into `OutputSchema` / `InputSchema`
/// (Constant / Formula carry no role-schema change here).
fn classify_current_role(
    key: &str,
    cur: &CellRole,
    prev_role: Option<&CellRole>,
    out: &mut Vec<(ChangeClass, String)>,
) {
    match cur.role {
        Role::Output if output_redefined(cur, prev_role) => {
            out.push((ChangeClass::OutputSchema, key.to_string()));
        },
        Role::Input if input_retyped(cur, prev_role) => {
            out.push((ChangeClass::InputSchema, key.to_string()));
        },
        Role::Output | Role::Input | Role::Constant | Role::Formula => {},
    }
}

/// A cell present in `prev` but removed in `current` is a schema/assumption change.
fn classify_removed_cell(key: &str, prev: &CellRole, out: &mut Vec<(ChangeClass, String)>) {
    if is_assumption(prev) {
        out.push((ChangeClass::Assumption, key.to_string()));
    } else if matches!(prev.role, Role::Input) {
        out.push((ChangeClass::InputSchema, key.to_string()));
    } else if matches!(prev.role, Role::Output) {
        out.push((ChangeClass::OutputSchema, key.to_string()));
    }
}

/// Diff the per-cell roles into `OutputSchema` / `InputSchema` / `Assumption`
/// (CR-01 symmetric). Thin orchestrator over the per-decision helpers above.
fn classify_cell_roles(prev: &Manifest, current: &Manifest, out: &mut Vec<(ChangeClass, String)>) {
    let prev_c = index_cells(prev);
    let cur_c = index_cells(current);

    for (key, cur) in &cur_c {
        let prev_role = prev_c.get(key).copied();

        // Assumption involvement short-circuits the role-specific arms.
        if classify_assumption(key, cur, prev_role, out) {
            continue;
        }
        classify_role_flip_away(key, cur, prev_role, out);
        classify_current_role(key, cur, prev_role, out);
    }

    // Removed inputs / removed assumptions are schema/assumption changes too.
    for (key, prev) in &prev_c {
        if !cur_c.contains_key(key) {
            classify_removed_cell(key, prev, out);
        }
    }
}

/// Compiled-IR (`Cell` AST) diff → `FormulaLogic` (keyed by the cell key). An
/// added / removed / AST-changed cell is a formula-logic change.
fn classify_formula_logic(
    prev_ir: &HashMap<String, Cell>,
    current_ir: &HashMap<String, Cell>,
    out: &mut Vec<(ChangeClass, String)>,
) {
    for (key, cur) in current_ir {
        match prev_ir.get(key) {
            Some(p) if p.expr == cur.expr => {},
            _ => out.push((ChangeClass::FormulaLogic, key.clone())),
        }
    }
    for key in prev_ir.keys() {
        if !current_ir.contains_key(key) {
            out.push((ChangeClass::FormulaLogic, key.clone()));
        }
    }
}

/// `capability_calls` diff → `CapabilityContract` (keyed by the capability cell).
fn classify_capabilities(
    prev: &Manifest,
    current: &Manifest,
    out: &mut Vec<(ChangeClass, String)>,
) {
    let prev_caps: HashMap<&str, &_> = prev
        .capability_calls
        .iter()
        .map(|c| (c.cell.as_str(), c))
        .collect();
    let cur_caps: HashMap<&str, &_> = current
        .capability_calls
        .iter()
        .map(|c| (c.cell.as_str(), c))
        .collect();
    for (cell, cur) in &cur_caps {
        match prev_caps.get(cell) {
            Some(p) if **p == **cur => {},
            _ => out.push((ChangeClass::CapabilityContract, (*cell).to_string())),
        }
    }
    for cell in prev_caps.keys() {
        if !cur_caps.contains_key(cell) {
            out.push((ChangeClass::CapabilityContract, (*cell).to_string()));
        }
    }
}

/// A stable rank for a [`ChangeClass`] so [`classify`] can sort canonically (the
/// shared enum derives `Copy` but not `Ord`, so we project an explicit rank).
fn class_rank(class: ChangeClass) -> u8 {
    match class {
        ChangeClass::OutputSchema => 0,
        ChangeClass::GovernedData => 1,
        ChangeClass::FormulaLogic => 2,
        ChangeClass::InputSchema => 3,
        ChangeClass::CapabilityContract => 4,
        ChangeClass::Assumption => 5,
    }
}

/// The D-09 routing policy for a SINGLE change class, encoded as an EXHAUSTIVE
/// match (the compiler enforces that a new [`ChangeClass`] variant cannot be added
/// without a routing decision):
///
/// - `GovernedData` → [`GatePolicy::HotReload`]
/// - `OutputSchema` / `InputSchema` / `FormulaLogic` / `CapabilityContract` → [`GatePolicy::BlockUntilAccept`]
/// - `Assumption` → [`GatePolicy::NeverAutoPromote`]
#[must_use]
pub fn policy(class: ChangeClass) -> GatePolicy {
    match class {
        ChangeClass::GovernedData => GatePolicy::HotReload,
        ChangeClass::OutputSchema
        | ChangeClass::InputSchema
        | ChangeClass::FormulaLogic
        | ChangeClass::CapabilityContract => GatePolicy::BlockUntilAccept,
        ChangeClass::Assumption => GatePolicy::NeverAutoPromote,
    }
}

/// The MULTI-CLASS strictest-policy reducer (WBGV-02): collapse a set of change
/// classes to the STRICTEST policy via the ordering
/// `NeverAutoPromote > BlockUntilAccept > HotReload`.
///
/// - if ANY class maps to `NeverAutoPromote` → `NeverAutoPromote` (an `Assumption`
///   forces a hard block regardless of what else changed)
/// - else if ANY class maps to `BlockUntilAccept` → `BlockUntilAccept` (so a
///   `GovernedData` + `FormulaLogic` compile resolves to a BLOCK — the formula
///   block WINS over hot-reload)
/// - else → `HotReload`
///
/// An EMPTY set (no changes) is `HotReload` (nothing to block).
#[must_use]
pub fn effective_policy(classes: &[ChangeClass]) -> GatePolicy {
    // `GatePolicy`'s derived `Ord` is the strictness ordering, so the strictest
    // policy is simply the max; an empty set (no changes) blocks nothing.
    classes
        .iter()
        .map(|&class| policy(class))
        .max()
        .unwrap_or(GatePolicy::HotReload)
}

/// The BA-actionable BLOCK message for a routed change.
///
/// For [`ChangeClass::Assumption`] this emits a DISTINCT manual-human-review
/// message stating the change CANNOT be `--accept`-promoted. The other classes get
/// a message naming the class + the `--accept` re-baseline path (or, for
/// governed-data, the hot-reload path).
#[must_use]
pub fn block_message(class: ChangeClass, region: &str) -> String {
    match class {
        ChangeClass::Assumption => format!(
            "BLOCKED (assumption): the yellow-assumption cell `{region}` changed. This requires \
             MANUAL human review and CANNOT be promoted with `--accept` — assumptions never \
             auto-promote (D-09 hard rule). A reviewer must re-author the reference workbook \
             before this change can ship."
        ),
        ChangeClass::GovernedData => format!(
            "governed-data change at `{region}` is hot-reload-eligible — promote with `--accept` \
             (records effective-date + approver + provenance)."
        ),
        ChangeClass::OutputSchema
        | ChangeClass::InputSchema
        | ChangeClass::FormulaLogic
        | ChangeClass::CapabilityContract => format!(
            "BLOCKED ({class:?}): the schema/formula/capability change at `{region}` blocks \
             promotion until a reviewer re-baselines with `--accept`."
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pmcp_workbook_runtime::manifest_model::{CapabilityDecl, Dtype};
    use pmcp_workbook_runtime::sheet_ir::CellExpr;
    use pmcp_workbook_runtime::CellValue;

    fn empty_manifest() -> Manifest {
        Manifest {
            schema_version: 1,
            workflow: "tax-calc".to_string(),
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

    fn role(cell: &str, r: Role, source: &str) -> CellRole {
        CellRole {
            cell: cell.to_string(),
            role: r,
            name: None,
            unit: Some("USD".to_string()),
            meaning: Some("total amount".to_string()),
            dtype: Dtype::Number,
            colour_evidence: None,
            source: source.to_string(),
            notes: None,
            tier: None,
            allowed_values: None,
        }
    }

    fn governed(key: &str, value: f64) -> GovernedDatum {
        GovernedDatum {
            key: key.to_string(),
            value: CellValue::Number(value),
            effective_date: None,
            approved_by: Some("approver".to_string()),
            provenance: None,
        }
    }

    fn empty_ir() -> HashMap<String, Cell> {
        HashMap::new()
    }

    #[test]
    fn policy_table_routes_each_class() {
        assert_eq!(policy(ChangeClass::GovernedData), GatePolicy::HotReload);
        assert_eq!(
            policy(ChangeClass::OutputSchema),
            GatePolicy::BlockUntilAccept
        );
        assert_eq!(
            policy(ChangeClass::InputSchema),
            GatePolicy::BlockUntilAccept
        );
        assert_eq!(
            policy(ChangeClass::FormulaLogic),
            GatePolicy::BlockUntilAccept
        );
        assert_eq!(
            policy(ChangeClass::CapabilityContract),
            GatePolicy::BlockUntilAccept
        );
        assert_eq!(
            policy(ChangeClass::Assumption),
            GatePolicy::NeverAutoPromote
        );
    }

    #[test]
    fn governed_plus_formula_reduces_to_block() {
        // A multi-class compile mixing a lenient governed-data edit with a formula
        // change must NOT hot-reload.
        let eff = effective_policy(&[ChangeClass::GovernedData, ChangeClass::FormulaLogic]);
        assert_eq!(eff, GatePolicy::BlockUntilAccept);
        // Order-independent.
        let eff2 = effective_policy(&[ChangeClass::FormulaLogic, ChangeClass::GovernedData]);
        assert_eq!(eff2, GatePolicy::BlockUntilAccept);
    }

    #[test]
    fn any_assumption_forces_never_auto_promote() {
        let eff = effective_policy(&[
            ChangeClass::GovernedData,
            ChangeClass::FormulaLogic,
            ChangeClass::Assumption,
        ]);
        assert_eq!(eff, GatePolicy::NeverAutoPromote);
    }

    #[test]
    fn empty_change_set_is_hot_reload() {
        assert_eq!(effective_policy(&[]), GatePolicy::HotReload);
    }

    #[test]
    fn governed_value_change_classifies_as_governed_data() {
        let mut prev = empty_manifest();
        prev.governed_data = vec![governed("const_margin", 0.37)];
        let mut cur = empty_manifest();
        cur.governed_data = vec![governed("const_margin", 0.40)];
        let classes = classify(&prev, &cur, &empty_ir(), &empty_ir());
        assert_eq!(
            classes,
            vec![(ChangeClass::GovernedData, "const_margin".to_string())]
        );
    }

    #[test]
    fn yellow_assumption_change_classifies_as_assumption_not_governed() {
        let mut prev = empty_manifest();
        prev.cells = vec![role("2_Constants!B2", Role::Constant, "yellow-assumption")];
        let mut cur = empty_manifest();
        let mut changed = role("2_Constants!B2", Role::Constant, "yellow-assumption");
        changed.meaning = Some("REDEFINED assumption".to_string());
        cur.cells = vec![changed];

        let classes = classify(&prev, &cur, &empty_ir(), &empty_ir());
        assert_eq!(
            classes,
            vec![(ChangeClass::Assumption, "2_Constants!B2".to_string())]
        );
        assert_eq!(
            policy(ChangeClass::Assumption),
            GatePolicy::NeverAutoPromote
        );
        let msg = block_message(ChangeClass::Assumption, "2_Constants!B2");
        let lower = msg.to_lowercase();
        assert!(
            lower.contains("manual"),
            "assumption block message mentions manual review"
        );
        assert!(
            lower.contains("--accept") && (lower.contains("cannot") || lower.contains("never")),
            "assumption block message states it cannot be --accept-promoted"
        );
    }

    #[test]
    fn output_schema_change_classifies_as_output_schema() {
        let mut prev = empty_manifest();
        prev.cells = vec![role("3_Outputs!B3", Role::Output, "colour+guide")];
        let mut cur = empty_manifest();
        let mut changed = role("3_Outputs!B3", Role::Output, "colour+guide");
        changed.unit = Some("cents".to_string());
        cur.cells = vec![changed];
        let classes = classify(&prev, &cur, &empty_ir(), &empty_ir());
        assert_eq!(
            classes,
            vec![(ChangeClass::OutputSchema, "3_Outputs!B3".to_string())]
        );
    }

    #[test]
    fn added_input_classifies_as_input_schema() {
        let prev = empty_manifest();
        let mut cur = empty_manifest();
        cur.cells = vec![role("1_Inputs!B2", Role::Input, "colour+guide")];
        let classes = classify(&prev, &cur, &empty_ir(), &empty_ir());
        assert_eq!(
            classes,
            vec![(ChangeClass::InputSchema, "1_Inputs!B2".to_string())]
        );
    }

    #[test]
    fn assumption_demotion_to_plain_constant_classifies_as_assumption() {
        // CR-01: re-classifying a yellow assumption AWAY (source edited from
        // "yellow-assumption" to an ordinary constant source — same key, same
        // role, same value) must classify as Assumption → NeverAutoPromote.
        let mut prev = empty_manifest();
        prev.cells = vec![role("2_Constants!B2", Role::Constant, "yellow-assumption")];
        let mut cur = empty_manifest();
        cur.cells = vec![role("2_Constants!B2", Role::Constant, "colour+guide")];
        let classes = classify(&prev, &cur, &empty_ir(), &empty_ir());
        assert_eq!(
            classes,
            vec![(ChangeClass::Assumption, "2_Constants!B2".to_string())],
            "an assumption DEMOTION is an Assumption change (D-09 hard rule)"
        );
        assert_eq!(
            effective_policy(&[ChangeClass::Assumption]),
            GatePolicy::NeverAutoPromote
        );
    }

    #[test]
    fn input_demoted_to_constant_classifies_as_input_schema() {
        // CR-01: retyping an input AWAY from Role::Input silently removes it from
        // the served schema — that flip must classify as InputSchema.
        let mut prev = empty_manifest();
        prev.cells = vec![role("1_Inputs!B2", Role::Input, "colour+guide")];
        let mut cur = empty_manifest();
        cur.cells = vec![role("1_Inputs!B2", Role::Constant, "colour+guide")];
        let classes = classify(&prev, &cur, &empty_ir(), &empty_ir());
        assert_eq!(
            classes,
            vec![(ChangeClass::InputSchema, "1_Inputs!B2".to_string())]
        );
    }

    #[test]
    fn output_demoted_to_formula_classifies_as_output_schema() {
        // CR-01: retyping an output AWAY from Role::Output removes it from the
        // served output contract — that flip must classify as OutputSchema.
        let mut prev = empty_manifest();
        prev.cells = vec![role("3_Outputs!B3", Role::Output, "colour+guide")];
        let mut cur = empty_manifest();
        cur.cells = vec![role("3_Outputs!B3", Role::Formula, "colour+guide")];
        let classes = classify(&prev, &cur, &empty_ir(), &empty_ir());
        assert_eq!(
            classes,
            vec![(ChangeClass::OutputSchema, "3_Outputs!B3".to_string())]
        );
    }

    /// An `allowed_values`-aware variant of `role(...)` (ENUM-07).
    fn role_with_enum(cell: &str, r: Role, source: &str, values: &[&str]) -> CellRole {
        let mut c = role(cell, r, source);
        c.allowed_values = Some(values.iter().map(|s| (*s).to_string()).collect());
        c
    }

    #[test]
    fn enum_drop_with_role_flip_still_classifies_as_input_schema() {
        // CR-01: dropping the frozen enum TOGETHER with a role flip away from
        // Input must still produce an InputSchema class.
        let mut prev = empty_manifest();
        prev.cells = vec![role_with_enum(
            "1_Inputs!B3",
            Role::Input,
            "colour+guide",
            &["single", "married"],
        )];
        let mut cur = empty_manifest();
        cur.cells = vec![role("1_Inputs!B3", Role::Constant, "colour+guide")];
        let classes = classify(&prev, &cur, &empty_ir(), &empty_ir());
        assert_eq!(
            classes,
            vec![(ChangeClass::InputSchema, "1_Inputs!B3".to_string())]
        );
    }

    #[test]
    fn enum_domain_flip_classifies_as_input_schema() {
        // ENUM-07: an enum-domain change on an EXISTING input is a schema-axis
        // change: InputSchema → BlockUntilAccept.
        let mut prev = empty_manifest();
        prev.cells = vec![role_with_enum(
            "1_Inputs!B3",
            Role::Input,
            "colour+guide",
            &["single", "married"],
        )];
        let mut cur = empty_manifest();
        cur.cells = vec![role_with_enum(
            "1_Inputs!B3",
            Role::Input,
            "colour+guide",
            &["single", "married", "head_of_household"],
        )];
        let classes = classify(&prev, &cur, &empty_ir(), &empty_ir());
        assert_eq!(
            classes,
            vec![(ChangeClass::InputSchema, "1_Inputs!B3".to_string())]
        );
        assert_eq!(
            policy(ChangeClass::InputSchema),
            GatePolicy::BlockUntilAccept
        );
    }

    #[test]
    fn unchanged_enum_domain_is_not_a_schema_change() {
        // The complement guard: an IDENTICAL allowed_values set produces NO class.
        let mut prev = empty_manifest();
        prev.cells = vec![role_with_enum(
            "1_Inputs!B3",
            Role::Input,
            "colour+guide",
            &["single", "married"],
        )];
        let mut cur = empty_manifest();
        cur.cells = vec![role_with_enum(
            "1_Inputs!B3",
            Role::Input,
            "colour+guide",
            &["single", "married"],
        )];
        let classes = classify(&prev, &cur, &empty_ir(), &empty_ir());
        assert!(classes.is_empty(), "no change → no class: {classes:?}");
    }

    #[test]
    fn ir_ast_change_classifies_as_formula_logic() {
        let mut prev_ir = empty_ir();
        prev_ir.insert(
            "S!A1".to_string(),
            Cell {
                key: "S!A1".to_string(),
                expr: CellExpr::Literal(CellValue::Number(1.0)),
            },
        );
        let mut cur_ir = empty_ir();
        cur_ir.insert(
            "S!A1".to_string(),
            Cell {
                key: "S!A1".to_string(),
                expr: CellExpr::Literal(CellValue::Number(2.0)),
            },
        );
        let classes = classify(&empty_manifest(), &empty_manifest(), &prev_ir, &cur_ir);
        assert_eq!(
            classes,
            vec![(ChangeClass::FormulaLogic, "S!A1".to_string())]
        );
    }

    #[test]
    fn capability_change_classifies_as_capability_contract() {
        let mut prev = empty_manifest();
        prev.capability_calls = vec![CapabilityDecl {
            cell: "9_Cap!A1".to_string(),
            kind: "rust".to_string(),
            declared_contract: "v1".to_string(),
        }];
        let mut cur = empty_manifest();
        cur.capability_calls = vec![CapabilityDecl {
            cell: "9_Cap!A1".to_string(),
            kind: "rust".to_string(),
            declared_contract: "v2".to_string(),
        }];
        let classes = classify(&prev, &cur, &empty_ir(), &empty_ir());
        assert_eq!(
            classes,
            vec![(ChangeClass::CapabilityContract, "9_Cap!A1".to_string())]
        );
    }

    #[test]
    fn classify_is_deterministically_ordered_and_deduped() {
        let mut prev = empty_manifest();
        prev.governed_data = vec![governed("const_margin", 0.37)];
        prev.cells = vec![role("3_Outputs!B3", Role::Output, "colour+guide")];
        let mut cur = empty_manifest();
        cur.governed_data = vec![governed("const_margin", 0.40)];
        let mut out_changed = role("3_Outputs!B3", Role::Output, "colour+guide");
        out_changed.unit = Some("cents".to_string());
        cur.cells = vec![out_changed];

        let classes = classify(&prev, &cur, &empty_ir(), &empty_ir());
        // OutputSchema (rank 0) sorts before GovernedData (rank 1).
        assert_eq!(
            classes,
            vec![
                (ChangeClass::OutputSchema, "3_Outputs!B3".to_string()),
                (ChangeClass::GovernedData, "const_margin".to_string()),
            ]
        );
    }

    // ---- Symmetry-cardinality PROPERTY test (ALWAYS requirement) ----

    /// Build a manifest from a list of `(cell, role, source)` specs.
    fn manifest_from(specs: &[(&str, Role, &str)]) -> Manifest {
        let mut m = empty_manifest();
        m.cells = specs.iter().map(|(c, r, s)| role(c, *r, s)).collect();
        m
    }

    #[test]
    fn classify_is_symmetric_in_cardinality() {
        // CR-01 symmetry property: for a set of manifest-pair scenarios, the number
        // of classes produced by classify(A→B) must EQUAL the number produced by
        // classify(B→A) — a change is exactly as classifiable in either direction
        // (a demotion is never silently dropped while its inverse promotion fires).
        let scenarios: Vec<(Manifest, Manifest)> = vec![
            // Input ⇄ Constant flip.
            (
                manifest_from(&[("1_Inputs!B2", Role::Input, "colour+guide")]),
                manifest_from(&[("1_Inputs!B2", Role::Constant, "colour+guide")]),
            ),
            // Output ⇄ Formula flip.
            (
                manifest_from(&[("3_Outputs!B3", Role::Output, "colour+guide")]),
                manifest_from(&[("3_Outputs!B3", Role::Formula, "colour+guide")]),
            ),
            // Assumption ⇄ ordinary constant (demotion/promotion).
            (
                manifest_from(&[("2_Constants!B2", Role::Constant, "yellow-assumption")]),
                manifest_from(&[("2_Constants!B2", Role::Constant, "colour+guide")]),
            ),
            // Added ⇄ removed input.
            (
                empty_manifest(),
                manifest_from(&[("1_Inputs!B9", Role::Input, "colour+guide")]),
            ),
        ];

        for (a, b) in &scenarios {
            let fwd = classify(a, b, &empty_ir(), &empty_ir());
            let rev = classify(b, a, &empty_ir(), &empty_ir());
            assert_eq!(
                fwd.len(),
                rev.len(),
                "symmetry-cardinality: A→B produced {fwd:?} but B→A produced {rev:?}"
            );
            // And neither direction is silently empty — every scenario is a real change.
            assert!(
                !fwd.is_empty(),
                "A→B must produce at least one class: {fwd:?}"
            );
            assert!(
                !rev.is_empty(),
                "B→A must produce at least one class: {rev:?}"
            );
        }
    }
}
