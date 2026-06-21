//! The PURE tool-surface projection + render behind `cargo pmcp workbook explain`
//! (WBV2-06, §8 / H1). Dependency-light (compiler projection + `serde`/`anyhow`,
//! NO `clap`/`GlobalFlags`) so it mounts into the lib target via `#[path]` — the
//! `workbook_explain` example and the `workbook_explain` integration test reach
//! [`explain_workbook`]/[`format_tool_surface`] through that seam, NOT the bin-only
//! `commands::*` tree (mirrors the `templates_workbook_server` convention).
//!
//! ## What it previews (the served multi-tool surface, H1)
//!
//! The served binary fans a compiled bundle out into ONE MCP tool per output Excel
//! Table — each with a DAG-derived `inputSchema` (only the inputs that flow into that
//! Table's outputs) and a non-empty `outputSchema`. This module previews that surface
//! by driving the SAME production projection the served binary registers
//! ([`pmcp_workbook_compiler::project_tool_surface_from_workbook`] →
//! [`pmcp_workbook_compiler::build_tools`] / `json_key_for_role`), so the preview
//! CANNOT diverge from the served surface by construction (the H1 fix: there is no
//! bespoke A1 walker / classification heuristic any more — every tool name, per-tool
//! input key, and output key comes straight off the production `Tool` list).
//!
//! It writes NO bundle and runs no compile GATE — a pure read-only projection (the
//! production projection STOPS before ratify/reconcile/emit).

use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use pmcp_workbook_compiler::{
    json_key_for_role, project_tool_surface_from_workbook, sanitize_tool_name, CellRole, Dtype,
    Manifest, Role, Tool, ToolSurfaceProjection,
};

/// One input parameter on a previewed tool's `inputSchema`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputParam {
    /// The LLM-facing JSON key (the served, prefix-STRIPPED input key).
    pub key: String,
    /// The JSON-schema type (`"number"` | `"string"` | `"boolean"`).
    pub ty: String,
    /// The declared unit (`USD`/`rate`/…), when authored.
    pub unit: Option<String>,
    /// The closed enum domain (from the value cell's list data-validation), when any.
    pub enum_values: Option<Vec<String>>,
}

/// One output field on a previewed tool's `outputSchema`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputField {
    /// The LLM-facing JSON key (the served, prefix-STRIPPED output key).
    pub key: String,
    /// The JSON-schema type (`"number"` | `"string"` | `"boolean"`).
    pub ty: String,
    /// The declared unit, when authored.
    pub unit: Option<String>,
}

/// One previewed tool — the served projection of ONE output Excel Table (H1): its
/// sanitized MCP name, its caption description, and its per-tool input/output schema
/// (the inputs DAG-derived from the Table's output-cell formula references via the
/// production `build_tools`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolSurface {
    /// The MCP tool name (the output-Table name, sanitized to the MCP charset).
    pub name: String,
    /// The tool description (the caption cell above the Table), when authored.
    pub description: Option<String>,
    /// The minimal per-tool input parameters (DAG-derived), sorted by key.
    pub inputs: Vec<InputParam>,
    /// The output fields this tool projects, in authored order.
    pub outputs: Vec<OutputField>,
}

/// Project the workbook's served tool surface read-only, driving the SAME production
/// projection the served binary registers (H1).
///
/// # Errors
/// Returns an error if the compiler projection fails (ingest/synth/parse) or the
/// workbook declares no servable output.
pub fn explain_workbook(path: &Path) -> Result<Vec<ToolSurface>> {
    let projection = project_tool_surface_from_workbook(path)
        .with_context(|| format!("failed to project the served surface of {}", path.display()))?;
    let tools = project_tool_surface(&projection);
    if tools.is_empty() {
        anyhow::bail!(
            "{} declares no output Table — a served workbook must have at least one \
             output Table to expose a tool",
            path.display()
        );
    }
    Ok(tools)
}

/// Map the production [`ToolSurfaceProjection`] (the served `Tool` list + manifest)
/// into the BA-facing [`ToolSurface`] render DTOs (H1) — names, per-tool typed
/// inputs, and outputs are read STRAIGHT off the production projection, so explain
/// cannot drift from the served surface. Returns the tools in production order.
#[must_use]
pub fn project_tool_surface(projection: &ToolSurfaceProjection) -> Vec<ToolSurface> {
    projection
        .tools
        .iter()
        .map(|tool| tool_surface_from_production(tool, &projection.manifest))
        .collect()
}

/// Build ONE [`ToolSurface`] from a production [`Tool`] + the role-promoted
/// [`Manifest`]: the sanitized name, the caption description, the typed per-tool
/// inputs (each `input_key` resolved to its `Role::Input` `CellRole` for
/// dtype/unit/enum), and the typed outputs (each output `CellEntry` resolved to its
/// `CellRole` for dtype). Kept separate so [`project_tool_surface`] stays a thin map.
fn tool_surface_from_production(tool: &Tool, manifest: &Manifest) -> ToolSurface {
    let mut inputs: Vec<InputParam> = tool
        .input_keys
        .iter()
        .map(|key| input_param_for_key(key, manifest))
        .collect();
    inputs.sort_by(|a, b| a.key.cmp(&b.key));

    let outputs: Vec<OutputField> = tool
        .outputs
        .iter()
        .map(|entry| OutputField {
            key: entry.json_key.clone(),
            ty: output_dtype(manifest, &entry.seed_coord),
            unit: entry.unit.clone(),
        })
        .collect();

    ToolSurface {
        name: sanitize(&tool.name),
        description: tool.description.clone(),
        inputs,
        outputs,
    }
}

/// Resolve one served input `key` (the stripped `json_key`) to its typed
/// [`InputParam`] by finding the `Role::Input` [`CellRole`] whose served
/// [`json_key_for_role`] equals `key`, then reading its dtype/unit/enum. The match is
/// over the SERVED key (post-strip), exactly as the served schema builder resolves it.
fn input_param_for_key(key: &str, manifest: &Manifest) -> InputParam {
    let role = manifest
        .cells
        .iter()
        .filter(|c| c.role == Role::Input)
        .find(|c| json_key_for_role(c) == key);
    match role {
        Some(role) => InputParam {
            key: key.to_string(),
            ty: dtype_json_type(role.dtype),
            unit: role.unit.clone(),
            enum_values: role.allowed_values.clone(),
        },
        None => InputParam {
            key: key.to_string(),
            ty: "string".to_string(),
            unit: None,
            enum_values: None,
        },
    }
}

/// The JSON-schema type of the output cell at `seed_coord` (`number` when no role is
/// found — the served output schema's same defaulting).
fn output_dtype(manifest: &Manifest, seed_coord: &str) -> String {
    manifest
        .cells
        .iter()
        .find(|c: &&CellRole| c.cell == seed_coord)
        .map_or_else(|| "number".to_string(), |role| dtype_json_type(role.dtype))
}

/// Render `tools` as a String in the requested `format` (PURE — no stdout).
///
/// `"json"` serializes the [`ToolSurface`] list directly; `"text"` renders one
/// block per tool (name / description / `inputs:` / `outputs:`) per OQ-3.
///
/// # Errors
/// Returns an error for an unknown `format` (naming the valid `text`/`json` values),
/// or if JSON serialization fails.
pub fn format_tool_surface(tools: &[ToolSurface], format: &str) -> Result<String> {
    match format {
        "json" => serde_json::to_string_pretty(tools)
            .context("failed to serialize the tool surface to JSON"),
        "text" => Ok(render_text(tools)),
        other => anyhow::bail!("unknown --format `{other}` (expected `text` or `json`)"),
    }
}

/// Render the tool surface as human text (the BA preview, OQ-3): one block per tool
/// — `tool <name>`, its `description`, an `inputs:` list (`key: type [unit] [enum]`)
/// and an `outputs:` list (`key: type [unit]`).
fn render_text(tools: &[ToolSurface]) -> String {
    if tools.is_empty() {
        return "no served tools (the workbook declares no output Table)".to_string();
    }
    let mut out = String::new();
    for tool in tools {
        out.push_str(&format!("tool {}\n", tool.name));
        out.push_str(&format!(
            "  description: {}\n",
            tool.description.as_deref().unwrap_or("(none)")
        ));
        out.push_str("  inputs:\n");
        for p in &tool.inputs {
            out.push_str(&format!("    {}\n", render_input(p)));
        }
        out.push_str("  outputs:\n");
        for o in &tool.outputs {
            out.push_str(&format!("    {}\n", render_output(o)));
        }
    }
    out
}

/// `key: type [unit] [enum: a|b]` for one input parameter.
fn render_input(p: &InputParam) -> String {
    let mut s = format!("{}: {}", p.key, p.ty);
    if let Some(unit) = &p.unit {
        s.push_str(&format!(" [{unit}]"));
    }
    if let Some(values) = &p.enum_values {
        s.push_str(&format!(" [enum: {}]", values.join("|")));
    }
    s
}

/// `key: type [unit]` for one output field.
fn render_output(o: &OutputField) -> String {
    match &o.unit {
        Some(unit) => format!("{}: {} [{unit}]", o.key, o.ty),
        None => format!("{}: {}", o.key, o.ty),
    }
}

/// Map a [`Dtype`] to its JSON-schema type string.
fn dtype_json_type(dtype: Dtype) -> String {
    match dtype {
        Dtype::Number => "number",
        Dtype::Text => "string",
        Dtype::Bool => "boolean",
    }
    .to_string()
}

/// Sanitize an output-Table name to the MCP tool-name charset (the SAME shared
/// sanitizer the served registration + compiler collision lint call); falls back to
/// the raw name when unmappable (the preview still names the offender).
fn sanitize(raw: &str) -> String {
    sanitize_tool_name(raw).unwrap_or_else(|_| raw.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_tools() -> Vec<ToolSurface> {
        vec![
            ToolSurface {
                name: "calculate_tax".to_string(),
                description: Some("Compute federal tax".to_string()),
                inputs: vec![InputParam {
                    key: "income".to_string(),
                    ty: "number".to_string(),
                    unit: Some("USD".to_string()),
                    enum_values: None,
                }],
                outputs: vec![OutputField {
                    key: "tax_owed".to_string(),
                    ty: "number".to_string(),
                    unit: None,
                }],
            },
            ToolSurface {
                name: "estimate_refund".to_string(),
                description: None,
                inputs: vec![
                    InputParam {
                        key: "filing".to_string(),
                        ty: "string".to_string(),
                        unit: None,
                        enum_values: Some(vec!["single".to_string(), "married".to_string()]),
                    },
                    InputParam {
                        key: "withheld".to_string(),
                        ty: "number".to_string(),
                        unit: Some("USD".to_string()),
                        enum_values: None,
                    },
                ],
                outputs: vec![OutputField {
                    key: "refund".to_string(),
                    ty: "number".to_string(),
                    unit: Some("USD".to_string()),
                }],
            },
        ]
    }

    #[test]
    fn text_render_names_each_tool_and_its_schema() {
        let text = format_tool_surface(&sample_tools(), "text").expect("text render");
        assert!(text.contains("tool calculate_tax"));
        assert!(text.contains("tool estimate_refund"));
        assert!(text.contains("description: Compute federal tax"));
        assert!(text.contains("income: number [USD]"));
        assert!(text.contains("tax_owed: number"));
        assert!(text.contains("refund: number [USD]"));
    }

    #[test]
    fn text_render_shows_enum_domain() {
        let text = format_tool_surface(&sample_tools(), "text").expect("text render");
        assert!(text.contains("filing: string [enum: single|married]"));
    }

    #[test]
    fn text_render_shows_none_description() {
        let text = format_tool_surface(&sample_tools(), "text").expect("text render");
        assert!(text.contains("description: (none)"));
    }

    #[test]
    fn json_render_round_trips_the_surface() {
        let json = format_tool_surface(&sample_tools(), "json").expect("json render");
        let back: Vec<ToolSurface> = serde_json::from_str(&json).expect("deserialize back");
        assert_eq!(back, sample_tools());
    }

    #[test]
    fn unknown_format_errors_naming_valid_formats() {
        let err = format_tool_surface(&sample_tools(), "yaml").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("text"), "got: {msg}");
        assert!(msg.contains("json"), "got: {msg}");
    }

    #[test]
    fn empty_surface_text_says_no_tools() {
        let text = format_tool_surface(&[], "text").expect("text render");
        assert!(text.contains("no served tools"));
    }

    // ---- projection-equivalence property arm (H1) ----------------------------
    //
    // The explain projection maps the PRODUCTION `Tool.input_keys` (the `build_tools`
    // DAG-derived set) into `InputParam`s WITHOUT re-deriving them — so for any
    // (manifest, tools) the projected per-tool input-key SET equals the production
    // tool's `input_keys` set. This is the structural guarantee that explain cannot
    // diverge from `build_tools` by construction (the H1 fix).
    use pmcp_workbook_compiler::{CellEntry, CellRole};
    use proptest::prelude::*;

    fn manifest_with_inputs(keys: &[String]) -> Manifest {
        let cells = keys
            .iter()
            .enumerate()
            .map(|(i, key)| CellRole {
                cell: format!("In!B{}", i + 2),
                role: Role::Input,
                // `name == key` so json_key_for_role(role) == key (no prefix here).
                name: Some(key.clone()),
                unit: None,
                meaning: None,
                dtype: Dtype::Number,
                colour_evidence: None,
                source: "proptest".to_string(),
                notes: None,
                tier: None,
                allowed_values: None,
            })
            .collect();
        Manifest {
            schema_version: 1,
            workflow: "prop".to_string(),
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

    fn tool_with_keys(name: &str, keys: Vec<String>) -> Tool {
        Tool {
            name: name.to_string(),
            description: None,
            input_keys: keys,
            outputs: vec![CellEntry {
                json_key: "out".to_string(),
                seed_coord: "Calc!B1".to_string(),
                unit: None,
            }],
            oracle: std::collections::BTreeMap::new(),
        }
    }

    // A simple identifier key the json_key strip leaves untouched (no in_/out_ prefix,
    // non-empty, charset-safe) so json_key_for_role(role) round-trips to the key.
    fn key_strategy() -> impl Strategy<Value = String> {
        "[a-z][a-z0-9_]{0,7}".prop_filter("no governance prefix", |s| {
            !s.starts_with("in_") && !s.starts_with("out_")
        })
    }

    proptest! {
        #[test]
        fn projection_preserves_build_tools_input_keys(
            tool_keys in proptest::collection::vec(key_strategy(), 0..6)
        ) {
            // Dedup the keys (a Tool's input_keys is a set; duplicate names would map
            // to one input role) and build a manifest carrying every input.
            let mut unique: Vec<String> = tool_keys.clone();
            unique.sort();
            unique.dedup();

            let manifest = manifest_with_inputs(&unique);
            let projection = ToolSurfaceProjection {
                manifest,
                tools: vec![tool_with_keys("T", unique.clone())],
            };
            let surfaces = project_tool_surface(&projection);
            prop_assert_eq!(surfaces.len(), 1);

            // The projected per-tool input-key SET == the production tool's input_keys SET.
            let projected: std::collections::BTreeSet<String> =
                surfaces[0].inputs.iter().map(|p| p.key.clone()).collect();
            let production: std::collections::BTreeSet<String> = unique.into_iter().collect();
            prop_assert_eq!(projected, production);
        }
    }
}
