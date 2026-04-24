//! Snapshot fixture for `#[mcp_tool]` (Phase 75 Wave 0 Task 1).
//!
//! Captures the macro output for a representative tool with typed args + typed
//! result so Wave 1b can detect any expansion drift via `cargo insta accept`.

use pmcp_macros::mcp_tool;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, JsonSchema)]
struct EchoArgs {
    message: String,
}

#[derive(Debug, Serialize, JsonSchema)]
struct EchoResult {
    echoed: String,
}

#[mcp_tool(name = "echo", description = "Echo a message back to the caller")]
async fn echo(args: EchoArgs) -> pmcp::Result<EchoResult> {
    Ok(EchoResult {
        echoed: args.message,
    })
}

fn main() {
    // Empty main keeps the fixture buildable as a binary; the snapshot
    // captures the macro expansion above, not runtime behavior.
    let _ = echo;
}
