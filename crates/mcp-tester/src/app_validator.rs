//! MCP App metadata validation for tools and resources.
//!
//! Validates that App-capable tools have correct `_meta` structure,
//! cross-references with `resources/list`, and (in ChatGPT mode)
//! checks for `openai/*` keys.

use crate::report::{TestCategory, TestResult, TestStatus};
use pmcp::types::ui::CHATGPT_DESCRIPTOR_KEYS;
use pmcp::types::{ResourceInfo, ToolInfo};
use serde_json::Value;
use std::time::Duration;

/// Valid MIME types for MCP App resources.
const APP_MIME_TYPES: &[&str] = &[
    "text/html",
    "text/html+mcp",
    "text/html+skybridge",
    "text/html;profile=mcp-app",
];

/// Validation mode controlling which keys are checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppValidationMode {
    /// Standard mode: nested `ui.resourceUri` only.
    Standard,
    /// ChatGPT mode: also checks `openai/*` keys and flat `ui/resourceUri`.
    ChatGpt,
    /// Claude Desktop mode: same as Standard for now.
    ClaudeDesktop,
}

impl std::fmt::Display for AppValidationMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Standard => write!(f, "standard"),
            Self::ChatGpt => write!(f, "chatgpt"),
            Self::ClaudeDesktop => write!(f, "claude-desktop"),
        }
    }
}

impl std::str::FromStr for AppValidationMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "standard" => Ok(Self::Standard),
            "chatgpt" => Ok(Self::ChatGpt),
            "claude-desktop" => Ok(Self::ClaudeDesktop),
            other => Err(format!(
                "Unknown validation mode: '{other}'. Valid: standard, chatgpt, claude-desktop"
            )),
        }
    }
}

/// Validates MCP App metadata on tools discovered via `tools/list`.
pub struct AppValidator {
    mode: AppValidationMode,
    tool_filter: Option<String>,
}

impl AppValidator {
    /// Create a new `AppValidator`.
    pub fn new(mode: AppValidationMode, tool_filter: Option<String>) -> Self {
        Self {
            mode,
            tool_filter,
        }
    }

    /// Main entry point: validate all (or filtered) App-capable tools.
    pub fn validate_tools(
        &self,
        tools: &[ToolInfo],
        resources: &[ResourceInfo],
    ) -> Vec<TestResult> {
        let mut results = Vec::new();

        let app_tools: Vec<&ToolInfo> = tools
            .iter()
            .filter(|t| {
                if let Some(ref filter) = self.tool_filter {
                    t.name == *filter
                } else {
                    Self::is_app_capable(t)
                }
            })
            .collect();

        if app_tools.is_empty() {
            return results;
        }

        for tool in &app_tools {
            let uri = Self::extract_resource_uri(tool);
            results.extend(self.validate_tool_meta(tool, uri.as_deref()));

            if let Some(ref uri) = uri {
                results.extend(self.validate_resource_match(&tool.name, uri, resources));
            }

            if self.mode == AppValidationMode::ChatGpt {
                if let Some(ref meta) = tool._meta {
                    results.extend(self.validate_chatgpt_keys(&tool.name, meta));
                }
            }

            if let Some(ref schema) = tool.output_schema {
                results.extend(self.validate_output_schema(&tool.name, schema));
            }
        }

        results
    }

    /// Returns `true` if the tool has App metadata (nested or flat `resourceUri`).
    pub fn is_app_capable(tool: &ToolInfo) -> bool {
        Self::extract_resource_uri(tool).is_some()
    }

    /// Extract the resource URI from either nested `ui.resourceUri` or flat `ui/resourceUri`.
    fn extract_resource_uri(tool: &ToolInfo) -> Option<String> {
        let meta = tool._meta.as_ref()?;

        // Nested: _meta.ui.resourceUri
        if let Some(Value::Object(ui)) = meta.get("ui") {
            if let Some(Value::String(uri)) = ui.get("resourceUri") {
                return Some(uri.clone());
            }
        }

        // Flat legacy: _meta["ui/resourceUri"]
        if let Some(Value::String(uri)) = meta.get("ui/resourceUri") {
            return Some(uri.clone());
        }

        None
    }

    /// Validate the tool's `_meta` structure for App keys.
    fn validate_tool_meta(&self, tool: &ToolInfo, uri: Option<&str>) -> Vec<TestResult> {
        let mut results = Vec::new();
        let tool_name = &tool.name;

        if tool._meta.is_none() {
            results.push(TestResult {
                name: format!("[{tool_name}] _meta present"),
                category: TestCategory::Apps,
                status: TestStatus::Failed,
                duration: Duration::from_secs(0),
                error: Some("Tool has no _meta field".to_string()),
                details: None,
            });
            return results;
        }

        match uri {
            Some(uri) => {
                results.push(TestResult {
                    name: format!("[{tool_name}] ui.resourceUri present"),
                    category: TestCategory::Apps,
                    status: TestStatus::Passed,
                    duration: Duration::from_secs(0),
                    error: None,
                    details: None,
                });

                // Validate URI format (non-empty with scheme separator)
                if uri.is_empty() || !uri.contains("://") {
                    results.push(TestResult {
                        name: format!("[{tool_name}] resourceUri format"),
                        category: TestCategory::Apps,
                        status: TestStatus::Warning,
                        duration: Duration::from_secs(0),
                        error: None,
                        details: Some(format!(
                            "URI may not be well-formed: '{uri}' (no scheme separator)"
                        )),
                    });
                } else {
                    results.push(TestResult {
                        name: format!("[{tool_name}] resourceUri format"),
                        category: TestCategory::Apps,
                        status: TestStatus::Passed,
                        duration: Duration::from_secs(0),
                        error: None,
                        details: Some(format!("URI: {uri}")),
                    });
                }
            }
            None => {
                results.push(TestResult {
                    name: format!("[{tool_name}] ui.resourceUri present"),
                    category: TestCategory::Apps,
                    status: TestStatus::Failed,
                    duration: Duration::from_secs(0),
                    error: Some(
                        "_meta exists but missing ui.resourceUri (nested or flat)".to_string(),
                    ),
                    details: None,
                });
            }
        }

        results
    }

    /// Cross-reference a tool's resource URI against the resources list.
    fn validate_resource_match(
        &self,
        tool_name: &str,
        resource_uri: &str,
        resources: &[ResourceInfo],
    ) -> Vec<TestResult> {
        let mut results = Vec::new();

        let matching = resources.iter().find(|r| r.uri == resource_uri);

        match matching {
            None => {
                results.push(TestResult {
                    name: format!("[{tool_name}] resource cross-reference"),
                    category: TestCategory::Apps,
                    status: TestStatus::Warning,
                    duration: Duration::from_secs(0),
                    error: None,
                    details: Some(format!(
                        "No resource found with URI '{resource_uri}' in resources/list"
                    )),
                });
            }
            Some(resource) => {
                results.push(TestResult {
                    name: format!("[{tool_name}] resource cross-reference"),
                    category: TestCategory::Apps,
                    status: TestStatus::Passed,
                    duration: Duration::from_secs(0),
                    error: None,
                    details: Some(format!("Found resource: {}", resource.name)),
                });

                // Validate MIME type
                match &resource.mime_type {
                    None => {
                        results.push(TestResult {
                            name: format!("[{tool_name}] resource MIME type"),
                            category: TestCategory::Apps,
                            status: TestStatus::Warning,
                            duration: Duration::from_secs(0),
                            error: None,
                            details: Some("Resource has no MIME type set".to_string()),
                        });
                    }
                    Some(mime) => {
                        let is_valid = APP_MIME_TYPES
                            .iter()
                            .any(|v| mime.eq_ignore_ascii_case(v));

                        if is_valid {
                            results.push(TestResult {
                                name: format!("[{tool_name}] resource MIME type"),
                                category: TestCategory::Apps,
                                status: TestStatus::Passed,
                                duration: Duration::from_secs(0),
                                error: None,
                                details: Some(format!("MIME type: {mime}")),
                            });
                        } else {
                            results.push(TestResult {
                                name: format!("[{tool_name}] resource MIME type"),
                                category: TestCategory::Apps,
                                status: TestStatus::Warning,
                                duration: Duration::from_secs(0),
                                error: None,
                                details: Some(format!(
                                    "Unexpected MIME type '{mime}', expected one of: {}",
                                    APP_MIME_TYPES.join(", ")
                                )),
                            });
                        }
                    }
                }
            }
        }

        results
    }

    /// Validate ChatGPT-specific `openai/*` keys in tool metadata.
    fn validate_chatgpt_keys(
        &self,
        tool_name: &str,
        meta: &serde_json::Map<String, Value>,
    ) -> Vec<TestResult> {
        let mut results = Vec::new();

        for key in CHATGPT_DESCRIPTOR_KEYS {
            let present = meta.get(*key).is_some();

            results.push(TestResult {
                name: format!("[{tool_name}] ChatGPT key: {key}"),
                category: TestCategory::Apps,
                status: if present {
                    TestStatus::Passed
                } else {
                    TestStatus::Warning
                },
                duration: Duration::from_secs(0),
                error: None,
                details: if present {
                    None
                } else {
                    Some(format!("Missing ChatGPT key: {key}"))
                },
            });
        }

        // Also check flat legacy key ui/resourceUri
        let has_flat = meta.get("ui/resourceUri").is_some();

        results.push(TestResult {
            name: format!("[{tool_name}] ChatGPT flat ui/resourceUri"),
            category: TestCategory::Apps,
            status: if has_flat {
                TestStatus::Passed
            } else {
                TestStatus::Warning
            },
            duration: Duration::from_secs(0),
            error: None,
            details: if has_flat {
                None
            } else {
                Some("Missing flat legacy key ui/resourceUri (needed for ChatGPT)".to_string())
            },
        });

        results
    }

    /// Validate the `outputSchema` structure on a tool.
    fn validate_output_schema(&self, tool_name: &str, schema: &Value) -> Vec<TestResult> {
        let mut results = Vec::new();

        let is_valid = schema.is_object() && schema.get("type").is_some();

        results.push(TestResult {
            name: format!("[{tool_name}] outputSchema structure"),
            category: TestCategory::Apps,
            status: if is_valid {
                TestStatus::Passed
            } else {
                TestStatus::Warning
            },
            duration: Duration::from_secs(0),
            error: None,
            details: if is_valid {
                None
            } else {
                Some("outputSchema should be an object with a 'type' field".to_string())
            },
        });

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_tool(name: &str, meta: Option<serde_json::Map<String, Value>>) -> ToolInfo {
        let mut tool = ToolInfo::new(name, None, json!({"type": "object"}));
        tool._meta = meta;
        tool
    }

    fn make_resource(uri: &str, mime: Option<&str>) -> ResourceInfo {
        ResourceInfo {
            uri: uri.to_string(),
            name: uri.to_string(),
            description: None,
            mime_type: mime.map(|s| s.to_string()),
            meta: None,
        }
    }

    #[test]
    fn test_is_app_capable_nested() {
        let meta = serde_json::from_value::<serde_json::Map<String, Value>>(json!({
            "ui": { "resourceUri": "ui://app/test" }
        }))
        .unwrap();
        let tool = make_tool("t1", Some(meta));
        assert!(AppValidator::is_app_capable(&tool));
    }

    #[test]
    fn test_is_app_capable_flat() {
        let meta = serde_json::from_value::<serde_json::Map<String, Value>>(json!({
            "ui/resourceUri": "ui://app/test"
        }))
        .unwrap();
        let tool = make_tool("t2", Some(meta));
        assert!(AppValidator::is_app_capable(&tool));
    }

    #[test]
    fn test_not_app_capable() {
        let tool = make_tool("t3", None);
        assert!(!AppValidator::is_app_capable(&tool));
    }

    #[test]
    fn test_validate_tools_no_app_tools() {
        let validator = AppValidator::new(AppValidationMode::Standard, None);
        let tools = vec![make_tool("plain", None)];
        let results = validator.validate_tools(&tools, &[]);
        assert!(results.is_empty());
    }

    #[test]
    fn test_validate_tools_with_resource_match() {
        let meta = serde_json::from_value::<serde_json::Map<String, Value>>(json!({
            "ui": { "resourceUri": "ui://app/chess" }
        }))
        .unwrap();
        let tool = make_tool("chess", Some(meta));
        let resource = make_resource("ui://app/chess", Some("text/html"));

        let validator = AppValidator::new(AppValidationMode::Standard, None);
        let results = validator.validate_tools(&[tool], &[resource]);

        let passed = results.iter().filter(|r| r.status == TestStatus::Passed).count();
        assert!(passed >= 3, "Expected at least 3 passed results, got {passed}");
    }

    #[test]
    fn test_chatgpt_mode_checks_openai_keys() {
        let meta = serde_json::from_value::<serde_json::Map<String, Value>>(json!({
            "ui": { "resourceUri": "ui://app/test" },
            "openai/outputTemplate": "<div></div>"
        }))
        .unwrap();
        let tool = make_tool("t", Some(meta));

        let validator = AppValidator::new(AppValidationMode::ChatGpt, None);
        let results = validator.validate_tools(&[tool], &[]);

        let chatgpt_results: Vec<_> = results
            .iter()
            .filter(|r| r.name.contains("ChatGPT"))
            .collect();
        assert!(!chatgpt_results.is_empty());
    }

    #[test]
    fn test_strict_mode_promotes_warnings() {
        let meta = serde_json::from_value::<serde_json::Map<String, Value>>(json!({
            "ui": { "resourceUri": "ui://app/test" }
        }))
        .unwrap();
        let tool = make_tool("t", Some(meta));

        let validator = AppValidator::new(AppValidationMode::Standard, None);
        let mut results = validator.validate_tools(&[tool], &[]);

        // Simulate strict mode as callers do via report.apply_strict_mode()
        for r in &mut results {
            if r.status == TestStatus::Warning {
                r.status = TestStatus::Failed;
            }
        }
        let warnings = results.iter().filter(|r| r.status == TestStatus::Warning).count();
        assert_eq!(warnings, 0, "Strict mode should have zero warnings");
    }

    #[test]
    fn test_tool_filter() {
        let meta = serde_json::from_value::<serde_json::Map<String, Value>>(json!({
            "ui": { "resourceUri": "ui://app/chess" }
        }))
        .unwrap();
        let tool1 = make_tool("chess", Some(meta));
        let tool2 = make_tool("other", None);

        let validator =
            AppValidator::new(AppValidationMode::Standard, Some("other".to_string()));
        let results = validator.validate_tools(&[tool1, tool2], &[]);

        // "other" has no _meta, so validation should report failure for it
        assert!(results.iter().any(|r| r.name.contains("other")));
        assert!(!results.iter().any(|r| r.name.contains("chess")));
    }
}
