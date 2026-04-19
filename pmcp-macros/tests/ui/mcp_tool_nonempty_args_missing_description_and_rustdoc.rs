use pmcp_macros::mcp_tool;
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
struct Args {
    x: i32,
}

// Non-empty args (name is set) but no description attr and no rustdoc —
// should still fail at compile time.
#[mcp_tool(name = "custom_name")]
async fn bad_tool_with_name(args: Args) -> pmcp::Result<serde_json::Value> {
    Ok(serde_json::json!({}))
}

fn main() {}
