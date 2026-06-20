//! WBV2-04/05 demonstration (the CLAUDE.md ALWAYS `cargo run --example`
//! requirement): the table-based authoring contract observed END TO END.
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
//! over it via [`WorkbookBuilderExt`], and PRINTS the emitted tool surface — the
//! per-Table tool name, its description (the Table's caption), and its per-tool
//! input/output schema. The two tools' input key sets are DISJOINT on `withheld`
//! (only `estimate_refund`'s refund formula consumes it) — the DAG-derivation proof
//! a reader can SEE in the printed schemas.
//!
//! Run with:
//! ```sh
//! cargo run --example workbook_table_authoring \
//!   --features workbook-embedded -p pmcp-server-toolkit
//! ```

use include_dir::{include_dir, Dir};
use pmcp::Server;
use pmcp_server_toolkit::workbook::{EmbeddedSource, WorkbookBuilderExt};
use serde_json::Value;

/// The committed two-Table golden bundle, baked in at compile time (byte-identical
/// to the on-disk golden the integration tests load).
static EMBEDDED_BUNDLE: Dir = include_dir!("$CARGO_MANIFEST_DIR/tests/fixtures/tax-calc@1.1.0");

/// The two per-Table compute tools the bundle fans out into (the generic single
/// `calculate` is retired in the multi-tool model). The four workbook-wide meta
/// tools (`explain`/`get_manifest`/`diff_version`/`render_workbook`) are also
/// registered but are not the focus of this authoring demonstration.
const TABLE_TOOLS: [&str; 2] = ["calculate_tax", "estimate_refund"];

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

fn main() -> Result<(), Box<dyn std::error::Error>> {
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
         input schemas are DAG-derived, so an LLM sees exactly the inputs each tool needs."
    );
    Ok(())
}
