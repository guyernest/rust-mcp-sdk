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

// Tests that the language attribute is accepted and changes tool metadata.
// Note: language = "javascript" requires the `openapi-code-mode` feature flag
// on pmcp-code-mode (it calls validate_javascript_code which is feature-gated).
#[derive(CodeMode)]
#[code_mode(language = "graphql")]
struct MyServer {
    code_mode_config: CodeModeConfig,
    token_secret: TokenSecret,
    policy_evaluator: Arc<NoopPolicyEvaluator>,
    code_executor: Arc<MyExecutor>,
}

fn main() {
    // Type-check: verify the generated method uses &self (NOT Arc<Self>)
    // This proves backward compatibility -- non-context_from users keep &self
    #[allow(deprecated)]
    fn _check_method(server: &MyServer, builder: pmcp::ServerBuilder) {
        let _builder: Result<pmcp::ServerBuilder, pmcp_code_mode::TokenError> =
            server.register_code_mode_tools(builder);
    }
}
