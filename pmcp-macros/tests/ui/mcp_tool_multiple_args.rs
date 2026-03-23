use pmcp_macros::mcp_tool;
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
struct ArgsA {
    x: i32,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ArgsB {
    y: String,
}

// Two non-special params -- should fail (Pitfall 3)
#[mcp_tool(description = "Bad tool")]
async fn bad_tool(a: ArgsA, b: ArgsB) -> pmcp::Result<serde_json::Value> {
    Ok(serde_json::json!({}))
}

fn main() {}
