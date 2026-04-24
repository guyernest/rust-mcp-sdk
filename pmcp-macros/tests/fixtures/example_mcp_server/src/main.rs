//! Snapshot fixture for `#[mcp_server]` (Phase 75 Wave 0 Task 1).
//!
//! Captures the macro output for an impl block with two tools so Wave 1b can
//! detect any expansion drift via `cargo insta accept`.

use pmcp_macros::mcp_server;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, JsonSchema)]
struct AddArgs {
    a: f64,
    b: f64,
}

#[derive(Debug, Serialize, JsonSchema)]
struct AddResult {
    result: f64,
}

struct Calculator;

#[mcp_server]
impl Calculator {
    #[mcp_tool(description = "Add two numbers")]
    async fn add(&self, args: AddArgs) -> pmcp::Result<AddResult> {
        Ok(AddResult {
            result: args.a + args.b,
        })
    }

    #[mcp_tool(description = "Subtract two numbers")]
    async fn subtract(&self, args: AddArgs) -> pmcp::Result<AddResult> {
        Ok(AddResult {
            result: args.a - args.b,
        })
    }
}

fn main() {
    let _calc = Calculator;
}
