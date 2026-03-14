//! Protocol compliance testing tool.
//!
//! Wraps `mcp_tester::ServerTester::run_compliance_tests()` to let users
//! test any MCP server for protocol compliance via a single tool call.

use async_trait::async_trait;
use mcp_tester::ServerTester;
use pmcp::types::ToolInfo;
use pmcp::ToolHandler;
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Duration;

/// Input parameters for the `test_check` tool.
#[derive(Deserialize)]
struct TestCheckInput {
    /// MCP server URL to test.
    url: String,
    /// Enable strict compliance mode (warnings become failures).
    #[serde(default)]
    strict: bool,
    /// Timeout in seconds (default: 30).
    #[serde(default = "default_timeout")]
    timeout: u64,
}

const fn default_timeout() -> u64 {
    30
}

/// Protocol compliance testing tool.
///
/// Connects to a remote MCP server and runs the full compliance test suite,
/// returning a structured `TestReport` as JSON.
pub struct TestCheckTool;

#[async_trait]
impl ToolHandler for TestCheckTool {
    async fn handle(&self, args: Value, _extra: pmcp::RequestHandlerExtra) -> pmcp::Result<Value> {
        let params: TestCheckInput = serde_json::from_value(args)
            .map_err(|e| pmcp::Error::validation(format!("Invalid arguments: {e}")))?;

        let mut tester = ServerTester::new(
            &params.url,
            Duration::from_secs(params.timeout),
            false, // insecure
            None,  // api_key
            None,  // transport (auto-detect)
            None,  // http_middleware_chain
        )
        .map_err(|e| pmcp::Error::Internal(e.to_string()))?;

        let report = tester
            .run_compliance_tests(params.strict)
            .await
            .map_err(|e| pmcp::Error::Internal(e.to_string()))?;

        serde_json::to_value(&report).map_err(|e| pmcp::Error::Internal(e.to_string()))
    }

    fn metadata(&self) -> Option<ToolInfo> {
        Some(ToolInfo::new(
            "test_check",
            Some("Run MCP protocol compliance checks against a remote server".to_string()),
            json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "MCP server URL to test"
                    },
                    "strict": {
                        "type": "boolean",
                        "description": "Enable strict compliance mode (warnings become failures)",
                        "default": false
                    },
                    "timeout": {
                        "type": "integer",
                        "description": "Timeout in seconds",
                        "default": 30
                    }
                },
                "required": ["url"]
            }),
        ))
    }
}
