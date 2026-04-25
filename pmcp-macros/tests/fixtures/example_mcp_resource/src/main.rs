//! Snapshot fixture for `#[mcp_resource]` (Phase 75 Wave 0 Task 1).
//!
//! Captures the macro output for a templated resource provider so Wave 1b can
//! detect any expansion drift via `cargo insta accept`.

use pmcp_macros::mcp_resource;

#[mcp_resource(uri = "docs://{topic}", description = "Documentation pages")]
async fn read_doc(topic: String) -> pmcp::Result<String> {
    Ok(format!("# {topic}\n\nDocumentation content for `{topic}`."))
}

fn main() {
    let _provider = read_doc();
}
