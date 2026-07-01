//! WBV2-04/05 + WBVER-01/02/03 demonstration (the CLAUDE.md ALWAYS
//! `cargo run --example` requirement): the table-based authoring contract AND the
//! accuracy-verification surface, observed END TO END over one bundle.
//!
//! "Your Excel process becomes a governed, AI-callable tool surface." A business
//! analyst authors an Inputs Excel Table + one named OUTPUT Table per result group;
//! the compiler harvests those Tables into a versioned, integrity-checked bundle;
//! and the served toolkit fans the bundle out into ONE named MCP tool PER output
//! Table — each with a DAG-derived `inputSchema` (only the inputs that flow into
//! that Table's outputs) and a non-empty `outputSchema`.
//!
//! This example loads the committed `tax-calc@1.1.0` bundle (the compiled form of
//! the two-Table tax suite: `Calculate_Tax` + `Estimate_Refund`), boots a server
//! over it via [`WorkbookBuilderExt`], and:
//!
//! 1. PRINTS the emitted per-Table tool surface — the per-Table tool name, its
//!    description (the Table's caption), and its per-tool input/output schema. The
//!    two tools' input key sets are DISJOINT on `withheld` (only `estimate_refund`'s
//!    refund formula consumes it) — the DAG-derivation proof a reader can SEE.
//!
//! 2. Demonstrates the THREE accuracy-verification capabilities the BA uses to
//!    TRUST the served numbers (Phase 100):
//!    - `render_workbook(mode = "filled")` (WBVER-01): a `workbook://` download
//!      whose formula cells (now INCLUDING the text `bracket_label` and the bool
//!      `is_taxable`) carry the formula AND its cached result — Excel reopens and
//!      independently recomputes EVERY output type, not just the numeric ones.
//!    - `render_workbook(mode = "inputs_only")` (WBVER-02): the double-entry copy —
//!      bare formulas, the SERVER contributes zero output values, Excel is the sole
//!      oracle on load (`fullCalcOnLoad`).
//!    - `verify_accuracy` (WBVER-03): the compile-time penny-reconcile made
//!      runtime-inspectable — re-runs the served engine at the workbook's reference
//!      inputs and diffs every authored output (incl. the text + bool ones) against
//!      its oracle within tolerance.
//!
//! Run with:
//! ```sh
//! cargo run --example workbook_table_authoring \
//!   --features workbook-embedded -p pmcp-server-toolkit
//! ```

use include_dir::{include_dir, Dir};
use pmcp::{RequestHandlerExtra, Server};
use pmcp_server_toolkit::workbook::{
    EmbeddedSource, RenderWorkbookHandler, VerifyAccuracyHandler, WorkbookBuilderExt,
};
use serde_json::{json, Value};

/// The committed two-Table golden bundle, baked in at compile time (byte-identical
/// to the on-disk golden the integration tests load).
static EMBEDDED_BUNDLE: Dir = include_dir!("$CARGO_MANIFEST_DIR/tests/fixtures/tax-calc@1.1.0");

/// The two per-Table compute tools the bundle fans out into (the generic single
/// `calculate` is retired in the multi-tool model). The workbook-wide meta tools
/// (`explain`/`get_manifest`/`diff_version`/`render_workbook`/`verify_accuracy`)
/// are also registered; the two render modes + `verify_accuracy` are demonstrated
/// below.
const TABLE_TOOLS: [&str; 2] = ["calculate_tax", "estimate_refund"];

/// A concrete reference input set the BA might submit. The workbook's tier defaults
/// already define a reference point (`gross_income = 60000`, `deductions = 12000`);
/// these explicit inputs drive the render demonstration with a real, named filing.
fn demo_inputs() -> Value {
    json!({ "gross_income": 60000.0, "filing_status": "single" })
}

/// Print one tool's authored surface: name, description, and the input/output
/// schema property keys.
fn print_tool(server: &Server, name: &str) {
    let Some(handler) = server.get_tool(name) else {
        println!("  (tool `{name}` not registered)");
        return;
    };
    let Some(info) = handler.metadata() else {
        println!("  (tool `{name}` advertises no metadata)");
        return;
    };

    let inputs = schema_keys(&info.input_schema["properties"]["inputs"]["properties"]);
    let outputs = info
        .output_schema
        .as_ref()
        .map(|s| schema_keys(&s["properties"]["outputs"]["properties"]))
        .unwrap_or_default();

    println!("  tool: {}", info.name);
    println!(
        "    description: {}",
        info.description.as_deref().unwrap_or("(none)")
    );
    println!("    inputSchema  (DAG-derived): {inputs:?}");
    println!("    outputSchema (structuredContent): {outputs:?}");
}

/// The sorted property keys of a JSON-Schema `properties` object.
fn schema_keys(props: &Value) -> Vec<String> {
    let mut keys: Vec<String> = props
        .as_object()
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default();
    keys.sort();
    keys
}

/// Invoke a registered meta-tool by name with the given args, returning the tool's
/// `structuredContent` payload (or a domain `isError` envelope — never a panic).
async fn call_tool(
    server: &Server,
    name: &str,
    args: Value,
) -> Result<Value, Box<dyn std::error::Error>> {
    let handler = server
        .get_tool(name)
        .ok_or_else(|| format!("tool `{name}` is not registered"))?;
    let payload = handler.handle(args, RequestHandlerExtra::default()).await?;
    Ok(payload)
}

/// Drive `render_workbook` in one mode and print the returned `workbook://` URI.
/// The URI is the POINTER (read it via `resources/read` to obtain the base64 .xlsx);
/// the bytes are regenerated statelessly from the URI on each read.
async fn show_render(
    server: &Server,
    mode: &str,
    narrative: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let payload = call_tool(
        server,
        RenderWorkbookHandler::NAME,
        json!({ "inputs": demo_inputs(), "mode": mode }),
    )
    .await?;

    if payload.get("isError") == Some(&json!(true)) {
        println!("  render_workbook(mode = {mode:?}) -> ERROR: {payload}");
        return Err(format!("render_workbook(mode = {mode}) returned an error envelope").into());
    }

    let uri = payload["resource_uri"]
        .as_str()
        .ok_or("render_workbook returned no resource_uri")?;
    let mime = payload["mime_type"].as_str().unwrap_or("(none)");
    println!("  render_workbook(mode = {mode:?}) — {narrative}");
    println!("    mime_type:    {mime}");
    println!("    resource_uri: {uri}");
    Ok(())
}

/// Drive `verify_accuracy` (no filter) and print the reconciliation report: the
/// top-level rollup plus every per-tool, per-output row (key, A1 cell, server vs
/// oracle, |delta|, within-tol). Proves the text + bool outputs reconcile too.
async fn show_verify_accuracy(server: &Server) -> Result<(), Box<dyn std::error::Error>> {
    let report = call_tool(server, VerifyAccuracyHandler::NAME, json!({})).await?;

    if report.get("isError") == Some(&json!(true)) {
        println!("  verify_accuracy -> ERROR: {report}");
        return Err("verify_accuracy returned an error envelope".into());
    }

    let all_ok = report["all_within_tol"].as_bool().unwrap_or(false);
    let checked = report["cells_checked"].as_u64().unwrap_or(0);
    let tol = report["tolerance"].as_f64().unwrap_or(f64::NAN);
    println!(
        "  verify_accuracy: all_within_tol = {all_ok}, cells_checked = {checked}, tolerance = {tol}"
    );

    for tool in report["tools"].as_array().into_iter().flatten() {
        let tool_name = tool["tool"].as_str().unwrap_or("(unknown)");
        println!("    tool {tool_name}:");
        for row in tool["outputs"].as_array().into_iter().flatten() {
            let key = row["key"].as_str().unwrap_or("(?)");
            let cell = row["cell"].as_str().unwrap_or("(no cell)");
            let server_value = &row["server_value"];
            let oracle_value = &row["oracle_value"];
            let within = row["within_tol"].as_bool().unwrap_or(false);
            let delta = row["abs_delta"].as_f64().unwrap_or(f64::NAN);
            let mark = if within { "OK" } else { "MISMATCH" };
            println!(
                "      [{mark}] {key} @ {cell}: server = {server_value}, oracle = {oracle_value}, \
                 |delta| = {delta} (within_tol = {within})"
            );
        }
    }

    if !all_ok {
        return Err("verify_accuracy reports a value outside tolerance".into());
    }
    Ok(())
}

/// Illustrate the D-03 fail-closed contract: an unknown `tool` filter is an
/// `isError` envelope listing the available tools — never a silent empty pass.
async fn show_unknown_filter(server: &Server) -> Result<(), Box<dyn std::error::Error>> {
    let report = call_tool(
        server,
        VerifyAccuracyHandler::NAME,
        json!({ "tool": "no_such_tool" }),
    )
    .await?;
    let is_error = report.get("isError") == Some(&json!(true));
    println!(
        "  verify_accuracy(tool = \"no_such_tool\") -> isError = {is_error} \
         (D-03: an unknown filter fails closed, listing the available tools)"
    );
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "== Table-based workbook authoring: your Excel process as a governed MCP tool surface ==\n"
    );
    println!(
        "Loading the compiled two-Table tax-suite bundle (Calculate_Tax + Estimate_Refund)...\n"
    );

    // Boot a server over the bundle — fail-closed integrity verification at load.
    let server = Server::builder()
        .name("workbook-tax-calc")
        .version("1.1.0")
        .try_with_workbook_bundle(&EmbeddedSource::new(&EMBEDDED_BUNDLE))?
        .build()?;

    println!("Emitted tool surface — ONE named MCP tool per output Table:\n");
    for name in TABLE_TOOLS {
        print_tool(&server, name);
        println!();
    }

    println!(
        "Note: only `estimate_refund` advertises `withheld` — its refund formula \n\
         (withheld - tax_owed) is the only path that reaches that input. The per-tool\n\
         input schemas are DAG-derived, so an LLM sees exactly the inputs each tool needs.\n"
    );

    println!("== Accuracy-verification surface (Phase 100): how the BA TRUSTS the numbers ==\n");

    // WBVER-01: filled render — formula + cached result for EVERY output type
    // (now incl. the text `bracket_label` and the bool `is_taxable`), so Excel can
    // independently recompute all outputs on reopen.
    show_render(
        &server,
        "filled",
        "WBVER-01: formula cells carry the formula AND its cached result, so reopening in \n\
         Excel re-verifies every output — including the text `bracket_label` and bool `is_taxable`.",
    )
    .await?;
    println!();

    // WBVER-02: inputs_only render — the double-entry copy (bare formulas, Excel is
    // the sole oracle on load).
    show_render(
        &server,
        "inputs_only",
        "WBVER-02: the double-entry download — bare formulas, the server contributes ZERO \n\
         output values, Excel computes everything from the inputs on load (fullCalcOnLoad).",
    )
    .await?;
    println!();

    // WBVER-03: verify_accuracy — the compile-time penny-reconcile, runtime-inspectable.
    println!(
        "  WBVER-03: verify_accuracy re-runs the served engine at the workbook's reference\n\
         inputs and diffs every authored output against its oracle within tolerance.\n"
    );
    show_verify_accuracy(&server).await?;
    println!();
    show_unknown_filter(&server).await?;
    println!();

    println!(
        "All three accuracy-verification capabilities demonstrated over tax-calc@1.1.0:\n\
         render_workbook(filled) + render_workbook(inputs_only) + verify_accuracy — \n\
         the served numbers are now independently checkable, end to end."
    );
    Ok(())
}
