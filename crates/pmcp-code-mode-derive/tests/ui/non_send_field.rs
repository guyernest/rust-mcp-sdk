use pmcp_code_mode::{
    CodeExecutor, CodeModeConfig, ExecutionError, NoopPolicyEvaluator, TokenSecret,
};
use pmcp_code_mode_derive::CodeMode;
use std::rc::Rc;
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
    policy_evaluator: Arc<NoopPolicyEvaluator>,
    code_executor: Arc<MyExecutor>,
    non_send: Rc<String>, // Rc is not Send!
}

fn main() {}
