use pmcp_macros::mcp_tool;
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
struct Args {
    x: i32,
}

// No rustdoc AND empty args -- should fail at compile time (PARITY-MACRO-01).
// Exercises the empty-args branch of the missing-description error path.
#[mcp_tool()]
async fn bad_tool(args: Args) -> pmcp::Result<serde_json::Value> {
    Ok(serde_json::json!({}))
}

fn main() {}
