//! MCP Apps metadata validation tool.
//!
//! Wraps `mcp_tester::AppValidator::validate_tools()` to validate that
//! App-capable tools have correct `_meta` structure and resource cross-references.

use async_trait::async_trait;
use mcp_tester::{AppValidationMode, AppValidator, ServerTester};
use pmcp::types::ToolInfo;
use pmcp::ToolHandler;
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Duration;

/// Input parameters for the `test_apps` tool.
#[derive(Deserialize)]
struct TestAppsInput {
    /// MCP server URL to validate.
    url: String,
    /// Validation mode: "standard", "chatgpt", "claude", or "all".
    #[serde(default = "default_mode")]
    mode: String,
    /// Filter to a single tool name.
    #[serde(default)]
    tool_filter: Option<String>,
    /// Promote warnings to failures.
    #[serde(default)]
    strict: bool,
}

fn default_mode() -> String {
    "standard".to_string()
}

/// Parse a mode string into one or more `AppValidationMode` values.
///
/// The "all" mode is not a single enum variant; instead it runs validation
/// in every mode and combines the results.
fn parse_modes(mode: &str) -> Result<Vec<AppValidationMode>, String> {
    match mode {
        "standard" => Ok(vec![AppValidationMode::Standard]),
        "chatgpt" => Ok(vec![AppValidationMode::ChatGpt]),
        "claude" | "claude-desktop" => Ok(vec![AppValidationMode::ClaudeDesktop]),
        "all" => Ok(vec![
            AppValidationMode::Standard,
            AppValidationMode::ChatGpt,
            AppValidationMode::ClaudeDesktop,
        ]),
        other => Err(format!(
            "Unknown validation mode: '{other}'. Valid: standard, chatgpt, claude, all"
        )),
    }
}

/// MCP Apps metadata validation tool.
///
/// Connects to a remote MCP server, discovers its tools and resources,
/// then validates App metadata structure and cross-references.
pub struct TestAppsTool;

#[async_trait]
impl ToolHandler for TestAppsTool {
    async fn handle(&self, args: Value, _extra: pmcp::RequestHandlerExtra) -> pmcp::Result<Value> {
        let params: TestAppsInput = serde_json::from_value(args)
            .map_err(|e| pmcp::Error::validation(format!("Invalid arguments: {e}")))?;

        let modes =
            parse_modes(&params.mode).map_err(pmcp::Error::validation)?;

        let mut tester = ServerTester::new(
            &params.url,
            Duration::from_secs(30),
            false, // insecure
            None,  // api_key
            None,  // transport (auto-detect)
            None,  // http_middleware_chain
        )
        .map_err(|e| pmcp::Error::Internal(e.to_string()))?;

        // Initialize the server connection to discover tools and resources.
        tester
            .run_quick_test()
            .await
            .map_err(|e| pmcp::Error::Internal(e.to_string()))?;

        let tools = tester.get_tools().cloned().unwrap_or_default();
        let resources = tester
            .list_resources()
            .await
            .map(|r| r.resources)
            .unwrap_or_default();

        let mut all_results = Vec::new();
        for mode in modes {
            let validator = AppValidator::new(mode, params.tool_filter.clone());
            let mut results = validator.validate_tools(&tools, &resources);

            if params.strict {
                for r in &mut results {
                    if r.status == mcp_tester::TestStatus::Warning {
                        r.status = mcp_tester::TestStatus::Failed;
                    }
                }
            }

            all_results.extend(results);
        }

        serde_json::to_value(&all_results).map_err(|e| pmcp::Error::Internal(e.to_string()))
    }

    fn metadata(&self) -> Option<ToolInfo> {
        Some(ToolInfo::new(
            "test_apps",
            Some("Validate MCP Apps metadata on a remote server's tools".to_string()),
            json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "MCP server URL to validate"
                    },
                    "mode": {
                        "type": "string",
                        "description": "Validation mode",
                        "enum": ["standard", "chatgpt", "claude", "all"],
                        "default": "standard"
                    },
                    "tool_filter": {
                        "type": "string",
                        "description": "Filter to a single tool name"
                    },
                    "strict": {
                        "type": "boolean",
                        "description": "Promote warnings to failures",
                        "default": false
                    }
                },
                "required": ["url"]
            }),
        ))
    }
}
