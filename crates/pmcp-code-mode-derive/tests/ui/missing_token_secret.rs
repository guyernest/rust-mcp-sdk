use pmcp_code_mode::{CodeExecutor, CodeModeConfig, ExecutionError, NoopPolicyEvaluator};
use pmcp_code_mode_derive::CodeMode;
use std::sync::Arc;

struct MyExecutor;

#[pmcp_code_mode::async_trait]
impl CodeExecutor for MyExecutor {
    async fn execute(
        &self,
        _code: &str,
        _variables: Option<&serde_json::Value>,
    ) -> Result<serde_json::Value, ExecutionError> {
        Ok(serde_json::json!({"status": "ok"}))
    }
}

#[derive(CodeMode)]
struct MissingTokenSecret {
    code_mode_config: CodeModeConfig,
    // token_secret is MISSING
    policy_evaluator: Arc<NoopPolicyEvaluator>,
    code_executor: Arc<MyExecutor>,
}

fn main() {}
