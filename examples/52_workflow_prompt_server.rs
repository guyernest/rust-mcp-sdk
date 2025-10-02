//! Workflow-based prompt server example
//!
//! This example demonstrates:
//! - Using workflows in a real MCP prompt server
//! - Strict mode (handles) vs loose mode (text) comparison
//! - Building workflows from prompt arguments
//! - Converting internal workflow types to protocol types
//! - Creating ExpansionContext from registered tools/resources
//!
//! The server provides two prompts:
//! - math_solver_strict: Uses ToolHandles (strict mode)
//! - math_solver_loose: Uses plain text (loose mode)
//!
//! Both solve the same problem but demonstrate different approaches.

use pmcp::server::workflow::{
    conversion::{ExpansionContext, ResourceInfo, ToolInfo},
    dsl::{constant, field, from_step, prompt_arg},
    handles::ToolHandle,
    prompt_content::{InternalPromptMessage, PromptContent},
    SequentialWorkflow, WorkflowStep,
};
use pmcp::types::{GetPromptResult, MessageContent, PromptMessage, Role, ServerCapabilities};
use pmcp::{Error, RequestHandlerExtra, Result, Server, SimpleTool, SyncPrompt};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;

// ============================================================================
// Helper: Build ExpansionContext from registry
// ============================================================================

/// Build ExpansionContext from tool/resource registries
///
/// Note: This is a temporary solution until the server exposes its internal
/// registry. In a future version, you'd get this directly from the server.
fn build_expansion_context() -> (HashMap<Arc<str>, ToolInfo>, HashMap<Arc<str>, ResourceInfo>) {
    let mut tools = HashMap::new();

    // Mirror the registered tools
    tools.insert(
        Arc::from("calculator"),
        ToolInfo {
            name: "calculator".to_string(),
            description: "Perform mathematical calculations".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "enum": ["add", "subtract", "multiply", "divide", "sqrt", "discriminant"]
                    },
                    "a": {"type": "number", "description": "First operand"},
                    "b": {"type": "number", "description": "Second operand (optional for some ops)"}
                },
                "required": ["operation", "a"]
            }),
        },
    );

    tools.insert(
        Arc::from("formatter"),
        ToolInfo {
            name: "formatter".to_string(),
            description: "Format mathematical results".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "value": {"type": "number"},
                    "format": {"type": "string", "enum": ["plain", "scientific", "latex"]}
                },
                "required": ["value"]
            }),
        },
    );

    tools.insert(
        Arc::from("solver"),
        ToolInfo {
            name: "solver".to_string(),
            description: "Solve equations step by step".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "equation_type": {"type": "string", "enum": ["linear", "quadratic"]},
                    "coefficients": {"type": "object"}
                },
                "required": ["equation_type", "coefficients"]
            }),
        },
    );

    let mut resources = HashMap::new();

    resources.insert(
        Arc::from("resource://math/formulas"),
        ResourceInfo {
            uri: "resource://math/formulas".to_string(),
            name: Some("Mathematical Formulas Guide".to_string()),
            mime_type: Some("text/markdown".to_string()),
        },
    );

    (tools, resources)
}

// ============================================================================
// Strict Mode Prompt: Uses ToolHandles
// ============================================================================

fn create_strict_mode_prompt(
) -> SyncPrompt<impl Fn(HashMap<String, String>) -> Result<GetPromptResult> + Send + Sync> {
    SyncPrompt::new("math_solver_strict", |args| {
        // Extract arguments
        let _equation_type = args
            .get("equation_type")
            .map(|s| s.as_str())
            .unwrap_or("quadratic");
        let a = args
            .get("a")
            .ok_or_else(|| Error::validation("Missing required argument 'a'"))?;
        let b = args
            .get("b")
            .ok_or_else(|| Error::validation("Missing required argument 'b'"))?;
        let c = args
            .get("c")
            .ok_or_else(|| Error::validation("Missing required argument 'c'"))?;

        // Build workflow using strict mode (handles)
        let workflow = SequentialWorkflow::new(
            "quadratic_solver_strict",
            "Solve quadratic equation using type-safe handles",
        )
        .argument("a", "Coefficient a", true)
        .argument("b", "Coefficient b", true)
        .argument("c", "Coefficient c", true)
        .instruction(InternalPromptMessage::system(
            "Solve the quadratic equation ax² + bx + c = 0 step by step",
        ))
        .step(
            WorkflowStep::new("calc_discriminant", ToolHandle::new("calculator"))
                .arg("operation", constant(json!("discriminant")))
                .arg("a", prompt_arg("a"))
                .arg("b", prompt_arg("b"))
                .arg("c", prompt_arg("c"))
                .bind("discriminant"),
        )
        .step(
            WorkflowStep::new("calc_root1", ToolHandle::new("calculator"))
                .arg("operation", constant(json!("sqrt")))
                .arg("a", field("discriminant", "value"))
                .bind("sqrt_d"),
        )
        .step(
            WorkflowStep::new("format_results", ToolHandle::new("formatter"))
                .arg("value", from_step("sqrt_d"))
                .arg("format", constant(json!("latex")))
                .bind("formatted"),
        );

        // Validate workflow
        workflow
            .validate()
            .map_err(|e| Error::validation(format!("Workflow validation failed: {}", e)))?;

        // Build ExpansionContext
        let (tools, resources) = build_expansion_context();
        let ctx = ExpansionContext {
            tools: &tools,
            resources: &resources,
        };

        // Build messages - start with workflow instructions
        let mut internal_messages = workflow.instructions().to_vec();

        // Add tool handles (strict mode) - these will expand to embedded schemas
        internal_messages.push(InternalPromptMessage::new(
            Role::System,
            PromptContent::ToolHandle(ToolHandle::new("calculator")),
        ));

        internal_messages.push(InternalPromptMessage::new(
            Role::System,
            PromptContent::ToolHandle(ToolHandle::new("formatter")),
        ));

        // Add user message with equation
        internal_messages.push(InternalPromptMessage::user(format!(
            "Solve: {}x² + {}x + {} = 0",
            a, b, c
        )));

        // Convert to protocol messages
        let protocol_messages: std::result::Result<Vec<PromptMessage>, _> = internal_messages
            .iter()
            .map(|msg| msg.to_protocol(&ctx))
            .collect();

        let messages = protocol_messages
            .map_err(|e| Error::validation(format!("Failed to convert to protocol: {}", e)))?;

        Ok(GetPromptResult {
            messages,
            description: Some(format!(
                "Strict mode: Solve {}x² + {}x + {} = 0 (with embedded tool schemas)",
                a, b, c
            )),
        })
    })
    .with_description("Solve quadratic equations using strict mode (tool handles)")
    .with_argument(
        "equation_type",
        "Type of equation (quadratic, linear)",
        false,
    )
    .with_argument("a", "Coefficient a (x² term)", true)
    .with_argument("b", "Coefficient b (x term)", true)
    .with_argument("c", "Coefficient c (constant)", true)
}

// ============================================================================
// Loose Mode Prompt: Uses plain text
// ============================================================================

fn create_loose_mode_prompt(
) -> SyncPrompt<impl Fn(HashMap<String, String>) -> Result<GetPromptResult> + Send + Sync> {
    SyncPrompt::new("math_solver_loose", |args| {
        let a = args
            .get("a")
            .ok_or_else(|| Error::validation("Missing required argument 'a'"))?;
        let b = args
            .get("b")
            .ok_or_else(|| Error::validation("Missing required argument 'b'"))?;
        let c = args
            .get("c")
            .ok_or_else(|| Error::validation("Missing required argument 'c'"))?;

        // Build workflow using loose mode (plain text/strings)
        let workflow = SequentialWorkflow::new(
            "quadratic_solver_loose",
            "Solve quadratic equation using plain text",
        )
        .argument("a", "Coefficient a", true)
        .argument("b", "Coefficient b", true)
        .argument("c", "Coefficient c", true)
        .instruction(InternalPromptMessage::system(
            "You have access to calculator and formatter tools.",
        ))
        .instruction(InternalPromptMessage::system(
            "Solve the quadratic equation step by step.",
        ));

        // Validate workflow
        workflow.validate().map_err(|e| {
            Error::validation(format!("Workflow validation failed: {}", e))
        })?;

        // Build messages using plain text (loose mode)
        let mut messages = vec![];

        // System instructions from workflow
        for instruction in workflow.instructions() {
            messages.push(PromptMessage {
                role: instruction.role,
                content: MessageContent::Text {
                    text: match &instruction.content {
                        PromptContent::Text(t) => t.clone(),
                        _ => "System instruction".to_string(),
                    },
                },
            });
        }

        // Manually describe tools (loose mode - no auto-expansion)
        messages.push(PromptMessage {
            role: Role::System,
            content: MessageContent::Text {
                text: "Available tools:\n\
                       1. calculator: Perform math operations (add, subtract, multiply, divide, sqrt, discriminant)\n\
                       2. formatter: Format results (plain, scientific, latex)"
                    .to_string(),
            },
        });

        // User message
        messages.push(PromptMessage {
            role: Role::User,
            content: MessageContent::Text {
                text: format!("Solve: {}x² + {}x + {} = 0", a, b, c),
            },
        });

        Ok(GetPromptResult {
            messages,
            description: Some(format!(
                "Loose mode: Solve {}x² + {}x + {} = 0 (manual tool descriptions)",
                a, b, c
            )),
        })
    })
    .with_description("Solve quadratic equations using loose mode (plain text)")
    .with_argument("a", "Coefficient a (x² term)", true)
    .with_argument("b", "Coefficient b (x term)", true)
    .with_argument("c", "Coefficient c (constant)", true)
}

// ============================================================================
// Tools (Simple implementations for demonstration)
// ============================================================================

#[allow(clippy::type_complexity)]
fn create_calculator_tool() -> SimpleTool<
    impl Fn(
            Value,
            RequestHandlerExtra,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Value>> + Send>>
        + Send
        + Sync,
> {
    SimpleTool::new("calculator", |args: Value, _extra: RequestHandlerExtra| {
        Box::pin(async move {
            let operation = args
                .get("operation")
                .and_then(|v| v.as_str())
                .unwrap_or("add");
            let a = args.get("a").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let b = args.get("b").and_then(|v| v.as_f64()).unwrap_or(0.0);

            let result = match operation {
                "add" => a + b,
                "subtract" => a - b,
                "multiply" => a * b,
                "divide" => {
                    if b == 0.0 {
                        return Err(Error::validation("Division by zero"));
                    }
                    a / b
                },
                "sqrt" => a.sqrt(),
                "discriminant" => {
                    let c = args.get("c").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    b * b - 4.0 * a * c
                },
                _ => return Err(Error::validation("Unknown operation")),
            };

            Ok(json!({
                "value": result,
                "operation": operation
            }))
        })
    })
    .with_description("Perform mathematical calculations")
    .with_schema(json!({
        "type": "object",
        "properties": {
            "operation": {
                "type": "string",
                "enum": ["add", "subtract", "multiply", "divide", "sqrt", "discriminant"]
            },
            "a": {"type": "number"},
            "b": {"type": "number"},
            "c": {"type": "number"}
        },
        "required": ["operation", "a"]
    }))
}

#[allow(clippy::type_complexity)]
fn create_formatter_tool() -> SimpleTool<
    impl Fn(
            Value,
            RequestHandlerExtra,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Value>> + Send>>
        + Send
        + Sync,
> {
    SimpleTool::new("formatter", |args: Value, _extra: RequestHandlerExtra| {
        Box::pin(async move {
            let value = args.get("value").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let format = args
                .get("format")
                .and_then(|v| v.as_str())
                .unwrap_or("plain");

            let formatted = match format {
                "plain" => format!("{}", value),
                "scientific" => format!("{:e}", value),
                "latex" => format!("${}$", value),
                _ => return Err(Error::validation("Unknown format")),
            };

            Ok(json!({
                "formatted": formatted,
                "original": value
            }))
        })
    })
    .with_description("Format mathematical results")
    .with_schema(json!({
        "type": "object",
        "properties": {
            "value": {"type": "number"},
            "format": {"type": "string", "enum": ["plain", "scientific", "latex"]}
        },
        "required": ["value"]
    }))
}

#[allow(clippy::type_complexity)]
fn create_solver_tool() -> SimpleTool<
    impl Fn(
            Value,
            RequestHandlerExtra,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Value>> + Send>>
        + Send
        + Sync,
> {
    SimpleTool::new("solver", |args: Value, _extra: RequestHandlerExtra| {
        Box::pin(async move {
            let equation_type = args
                .get("equation_type")
                .and_then(|v| v.as_str())
                .unwrap_or("linear");

            Ok(json!({
                "solution": format!("Solving {} equation", equation_type),
                "steps": ["Step 1", "Step 2", "Step 3"]
            }))
        })
    })
    .with_description("Solve equations step by step")
    .with_schema(json!({
        "type": "object",
        "properties": {
            "equation_type": {"type": "string", "enum": ["linear", "quadratic"]},
            "coefficients": {"type": "object"}
        },
        "required": ["equation_type"]
    }))
}

// ============================================================================
// Main
// ============================================================================

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter("pmcp=debug")
        .init();

    println!("=== Workflow-based Prompt Server ===\n");

    println!("📦 Registering tools...");
    let calculator = create_calculator_tool();
    let formatter = create_formatter_tool();
    let solver = create_solver_tool();

    println!("📝 Creating prompts...");
    let strict_prompt = create_strict_mode_prompt();
    let loose_prompt = create_loose_mode_prompt();

    println!("🔧 Building server...\n");
    let server = Server::builder()
        .name("workflow-prompt-server")
        .version("1.0.0")
        .capabilities(ServerCapabilities::default())
        .tool("calculator", calculator)
        .tool("formatter", formatter)
        .tool("solver", solver)
        .prompt("math_solver_strict", strict_prompt)
        .prompt("math_solver_loose", loose_prompt)
        .build()?;

    println!("✨ Server ready!");
    println!("\nPrompts available:");
    println!("  1. math_solver_strict - Uses ToolHandles (strict mode)");
    println!("     • Tool schemas automatically embedded in messages");
    println!("     • Type-safe handle validation");
    println!("     • Requires: a, b, c (equation coefficients)");
    println!();
    println!("  2. math_solver_loose - Uses plain text (loose mode)");
    println!("     • Manual tool descriptions");
    println!("     • No automatic schema embedding");
    println!("     • Requires: a, b, c (equation coefficients)");
    println!();
    println!("Tools available:");
    println!("  • calculator - Math operations");
    println!("  • formatter - Result formatting");
    println!("  • solver - Equation solving");
    println!();
    println!("💡 Key Differences:");
    println!("  Strict Mode:");
    println!("    - ToolHandle automatically expands to embedded schema");
    println!("    - ExpansionContext validates against registry");
    println!("    - Type-safe at build time");
    println!();
    println!("  Loose Mode:");
    println!("    - Plain text messages");
    println!("    - Manual tool descriptions");
    println!("    - Easy migration path");
    println!();
    println!("Listening on stdio...");

    server.run_stdio().await?;

    Ok(())
}
