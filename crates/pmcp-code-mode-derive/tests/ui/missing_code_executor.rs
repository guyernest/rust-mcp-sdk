use pmcp_code_mode::{CodeModeConfig, NoopPolicyEvaluator, TokenSecret};
use pmcp_code_mode_derive::CodeMode;
use std::sync::Arc;

#[derive(CodeMode)]
struct MissingCodeExecutor {
    code_mode_config: CodeModeConfig,
    token_secret: TokenSecret,
    policy_evaluator: Arc<NoopPolicyEvaluator>,
    // code_executor is MISSING
}

fn main() {}
