//! Test scenario generation tool.
//!
//! Wraps `mcp_tester::ScenarioGenerator::create_scenario_struct()` to let
//! users generate test scenarios from a live MCP server's capabilities.

use async_trait::async_trait;
use mcp_tester::{ScenarioGenerator, ServerTester};
use pmcp::types::ToolInfo;
use pmcp::ToolHandler;
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Duration;

/// Input parameters for the `test_generate` tool.
#[derive(Deserialize)]
struct TestGenerateInput {
    /// MCP server URL to discover capabilities from.
    url: String,
    /// Include all discovered tools in the scenario.
    #[serde(default = "default_true")]
    all_tools: bool,
    /// Include resource testing in the scenario.
    #[serde(default)]
    with_resources: bool,
    /// Include prompt testing in the scenario.
    #[serde(default)]
    with_prompts: bool,
}

const fn default_true() -> bool {
    true
}

/// Test scenario generation tool.
///
/// Connects to a remote MCP server, discovers its tools, resources, and
/// prompts, then generates a structured test scenario as JSON.
pub struct TestGenerateTool;

#[async_trait]
impl ToolHandler for TestGenerateTool {
    async fn handle(&self, args: Value, _extra: pmcp::RequestHandlerExtra) -> pmcp::Result<Value> {
        let params: TestGenerateInput = serde_json::from_value(args)
            .map_err(|e| pmcp::Error::validation(format!("Invalid arguments: {e}")))?;

        let mut tester = ServerTester::new(
            &params.url,
            Duration::from_secs(30),
            false, // insecure
            None,  // api_key
            None,  // transport (auto-detect)
            None,  // http_middleware_chain
        )
        .map_err(|e| pmcp::Error::Internal(e.to_string()))?;

        let generator = ScenarioGenerator::new(
            params.url,
            params.all_tools,
            params.with_resources,
            params.with_prompts,
        );

        let scenario = generator
            .create_scenario_struct(&mut tester)
            .await
            .map_err(|e| pmcp::Error::Internal(e.to_string()))?;

        serde_json::to_value(&scenario).map_err(|e| pmcp::Error::Internal(e.to_string()))
    }

    fn metadata(&self) -> Option<ToolInfo> {
        Some(ToolInfo::new(
            "test_generate",
            Some(
                "Generate test scenarios from a remote MCP server's capabilities".to_string(),
            ),
            json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "MCP server URL to discover capabilities from"
                    },
                    "all_tools": {
                        "type": "boolean",
                        "description": "Include all discovered tools in the scenario",
                        "default": true
                    },
                    "with_resources": {
                        "type": "boolean",
                        "description": "Include resource testing in the scenario",
                        "default": false
                    },
                    "with_prompts": {
                        "type": "boolean",
                        "description": "Include prompt testing in the scenario",
                        "default": false
                    }
                },
                "required": ["url"]
            }),
        ))
    }
}
