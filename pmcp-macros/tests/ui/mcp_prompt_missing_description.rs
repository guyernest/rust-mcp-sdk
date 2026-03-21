use pmcp_macros::mcp_prompt;

#[mcp_prompt()]
async fn bad_prompt() -> pmcp::Result<pmcp::types::GetPromptResult> {
    Ok(pmcp::types::GetPromptResult::new(vec![], None))
}

fn main() {}
