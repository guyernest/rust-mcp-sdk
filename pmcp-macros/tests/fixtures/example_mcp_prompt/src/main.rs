//! Snapshot fixture for `#[mcp_prompt]` (Phase 75 Wave 0 Task 1).
//!
//! Captures the macro output for a typed-args prompt so Wave 1b can detect any
//! expansion drift via `cargo insta accept`.

use pmcp::types::{Content, GetPromptResult, PromptMessage};
use pmcp_macros::mcp_prompt;
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
struct ReviewArgs {
    language: String,
    code: String,
}

#[mcp_prompt(description = "Review code for issues")]
async fn code_review(args: ReviewArgs) -> pmcp::Result<GetPromptResult> {
    Ok(GetPromptResult::new(
        vec![PromptMessage::user(Content::text(format!(
            "Review this {} code:\n{}",
            args.language, args.code
        )))],
        Some("Code review".to_string()),
    ))
}

fn main() {
    let _prompt = code_review();
}
