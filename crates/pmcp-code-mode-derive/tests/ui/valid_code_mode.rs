use pmcp_code_mode::{
    CodeExecutor, CodeModeConfig, ExecutionError, NoopPolicyEvaluator, TokenSecret,
};
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
struct MyServer {
    code_mode_config: CodeModeConfig,
    token_secret: TokenSecret,
    policy_evaluator: Arc<NoopPolicyEvaluator>,
    code_executor: Arc<MyExecutor>,
}

fn main() {
    // Type-check: verify the generated method signature exists
    #[allow(deprecated)]
    fn _check_method_exists(server: &MyServer, builder: pmcp::ServerBuilder) {
        let _builder: Result<pmcp::ServerBuilder, pmcp_code_mode::TokenError> =
            server.register_code_mode_tools(builder);
    }
}
