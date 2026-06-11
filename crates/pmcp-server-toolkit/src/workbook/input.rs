//! The strict `calculate`/`explain` input DTO + fail-closed tier enforcement
//! (WBSV-06, WR-02/WR-05/V4).
//!
//! The wire input is `{ "inputs": { <json_key>: <value>, … },
//! "overrides": { <param_key>: <value>, … } }`. The DTO is
//! `#[serde(deny_unknown_fields)]` (`additionalProperties:false`): an unknown
//! TOP-LEVEL key is rejected as `invalid_input`.
//!
//! Every gate fails CLOSED — a `?`-or-reject arm, NEVER an `if let Some(...)`
//! skip that fails open:
//!
//! - **WR-05** — a supplied input whose `cell_map` entry's `seed_coord` has no
//!   matching manifest role is rejected (the manifest and cell_map are separate
//!   embedded artifacts and can skew across a partial regeneration; a roleless
//!   seed would bypass the dtype + enum gates).
//! - **WR-02** — the enum-membership test is STRING-ONLY: a non-string value
//!   tested against a string `allowed_values` set can never be a member and is
//!   rejected (a SKEWED `Dtype::Number` + `allowed_values` manifest fails closed
//!   here rather than silently seeding).
//! - **V4** — a per-call override of a strict-constant (BA-governed) cell is
//!   rejected via [`pmcp_workbook_runtime::is_strict_constant`].
//!
//! Every rejection populates a self-repair field (`allowed`/`range`/`required`)
//! so the `isError` envelope carries it.

// Compiler/clippy-enforced panic-freedom on the value path (mirrors the runtime).
#![cfg_attr(
    not(test),
    deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)
)]

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::Value;

use pmcp_workbook_runtime::{
    is_computed, is_strict_constant, CellMap, CellRole, CellValue, Dtype, InputTier, Manifest, Role,
};

use super::error::WorkbookToolError;

/// The strict wire input DTO: `inputs` (the per-call values keyed by their
/// neutral `json_key`) and optional `overrides` (variable-tier param tweaks).
/// `deny_unknown_fields` rejects any other top-level key.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CalculateInput {
    /// The inputs keyed by their neutral `json_key` (the manifest `Role::Input`
    /// columns). Defaults empty (the manifest tier defaults fill any omitted
    /// input).
    #[serde(default)]
    pub inputs: BTreeMap<String, Value>,
    /// Variable-tier parameter overrides. A strict constant key here is rejected
    /// (V4).
    #[serde(default)]
    pub overrides: BTreeMap<String, Value>,
}

/// A validated, tier-checked input ready to seed the
/// [`pmcp_workbook_runtime`] executor. `seeds` is `cell_key -> value` (the inputs
/// mapped through the embedded `cell_map`, plus any accepted variable-tier
/// overrides).
#[derive(Debug, Clone)]
pub struct ValidatedInput {
    /// `cell_key -> seed value` for the executor `CellEnv`.
    pub seeds: BTreeMap<String, Value>,
    /// The accepted variable-tier override keys (for explain/audit).
    pub accepted_overrides: Vec<String>,
    /// The NORMALIZED canonical wire DTO (caller `inputs` + accepted `overrides`,
    /// both `BTreeMap`-ordered → deterministic). This is the SAME shape
    /// [`validate_input`] accepts, so it round-trips.
    pub canonical_dto: Value,
}

/// Validate + tier-check the raw tool args against the manifest + cell_map,
/// fail-closed.
///
/// # Errors
///
/// Returns a [`WorkbookToolError`] (NOT an `Err` the handler surfaces as a
/// protocol error — the handler renders it as `isError:true`) when:
/// - arg-parse / `deny_unknown_fields` fails (`invalid_input`);
/// - an `inputs` key is not a known `cell_map` `json_key` (`invalid_input`, WR-05);
/// - a supplied input's `seed_coord` has no manifest role (`invalid_input`, WR-05);
/// - a value fails its declared dtype or closed-enum membership (`invalid_input`,
///   WR-01/WR-02);
/// - an `overrides` key resolves to a strict constant (`strict_constant_override`,
///   V4) or to no manifest cell (`unsupported_option`).
#[allow(clippy::result_large_err)]
pub fn validate_input(
    args: Value,
    manifest: &Manifest,
    cell_map: &CellMap,
) -> Result<ValidatedInput, WorkbookToolError> {
    let input: CalculateInput = serde_json::from_value(args)
        .map_err(|e| WorkbookToolError::invalid_input(format!("invalid arguments: {e}")))?;

    let mut seeds: BTreeMap<String, Value> = BTreeMap::new();

    // 1. Seed every manifest input with its tier default (so an omitted input
    //    still resolves), then overlay the caller-supplied values.
    for role in &manifest.cells {
        if matches!(role.role, Role::Input) {
            if let Some(default) = tier_default(role) {
                seeds.insert(role.cell.clone(), default);
            }
        }
    }

    // 2. Map each supplied input key → seed_coord via the cell_map. WR-05: an
    //    unknown input key is `invalid_input` (a bad FIELD).
    for (key, value) in &input.inputs {
        let entry = cell_map
            .inputs
            .iter()
            .find(|e| &e.json_key == key)
            .ok_or_else(|| {
                WorkbookToolError::invalid_input_field(key.clone(), known_input_keys(cell_map))
            })?;
        // WR-05 (fail-closed): the supplied input's seed_coord MUST have a
        // manifest role — `?`-or-reject, NEVER an if-let-Some skip. A roleless
        // seed would bypass the dtype + enum gates below.
        let role =
            pmcp_workbook_runtime::role_for_cell(manifest, &entry.seed_coord).ok_or_else(|| {
                WorkbookToolError::invalid_input(format!(
                    "internal: input '{key}' maps to {} which has no manifest role",
                    entry.seed_coord
                ))
            })?;
        check_value_dtype(role, key, value)?;
        seeds.insert(entry.seed_coord.clone(), value.clone());
    }

    // 3. Tier-check each override; accept variable-tier, reject strict constants.
    let mut accepted_overrides = Vec::new();
    for (key, value) in &input.overrides {
        match find_role_by_key(manifest, key) {
            Some(r) if is_strict_constant(r) => {
                return Err(WorkbookToolError::strict_constant_override(
                    key.clone(),
                    variable_tier_keys(manifest),
                ));
            },
            // WR-02: reject an override targeting a computed cell — seeding one
            // would (after 92-06's seed-preserving executor) let a caller pin a
            // served output under a valid provenance stamp (output forging). The
            // shared `is_computed` predicate is the same one `variable_tier_keys`
            // filters on, so the reject gate and the advertised allow-list cannot
            // drift. A forbidden-role override surfaces the same machine-actionable
            // allowed-list as an unknown key.
            Some(r) if is_computed(r) => {
                return Err(WorkbookToolError::unsupported_option(
                    key.clone(),
                    variable_tier_keys(manifest),
                ));
            },
            Some(r) => {
                check_value_dtype(r, key, value)?;
                seeds.insert(r.cell.clone(), value.clone());
                accepted_overrides.push(key.clone());
            },
            None => {
                return Err(WorkbookToolError::unsupported_option(
                    key.clone(),
                    variable_tier_keys(manifest),
                ));
            },
        }
    }

    let canonical_dto = serde_json::json!({
        "inputs": &input.inputs,
        "overrides": &input.overrides,
    });

    Ok(ValidatedInput {
        seeds,
        accepted_overrides,
        canonical_dto,
    })
}

/// The tier default value for an input cell (`None` if the cell carries no tier).
fn tier_default(role: &CellRole) -> Option<Value> {
    match &role.tier {
        Some(InputTier::Variable { default })
        | Some(InputTier::BoundedVariable { default, .. }) => cell_value_to_json(default),
        None => None,
    }
}

/// Map a manifest [`CellValue`] default to a JSON value.
fn cell_value_to_json(v: &CellValue) -> Option<Value> {
    match v {
        CellValue::Number(n) => serde_json::Number::from_f64(*n).map(Value::Number),
        CellValue::Text(s) => Some(Value::String(s.clone())),
        CellValue::Bool(b) => Some(Value::Bool(*b)),
        CellValue::Empty => Some(Value::Null),
        CellValue::Error(_) => None,
    }
}

/// WR-01/WR-02: type-check a caller-supplied JSON value against the declared
/// [`Dtype`] of the manifest [`CellRole`] it will seed, then (for a frozen input)
/// enum-membership, BEFORE it reaches the evaluator. A `null` carries no type
/// information — the evaluator's empty-cell semantics handle it. The enum gate is
/// STRING-ONLY: a non-string value can never be a member of a string
/// `allowed_values` set, so a SKEWED `Dtype::Number` + `allowed_values` manifest
/// fails closed here (WR-02).
#[allow(clippy::result_large_err)]
fn check_value_dtype(role: &CellRole, field: &str, value: &Value) -> Result<(), WorkbookToolError> {
    if value.is_null() {
        return Ok(());
    }
    let ok = match role.dtype {
        Dtype::Number => value.is_number(),
        Dtype::Text => value.is_string(),
        Dtype::Bool => value.is_boolean(),
    };
    if !ok {
        let expected = super::schema::dtype_json_type(role.dtype);
        return Err(WorkbookToolError::invalid_input(format!(
            "input '{field}' must be a {expected} (cell {} is declared {expected})",
            role.cell
        )));
    }
    // Enum-membership gate (WR-02): STRING-ONLY membership runs AFTER the dtype
    // gate. A non-string value can never be a member, so a skewed manifest fails
    // closed here.
    if let Some(allowed) = &role.allowed_values {
        let is_member = value
            .as_str()
            .is_some_and(|s| allowed.iter().any(|a| a == s));
        if !is_member {
            return Err(WorkbookToolError::invalid_enum(
                field,
                allowed.clone(),
                format!(
                    "input '{field}' must be one of the allowed values \
                     (cell {} is a closed enum)",
                    role.cell
                ),
            ));
        }
    }
    Ok(())
}

/// Find a manifest [`CellRole`] for an override key by its `name` or its
/// fully-qualified `cell` key only (NOT by free-text `meaning` — ambiguous).
fn find_role_by_key<'a>(manifest: &'a Manifest, key: &str) -> Option<&'a CellRole> {
    manifest
        .cells
        .iter()
        .find(|r| r.name.as_deref() == Some(key) || r.cell == key)
}

/// The variable-tier override keys a caller MAY set (the
/// `strict_constant_override` allowed alternatives).
fn variable_tier_keys(manifest: &Manifest) -> Vec<String> {
    manifest
        .cells
        .iter()
        .filter(|r| !is_strict_constant(r) && !is_computed(r))
        .filter_map(|r| r.name.clone().or_else(|| Some(r.cell.clone())))
        .collect()
}

/// The known input `json_key`s (for an unknown-field allowed-list).
fn known_input_keys(cell_map: &CellMap) -> Vec<String> {
    cell_map.inputs.iter().map(|e| e.json_key.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pmcp_workbook_runtime::{CellEntry, CellMap};
    use proptest::prelude::*;
    use serde_json::json;

    // ---- Tax-domain fixtures (S-4: gross_income / filing_status; zero customer
    //      identifiers) ---------------------------------------------------------

    fn input_role(
        cell: &str,
        dtype: Dtype,
        name: &str,
        tier: Option<InputTier>,
        allowed: Option<Vec<String>>,
    ) -> CellRole {
        CellRole {
            cell: cell.to_string(),
            role: Role::Input,
            name: Some(name.to_string()),
            unit: None,
            meaning: None,
            dtype,
            colour_evidence: None,
            source: "test".to_string(),
            notes: None,
            tier,
            allowed_values: allowed,
        }
    }

    fn manifest() -> Manifest {
        Manifest {
            schema_version: 1,
            workflow: "tax-calc".to_string(),
            workbook_hash: None,
            ratified: true,
            ratified_by: None,
            ratified_at: None,
            cells: vec![
                input_role(
                    "1_Inputs!B2",
                    Dtype::Number,
                    "gross_income",
                    Some(InputTier::Variable {
                        default: CellValue::Number(0.0),
                    }),
                    None,
                ),
                input_role(
                    "1_Inputs!B3",
                    Dtype::Text,
                    "filing_status",
                    Some(InputTier::Variable {
                        default: CellValue::Text("single".to_string()),
                    }),
                    Some(vec![
                        "single".to_string(),
                        "married_joint".to_string(),
                        "head_of_household".to_string(),
                    ]),
                ),
                // A strict (BA-governed) constant: Role::Constant + tier None.
                CellRole {
                    cell: "2_Rates!B2".to_string(),
                    role: Role::Constant,
                    name: Some("const_rate".to_string()),
                    unit: None,
                    meaning: None,
                    dtype: Dtype::Number,
                    colour_evidence: None,
                    source: "test".to_string(),
                    notes: None,
                    tier: None,
                    allowed_values: None,
                },
            ],
            loop_block: None,
            governed_data: vec![],
            changelog: vec![],
            capability_calls: vec![],
            annotations: vec![],
        }
    }

    fn cell_map() -> CellMap {
        CellMap {
            inputs: vec![
                CellEntry {
                    json_key: "gross_income".to_string(),
                    seed_coord: "1_Inputs!B2".to_string(),
                    unit: Some("USD".to_string()),
                },
                CellEntry {
                    json_key: "filing_status".to_string(),
                    seed_coord: "1_Inputs!B3".to_string(),
                    unit: None,
                },
            ],
            outputs: vec![CellEntry {
                json_key: "tax_owed".to_string(),
                seed_coord: "3_Outputs!B3".to_string(),
                unit: Some("USD".to_string()),
            }],
        }
    }

    #[test]
    fn valid_inputs_seed_their_cells() {
        let args = json!({ "inputs": { "gross_income": 60000.0 } });
        let v = validate_input(args, &manifest(), &cell_map()).expect("valid");
        assert_eq!(v.seeds.get("1_Inputs!B2"), Some(&json!(60000.0)));
    }

    #[test]
    fn unknown_top_level_field_is_rejected() {
        let args = json!({ "bogus": 1 });
        let err = validate_input(args, &manifest(), &cell_map())
            .expect_err("unknown top-level field rejected (deny_unknown_fields)");
        assert_eq!(err.code, "invalid_input");
    }

    #[test]
    fn unknown_input_key_is_invalid_input_with_allowed() {
        // WR-05: an unknown input KEY is a bad FIELD → invalid_input.
        let args = json!({ "inputs": { "not_a_real_input": 1 } });
        let err = validate_input(args, &manifest(), &cell_map())
            .expect_err("an unknown input key is rejected (WR-05)");
        assert_eq!(err.code, "invalid_input");
        assert_eq!(err.field.as_deref(), Some("not_a_real_input"));
        assert!(err.allowed.is_some(), "carries the known input keys");
    }

    #[test]
    fn cell_map_entry_without_manifest_role_is_rejected_fail_closed() {
        // WR-05: a cell_map input whose seed_coord has NO manifest role must be
        // rejected (fail-closed, NOT an if-let-Some skip).
        let mut cm = cell_map();
        cm.inputs.push(CellEntry {
            json_key: "orphan".to_string(),
            seed_coord: "9_Nowhere!Z99".to_string(),
            unit: None,
        });
        let args = json!({ "inputs": { "orphan": "oops" } });
        let err = validate_input(args, &manifest(), &cm)
            .expect_err("a cell_map entry with no manifest role is rejected (WR-05)");
        assert_eq!(err.code, "invalid_input");
        assert!(
            err.reason.contains("no manifest role") && err.reason.contains("9_Nowhere!Z99"),
            "the error names the internal-consistency failure: {}",
            err.reason
        );
    }

    #[test]
    fn non_numeric_value_for_number_cell_is_rejected() {
        let args = json!({ "inputs": { "gross_income": "oops" } });
        let err = validate_input(args, &manifest(), &cell_map())
            .expect_err("a non-numeric value for a numeric input is rejected (WR-01)");
        assert_eq!(err.code, "invalid_input");
        assert!(err.reason.contains("number"), "names the expected type");
    }

    #[test]
    fn out_of_enum_value_is_rejected_with_allowed() {
        let args = json!({ "inputs": { "filing_status": "alien" } });
        let err = validate_input(args, &manifest(), &cell_map())
            .expect_err("an out-of-enum value is rejected (WR-02)");
        assert_eq!(err.code, "invalid_input");
        assert_eq!(err.field.as_deref(), Some("filing_status"));
        assert_eq!(
            err.allowed,
            Some(vec![
                "single".to_string(),
                "married_joint".to_string(),
                "head_of_household".to_string(),
            ]),
            "the allowed enum members live in the error"
        );
    }

    #[test]
    fn in_enum_value_passes_the_gate() {
        for legal in ["single", "married_joint", "head_of_household"] {
            let args = json!({ "inputs": { "filing_status": legal } });
            let v = validate_input(args, &manifest(), &cell_map())
                .expect("an in-enum value passes the membership gate");
            assert_eq!(v.seeds.get("1_Inputs!B3"), Some(&json!(legal)));
        }
    }

    #[test]
    fn non_string_value_on_string_enum_is_rejected_fail_closed() {
        // WR-02: a SKEWED manifest (Dtype::Number + allowed_values) must fail
        // CLOSED — a number can never be a member of a string enum.
        let mut m = manifest();
        // skew filing_status to Dtype::Number while keeping its string enum.
        m.cells[1].dtype = Dtype::Number;
        let args = json!({ "inputs": { "filing_status": 42 } });
        let err = validate_input(args, &m, &cell_map())
            .expect_err("a non-string value on a string-enum input is rejected (WR-02)");
        assert_eq!(err.code, "invalid_input");
        assert_eq!(err.field.as_deref(), Some("filing_status"));
        assert!(
            err.allowed.is_some(),
            "still carries the allowed repair field"
        );
    }

    #[test]
    fn strict_constant_override_is_rejected() {
        // V4: a per-call override of a strict-constant cell is rejected.
        let args = json!({ "overrides": { "const_rate": 0.40 } });
        let err = validate_input(args, &manifest(), &cell_map())
            .expect_err("a strict-constant override is rejected (V4)");
        assert_eq!(err.code, "strict_constant_override");
        assert_eq!(err.field.as_deref(), Some("const_rate"));
        assert!(err.allowed.is_some(), "carries variable-tier alternatives");
    }

    #[test]
    fn override_naming_no_cell_is_unsupported_option() {
        let args = json!({ "overrides": { "ghost_param": 1 } });
        let err = validate_input(args, &manifest(), &cell_map())
            .expect_err("an override naming no manifest cell is unsupported_option");
        assert_eq!(err.code, "unsupported_option");
    }

    /// A manifest extending the tax fixture with a `Role::Output` cell (`tax_owed`)
    /// and a `Role::Formula` cell (`taxable_income`) — the two computed roles an
    /// override must never be allowed to seed (WR-02).
    fn manifest_with_computed_cells() -> Manifest {
        let mut m = manifest();
        for (cell, role, name) in [
            ("3_Outputs!B3", Role::Output, "tax_owed"),
            ("3_Outputs!B2", Role::Formula, "taxable_income"),
        ] {
            m.cells.push(CellRole {
                role,
                unit: Some("USD".to_string()),
                ..input_role(cell, Dtype::Number, name, None, None)
            });
        }
        m
    }

    #[test]
    fn override_on_computed_cell_is_rejected_unsupported_option() {
        // WR-02: an override targeting a computed (Role::Output/Role::Formula) cell
        // is the live output-forging vector after 92-06 (a seeded value now wins over
        // the IR formula). It must be rejected with unsupported_option and NEVER
        // appear in accepted_overrides. Target each by name and by cell key.
        for key in ["tax_owed", "3_Outputs!B3", "taxable_income", "3_Outputs!B2"] {
            let args = json!({ "overrides": { key: 999.0 } });
            let err = validate_input(args, &manifest_with_computed_cells(), &cell_map())
                .expect_err("a computed-cell override is rejected (WR-02)");
            assert_eq!(err.code, "unsupported_option", "key {key} rejected");
            // The allow-list it surfaces never offers a computed key.
            let allowed = err
                .allowed
                .clone()
                .expect("carries the variable-tier allowed-list");
            assert!(
                !allowed.iter().any(|k| {
                    ["tax_owed", "3_Outputs!B3", "taxable_income", "3_Outputs!B2"]
                        .contains(&k.as_str())
                }),
                "a computed key is never offered as an allowed override (key {key}): {allowed:?}"
            );
        }
    }

    // ---- Gemini Excel-edge seeds: empty-string vs null --------------------

    #[test]
    fn empty_string_for_enum_input_is_rejected() {
        // An empty string is NOT a valid enum member — it must not be silently
        // coerced to a legal member.
        let args = json!({ "inputs": { "filing_status": "" } });
        let err = validate_input(args, &manifest(), &cell_map())
            .expect_err("an empty string for an enum input is rejected");
        assert_eq!(err.code, "invalid_input");
        assert_eq!(err.field.as_deref(), Some("filing_status"));
    }

    #[test]
    fn null_for_required_input_is_handled_by_empty_cell_semantics() {
        // A JSON null carries no type — it passes the dtype/enum gates (the
        // evaluator's empty-cell semantics handle it). It does NOT spuriously
        // reject, and it does NOT seed a bogus typed value.
        let args = json!({ "inputs": { "gross_income": null } });
        let v = validate_input(args, &manifest(), &cell_map())
            .expect("null passes the gate (empty-cell semantics)");
        assert_eq!(v.seeds.get("1_Inputs!B2"), Some(&Value::Null));
    }

    #[test]
    fn null_for_enum_input_is_not_silently_coerced() {
        // null on an enum input passes the present-only gate (no membership check
        // runs on null), but it is seeded as null — never coerced to a member.
        let args = json!({ "inputs": { "filing_status": null } });
        let v = validate_input(args, &manifest(), &cell_map())
            .expect("null on an enum input passes (empty-cell semantics)");
        assert_eq!(v.seeds.get("1_Inputs!B3"), Some(&Value::Null));
    }

    // ---- proptest fuzz: validate_input is TOTAL over adversarial inputs ----

    /// An arbitrary JSON scalar/array generator covering the adversarial shapes
    /// (mixed types, out-of-range numbers, oversized strings) plus the seeded
    /// Excel coercion edges (empty string, null).
    fn arb_json_value() -> impl Strategy<Value = Value> {
        prop_oneof![
            Just(Value::Null),
            Just(json!("")),
            any::<bool>().prop_map(Value::Bool),
            any::<f64>()
                .prop_filter("finite", |n| n.is_finite())
                .prop_map(|n| json!(n)),
            ".*".prop_map(Value::String),
            prop::collection::vec(any::<i64>(), 0..4).prop_map(|v| json!(v)),
        ]
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(512))]

        /// T-92-09: validate_input is TOTAL over arbitrary/adversarial input maps
        /// — it NEVER panics and ALWAYS returns either Ok or a WorkbookToolError.
        /// Random keys (some valid json_keys, some unknown), mixed JSON value
        /// types, and the empty-string/null edges are all covered.
        #[test]
        fn prop_validate_input_total(
            keys in prop::collection::vec(
                prop_oneof![
                    Just("gross_income".to_string()),
                    Just("filing_status".to_string()),
                    Just("const_rate".to_string()),
                    "[a-z_]{1,12}",
                ],
                0..5,
            ),
            vals in prop::collection::vec(arb_json_value(), 0..5),
            use_overrides in any::<bool>(),
        ) {
            let mut map = serde_json::Map::new();
            for (k, v) in keys.iter().zip(vals.iter()) {
                map.insert(k.clone(), v.clone());
            }
            let bucket = if use_overrides { "overrides" } else { "inputs" };
            let args = json!({ bucket: Value::Object(map) });

            // The ONLY contract: total + fail-closed. No panic; Ok or Err.
            match validate_input(args, &manifest(), &cell_map()) {
                Ok(_) | Err(_) => {},
            }
        }

        /// The empty-string and null edges are deterministically covered in the
        /// proptest corpus by feeding them directly on both an enum and a numeric
        /// input — validate_input stays total.
        #[test]
        fn prop_excel_edge_cases_are_total(
            edge in prop_oneof![Just(json!("")), Just(Value::Null)],
            on_enum in any::<bool>(),
        ) {
            let key = if on_enum { "filing_status" } else { "gross_income" };
            let args = json!({ "inputs": { key: edge } });
            match validate_input(args, &manifest(), &cell_map()) {
                Ok(_) | Err(_) => {},
            }
        }
    }
}
