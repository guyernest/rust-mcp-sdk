//! Scaffold tool -- returns code templates as structured JSON.
//!
//! The tool does NOT write files. It returns file paths and content
//! for the AI agent or user to apply.

use async_trait::async_trait;
use pmcp::types::ToolInfo;
use serde::Deserialize;
use serde_json::{json, Value};

/// Template variants available for scaffolding.
const TEMPLATE_VARIANTS: &[&str] = &[
    "minimal",
    "calculator",
    "with-resources",
    "with-prompts",
    "mcp-app",
];

// ---------------------------------------------------------------------------
// Embedded template content
//
// Templates use `{name}` and `{name_underscore}` as placeholders.
// Curly braces that are NOT placeholders are doubled for correct output
// when the `apply()` function performs string replacement.
// ---------------------------------------------------------------------------

const MINIMAL_CARGO_TOML: &str = r#"[workspace]
resolver = "2"
members = ["crates/mcp-{name}-core", "{name}-server"]

[workspace.dependencies]
pmcp = { version = "1", features = ["streamable-http"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
async-trait = "0.1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
"#;

const MINIMAL_CORE_LIB: &str = r#"use async_trait::async_trait;
use pmcp::{ToolHandler, RequestHandlerExtra, Error};
use serde_json::{json, Value};

pub struct EchoTool;

#[async_trait]
impl ToolHandler for EchoTool {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> Result<Value, Error> {
        let message = args.get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("Hello from {name}!");
        Ok(json!({"echo": message}))
    }

    fn metadata(&self) -> Option<pmcp::types::ToolInfo> {
        Some(pmcp::types::ToolInfo::new(
            "echo",
            Some("Echo back a message".into()),
            json!({
                "type": "object",
                "properties": {
                    "message": { "type": "string", "description": "Message to echo" }
                }
            }),
        ))
    }
}
"#;

const MINIMAL_SERVER_MAIN: &str = r#"use pmcp::Server;
use pmcp::shared::streamable_http::StreamableHttpServer;
use mcp_{name_underscore}_core::EchoTool;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();

    let server = Server::builder()
        .name("{name}")
        .version("0.1.0")
        .tool("echo", EchoTool)
        .build()?;

    StreamableHttpServer::new(server)
        .bind("0.0.0.0:8080")
        .start()
        .await?;
    Ok(())
}
"#;

const CALCULATOR_CORE_LIB: &str = r#"use async_trait::async_trait;
use pmcp::{ToolHandler, RequestHandlerExtra, Error};
use serde_json::{json, Value};

pub struct AddTool;
pub struct SubtractTool;
pub struct MultiplyTool;
pub struct DivideTool;

macro_rules! math_tool {
    ($name:ident, $op:tt, $tool_name:expr, $desc:expr) => {
        #[async_trait]
        impl ToolHandler for $name {
            async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> Result<Value, Error> {
                let a = args.get("a").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let b = args.get("b").and_then(|v| v.as_f64()).unwrap_or(0.0);
                Ok(json!({"result": a $op b}))
            }
            fn metadata(&self) -> Option<pmcp::types::ToolInfo> {
                Some(pmcp::types::ToolInfo::new($tool_name, Some($desc.into()), json!({
                    "type": "object",
                    "properties": {
                        "a": { "type": "number" },
                        "b": { "type": "number" }
                    },
                    "required": ["a", "b"]
                })))
            }
        }
    };
}

math_tool!(AddTool, +, "add", "Add two numbers");
math_tool!(SubtractTool, -, "subtract", "Subtract b from a");
math_tool!(MultiplyTool, *, "multiply", "Multiply two numbers");
math_tool!(DivideTool, /, "divide", "Divide a by b");
"#;

const CALCULATOR_SERVER_MAIN: &str = r#"use pmcp::Server;
use pmcp::shared::streamable_http::StreamableHttpServer;
use mcp_{name_underscore}_core::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();

    let server = Server::builder()
        .name("{name}")
        .version("0.1.0")
        .tool("add", AddTool)
        .tool("subtract", SubtractTool)
        .tool("multiply", MultiplyTool)
        .tool("divide", DivideTool)
        .build()?;

    StreamableHttpServer::new(server)
        .bind("0.0.0.0:8080")
        .start()
        .await?;
    Ok(())
}
"#;

const WITH_RESOURCES_CORE_LIB: &str = r#"use async_trait::async_trait;
use pmcp::{ToolHandler, RequestHandlerExtra, Error, StaticResource};
use serde_json::{json, Value};

pub struct GreetTool;

#[async_trait]
impl ToolHandler for GreetTool {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> Result<Value, Error> {
        let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("World");
        Ok(json!({"greeting": format!("Hello, {}!", name)}))
    }

    fn metadata(&self) -> Option<pmcp::types::ToolInfo> {
        Some(pmcp::types::ToolInfo::new("greet", Some("Greet someone".into()), json!({
            "type": "object",
            "properties": { "name": { "type": "string" } }
        })))
    }
}

pub fn readme_resource() -> StaticResource {
    StaticResource::text("resource://readme", "README", "text/plain",
        "Welcome to {name}! This server demonstrates tools and resources.")
}
"#;

const WITH_RESOURCES_SERVER_MAIN: &str = r#"use pmcp::Server;
use pmcp::shared::streamable_http::StreamableHttpServer;
use mcp_{name_underscore}_core::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();

    let server = Server::builder()
        .name("{name}")
        .version("0.1.0")
        .tool("greet", GreetTool)
        .resource(readme_resource())
        .build()?;

    StreamableHttpServer::new(server)
        .bind("0.0.0.0:8080")
        .start()
        .await?;
    Ok(())
}
"#;

const WITH_PROMPTS_CORE_LIB: &str = r#"use async_trait::async_trait;
use pmcp::{ToolHandler, RequestHandlerExtra, Error};
use pmcp::server::workflow::{SequentialWorkflow, WorkflowStep, ToolHandle};
use serde_json::{json, Value};

pub struct AnalyzeTool;

#[async_trait]
impl ToolHandler for AnalyzeTool {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> Result<Value, Error> {
        let topic = args.get("topic").and_then(|v| v.as_str()).unwrap_or("general");
        Ok(json!({"analysis": format!("Analysis of: {}", topic)}))
    }

    fn metadata(&self) -> Option<pmcp::types::ToolInfo> {
        Some(pmcp::types::ToolInfo::new("analyze", Some("Analyze a topic".into()), json!({
            "type": "object",
            "properties": { "topic": { "type": "string" } }
        })))
    }
}

pub fn research_workflow() -> SequentialWorkflow {
    SequentialWorkflow::new("research", "Research and analyze a topic")
        .step(WorkflowStep::new("analyze", ToolHandle::new("analyze")))
}
"#;

const WITH_PROMPTS_SERVER_MAIN: &str = r#"use pmcp::Server;
use pmcp::shared::streamable_http::StreamableHttpServer;
use mcp_{name_underscore}_core::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();

    let server = Server::builder()
        .name("{name}")
        .version("0.1.0")
        .tool("analyze", AnalyzeTool)
        .prompt_workflow(research_workflow())?
        .build()?;

    StreamableHttpServer::new(server)
        .bind("0.0.0.0:8080")
        .start()
        .await?;
    Ok(())
}
"#;

const MCP_APP_CORE_LIB: &str = r##"use async_trait::async_trait;
use pmcp::{ToolHandler, RequestHandlerExtra, Error};
use pmcp::types::ToolInfo;
use pmcp::types::ui::{UIResource, UIMimeType};
use serde_json::{json, Value};

const WIDGET_HTML: &str = r#"<!DOCTYPE html>
<html><body>
<h2>Result</h2>
<pre id="out"></pre>
<script type="module">
import { App } from 'https://cdn.jsdelivr.net/npm/@anthropic-ai/sdk/ext-apps';
const app = new App();
app.onToolResult((result) => {
  document.getElementById('out').textContent = JSON.stringify(result, null, 2);
});
</script>
</body></html>"#;

pub struct DashboardTool;

#[async_trait]
impl ToolHandler for DashboardTool {
    async fn handle(&self, args: Value, _extra: RequestHandlerExtra) -> Result<Value, Error> {
        let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("default");
        Ok(json!({"query": query, "rows": 42}))
    }

    fn metadata(&self) -> Option<ToolInfo> {
        let ui = UIResource::new("widget://dashboard", "Dashboard", UIMimeType::HtmlMcpApp);
        Some(ToolInfo::new("dashboard", Some("Query dashboard".into()), json!({
            "type": "object",
            "properties": { "query": { "type": "string" } }
        })).with_ui(&ui, WIDGET_HTML))
    }
}
"##;

const MCP_APP_SERVER_MAIN: &str = r#"use pmcp::Server;
use pmcp::shared::streamable_http::StreamableHttpServer;
use mcp_{name_underscore}_core::DashboardTool;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();

    let server = Server::builder()
        .name("{name}")
        .version("0.1.0")
        .tool("dashboard", DashboardTool)
        .build()?;

    StreamableHttpServer::new(server)
        .bind("0.0.0.0:8080")
        .start()
        .await?;
    Ok(())
}
"#;

// ---------------------------------------------------------------------------
// Core Cargo.toml template (shared across all variants)
// ---------------------------------------------------------------------------

const CORE_CARGO_TOML: &str = r#"[package]
name = "mcp-{name}-core"
version.workspace = true
edition.workspace = true

[dependencies]
pmcp.workspace = true
serde.workspace = true
serde_json.workspace = true
async-trait.workspace = true
"#;

const SERVER_CARGO_TOML: &str = r#"[package]
name = "{name}-server"
version.workspace = true
edition.workspace = true

[dependencies]
mcp-{name}-core = { path = "../crates/mcp-{name}-core" }
pmcp.workspace = true
tokio.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
anyhow = "1"
"#;

// ---------------------------------------------------------------------------
// Input deserialization
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct ScaffoldInput {
    template: String,
    name: String,
}

// ---------------------------------------------------------------------------
// ScaffoldTool
// ---------------------------------------------------------------------------

/// Code template generation tool.
///
/// Returns structured JSON with file paths and content for the AI agent
/// or user to apply. Does NOT write files to the filesystem.
pub struct ScaffoldTool;

impl ScaffoldTool {
    /// Replace `{name}` and `{name_underscore}` placeholders in template text.
    fn apply(template: &str, name: &str, name_underscore: &str) -> String {
        template
            .replace("{name}", name)
            .replace("{name_underscore}", name_underscore)
    }
}

#[async_trait]
impl pmcp::server::ToolHandler for ScaffoldTool {
    async fn handle(
        &self,
        args: Value,
        _extra: pmcp::RequestHandlerExtra,
    ) -> Result<Value, pmcp::Error> {
        let input: ScaffoldInput = serde_json::from_value(args)
            .map_err(|e| pmcp::Error::Internal(format!("Invalid arguments: {e}")))?;

        if !TEMPLATE_VARIANTS.contains(&input.template.as_str()) {
            return Err(pmcp::Error::Internal(format!(
                "Unknown template '{}'. Available: {}",
                input.template,
                TEMPLATE_VARIANTS.join(", ")
            )));
        }

        let name = &input.name;
        let name_underscore = name.replace('-', "_");

        let (instructions, core_lib, server_main) = match input.template.as_str() {
            "minimal" => (
                "Create a minimal MCP server workspace with a single echo tool.",
                MINIMAL_CORE_LIB,
                MINIMAL_SERVER_MAIN,
            ),
            "calculator" => (
                "Create an MCP server with add/subtract/multiply/divide tools.",
                CALCULATOR_CORE_LIB,
                CALCULATOR_SERVER_MAIN,
            ),
            "with-resources" => (
                "Create an MCP server with a tool and a static resource.",
                WITH_RESOURCES_CORE_LIB,
                WITH_RESOURCES_SERVER_MAIN,
            ),
            "with-prompts" => (
                "Create an MCP server with a tool and a workflow prompt template.",
                WITH_PROMPTS_CORE_LIB,
                WITH_PROMPTS_SERVER_MAIN,
            ),
            "mcp-app" => (
                "Create an MCP server with an MCP Apps widget for rich UI output.",
                MCP_APP_CORE_LIB,
                MCP_APP_SERVER_MAIN,
            ),
            _ => unreachable!(),
        };

        let cargo_toml = Self::apply(MINIMAL_CARGO_TOML, name, &name_underscore);
        let core_cargo = Self::apply(CORE_CARGO_TOML, name, &name_underscore);
        let server_cargo = Self::apply(SERVER_CARGO_TOML, name, &name_underscore);
        let core_content = Self::apply(core_lib, name, &name_underscore);
        let server_content = Self::apply(server_main, name, &name_underscore);

        let core_toml_path = format!("crates/mcp-{name}-core/Cargo.toml");
        let core_lib_path = format!("crates/mcp-{name}-core/src/lib.rs");
        let server_toml_path = format!("{name}-server/Cargo.toml");
        let server_main_path = format!("{name}-server/src/main.rs");

        let files = json!([
            {
                "path": "Cargo.toml",
                "content": cargo_toml,
                "description": "Workspace manifest"
            },
            {
                "path": core_toml_path,
                "content": core_cargo,
                "description": "Core library manifest"
            },
            {
                "path": core_lib_path,
                "content": core_content,
                "description": "Core library with tool implementations"
            },
            {
                "path": server_toml_path,
                "content": server_cargo,
                "description": "Server binary manifest"
            },
            {
                "path": server_main_path,
                "content": server_content,
                "description": "Server binary entry point"
            }
        ]);

        let next_step_run = format!("Run `cargo run -p {name}-server` to start the server");

        Ok(json!({
            "template": input.template,
            "name": name,
            "instructions": instructions,
            "files": files,
            "next_steps": [
                "Run `cargo build` to verify compilation",
                next_step_run,
                "Run `cargo pmcp test check http://localhost:8080` to validate"
            ]
        }))
    }

    fn metadata(&self) -> Option<ToolInfo> {
        Some(ToolInfo::new(
            "scaffold",
            Some(
                "Generate PMCP project templates. Returns structured JSON with file paths \
                 and content -- does NOT write files."
                    .into(),
            ),
            json!({
                "type": "object",
                "properties": {
                    "template": {
                        "type": "string",
                        "enum": TEMPLATE_VARIANTS,
                        "description": "Template type: minimal, calculator, with-resources, with-prompts, or mcp-app"
                    },
                    "name": {
                        "type": "string",
                        "description": "Project name (used in Cargo.toml and module names)"
                    }
                },
                "required": ["template", "name"]
            }),
        ))
    }
}
