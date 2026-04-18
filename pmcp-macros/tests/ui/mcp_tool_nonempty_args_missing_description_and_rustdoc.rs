use pmcp_macros::mcp_tool;
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
struct Args {
    x: i32,
}

// Non-empty args (name is set) but NO description attr AND NO rustdoc -- should
// still fail at compile time (PARITY-MACRO-01). Locks the "args present, but
// description still missing" path against regression (MEDIUM-2).
#[mcp_tool(name = "custom_name")]
async fn bad_tool_with_name(args: Args) -> pmcp::Result<serde_json::Value> {
    Ok(serde_json::json!({}))
}

fn main() {}
