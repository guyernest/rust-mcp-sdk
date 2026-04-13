use pmcp_code_mode::{
    CodeExecutor, CodeModeConfig, ExecutionError, NoopPolicyEvaluator, TokenSecret,
    ValidationContext,
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
#[code_mode(context_from = "get_context")]
struct MyServer {
    code_mode_config: CodeModeConfig,
    token_secret: TokenSecret,
    policy_evaluator: Arc<NoopPolicyEvaluator>,
    code_executor: Arc<MyExecutor>,
}

impl MyServer {
    fn get_context(&self, _extra: &pmcp::RequestHandlerExtra) -> ValidationContext {
        ValidationContext::new("user-1", "session-1", "schema-v1", "perms-v1")
    }
}

fn main() {
    // Type-check: verify the generated method requires Arc<Self>
    fn _check_arc_method(server: &Arc<MyServer>, builder: pmcp::ServerBuilder) {
        let _builder: Result<pmcp::ServerBuilder, pmcp_code_mode::TokenError> =
            server.register_code_mode_tools(builder);
    }
}
