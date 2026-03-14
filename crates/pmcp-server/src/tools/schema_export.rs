//! Schema discovery and export tool.
//!
//! Connects to a remote MCP server, discovers its tools, resources, and
//! prompts, then returns the schemas as JSON or generates Rust type stubs.

use async_trait::async_trait;
use mcp_tester::ServerTester;
use pmcp::types::ToolInfo;
use pmcp::ToolHandler;
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Duration;

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

const fn default_timeout() -> u64 {
    30
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

        let mut tester = ServerTester::new(
            &params.url,
            Duration::from_secs(params.timeout),
            false, // insecure
            None,  // api_key
            None,  // transport (auto-detect)
            None,  // http_middleware_chain
        )
        .map_err(|e| pmcp::Error::Internal(e.to_string()))?;

        // Initialize the connection and discover capabilities.
        let _report = tester
            .run_quick_test()
            .await
            .map_err(|e| pmcp::Error::Internal(e.to_string()))?;

        let server_name = tester
            .get_server_name()
            .unwrap_or_else(|| "unknown".to_string());
        let server_version = tester
            .get_server_version()
            .unwrap_or_else(|| "unknown".to_string());

        // Collect tools.
        let tools_value: Value = match tester.get_tools() {
            Some(tools) => {
                serde_json::to_value(tools).map_err(|e| pmcp::Error::Internal(e.to_string()))?
            },
            None => json!([]),
        };

        // Collect resources (non-fatal on failure).
        let resources_value: Value = match tester.list_resources().await {
            Ok(res) => serde_json::to_value(&res.resources)
                .map_err(|e| pmcp::Error::Internal(e.to_string()))?,
            Err(_) => json!([]),
        };

        // Collect prompts (non-fatal on failure).
        let prompts_value: Value = match tester.list_prompts().await {
            Ok(res) => serde_json::to_value(&res.prompts)
                .map_err(|e| pmcp::Error::Internal(e.to_string()))?,
            Err(_) => json!([]),
        };

        let mut response = json!({
            "server_name": server_name,
            "server_version": server_version,
            "format": params.format,
            "tools": tools_value,
            "resources": resources_value,
            "prompts": prompts_value
        });

        // Generate Rust type stubs when requested.
        if params.format == "rust" {
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

/// Convert a snake_case or kebab-case tool name to PascalCase struct name.
fn to_pascal_case(s: &str) -> String {
    s.split(|c: char| c == '_' || c == '-')
        .filter(|seg| !seg.is_empty())
        .map(|seg| {
            let mut chars = seg.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => {
                    let upper: String = first.to_uppercase().collect();
                    upper + chars.as_str()
                },
            }
        })
        .collect()
}

/// Generate Rust type stubs from discovered tool schemas.
fn generate_rust_types(tools: Option<&Vec<pmcp::types::ToolInfo>>) -> String {
    let tools = match tools {
        Some(t) if !t.is_empty() => t,
        _ => return "// No tools discovered -- no types to generate.\n".to_string(),
    };

    let mut output = String::from("use serde::Deserialize;\n\n");
    let mut has_camel_case = false;

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

        // Check for camelCase field names.
        if let Some(props) = properties {
            if props.keys().any(|k| k.chars().any(|c| c.is_uppercase())) {
                has_camel_case = true;
            }
        }

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
