//! Example showing user input elicitation in tools.
//!
//! Uses the MCP 2025-11-25 spec-compliant elicitation API with JSON Schema
//! for form-based user input.

use async_trait::async_trait;
use pmcp::error::Result as PmcpResult;
use pmcp::server::elicitation::{ElicitInput, ElicitationContext, ElicitationManager};
use pmcp::server::{Server, ToolHandler};
use pmcp::types::capabilities::ServerCapabilities;
use pmcp::types::elicitation::{ElicitAction, ElicitRequestParams, ElicitResult};
use pmcp::RequestHandlerExtra;
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::info;

/// Tool that demonstrates form-based elicitation using JSON Schema.
struct InteractiveConfigTool {
    elicitation: Arc<ElicitationContext>,
}

#[async_trait]
impl ToolHandler for InteractiveConfigTool {
    async fn handle(&self, _args: Value, _extra: RequestHandlerExtra) -> PmcpResult<Value> {
        info!("Starting interactive configuration...");

        // Elicit project configuration via JSON Schema form
        let schema = json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Project name (used as the package name)",
                    "minLength": 3,
                    "maxLength": 50,
                    "pattern": "^[a-z][a-z0-9-]*$"
                },
                "project_type": {
                    "type": "string",
                    "description": "Type of project",
                    "enum": ["library", "application", "cli"]
                },
                "version": {
                    "type": "string",
                    "description": "Semantic version (e.g., 0.1.0)",
                    "default": "0.1.0"
                },
                "include_tests": {
                    "type": "boolean",
                    "description": "Include test setup",
                    "default": true
                }
            },
            "required": ["name"]
        });

        let request = ElicitRequestParams::Form {
            message: "Configure your new project".to_string(),
            requested_schema: schema,
        };

        let response: ElicitResult = self.elicitation.elicit_input(request).await?;

        match response.action {
            ElicitAction::Accept => {
                let content = response.content.unwrap_or_default();
                let name = content
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("my-project");
                Ok(json!({
                    "status": "success",
                    "configuration": content,
                    "message": format!("Project '{}' configured successfully", name)
                }))
            },
            ElicitAction::Decline | ElicitAction::Cancel => {
                Ok(json!({"status": "cancelled", "message": "Configuration cancelled by user"}))
            },
        }
    }
}

/// Tool that demonstrates confirmation elicitation.
struct SensitiveDataTool {
    elicitation: Arc<ElicitationContext>,
}

#[async_trait]
impl ToolHandler for SensitiveDataTool {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> PmcpResult<Value> {
        let operation = args
            .get("operation")
            .and_then(|v| v.as_str())
            .unwrap_or("read");

        // Confirm sensitive operation via form elicitation
        let schema = json!({
            "type": "object",
            "properties": {
                "confirmed": {
                    "type": "boolean",
                    "description": "This operation cannot be undone"
                }
            },
            "required": ["confirmed"]
        });

        let request = ElicitRequestParams::Form {
            message: format!("Are you sure you want to {} sensitive data?", operation),
            requested_schema: schema,
        };

        let response = self.elicitation.elicit_input(request).await?;

        match response.action {
            ElicitAction::Accept => {
                let confirmed = response
                    .content
                    .as_ref()
                    .and_then(|c| c.get("confirmed"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                if !confirmed {
                    return Ok(json!({
                        "status": "aborted",
                        "message": "Operation not confirmed"
                    }));
                }

                Ok(json!({
                    "status": "success",
                    "operation": operation,
                    "message": format!("Sensitive {} operation completed", operation)
                }))
            },
            ElicitAction::Decline | ElicitAction::Cancel => Ok(json!({
                "status": "cancelled",
                "message": "Operation cancelled by user"
            })),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    // Create elicitation manager
    let elicitation_manager = Arc::new(ElicitationManager::new());
    let elicitation_ctx = Arc::new(ElicitationContext::new(elicitation_manager.clone()));

    // Create server
    let server = Server::builder()
        .name("elicit-input-example")
        .version("1.0.0")
        .capabilities(ServerCapabilities::tools_only())
        .tool(
            "configure_project",
            InteractiveConfigTool {
                elicitation: elicitation_ctx.clone(),
            },
        )
        .tool(
            "sensitive_operation",
            SensitiveDataTool {
                elicitation: elicitation_ctx.clone(),
            },
        )
        .build()?;

    info!("Starting server with input elicitation examples...");
    info!("\nAvailable tools:");
    info!("1. configure_project - Interactive project configuration");
    info!("   Uses JSON Schema form for name, type, version, tests");
    info!("\n2. sensitive_operation - Operations requiring confirmation");
    info!("   Arguments: operation (read, write, delete)");

    info!("\nElicitation features (MCP 2025-11-25):");
    info!("- Form mode: JSON Schema-based input forms");
    info!("- Accept/decline/cancel actions");
    info!("- Structured content responses");

    // Run server
    server.run_stdio().await?;

    Ok(())
}
