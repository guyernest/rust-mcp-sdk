//! WBV2-04 multi-tool integration test — proves the served fan-out registers ONE
//! named MCP tool PER output Table, each with a per-tool DAG-derived `inputSchema`
//! (only its own reachable inputs) and a non-empty `outputSchema`.
//!
//! Boots a server over the committed `tax-calc@1.1.0` golden (regenerated into the
//! two-Table shape: `Calculate_Tax` + `Estimate_Refund`) via `with_workbook_bundle`
//! and reads the registered tool surface through `Server::get_tool`.
//!
//! The proof: the two compute tools carry DISJOINT, DAG-derived input key sets —
//! `estimate_refund` carries `withheld` (its refund formula consumes it) while
//! `calculate_tax` does NOT (its outputs never reference `withheld`). That
//! disjointness is read straight off the per-tool `inputSchema`, so it can only
//! hold if the schema is genuinely DAG-derived per tool, not a workbook-wide union.
#![cfg(feature = "workbook")]

use pmcp::Server;
use pmcp_server_toolkit::workbook::{LocalDirSource, WorkbookBuilderExt};
use serde_json::Value;

mod support;

use support::tamper::golden_dir;

/// The input-key property names declared on a tool's `inputSchema`
/// (`properties.inputs.properties` keys).
fn input_keys(tool: &pmcp::types::ToolInfo) -> Vec<String> {
    tool.input_schema["properties"]["inputs"]["properties"]
        .as_object()
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default()
}

/// The output-column names declared on a tool's `outputSchema`
/// (`properties.outputs.properties` keys).
fn output_keys(tool: &pmcp::types::ToolInfo) -> Vec<String> {
    tool.output_schema
        .as_ref()
        .and_then(|s| s["properties"]["outputs"]["properties"].as_object())
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default()
}

#[test]
fn tools_list_returns_one_named_tool_per_output_table() {
    let server = Server::builder()
        .name("workbook-tax-calc")
        .version("1.1.0")
        .with_workbook_bundle(&LocalDirSource::new(golden_dir()))
        .build()
        .expect("server builds from the two-tool golden bundle");

    // EXACTLY the two per-Table compute tools are registered (the generic
    // `calculate` is retired). `get_tool` yields the handler; its `metadata()` is
    // the advertised `ToolInfo` (name + I/O schemas).
    let calc = server
        .get_tool("calculate_tax")
        .expect("calculate_tax registered (Calculate_Tax output Table)")
        .metadata()
        .expect("calculate_tax advertises metadata");
    let refund = server
        .get_tool("estimate_refund")
        .expect("estimate_refund registered (Estimate_Refund output Table)")
        .metadata()
        .expect("estimate_refund advertises metadata");
    assert!(
        server.get_tool("calculate").is_none(),
        "the generic single `calculate` tool is gone (multi-tool model)"
    );

    // Each tool's outputSchema is NON-EMPTY (TypedToolWithOutput invariant).
    assert!(
        calc.output_schema.is_some() && refund.output_schema.is_some(),
        "every per-Table tool advertises a non-empty outputSchema"
    );
    assert!(
        !output_keys(&calc).is_empty() && !output_keys(&refund).is_empty(),
        "each tool enumerates at least one output column"
    );

    // calculate_tax projects the four tax outputs; estimate_refund projects refund.
    let mut calc_out = output_keys(&calc);
    calc_out.sort();
    assert_eq!(
        calc_out,
        vec![
            "effective_rate",
            "marginal_rate",
            "tax_owed",
            "taxable_income"
        ],
        "calculate_tax projects exactly its own Table's outputs"
    );
    assert_eq!(
        output_keys(&refund),
        vec!["refund".to_string()],
        "estimate_refund projects exactly its own Table's output"
    );

    // ---- the DAG-derivation proof: DISJOINT, per-tool input key sets ----
    let mut calc_in = input_keys(&calc);
    calc_in.sort();
    let mut refund_in = input_keys(&refund);
    refund_in.sort();

    assert_eq!(
        calc_in,
        vec!["deductions".to_string(), "gross_income".to_string()],
        "calculate_tax's inputSchema carries ONLY its DAG-reachable inputs (no withheld)"
    );
    assert_eq!(
        refund_in,
        vec![
            "deductions".to_string(),
            "gross_income".to_string(),
            "withheld".to_string()
        ],
        "estimate_refund's inputSchema additionally carries `withheld` (its refund formula uses it)"
    );

    // The two key sets DIFFER — they are not a single workbook-wide union (proof
    // the schemas are genuinely DAG-derived per tool).
    assert_ne!(
        calc_in, refund_in,
        "the two tools' input key sets are disjoint on `withheld` (DAG-derived, per-tool)"
    );
    assert!(
        refund_in.contains(&"withheld".to_string()) && !calc_in.contains(&"withheld".to_string()),
        "`withheld` is advertised by estimate_refund but NOT by calculate_tax"
    );

    // Both per-tool input schemas keep the strict envelope (V5).
    for tool in [&calc, &refund] {
        assert_eq!(
            tool.input_schema["additionalProperties"],
            Value::Bool(false),
            "every per-tool inputSchema keeps additionalProperties:false (strict envelope)"
        );
    }
}
