//! Tool schema builders (WBSV-07) — the mandatory non-empty `outputSchema` + the
//! per-tool input schema, projected ENTIRELY from the embedded
//! [`Manifest`](pmcp_workbook_runtime::Manifest) +
//! [`CellMap`](pmcp_workbook_runtime::CellMap).
//!
//! There is NO privileged headline field (S-1): the output schema projects ALL
//! named outputs uniformly from `cell_map.outputs`, each as a `{ value, unit }`
//! pair carrying its declared `unit`/`meaning`. The input-schema envelope is
//! strict (`additionalProperties: false`) and mirrors the runtime DTO gate so a
//! client trusting the schema never sends a key the runtime then rejects.

// Compiler/clippy-enforced panic-freedom on the value path (mirrors the runtime).
#![cfg_attr(
    not(test),
    deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)
)]

use std::collections::HashSet;

use serde_json::{json, Map, Value};

use pmcp_workbook_runtime::{CellEntry, CellMap, CellRole, Dtype, Manifest, Tool};

/// Map a manifest [`Dtype`] to its JSON Schema primitive type string. `pub(crate)`
/// so input.rs's type-check reuses the SAME `Dtype`→string mapping (one place).
pub(crate) fn dtype_json_type(dtype: Dtype) -> &'static str {
    match dtype {
        Dtype::Number => "number",
        Dtype::Text => "string",
        Dtype::Bool => "boolean",
    }
}

/// Find the manifest [`CellRole`] for a `cell_map` entry's seed coordinate —
/// the runtime's shared exact-cell-key lookup.
fn role_for_seed<'a>(manifest: &'a Manifest, seed_coord: &str) -> Option<&'a CellRole> {
    pmcp_workbook_runtime::role_for_cell(manifest, seed_coord)
}

/// Build the per-output-column schema for the `calculate` result `outputs` map.
///
/// `project_outputs` (handler.rs) emits each column as a `{ value, unit }` pair,
/// NOT a bare typed scalar — so the advertised schema MUST describe that nested
/// shape or a client validating the result rejects every column.
fn output_column_schema(unit: Option<&str>, role: Option<&CellRole>) -> Value {
    let dtype = role.map_or(Dtype::Number, |r| r.dtype);
    let mut value_prop = Map::new();
    value_prop.insert("type".to_string(), json!(dtype_json_type(dtype)));
    if let Some(u) = unit {
        value_prop.insert("unit".to_string(), json!(u));
    }

    let mut props = Map::new();
    props.insert("value".to_string(), Value::Object(value_prop));
    props.insert("unit".to_string(), json!({ "type": ["string", "null"] }));

    let mut col = Map::new();
    col.insert("type".to_string(), json!("object"));
    col.insert("additionalProperties".to_string(), json!(false));
    col.insert("properties".to_string(), Value::Object(props));
    col.insert("required".to_string(), json!(["value"]));
    if let Some(meaning) = role.and_then(|r| r.meaning.as_deref()) {
        col.insert("description".to_string(), json!(meaning));
    }
    Value::Object(col)
}

/// Build the mandatory non-empty `outputSchema` (WBSV-07) from the embedded
/// [`Manifest`] + [`CellMap`].
///
/// S-1: ALL named outputs are projected uniformly from `cell_map.outputs` (keyed
/// by their neutral `json_key`) — there is NO privileged top-level headline
/// field. Each column projects to a `{ value, unit }` pair carrying its declared
/// `unit`/`meaning`. The error-envelope fields ride in the SAME `structuredContent`
/// slot, so [`result_envelope_schema`] folds them in (the result root is
/// `additionalProperties:true` and `required:["provenance"]`).
#[must_use]
pub fn output_schema_for_manifest(manifest: &Manifest, cell_map: &CellMap) -> Value {
    // The union of every tool's outputs (the workbook-WIDE output surface), kept for
    // the meta/generalization consumers (the WBEX-01 reemit proofs). The per-TOOL
    // served schema is `output_schema_for_tool`.
    let all_outputs: Vec<CellEntry> = cell_map
        .tools
        .iter()
        .flat_map(|t| t.outputs.iter().cloned())
        .collect();
    let output_props = output_props_for_entries(manifest, &all_outputs);

    let mut success = Map::new();
    success.insert(
        "outputs".to_string(),
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": Value::Object(output_props),
        }),
    );
    success.insert(
        "accepted_overrides".to_string(),
        json!({ "type": "array", "items": { "type": "string" } }),
    );
    result_envelope_schema(success)
}

/// Build the per-output schema map for a set of [`CellEntry`] output columns —
/// the shared projection [`output_schema_for_manifest`] and
/// [`output_schema_for_tool`] both use.
fn output_props_for_entries(manifest: &Manifest, outputs: &[CellEntry]) -> Map<String, Value> {
    let mut output_props = Map::new();
    for entry in outputs {
        let role = role_for_seed(manifest, &entry.seed_coord);
        output_props.insert(
            entry.json_key.clone(),
            output_column_schema(entry.unit.as_deref(), role),
        );
    }
    output_props
}

/// Build ONE tool's non-empty `outputSchema` (WBSV-07 / WBV2-04) over the tool's
/// OWN `outputs` only (NOT the union across tools). Each output Table becomes its
/// own MCP tool, so its schema enumerates exactly that Table's output columns —
/// the TypedToolWithOutput dual-surface invariant (every tool emits a non-empty
/// outputSchema → `structuredContent`).
#[must_use]
pub fn output_schema_for_tool(manifest: &Manifest, tool: &Tool) -> Value {
    let output_props = output_props_for_entries(manifest, &tool.outputs);

    let mut success = Map::new();
    success.insert(
        "outputs".to_string(),
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": Value::Object(output_props),
        }),
    );
    success.insert(
        "accepted_overrides".to_string(),
        json!({ "type": "array", "items": { "type": "string" } }),
    );
    result_envelope_schema(success)
}

/// Build a tool result `outputSchema` that accepts BOTH the tool's success shape
/// AND the shared `isError` envelope, generalizing the contract to every tool.
///
/// Each tool contributes only its SUCCESS-specific properties in `success_props`;
/// this builder folds in the shared parts every tool's result carries:
/// - the error-envelope fields (`isError`/`code`/`reason`/`field`/`allowed`/
///   `range`/`required`) — the error rides in the SAME `structuredContent` slot,
///   so a strict client validating an ERROR result must accept it;
/// - the `provenance` stamp — present on success AND error;
/// - `additionalProperties:true` (the success and error key sets are disjoint, so
///   the root cannot be closed) and `required:["provenance"]` (the ONLY field on
///   both paths).
#[must_use]
pub fn result_envelope_schema(success_props: Map<String, Value>) -> Value {
    let mut props = success_props;
    // ---- shared isError envelope fields ----
    props.insert("isError".to_string(), json!({ "type": "boolean" }));
    props.insert("code".to_string(), json!({ "type": "string" }));
    props.insert("reason".to_string(), json!({ "type": "string" }));
    props.insert("field".to_string(), json!({ "type": "string" }));
    props.insert(
        "allowed".to_string(),
        json!({ "type": "array", "items": {} }),
    );
    props.insert("range".to_string(), json!({ "type": "array" }));
    props.insert(
        "required".to_string(),
        json!({ "type": "array", "items": { "type": "string" } }),
    );
    // ---- always present (success AND error) ----
    props.insert("provenance".to_string(), provenance_schema());

    json!({
        "type": "object",
        "additionalProperties": true,
        "properties": Value::Object(props),
        "required": ["provenance"],
    })
}

/// The `explain` result `outputSchema` (WBSV-07), composed over the shared result
/// envelope. The success-specific fields are the ordered `steps` trace + the
/// generic manifest-declared `annotations` object (S-2 — any domain-specific
/// keystone step is generalized into manifest-declared annotations).
#[must_use]
pub fn explain_output_schema() -> Value {
    let mut success = Map::new();
    success.insert(
        "steps".to_string(),
        json!({
            "type": "array",
            "description": "Ordered business-language derivation steps.",
            "items": {
                "type": "object",
                "additionalProperties": true,
                "properties": {
                    "step": { "type": "string" },
                    "cell": { "type": "string" },
                },
            },
        }),
    );
    success.insert(
        "annotations".to_string(),
        json!({
            "type": "object",
            "description": "Manifest-declared annotations (keyed by AnnotationDecl name).",
            "additionalProperties": { "type": "object" },
        }),
    );
    result_envelope_schema(success)
}

/// The `get_manifest` result `outputSchema` (WBSV-07), composed over the shared
/// result envelope. `get_manifest` has no domain-error trigger today, but
/// composing the SAME envelope keeps every tool's schema uniform.
#[must_use]
pub fn get_manifest_output_schema() -> Value {
    let mut success = Map::new();
    success.insert("bundle_id".to_string(), json!({ "type": "string" }));
    success.insert("version".to_string(), json!({ "type": "string" }));
    success.insert("combined_hash".to_string(), json!({ "type": "string" }));
    for field in ["inputs", "outputs", "governed_data", "changelog"] {
        success.insert(
            field.to_string(),
            json!({ "type": "array", "items": { "type": "object" } }),
        );
    }
    result_envelope_schema(success)
}

/// The `diff_version` result `outputSchema` (WBSV-07), composed over the shared
/// result envelope. The success-specific fields describe the served recorded
/// changelog: `from_version`/`to_version`, `deltas` (per-output machine records),
/// and a human-readable `summary`.
#[must_use]
pub fn diff_version_output_schema() -> Value {
    let mut success = Map::new();
    success.insert("from_version".to_string(), json!({ "type": "string" }));
    success.insert("to_version".to_string(), json!({ "type": "string" }));
    success.insert(
        "deltas".to_string(),
        json!({
            "type": "array",
            "description": "Per-output change records (region, change class, old/new \
                            meaning+unit+provenance, drift/redefinition severity).",
            "items": {
                "type": "object",
                "additionalProperties": true,
                "properties": {
                    "region": { "type": "string" },
                    "change_class": { "type": "string" },
                    "severity": { "type": "string" },
                },
            },
        }),
    );
    success.insert(
        "summary".to_string(),
        json!({ "type": "string", "description": "Human-readable transition summary." }),
    );
    result_envelope_schema(success)
}

/// The `render_workbook` output schema (WBSV-05, WBSV-07): a `workbook://`
/// resource-URI POINTER + its MIME type — NOT the `.xlsx` bytes (those arrive on
/// `resources/read`). Non-empty so the `outputSchema`-advertise contract holds.
#[must_use]
pub fn render_workbook_output_schema() -> Value {
    let mut success = Map::new();
    success.insert(
        "resource_uri".to_string(),
        json!({
            "type": "string",
            "description": "A provenance-bound workbook:// resource URI. Read it via \
                            resources/read to obtain the base64-encoded .xlsx, which is \
                            regenerated statelessly from the URI on each read. The URI \
                            encodes the inputs — treat it as sensitive.",
        }),
    );
    success.insert(
        "mime_type".to_string(),
        json!({
            "type": "string",
            "description": "The MIME type of the resource the URI resolves to (the OOXML \
                            spreadsheet type).",
        }),
    );
    result_envelope_schema(success)
}

/// The provenance stamp sub-schema — present on every result. Carries
/// `bundle_id`/`version`/`combined_hash` (NEVER `workbook_hash` — Codex HIGH #3).
#[must_use]
pub fn provenance_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "bundle_id": { "type": "string" },
            "version": { "type": "string" },
            "combined_hash": { "type": "string" },
        },
        "required": ["bundle_id", "version", "combined_hash"],
    })
}

/// The strict input schema for `calculate`/`explain`: an `object` with
/// `additionalProperties:false` accepting only the manifest `Role::Input`
/// columns (by their neutral `json_key`) plus an optional `overrides` map for
/// variable-tier params. The DTO's `deny_unknown_fields` is the runtime gate;
/// this schema mirrors it for discovery.
#[must_use]
pub fn input_schema_for_manifest(manifest: &Manifest, cell_map: &CellMap) -> Value {
    let mut input_props = Map::new();
    for entry in &cell_map.inputs {
        input_props.insert(
            entry.json_key.clone(),
            input_prop_for_entry(manifest, entry),
        );
    }
    assemble_input_schema(manifest, input_props)
}

/// Build the strict per-tool input schema (WBV2-04): an `object` with
/// `additionalProperties:false` carrying ONLY this tool's DAG-derived
/// `input_keys` (the subset of the shared `cell_map.inputs` pool transitively
/// reachable upstream of this tool's outputs), plus the F2 `overrides` block.
///
/// The strict envelope (`additionalProperties:false`, V5) MUST survive: a client
/// trusting the advertised schema must never be able to send a key the runtime
/// then rejects.
#[must_use]
pub fn input_schema_for_tool(manifest: &Manifest, cell_map: &CellMap, tool: &Tool) -> Value {
    // O(1) membership for the per-tool key projection (was a linear scan of
    // `input_keys` per input — O(inputs × keys)).
    let reached: HashSet<&str> = tool.input_keys.iter().map(String::as_str).collect();
    // CR-02 defense-in-depth: a tool with an EMPTY input_keys advertised an empty
    // inputs.properties while `validate_input` accepts the full shared pool — the
    // served schema was STRICTER than the runtime (the V5 invariant inverted). When
    // no DAG derivation populated input_keys (a hand-built / single-tool fallback
    // bundle), project the FULL shared-input pool ("no derivation" => "all shared
    // inputs") so the advertised schema is never stricter than what the runtime
    // accepts. The production multi-tool path always populates input_keys, so this
    // fires only for the fallback shape.
    let project_all = tool.input_keys.is_empty();
    let mut input_props = Map::new();
    for entry in &cell_map.inputs {
        // Project this tool's DAG-derived keys (or the full pool when empty — CR-02).
        if project_all || reached.contains(entry.json_key.as_str()) {
            input_props.insert(
                entry.json_key.clone(),
                input_prop_for_entry(manifest, entry),
            );
        }
    }
    assemble_input_schema(manifest, input_props)
}

/// Build the typed JSON-Schema property for one input [`CellEntry`] — its dtype,
/// unit, meaning, and (for a frozen input) closed-enum domain. Shared by the
/// manifest-level and per-tool input-schema builders so the per-input shape
/// cannot drift.
fn input_prop_for_entry(manifest: &Manifest, entry: &CellEntry) -> Value {
    let role = role_for_seed(manifest, &entry.seed_coord);
    let dtype = role.map_or(Dtype::Number, |r| r.dtype);
    let mut prop = Map::new();
    prop.insert("type".to_string(), json!(dtype_json_type(dtype)));
    if let Some(unit) = entry.unit.as_deref() {
        prop.insert("unit".to_string(), json!(unit));
    }
    if let Some(meaning) = role.and_then(|r| r.meaning.as_deref()) {
        prop.insert("description".to_string(), json!(meaning));
    }
    // A frozen input (allowed_values from the workbook) advertises its closed
    // domain as a JSON-Schema enum, verbatim workbook order. The input stays
    // OPTIONAL — this fn builds no `required` array.
    if let Some(allowed) = role.and_then(|r| r.allowed_values.as_ref()) {
        prop.insert("enum".to_string(), json!(allowed));
    }
    Value::Object(prop)
}

/// Assemble the strict input-schema envelope from the already-built per-input
/// `properties` map: the `inputs` object (strict) + the F2 `overrides` block
/// (advertising the variable-tier override keys). Shared by the manifest-level
/// and per-tool builders.
fn assemble_input_schema(manifest: &Manifest, input_props: Map<String, Value>) -> Value {
    // F2: ADVERTISE the legal override keys so an LLM/caller can DISCOVER them,
    // rather than leaving `overrides` an opaque open map described in prose only.
    // The keys are the SAME variable-tier list `validate_input` accepts (one source
    // of truth — `crate::workbook::input::variable_tier_keys`), so advertisement and
    // acceptance cannot drift. `additionalProperties` stays permissive (the open
    // value-typed map) so this is a DISCOVERABILITY change only — `validate_input`'s
    // accept/reject behavior is unchanged.
    let mut override_props = Map::new();
    for key in crate::workbook::input::variable_tier_keys(manifest) {
        override_props.insert(
            key,
            json!({ "type": ["number", "string", "boolean", "null"] }),
        );
    }

    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "inputs": {
                "type": "object",
                "additionalProperties": false,
                "properties": Value::Object(input_props),
            },
            "overrides": {
                "type": "object",
                "additionalProperties": { "type": ["number", "string", "boolean", "null"] },
                "properties": Value::Object(override_props),
                "description": "Variable-tier parameter overrides, keyed by parameter \
                                name or cell key. Strict (BA-governed) constants are rejected.",
            },
        },
    })
}

/// `get_manifest`/`diff_version` have no input — an empty strict object schema.
#[must_use]
pub fn empty_input_schema() -> Value {
    json!({ "type": "object", "additionalProperties": false })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pmcp_workbook_runtime::CellValue;
    use pmcp_workbook_runtime::{CellEntry, CellMap, InputTier, Role, Tool};

    fn input_role(
        cell: &str,
        dtype: Dtype,
        meaning: &str,
        allowed: Option<Vec<String>>,
    ) -> CellRole {
        CellRole {
            cell: cell.to_string(),
            role: Role::Input,
            name: None,
            unit: Some("USD".to_string()),
            meaning: Some(meaning.to_string()),
            dtype,
            colour_evidence: None,
            source: "test".to_string(),
            notes: None,
            tier: Some(InputTier::Variable {
                default: CellValue::Number(0.0),
            }),
            allowed_values: allowed,
        }
    }

    fn output_role(cell: &str, meaning: &str) -> CellRole {
        CellRole {
            cell: cell.to_string(),
            role: Role::Output,
            name: None,
            unit: Some("USD".to_string()),
            meaning: Some(meaning.to_string()),
            dtype: Dtype::Number,
            colour_evidence: None,
            source: "test".to_string(),
            notes: None,
            tier: None,
            allowed_values: None,
        }
    }

    fn manifest_with(cells: Vec<CellRole>) -> Manifest {
        Manifest {
            schema_version: 1,
            workflow: "tax-calc".to_string(),
            workbook_hash: None,
            ratified: true,
            ratified_by: None,
            ratified_at: None,
            cells,
            loop_block: None,
            governed_data: vec![],
            changelog: vec![],
            capability_calls: vec![],
            annotations: vec![],
        }
    }

    fn three_input_manifest_and_map() -> (Manifest, CellMap) {
        let manifest = manifest_with(vec![
            input_role("1_Inputs!B2", Dtype::Number, "Gross income", None),
            input_role(
                "1_Inputs!B3",
                Dtype::Text,
                "Filing status",
                Some(vec!["single".to_string(), "married_joint".to_string()]),
            ),
            input_role("1_Inputs!B4", Dtype::Number, "Deductions", None),
            output_role("3_Outputs!B2", "Taxable income"),
            output_role("3_Outputs!B3", "Tax owed"),
        ]);
        let cell_map = CellMap {
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
                CellEntry {
                    json_key: "deductions".to_string(),
                    seed_coord: "1_Inputs!B4".to_string(),
                    unit: Some("USD".to_string()),
                },
            ],
            tools: vec![Tool {
                name: "calculate".to_string(),
                description: None,
                input_keys: Vec::new(),
                outputs: vec![
                    CellEntry {
                        json_key: "taxable_income".to_string(),
                        seed_coord: "3_Outputs!B2".to_string(),
                        unit: Some("USD".to_string()),
                    },
                    CellEntry {
                        json_key: "tax_owed".to_string(),
                        seed_coord: "3_Outputs!B3".to_string(),
                        unit: Some("USD".to_string()),
                    },
                ],
                oracle: std::collections::BTreeMap::new(),
            }],
        };
        (manifest, cell_map)
    }

    #[test]
    fn input_schema_is_strict_and_projects_all_inputs() {
        let (m, cm) = three_input_manifest_and_map();
        let schema = input_schema_for_manifest(&m, &cm);
        // additionalProperties:false at the root (strict envelope).
        assert_eq!(schema["additionalProperties"], false);
        let props = &schema["properties"]["inputs"]["properties"];
        // One typed property per input, dtype/unit/meaning carried.
        assert_eq!(props["gross_income"]["type"], json!("number"));
        assert_eq!(props["gross_income"]["unit"], json!("USD"));
        assert_eq!(props["gross_income"]["description"], json!("Gross income"));
        assert_eq!(props["filing_status"]["type"], json!("string"));
        assert_eq!(props["deductions"]["type"], json!("number"));
        // The inputs object is also strict.
        assert_eq!(
            schema["properties"]["inputs"]["additionalProperties"],
            false
        );
    }

    #[test]
    fn input_schema_emits_enum_for_allowed_values_and_keeps_it_optional() {
        let (m, cm) = three_input_manifest_and_map();
        let schema = input_schema_for_manifest(&m, &cm);
        let props = &schema["properties"]["inputs"]["properties"];
        assert_eq!(
            props["filing_status"]["enum"],
            json!(["single", "married_joint"]),
            "allowed_values surfaces as a JSON-Schema enum (verbatim order)"
        );
        // A non-enum input grows no enum key.
        assert!(props["gross_income"].get("enum").is_none());
        // The enum input is NOT required.
        assert!(schema["properties"]["inputs"].get("required").is_none());
    }

    #[test]
    fn output_schema_is_non_empty_and_carries_every_named_output() {
        let (m, cm) = three_input_manifest_and_map();
        let schema = output_schema_for_manifest(&m, &cm);
        let outputs = &schema["properties"]["outputs"]["properties"];
        // Every named output is present as a { value, unit } column.
        assert!(outputs["taxable_income"].is_object());
        assert!(outputs["tax_owed"].is_object());
        assert_eq!(
            outputs["taxable_income"]["properties"]["value"]["unit"],
            "USD"
        );
        assert_eq!(
            outputs["taxable_income"]["properties"]["value"]["type"],
            "number"
        );
        // S-1: the success root enumerates exactly outputs/accepted_overrides
        // (+ the shared envelope), with NO privileged headline scalar elevated
        // above the uniform all-outputs projection. The forbidden headline key
        // name is built dynamically so the literal does not appear in this file.
        let headline_key = ["supply", "_", "total"].concat();
        assert!(
            schema["properties"].get(&headline_key).is_none(),
            "no privileged headline field at the root (S-1)"
        );
        // The outputSchema is non-empty (WBSV-07): it has properties + provenance.
        assert!(schema["properties"]["provenance"].is_object());
        assert!(
            !outputs
                .as_object()
                .expect("outputs is an object")
                .is_empty(),
            "outputSchema must enumerate at least one output"
        );
    }

    #[test]
    fn result_envelope_accepts_both_success_and_iserror_shapes() {
        let (m, cm) = three_input_manifest_and_map();
        let schema = output_schema_for_manifest(&m, &cm);
        // The error-envelope fields are declared so an error result validates.
        assert_eq!(schema["properties"]["isError"]["type"], "boolean");
        assert_eq!(schema["properties"]["code"]["type"], "string");
        assert_eq!(schema["properties"]["reason"]["type"], "string");
        // The root cannot be closed (success/error key sets are disjoint).
        assert_eq!(schema["additionalProperties"], true);
        // provenance is the only required field (present on both paths).
        assert_eq!(schema["required"], json!(["provenance"]));
    }

    #[test]
    fn overrides_advertise_variable_tier_keys() {
        // F2: the overrides block carries a `properties` map keyed by the legal
        // variable-tier override keys (the SAME list validate_input accepts).
        let (m, cm) = three_input_manifest_and_map();
        let schema = input_schema_for_manifest(&m, &cm);
        let override_props = &schema["properties"]["overrides"]["properties"];
        let props = override_props
            .as_object()
            .expect("overrides.properties is an object");
        // The three variable-tier inputs (Some(Variable) tier, not computed) are
        // advertised by their `variable_tier_keys` identity (name.or(cell) — here
        // the cell key, since these fixtures carry no `name`).
        for key in ["1_Inputs!B2", "1_Inputs!B3", "1_Inputs!B4"] {
            assert!(
                props.contains_key(key),
                "overrides advertises the variable-tier key `{key}` (got {props:?})"
            );
            assert_eq!(
                override_props[key]["type"],
                json!(["number", "string", "boolean", "null"]),
                "each advertised override carries the permissive value-type union"
            );
        }
        // Computed outputs are NEVER advertised as overridable (WR-02).
        assert!(
            !props.contains_key("3_Outputs!B2") && !props.contains_key("taxable_income"),
            "a computed output is never an advertised override key"
        );
    }

    #[test]
    fn overrides_advertise_named_param_keys() {
        // With a NAMED variable-tier input, the advertised override key is the
        // human param name (variable_tier_keys uses name.or(cell)).
        let mut named = input_role("1_Inputs!B2", Dtype::Number, "Gross income", None);
        named.name = Some("in_gross_income".to_string());
        let m = manifest_with(vec![named, output_role("3_Outputs!B2", "Taxable income")]);
        let cm = CellMap {
            inputs: vec![CellEntry {
                json_key: "gross_income".to_string(),
                seed_coord: "1_Inputs!B2".to_string(),
                unit: Some("USD".to_string()),
            }],
            tools: vec![Tool {
                name: "calculate".to_string(),
                description: None,
                input_keys: Vec::new(),
                outputs: vec![CellEntry {
                    json_key: "taxable_income".to_string(),
                    seed_coord: "3_Outputs!B2".to_string(),
                    unit: Some("USD".to_string()),
                }],
                oracle: std::collections::BTreeMap::new(),
            }],
        };
        let schema = input_schema_for_manifest(&m, &cm);
        let override_props = &schema["properties"]["overrides"]["properties"];
        assert!(
            override_props["in_gross_income"].is_object(),
            "the named variable-tier param is advertised under its name"
        );
    }

    #[test]
    fn overrides_keep_open_additional_properties_for_discoverability_only() {
        // F2 is advertisement-only: the open value-typed additionalProperties map
        // is PRESERVED (validate_input's accept/reject behavior is unchanged) and
        // the prose description stays.
        let (m, cm) = three_input_manifest_and_map();
        let schema = input_schema_for_manifest(&m, &cm);
        let overrides = &schema["properties"]["overrides"];
        assert_eq!(
            overrides["additionalProperties"],
            json!({ "type": ["number", "string", "boolean", "null"] }),
            "the open value-typed additionalProperties map is preserved"
        );
        assert!(
            overrides["description"].as_str().is_some(),
            "the prose override description is retained"
        );
    }

    #[test]
    fn provenance_schema_uses_combined_hash_never_workbook_hash() {
        let schema = provenance_schema();
        let props = &schema["properties"];
        assert!(props["combined_hash"].is_object());
        assert!(
            props.get("workbook_hash").is_none(),
            "the provenance schema must never carry workbook_hash (Codex HIGH #3)"
        );
        assert_eq!(
            schema["required"],
            json!(["bundle_id", "version", "combined_hash"])
        );
    }

    #[test]
    fn empty_input_keys_projects_full_pool() {
        // CR-02 defense-in-depth: a Tool with EMPTY input_keys must advertise the
        // FULL shared-input pool (never an empty inputs.properties while
        // validate_input accepts the pool — the served schema can never be stricter
        // than the runtime). three_input_manifest_and_map's single tool has empty
        // input_keys.
        let (m, cm) = three_input_manifest_and_map();
        let tool = &cm.tools[0];
        assert!(
            tool.input_keys.is_empty(),
            "the fixture tool has no derived keys"
        );
        let schema = input_schema_for_tool(&m, &cm, tool);
        let props = schema["properties"]["inputs"]["properties"]
            .as_object()
            .expect("inputs.properties object");
        assert!(
            !props.is_empty(),
            "an empty-input_keys tool advertises the full pool, not an empty schema"
        );
        // Every shared input key is advertised (the full pool projection).
        for key in ["gross_income", "filing_status", "deductions"] {
            assert!(
                props.contains_key(key),
                "the full shared-input pool is advertised: missing {key}"
            );
        }
        // The strict envelope survives (V5).
        assert_eq!(
            schema["properties"]["inputs"]["additionalProperties"],
            json!(false),
            "the strict per-tool envelope is preserved"
        );
    }

    #[test]
    fn populated_input_keys_projects_only_reached() {
        // The complement: a populated input_keys projects EXACTLY those keys (the
        // production multi-tool shape — unchanged by the CR-02 fallback).
        let (m, mut cm) = three_input_manifest_and_map();
        cm.tools[0].input_keys = vec!["gross_income".to_string()];
        let schema = input_schema_for_tool(&m, &cm, &cm.tools[0]);
        let props = schema["properties"]["inputs"]["properties"]
            .as_object()
            .expect("inputs.properties object");
        assert_eq!(props.len(), 1, "only the reached key is projected");
        assert!(props.contains_key("gross_income"));
        assert!(!props.contains_key("deductions"));
    }
}
