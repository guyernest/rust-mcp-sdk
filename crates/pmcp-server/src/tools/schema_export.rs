//! Schema discovery and export tool.
//!
//! Connects to a remote MCP server, discovers its tools, resources, and
//! prompts, then returns the schemas as JSON or generates Rust type stubs.

use async_trait::async_trait;
use pmcp::types::ToolInfo;
use pmcp::ToolHandler;
use serde::Deserialize;
use serde_json::{json, Value};

use super::{create_tester, default_timeout, internal_err};
use crate::util::to_pascal_case;

/// Input parameters for the `schema_export` tool.
#[derive(Deserialize)]
struct SchemaExportInput {
    /// MCP server URL to export schemas from.
    url: String,
    /// Output format: "json" (default) or "rust".
    #[serde(default = "default_format")]
    format: String,
    /// Timeout in seconds (default: 30).
    #[serde(default = "default_timeout")]
    timeout: u64,
}

fn default_format() -> String {
    "json".to_string()
}

/// Schema discovery and export tool.
///
/// Connects to a remote MCP server and exports discovered tool, resource,
/// and prompt schemas in JSON or Rust type-stub format.
pub struct SchemaExportTool;

#[async_trait]
impl ToolHandler for SchemaExportTool {
    async fn handle(&self, args: Value, _extra: pmcp::RequestHandlerExtra) -> pmcp::Result<Value> {
        let params: SchemaExportInput = serde_json::from_value(args)
            .map_err(|e| pmcp::Error::validation(format!("Invalid arguments: {e}")))?;

        if params.format != "json" && params.format != "rust" {
            return Err(pmcp::Error::validation(format!(
                "Unknown format '{}'. Available: json, rust",
                params.format
            )));
        }

        let mut tester = create_tester(&params.url, params.timeout)?;

        // Initialize the connection.
        tester
            .run_quick_test()
            .await
            .map_err(internal_err)?;

        // Explicitly load tools (run_quick_test only initializes, doesn't list tools).
        let tools_result = tester.test_tools_list().await;
        if tools_result.status == mcp_tester::TestStatus::Failed {
            return Err(internal_err(
                tools_result.error.unwrap_or_else(|| "failed to list tools".into()),
            ));
        }

        let server_name = tester
            .get_server_name()
            .unwrap_or_else(|| "unknown".to_string());
        let server_version = tester
            .get_server_version()
            .unwrap_or_else(|| "unknown".to_string());

        let mut response = json!({
            "server_name": server_name,
            "server_version": server_version,
            "format": params.format,
        });

        if params.format == "json" {
            // JSON format: serialize all schemas.
            let tools_value: Value = match tester.get_tools() {
                Some(tools) => serde_json::to_value(tools).map_err(internal_err)?,
                None => json!([]),
            };
            let resources_value: Value = match tester.list_resources().await {
                Ok(res) => serde_json::to_value(&res.resources).map_err(internal_err)?,
                Err(_) => json!([]),
            };
            let prompts_value: Value = match tester.list_prompts().await {
                Ok(res) => serde_json::to_value(&res.prompts).map_err(internal_err)?,
                Err(_) => json!([]),
            };
            response["tools"] = tools_value;
            response["resources"] = resources_value;
            response["prompts"] = prompts_value;
        } else {
            // Rust format: generate type stubs from tool schemas.
            let rust_types = generate_rust_types(tester.get_tools());
            response["rust_types"] = json!(rust_types);
        }

        Ok(response)
    }

    fn metadata(&self) -> Option<ToolInfo> {
        Some(ToolInfo::new(
            "schema_export",
            Some(
                "Connect to a remote MCP server and export its tool/resource/prompt schemas"
                    .to_string(),
            ),
            json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "MCP server URL to export schemas from"
                    },
                    "format": {
                        "type": "string",
                        "enum": ["json", "rust"],
                        "description": "Output format (default: json)",
                        "default": "json"
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

// ---------------------------------------------------------------------------
// Rust type generation from JSON Schema
// ---------------------------------------------------------------------------

/// Map a JSON Schema type string to its Rust equivalent.
fn json_type_to_rust(json_type: &str) -> &str {
    match json_type {
        "string" => "String",
        "number" => "f64",
        "integer" => "i64",
        "boolean" => "bool",
        "array" => "Vec<serde_json::Value>",
        "object" => "serde_json::Value",
        _ => "serde_json::Value",
    }
}

/// Generate Rust type stubs from discovered tool schemas.
fn generate_rust_types(tools: Option<&Vec<pmcp::types::ToolInfo>>) -> String {
    let tools = match tools {
        Some(t) if !t.is_empty() => t,
        _ => return "// No tools discovered -- no types to generate.\n".to_string(),
    };

    let mut output = String::from("use serde::Deserialize;\n\n");

    for tool in tools {
        let struct_name = format!("{}Input", to_pascal_case(&tool.name));
        let properties = tool
            .input_schema
            .get("properties")
            .and_then(|p| p.as_object());

        let required: Vec<&str> = tool
            .input_schema
            .get("required")
            .and_then(|r| r.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
            .unwrap_or_default();

        // Check for camelCase field names (scoped per tool).
        let has_camel_case = properties
            .map(|props| props.keys().any(|k| k.chars().any(|c| c.is_uppercase())))
            .unwrap_or(false);

        output.push_str("#[derive(Deserialize)]\n");
        if has_camel_case {
            output.push_str("#[serde(rename_all = \"camelCase\")]\n");
        }
        output.push_str(&format!("pub struct {struct_name} {{\n"));

        if let Some(props) = properties {
            for (field_name, field_schema) in props {
                let field_type_str = field_schema
                    .get("type")
                    .and_then(|t| t.as_str())
                    .unwrap_or("object");
                let rust_type = json_type_to_rust(field_type_str);

                // Add doc comment from description if present.
                if let Some(desc) = field_schema.get("description").and_then(|d| d.as_str()) {
                    output.push_str(&format!("    /// {desc}\n"));
                }

                let is_required = required.contains(&field_name.as_str());
                if is_required {
                    output.push_str(&format!("    pub {field_name}: {rust_type},\n"));
                } else {
                    output.push_str(&format!("    pub {field_name}: Option<{rust_type}>,\n"));
                }
            }
        }

        output.push_str("}\n\n");
    }

    output
}
