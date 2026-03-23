use pmcp_macros::mcp_tool;
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
struct Args {
    x: i32,
}

// Missing description -- should fail at compile time (D-05)
#[mcp_tool()]
async fn bad_tool(args: Args) -> pmcp::Result<serde_json::Value> {
    Ok(serde_json::json!({}))
}

fn main() {}
