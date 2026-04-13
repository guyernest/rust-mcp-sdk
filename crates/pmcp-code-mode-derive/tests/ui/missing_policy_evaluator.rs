use pmcp_code_mode::{CodeExecutor, CodeModeConfig, ExecutionError, TokenSecret};
use pmcp_code_mode_derive::CodeMode;
use std::sync::Arc;

struct MyExecutor;

#[pmcp_code_mode::async_trait]
impl CodeExecutor for MyExecutor {
    async fn execute(
        &self,
        _code: &str,
        _vars: Option<&serde_json::Value>,
    ) -> Result<serde_json::Value, ExecutionError> {
        Ok(serde_json::json!({}))
    }
}

#[derive(CodeMode)]
struct MyServer {
    code_mode_config: CodeModeConfig,
    token_secret: TokenSecret,
    // Missing: policy_evaluator
    code_executor: Arc<MyExecutor>,
}

fn main() {}
