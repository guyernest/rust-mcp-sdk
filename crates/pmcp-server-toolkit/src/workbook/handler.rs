//! The curated workbook tool handlers (WBSV-01/02/03/04): `calculate`,
//! `explain`, `get_manifest`, `diff_version`.
//!
//! All are native [`pmcp::ToolHandler`] impls registered via `tool_arc` and
//! [`pmcp::types::ToolInfo::with_ui`] (so the returned `Value` lands in
//! `structuredContent`). Each attaches the provenance stamp and advertises a
//! non-empty `outputSchema` (WBSV-07). Domain failures return the `isError:true`
//! envelope via [`to_iserror_result`] — NEVER a protocol-level error (T-92-10).
//!
//! `calculate`/`explain` re-run the SERVE-time
//! [`pmcp_workbook_runtime::run_executor`] over the pre-built `bundle.dag`
//! (no compiler, no second evaluator), seeding the `CellEnv` via the embedded
//! `cell_map`. There is NO privileged headline output (S-1): [`project_outputs`]
//! iterates ALL `cell_map.outputs` uniformly.

// Compiler/clippy-enforced panic-freedom on the value path (mirrors the runtime).
#![cfg_attr(
    not(test),
    deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)
)]

use std::collections::BTreeMap;
use std::sync::Arc;

use async_trait::async_trait;
use pmcp::types::ToolInfo;
use pmcp::{RequestHandlerExtra, ToolHandler};
use serde_json::{json, Value};

use pmcp_workbook_runtime::{run_executor, CellEnv, CellValue, RunResult};

use super::error::{to_iserror_result, WorkbookToolError};
use super::input::validate_input;
use super::render_uri;
use super::schema::{
    diff_version_output_schema, empty_input_schema, explain_output_schema,
    get_manifest_output_schema, input_schema_for_manifest, output_schema_for_manifest,
    render_workbook_output_schema,
};
use super::{ProvStamp, WorkbookBundle, WORKBOOK_TOOL_UI};

// ---- Shared handler helpers (kept decomposed so each handler fn stays under
//      cognitive complexity 25) -------------------------------------------------

/// Re-run the embedded IR over the validated seeds and return the [`RunResult`].
/// The per-cell DAG is the one built ONCE at bundle load (`bundle.dag`). A DAG
/// cycle (impossible for a conforming bundle) surfaces as an `invalid_input`
/// error rather than a panic.
#[allow(clippy::result_large_err)]
pub(crate) fn run_bundle(
    bundle: &WorkbookBundle,
    seeds: BTreeMap<String, Value>,
) -> Result<RunResult, WorkbookToolError> {
    let mut env = CellEnv::new();
    for (key, value) in seeds {
        env = env.with_value(key, value);
    }
    run_executor(&bundle.ir, &bundle.dag, &env).map_err(|f| {
        WorkbookToolError::invalid_input(format!("executor failed: {} ({})", f.message, f.rule))
    })
}

/// Project the computed outputs into the typed `{ <json_key>: { value, unit } }`
/// map carrying units (read from the cell_map). ALL named outputs are projected
/// uniformly (S-1 — no privileged headline).
///
/// WR-06: every projected numeric output is finiteness-checked — a non-finite
/// f64 cannot be represented in JSON (serde_json substitutes `null`), so a
/// non-finite output cell surfaces as an `invalid_input` error rather than a
/// silent `null` masquerading as a success value.
#[allow(clippy::result_large_err)]
pub(crate) fn project_outputs(
    bundle: &WorkbookBundle,
    run: &RunResult,
) -> Result<Value, WorkbookToolError> {
    let mut outputs = serde_json::Map::new();
    for entry in &bundle.cell_map.outputs {
        // WR-04: fail closed on a declared-but-uncomputed output. A cell_map output
        // (already verified at boot) absent from the run result is a cell_map/IR skew,
        // not a success — silently dropping it would let the served payload diverge
        // from the advertised outputSchema (WBSV-07). Surface it as an `invalid_input`
        // error so the contract and the payload can never disagree.
        let Some(value) = run.computed.get(&entry.seed_coord) else {
            return Err(WorkbookToolError::invalid_input(format!(
                "internal: declared output '{}' ({}) was not computed by the bundle IR",
                entry.json_key, entry.seed_coord
            )));
        };
        let projected = finite_output_value(value, &entry.seed_coord, &entry.json_key)?;
        outputs.insert(
            entry.json_key.clone(),
            json!({ "value": projected, "unit": entry.unit }),
        );
    }
    Ok(Value::Object(outputs))
}

/// Project one computed [`CellValue`] into its JSON `value`, finiteness-checking
/// numbers (WR-06). A non-finite number is an error, NOT a JSON `null`.
#[allow(clippy::result_large_err)]
fn finite_output_value(
    value: &CellValue,
    seed_coord: &str,
    json_key: &str,
) -> Result<Value, WorkbookToolError> {
    match value {
        CellValue::Number(n) if n.is_finite() => Ok(json!(n)),
        CellValue::Number(_) => Err(WorkbookToolError::invalid_input(format!(
            "output cell {seed_coord} ({json_key}) did not compute to a finite number"
        ))),
        CellValue::Text(s) => Ok(json!(s)),
        CellValue::Bool(b) => Ok(json!(b)),
        CellValue::Empty => Ok(Value::Null),
        CellValue::Error(e) => Err(WorkbookToolError::invalid_input(format!(
            "output cell {seed_coord} ({json_key}) computed to an error: {e:?}"
        ))),
    }
}

/// Append the provenance stamp to a success payload object.
pub(crate) fn with_provenance(mut payload: Value, stamp: &ProvStamp) -> Value {
    if let Some(obj) = payload.as_object_mut() {
        obj.insert("provenance".to_string(), stamp.to_json());
    }
    payload
}

/// Render a fallible compute pipeline once at the boundary: a domain failure
/// becomes the `isError:true` envelope (in `structuredContent`), never a
/// protocol-level error.
#[allow(clippy::result_large_err)]
pub(crate) fn render_at_boundary(
    result: Result<Value, WorkbookToolError>,
    stamp: &ProvStamp,
) -> Value {
    result.unwrap_or_else(|e| to_iserror_result(&e, stamp))
}

// ---- calculate ---------------------------------------------------------------

/// The `calculate` handler (WBSV-01): validate → seed via cell_map → re-run the
/// embedded IR → project ALL outputs (finite) → stamp.
pub struct CalculateHandler {
    bundle: Arc<WorkbookBundle>,
    stamp: ProvStamp,
}

impl CalculateHandler {
    /// The registered tool name — the single source for registration + metadata.
    pub const NAME: &str = "calculate";

    /// Build over the shared verified bundle.
    #[must_use]
    pub fn new(bundle: Arc<WorkbookBundle>) -> Self {
        let stamp = ProvStamp::from_bundle(&bundle);
        Self { bundle, stamp }
    }

    /// The linear `?`-chained `calculate` pipeline.
    #[allow(clippy::result_large_err)]
    fn compute(&self, args: Value) -> Result<Value, WorkbookToolError> {
        let validated = validate_input(args, &self.bundle.manifest, &self.bundle.cell_map)?;
        let run = run_bundle(&self.bundle, validated.seeds)?;
        let outputs = project_outputs(&self.bundle, &run)?;
        let payload = json!({
            "outputs": outputs,
            "accepted_overrides": validated.accepted_overrides,
        });
        Ok(with_provenance(payload, &self.stamp))
    }
}

#[async_trait]
impl ToolHandler for CalculateHandler {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        Ok(render_at_boundary(self.compute(args), &self.stamp))
    }

    fn metadata(&self) -> Option<ToolInfo> {
        Some(
            ToolInfo::with_ui(
                Self::NAME,
                Some(
                    "Compute the workbook outputs from the declared inputs by re-running \
                     the compiled workbook IR. Returns every named output as a \
                     units-bearing { value, unit } projection plus a \
                     bundle_id@version+combined_hash provenance stamp. Strict \
                     (BA-governed) constants cannot be overridden."
                        .into(),
                ),
                input_schema_for_manifest(&self.bundle.manifest, &self.bundle.cell_map),
                WORKBOOK_TOOL_UI,
            )
            .with_output_schema(output_schema_for_manifest(
                &self.bundle.manifest,
                &self.bundle.cell_map,
            )),
        )
    }
}

// ---- explain -----------------------------------------------------------------

/// A display projection of a [`CellValue`] for the explain trace.
fn cell_value_display(v: &CellValue) -> Value {
    match v {
        CellValue::Number(n) => json!(n),
        CellValue::Text(s) => json!(s),
        CellValue::Bool(b) => json!(b),
        CellValue::Empty => Value::Null,
        CellValue::Error(e) => json!(format!("{e:?}")),
    }
}

/// The `explain` handler (WBSV-02): a stateless re-run that renders the
/// derivation trace as ordered business-language steps, plus a GENERIC
/// manifest-declared `annotations` object (S-2 — any domain-specific keystone is
/// generalized into manifest-declared annotations; the engine reads only
/// `manifest.annotations` names, nothing domain-specific).
pub struct ExplainHandler {
    bundle: Arc<WorkbookBundle>,
    stamp: ProvStamp,
}

impl ExplainHandler {
    /// The registered tool name — the single source for registration + metadata.
    pub const NAME: &str = "explain";

    /// Build over the shared verified bundle.
    #[must_use]
    pub fn new(bundle: Arc<WorkbookBundle>) -> Self {
        let stamp = ProvStamp::from_bundle(&bundle);
        Self { bundle, stamp }
    }

    /// The linear `?`-chained `explain` pipeline: validate → re-run → ordered
    /// derivation steps + manifest annotations → stamp.
    #[allow(clippy::result_large_err)]
    fn compute(&self, args: Value) -> Result<Value, WorkbookToolError> {
        let validated = validate_input(args, &self.bundle.manifest, &self.bundle.cell_map)?;
        let run = run_bundle(&self.bundle, validated.seeds)?;
        let steps = self.render_steps(&run);
        let payload = json!({
            "steps": steps,
            "annotations": self.manifest_annotations(),
        });
        Ok(with_provenance(payload, &self.stamp))
    }

    /// Render the [`RunResult`] traces into ORDERED business-language steps
    /// (sorted by cell key for determinism), each carrying the formula + operand
    /// values + the manifest meaning.
    fn render_steps(&self, run: &RunResult) -> Vec<Value> {
        let mut entries: Vec<_> = run.traces.iter().collect();
        entries.sort_by(|a, b| a.0.cmp(b.0));
        let mut steps = Vec::with_capacity(entries.len());
        for (key, trace) in entries {
            steps.push(json!({
                "step": "derivation",
                "cell": key,
                "meaning": self.meaning_for(key),
                "formula": trace.formula,
                "dispatched_fn": trace.dispatched_fn,
                "resolved_refs": trace.resolved_refs.iter().map(|(k, v)| json!({
                    "cell": k,
                    "value": cell_value_display(v),
                })).collect::<Vec<_>>(),
                "result": run.computed.get(key).map(cell_value_display),
            }));
        }
        steps
    }

    /// The GENERIC manifest-declared annotations object (S-2): keyed by each
    /// [`pmcp_workbook_runtime::AnnotationDecl`] `name`, carrying its `target` +
    /// `meaning`. The engine reads ONLY manifest-declared names — nothing
    /// domain-specific.
    fn manifest_annotations(&self) -> Value {
        let mut obj = serde_json::Map::new();
        for ann in &self.bundle.manifest.annotations {
            obj.insert(
                ann.name.clone(),
                json!({ "target": ann.target, "meaning": ann.meaning }),
            );
        }
        Value::Object(obj)
    }

    /// The manifest meaning for a cell key (for the business-language prose).
    fn meaning_for(&self, key: &str) -> Option<String> {
        pmcp_workbook_runtime::role_for_cell(&self.bundle.manifest, key)
            .and_then(|c| c.meaning.clone().or_else(|| c.name.clone()))
    }
}

#[async_trait]
impl ToolHandler for ExplainHandler {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        Ok(render_at_boundary(self.compute(args), &self.stamp))
    }

    fn metadata(&self) -> Option<ToolInfo> {
        Some(
            ToolInfo::with_ui(
                Self::NAME,
                Some(
                    "Explain the computed workbook outputs: an ordered business-language \
                     derivation trace (formula + operands + meaning per step) plus a \
                     manifest-declared annotations object. Stamped + stateless (re-run \
                     from the same inputs)."
                        .into(),
                ),
                input_schema_for_manifest(&self.bundle.manifest, &self.bundle.cell_map),
                WORKBOOK_TOOL_UI,
            )
            .with_output_schema(explain_output_schema()),
        )
    }
}

// ---- get_manifest ------------------------------------------------------------

/// The `get_manifest` handler (WBSV-03): a CURATED agent-facing projection —
/// inputs (tier+default+unit), outputs (unit/meaning), governed-data summary,
/// versions/hashes, changelog — NOT the raw internal manifest.
pub struct GetManifestHandler {
    bundle: Arc<WorkbookBundle>,
    stamp: ProvStamp,
}

impl GetManifestHandler {
    /// The registered tool name — the single source for registration + metadata.
    pub const NAME: &str = "get_manifest";

    /// Build over the shared verified bundle.
    #[must_use]
    pub fn new(bundle: Arc<WorkbookBundle>) -> Self {
        let stamp = ProvStamp::from_bundle(&bundle);
        Self { bundle, stamp }
    }
}

/// Project one manifest input cell into its curated agent-facing record.
fn input_projection(role: &pmcp_workbook_runtime::CellRole) -> Value {
    use pmcp_workbook_runtime::InputTier;
    let (tier_kind, default) = match &role.tier {
        Some(InputTier::Variable { default }) => ("variable", cell_value_display(default)),
        Some(InputTier::BoundedVariable { default, .. }) => {
            ("bounded_variable", cell_value_display(default))
        },
        None => ("variable", Value::Null),
    };
    json!({
        "name": role.name,
        "unit": role.unit,
        "meaning": role.meaning,
        "tier": tier_kind,
        "default": default,
    })
}

/// Build the curated agent-facing manifest projection (WBSV-03) + stamp.
fn curated_manifest(bundle: &WorkbookBundle, stamp: &ProvStamp) -> Value {
    use pmcp_workbook_runtime::Role;

    let mut inputs = Vec::new();
    let mut outputs = Vec::new();
    for role in &bundle.manifest.cells {
        match role.role {
            Role::Input => inputs.push(input_projection(role)),
            Role::Output => outputs.push(json!({
                "name": role.name,
                "unit": role.unit,
                "meaning": role.meaning,
            })),
            Role::Constant | Role::Formula => {},
        }
    }

    let governed: Vec<Value> = bundle
        .manifest
        .governed_data
        .iter()
        .map(|g| {
            json!({
                "key": g.key,
                "value": cell_value_display(&g.value),
                "approved_by": g.approved_by,
                "provenance": g.provenance,
            })
        })
        .collect();

    let changelog: Vec<Value> = bundle
        .manifest
        .changelog
        .iter()
        .map(|c| json!({ "version": c.version, "note": c.note }))
        .collect();

    json!({
        "bundle_id": stamp.bundle_id,
        "version": stamp.version,
        "combined_hash": stamp.combined_hash,
        "inputs": inputs,
        "outputs": outputs,
        "governed_data": governed,
        "changelog": changelog,
        "provenance": stamp.to_json(),
    })
}

#[async_trait]
impl ToolHandler for GetManifestHandler {
    async fn handle(&self, _args: Value, _extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        Ok(curated_manifest(&self.bundle, &self.stamp))
    }

    fn metadata(&self) -> Option<ToolInfo> {
        Some(
            ToolInfo::with_ui(
                Self::NAME,
                Some(
                    "Describe the compiled workbook workflow: a curated agent-facing \
                     manifest projection (inputs with tier/default/unit, outputs with \
                     unit/meaning, governed-data summary, version/hashes, changelog) + \
                     provenance stamp."
                        .into(),
                ),
                empty_input_schema(),
                WORKBOOK_TOOL_UI,
            )
            .with_output_schema(get_manifest_output_schema()),
        )
    }
}

// ---- diff_version ------------------------------------------------------------

/// The `diff_version` handler (WBSV-04): serve the RECORDED prev→current
/// [`pmcp_workbook_runtime::VersionChangelog`] the offline promote step folded
/// into the bundle (hash-verified at boot — NOT a runtime computation), stamped.
pub struct DiffVersionHandler {
    bundle: Arc<WorkbookBundle>,
    stamp: ProvStamp,
}

impl DiffVersionHandler {
    /// The registered tool name — the single source for registration + metadata.
    pub const NAME: &str = "diff_version";

    /// Build over the shared verified bundle.
    #[must_use]
    pub fn new(bundle: Arc<WorkbookBundle>) -> Self {
        let stamp = ProvStamp::from_bundle(&bundle);
        Self { bundle, stamp }
    }
}

/// Serialize the recorded [`pmcp_workbook_runtime::VersionChangelog`] into the
/// served structured payload. Infallible — the changelog was hash-verified and
/// parsed at boot, so serving it cannot fail.
fn serve_changelog(bundle: &WorkbookBundle, stamp: &ProvStamp) -> Value {
    let cl = &bundle.changelog;
    let deltas: Vec<Value> = cl.deltas.iter().map(delta_to_json).collect();
    let payload = json!({
        "from_version": cl.from_version,
        "to_version": cl.to_version,
        "deltas": deltas,
        "summary": cl.summary,
    });
    with_provenance(payload, stamp)
}

/// Project one [`pmcp_workbook_runtime::OutputDelta`] into its served JSON shape.
fn delta_to_json(delta: &pmcp_workbook_runtime::OutputDelta) -> Value {
    json!({
        "region": delta.region,
        "change_class": delta.change_class,
        "old": meta_to_json(&delta.old),
        "new": meta_to_json(&delta.new),
        "severity": delta.severity,
    })
}

/// Project one [`pmcp_workbook_runtime::OutputMeta`] into its served JSON.
fn meta_to_json(meta: &pmcp_workbook_runtime::OutputMeta) -> Value {
    json!({
        "meaning": meta.meaning,
        "unit": meta.unit,
        "provenance": meta.provenance,
    })
}

#[async_trait]
impl ToolHandler for DiffVersionHandler {
    async fn handle(&self, _args: Value, _extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        Ok(serve_changelog(&self.bundle, &self.stamp))
    }

    fn metadata(&self) -> Option<ToolInfo> {
        Some(
            ToolInfo::with_ui(
                Self::NAME,
                Some(
                    "Describe what changed between two promoted workflow versions: the \
                     RECORDED, hash-verified prev→current changelog (per-output deltas \
                     with change class + drift/redefinition severity + a human-readable \
                     summary) + a provenance stamp. Served from the bundle's recorded \
                     evidence, not a runtime computation."
                        .into(),
                ),
                empty_input_schema(),
                WORKBOOK_TOOL_UI,
            )
            .with_output_schema(diff_version_output_schema()),
        )
    }
}

// ---- render_workbook ---------------------------------------------------------

/// The `render_workbook` handler (WBSV-05): validate the inputs, then return a
/// provenance-bound `workbook://` URI POINTER — NOT the `.xlsx` bytes. The bytes
/// are recomputed per `resources/read` by [`super::render_resource`] from the
/// decoded URI (stateless regen-on-read, Lambda-safe, V3).
///
/// The URI encodes the canonical inputs + the bundle [`ProvStamp`]
/// (`combined_hash`, Codex HIGH #3) via [`render_uri::encode`]. A domain failure
/// (invalid input, an un-encodable payload) routes through [`to_iserror_result`]
/// into `structuredContent` — never a protocol-level error (T-92-10).
pub struct RenderWorkbookHandler {
    bundle: Arc<WorkbookBundle>,
    stamp: ProvStamp,
}

impl RenderWorkbookHandler {
    /// The registered tool name — the single source for registration + metadata.
    pub const NAME: &str = "render_workbook";

    /// Build over the shared verified bundle.
    #[must_use]
    pub fn new(bundle: Arc<WorkbookBundle>) -> Self {
        let stamp = ProvStamp::from_bundle(&bundle);
        Self { bundle, stamp }
    }

    /// The linear `?`-chained `render_workbook` pipeline: validate → encode the
    /// canonical DTO + provenance into a `workbook://` URI → return the POINTER
    /// (plus the stamp), NOT the bytes.
    #[allow(clippy::result_large_err)]
    fn compute(&self, args: Value) -> Result<Value, WorkbookToolError> {
        let validated = validate_input(args, &self.bundle.manifest, &self.bundle.cell_map)?;
        let uri = render_uri::encode(&validated.canonical_dto, &self.stamp)?;
        let payload = json!({
            "resource_uri": uri,
            "mime_type": render_uri::WORKBOOK_XLSX_MIME,
        });
        Ok(with_provenance(payload, &self.stamp))
    }
}

#[async_trait]
impl ToolHandler for RenderWorkbookHandler {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> pmcp::Result<Value> {
        Ok(render_at_boundary(self.compute(args), &self.stamp))
    }

    fn metadata(&self) -> Option<ToolInfo> {
        Some(
            ToolInfo::with_ui(
                Self::NAME,
                Some(
                    "Render the computed workbook to a downloadable .xlsx. Returns a \
                     provenance-bound workbook:// resource URI (NOT the bytes) — read \
                     that URI via resources/read to obtain the base64-encoded .xlsx, \
                     which is regenerated statelessly from the URI on each read. The URI \
                     encodes the inputs; treat it as sensitive."
                        .into(),
                ),
                input_schema_for_manifest(&self.bundle.manifest, &self.bundle.cell_map),
                WORKBOOK_TOOL_UI,
            )
            .with_output_schema(render_workbook_output_schema()),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    use pmcp_workbook_runtime::{load_bundle, LocalDirSource};

    fn golden_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tax-calc@1.1.0")
    }

    fn golden_bundle() -> Arc<WorkbookBundle> {
        let source = LocalDirSource::new(golden_dir());
        Arc::new(load_bundle(&source).expect("golden bundle boots"))
    }

    #[test]
    fn calculate_returns_all_outputs_with_provenance_no_headline() {
        let handler = CalculateHandler::new(golden_bundle());
        let v = handler
            .compute(json!({ "inputs": { "gross_income": 60000.0, "filing_status": "single" } }))
            .expect("calculate succeeds");

        // Every named output is present as a { value, unit } pair.
        let outputs = v["outputs"].as_object().expect("outputs is an object");
        for key in [
            "taxable_income",
            "tax_owed",
            "effective_rate",
            "marginal_rate",
        ] {
            assert!(outputs.contains_key(key), "output {key} present");
            assert!(
                outputs[key]["value"].is_number() || outputs[key]["value"].is_null(),
                "output {key} carries a value"
            );
        }
        // S-1: the success payload has EXACTLY outputs/accepted_overrides/
        // provenance — no privileged headline scalar elevated above the
        // uniform all-outputs projection.
        let root = v.as_object().expect("payload is an object");
        let mut top_keys: Vec<&str> = root.keys().map(String::as_str).collect();
        top_keys.sort_unstable();
        assert_eq!(
            top_keys,
            ["accepted_overrides", "outputs", "provenance"],
            "no headline field elevated above the all-outputs projection (S-1)"
        );
        // Provenance stamp on every result.
        assert!(v["provenance"]["combined_hash"].is_string());
        assert!(v.get("isError").is_none(), "a success is not an error");
    }

    #[test]
    fn calculate_honors_non_default_input() {
        // CR-01 (92-VERIFICATION.md, Blocker 1): a caller-supplied input MUST drive
        // the computation, not be silently discarded in favour of the bundle's
        // baked-in default (gross_income=60000). Before this plan, the fixture
        // generator emitted the input cells as IR literals and the executor's
        // literal arm re-seeded them at topo-walk time, clobbering validate_input's
        // caller seed — so this assertion would have FAILED returning 48000.0.
        let handler = CalculateHandler::new(golden_bundle());

        // gross_income 100000, default deduction 12000 => taxable_income 88000.
        let v = handler
            .compute(json!({ "inputs": { "gross_income": 100000.0 } }))
            .expect("calculate honors a non-default gross_income");
        assert_eq!(
            v["outputs"]["taxable_income"]["value"],
            json!(88000.0),
            "taxable_income reflects the caller's gross_income (100000 - 12000), not the default"
        );

        // A DIFFERENT non-default input also flows (guards against a single-value
        // coincidence): gross_income 80000 - 12000 => 68000.
        let v = handler
            .compute(json!({ "inputs": { "gross_income": 80000.0 } }))
            .expect("calculate honors a second non-default gross_income");
        assert_eq!(
            v["outputs"]["taxable_income"]["value"],
            json!(68000.0),
            "a second caller input flows through (80000 - 12000)"
        );
    }

    #[test]
    fn calculate_invalid_input_returns_iserror_in_structured_content() {
        let bundle = golden_bundle();
        let handler = CalculateHandler::new(bundle.clone());
        // An out-of-enum filing_status is a domain failure.
        let v = render_at_boundary(
            handler.compute(json!({ "inputs": { "filing_status": "alien" } })),
            &ProvStamp::from_bundle(&bundle),
        );
        assert_eq!(
            v["isError"],
            json!(true),
            "isError rides in structuredContent"
        );
        assert_eq!(v["code"], json!("invalid_input"));
        assert!(v["provenance"]["combined_hash"].is_string());
    }

    #[test]
    fn non_finite_output_surfaces_as_error_not_null() {
        // WR-06: a non-finite f64 must surface as an error, never JSON null.
        let err = finite_output_value(&CellValue::Number(f64::NAN), "3_Outputs!B3", "tax_owed")
            .expect_err("NaN is rejected (WR-06)");
        assert_eq!(err.code, "invalid_input");
        let err = finite_output_value(&CellValue::Number(f64::INFINITY), "c", "k")
            .expect_err("Infinity is rejected (WR-06)");
        assert_eq!(err.code, "invalid_input");
        // A finite number projects fine.
        let ok = finite_output_value(&CellValue::Number(42.0), "c", "k").expect("finite ok");
        assert_eq!(ok, json!(42.0));
    }

    #[test]
    fn project_outputs_fails_closed_on_missing_declared_output() {
        // WR-04: a declared output (verified in cell_map at boot) absent from the run
        // result is a cell_map/IR skew, NOT a success. project_outputs must fail
        // closed with invalid_input so the served payload can never silently diverge
        // from the advertised outputSchema (WBSV-07) — never an `else { continue }`.
        let bundle = golden_bundle();
        // A crafted RunResult whose `computed` map is EMPTY — every declared output's
        // seed_coord is therefore absent.
        let run = RunResult::default();
        let err = project_outputs(&bundle, &run)
            .expect_err("a missing declared output fails closed (WR-04)");
        assert_eq!(err.code, "invalid_input");
        assert!(
            err.reason.contains("was not computed by the bundle IR"),
            "the error names the cell_map/IR skew: {}",
            err.reason
        );
        // The named, missing output is identified in the message.
        assert!(
            bundle
                .cell_map
                .outputs
                .iter()
                .any(|e| err.reason.contains(&e.json_key) || err.reason.contains(&e.seed_coord)),
            "the error identifies the uncomputed output: {}",
            err.reason
        );
    }

    #[test]
    fn project_outputs_succeeds_when_all_declared_outputs_present() {
        // Companion to the fail-closed test: when every declared output IS computed,
        // project_outputs returns the full { value, unit } map (no false positive).
        let bundle = golden_bundle();
        let mut run = RunResult::default();
        for entry in &bundle.cell_map.outputs {
            run.computed
                .insert(entry.seed_coord.clone(), CellValue::Number(1.0));
        }
        let projected = project_outputs(&bundle, &run).expect("all-present projects");
        let obj = projected.as_object().expect("outputs is an object");
        assert_eq!(
            obj.len(),
            bundle.cell_map.outputs.len(),
            "every declared output is projected"
        );
    }

    #[test]
    fn calculate_advertises_non_empty_output_schema() {
        let handler = CalculateHandler::new(golden_bundle());
        let meta = handler.metadata().expect("metadata present");
        let schema = meta
            .output_schema
            .expect("outputSchema advertised (WBSV-07)");
        let outputs = &schema["properties"]["outputs"]["properties"];
        assert!(
            outputs.as_object().is_some_and(|o| !o.is_empty()),
            "outputSchema enumerates the named outputs"
        );
    }

    // ---- explain (WBSV-02, S-2) ------------------------------------------

    #[test]
    fn explain_emits_ordered_trace_and_generic_manifest_annotations() {
        let handler = ExplainHandler::new(golden_bundle());
        let v = handler
            .compute(json!({ "inputs": { "gross_income": 60000.0, "filing_status": "single" } }))
            .expect("explain succeeds");

        // An ordered per-cell derivation trace.
        let steps = v["steps"].as_array().expect("steps is an array");
        assert!(!steps.is_empty(), "explain emits derivation steps");
        for step in steps {
            assert_eq!(step["step"], json!("derivation"));
            assert!(step["cell"].is_string());
        }

        // S-2: a GENERIC annotations object keyed by the manifest AnnotationDecl
        // names (the tax golden declares bracket_boundary_1/2) — nothing
        // domain-specific is hardcoded.
        let annotations = v["annotations"].as_object().expect("annotations object");
        assert!(annotations.contains_key("bracket_boundary_1"));
        assert!(annotations.contains_key("bracket_boundary_2"));
        assert_eq!(
            annotations["bracket_boundary_1"]["target"],
            json!("2_Brackets!A2")
        );
        assert!(annotations["bracket_boundary_1"]["meaning"].is_string());

        assert!(v["provenance"]["combined_hash"].is_string());
    }

    #[test]
    fn explain_invalid_input_returns_iserror() {
        let bundle = golden_bundle();
        let handler = ExplainHandler::new(bundle.clone());
        let v = render_at_boundary(
            handler.compute(json!({ "inputs": { "filing_status": "alien" } })),
            &ProvStamp::from_bundle(&bundle),
        );
        assert_eq!(v["isError"], json!(true));
        assert_eq!(v["code"], json!("invalid_input"));
    }

    // ---- get_manifest (WBSV-03) ------------------------------------------

    #[test]
    fn get_manifest_returns_curated_projection_with_no_input() {
        let bundle = golden_bundle();
        let v = curated_manifest(&bundle, &ProvStamp::from_bundle(&bundle));
        assert_eq!(v["bundle_id"], json!("tax-calc"));
        assert_eq!(v["version"], json!("1.1.0"));
        assert!(v["combined_hash"].is_string());
        // Curated inputs/outputs/governed_data/changelog projections.
        let inputs = v["inputs"].as_array().expect("inputs array");
        assert_eq!(inputs.len(), 3, "three inputs projected");
        assert!(inputs.iter().all(|i| i["tier"].is_string()));
        let outputs = v["outputs"].as_array().expect("outputs array");
        assert_eq!(outputs.len(), 4, "four outputs projected");
        assert!(v["governed_data"].is_array());
        assert!(v["changelog"].is_array());
        assert!(v["provenance"]["combined_hash"].is_string());
    }

    // ---- diff_version (WBSV-04) ------------------------------------------

    #[test]
    fn diff_version_serves_recorded_changelog() {
        let bundle = golden_bundle();
        let v = serve_changelog(&bundle, &ProvStamp::from_bundle(&bundle));

        // The served changelog matches the recorded one (not recomputed).
        assert_eq!(v["from_version"], json!(bundle.changelog.from_version));
        assert_eq!(v["to_version"], json!(bundle.changelog.to_version));
        assert_eq!(v["summary"], json!(bundle.changelog.summary));
        let deltas = v["deltas"].as_array().expect("deltas array");
        assert_eq!(deltas.len(), bundle.changelog.deltas.len());
        if let Some(first) = deltas.first() {
            assert!(first["region"].is_string());
            assert!(first["change_class"].is_string());
            assert!(first["severity"].is_string());
        }
        assert!(v["provenance"]["combined_hash"].is_string());
        assert!(
            v.get("isError").is_none(),
            "a served changelog is not an error"
        );
    }

    #[test]
    fn diff_version_advertises_output_schema() {
        let handler = DiffVersionHandler::new(golden_bundle());
        let meta = handler.metadata().expect("metadata present");
        let schema = meta.output_schema.expect("output schema advertised");
        assert_eq!(
            schema["properties"]["from_version"]["type"],
            json!("string")
        );
        assert_eq!(schema["properties"]["deltas"]["type"], json!("array"));
    }

    // ---- render_workbook (WBSV-05) ---------------------------------------

    #[test]
    fn render_workbook_returns_uri_pointer_not_bytes() {
        let bundle = golden_bundle();
        let handler = RenderWorkbookHandler::new(bundle.clone());
        let v = handler
            .compute(json!({ "inputs": { "gross_income": 60000.0, "filing_status": "single" } }))
            .expect("render_workbook succeeds");

        // The response carries a workbook:// pointer, NOT the bytes.
        let uri = v["resource_uri"]
            .as_str()
            .expect("resource_uri is a string");
        assert!(
            uri.starts_with(render_uri::RENDER_URI_PREFIX),
            "returns a workbook:// pointer"
        );
        assert!(
            v.get("bytes").is_none() && v.get("data").is_none(),
            "the bytes are NOT in the tool response"
        );
        // The pointer decodes back to the bound provenance (Codex HIGH #3).
        let decoded = render_uri::decode(uri).expect("pointer decodes");
        assert_eq!(decoded.provenance, ProvStamp::from_bundle(&bundle));
        assert_eq!(decoded.provenance.combined_hash, bundle.stamp.combined);
        // The success payload carries the provenance stamp.
        assert!(v["provenance"]["combined_hash"].is_string());
        assert!(v.get("isError").is_none(), "a success is not an error");
    }

    #[test]
    fn render_workbook_invalid_input_returns_iserror() {
        let bundle = golden_bundle();
        let handler = RenderWorkbookHandler::new(bundle.clone());
        let v = render_at_boundary(
            handler.compute(json!({ "inputs": { "filing_status": "alien" } })),
            &ProvStamp::from_bundle(&bundle),
        );
        assert_eq!(v["isError"], json!(true), "isError rides in the payload");
        assert_eq!(v["code"], json!("invalid_input"));
        assert!(v["provenance"]["combined_hash"].is_string());
    }

    #[test]
    fn render_workbook_advertises_non_empty_output_schema() {
        let handler = RenderWorkbookHandler::new(golden_bundle());
        let meta = handler.metadata().expect("metadata present");
        let schema = meta
            .output_schema
            .expect("outputSchema advertised (WBSV-07)");
        assert_eq!(
            schema["properties"]["resource_uri"]["type"],
            json!("string")
        );
    }
}
