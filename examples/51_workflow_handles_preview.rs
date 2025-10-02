//! Workflow handles and protocol conversion example
//!
//! This example demonstrates:
//! - Creating ToolHandle and ResourceHandle (strict mode)
//! - Building InternalPromptMessages with handle content
//! - Creating ExpansionContext with stub registry
//! - Converting to protocol types via to_protocol()
//! - How handles are expanded to embed tool schemas
//!
//! Strict mode provides type safety and automatic schema embedding for LLMs.

use pmcp::server::workflow::{
    conversion::{ExpansionContext, ResourceInfo, ToolInfo},
    handles::{ResourceHandle, ToolHandle},
    prompt_content::{InternalPromptMessage, PromptContent},
};
use pmcp::types::{MessageContent, Role};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;

fn main() {
    println!("=== Workflow Handles & Protocol Conversion Demo ===\n");

    // Step 1: Create tool and resource handles (strict mode)
    println!("📦 Step 1: Creating Handles (Strict Mode)\n");

    let calculator_tool = ToolHandle::new("calculator");
    let formatter_tool = ToolHandle::new("formatter");
    let guide_resource = ResourceHandle::new("resource://examples/style-guide").unwrap();
    let docs_resource = ResourceHandle::new("file:///docs/api-reference.md").unwrap();

    println!("  ✓ Created ToolHandle: {}", calculator_tool);
    println!("  ✓ Created ToolHandle: {}", formatter_tool);
    println!("  ✓ Created ResourceHandle: {}", guide_resource);
    println!("  ✓ Created ResourceHandle: {}", docs_resource);

    // Step 2: Build InternalPromptMessages with different content types
    println!("\n📝 Step 2: Building InternalPromptMessages\n");

    let messages = vec![
        // Plain text message (loose mode)
        InternalPromptMessage::system("You are a helpful assistant for solving math problems."),
        // Message with tool handle (strict mode - will expand to schema)
        InternalPromptMessage::new(
            Role::System,
            PromptContent::ToolHandle(calculator_tool.clone()),
        ),
        // Message with resource handle (strict mode - will expand to reference)
        InternalPromptMessage::new(
            Role::User,
            PromptContent::ResourceHandle(guide_resource.clone()),
        ),
        // Multi-part message combining text and handles
        InternalPromptMessage::new(
            Role::User,
            PromptContent::Multi(smallvec::smallvec![
                Box::new(PromptContent::Text(
                    "Please solve this equation using available tools:".to_string()
                )),
                Box::new(PromptContent::ToolHandle(formatter_tool.clone())),
                Box::new(PromptContent::Text("Format the result nicely.".to_string())),
            ]),
        ),
        // Image content
        InternalPromptMessage::new(
            Role::User,
            PromptContent::Image {
                data: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==".to_string(),
                mime_type: "image/png".to_string(),
            },
        ),
    ];

    println!("  ✓ Created {} messages:", messages.len());
    for (i, msg) in messages.iter().enumerate() {
        println!(
            "    {}. {:?} with {:?}",
            i + 1,
            msg.role,
            discriminant(&msg.content)
        );
    }

    // Step 3: Create ExpansionContext (stub registry)
    println!("\n🗂️  Step 3: Creating ExpansionContext (Stub Registry)\n");

    let mut tools_registry = HashMap::new();
    tools_registry.insert(
        Arc::from("calculator"),
        ToolInfo {
            name: "calculator".to_string(),
            description: "Perform mathematical calculations".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "enum": ["add", "subtract", "multiply", "divide", "sqrt"]
                    },
                    "a": {"type": "number"},
                    "b": {"type": "number"}
                },
                "required": ["operation", "a"]
            }),
        },
    );

    tools_registry.insert(
        Arc::from("formatter"),
        ToolInfo {
            name: "formatter".to_string(),
            description: "Format text with specified style".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string"},
                    "style": {
                        "type": "string",
                        "enum": ["plain", "markdown", "latex"]
                    }
                },
                "required": ["text"]
            }),
        },
    );

    let mut resources_registry = HashMap::new();
    resources_registry.insert(
        Arc::from("resource://examples/style-guide"),
        ResourceInfo {
            uri: "resource://examples/style-guide".to_string(),
            name: Some("Math Formatting Style Guide".to_string()),
            mime_type: Some("text/markdown".to_string()),
        },
    );

    resources_registry.insert(
        Arc::from("file:///docs/api-reference.md"),
        ResourceInfo {
            uri: "file:///docs/api-reference.md".to_string(),
            name: Some("API Reference".to_string()),
            mime_type: Some("text/markdown".to_string()),
        },
    );

    println!("  ✓ Registered {} tools:", tools_registry.len());
    for name in tools_registry.keys() {
        println!("    - {}", name);
    }

    println!("  ✓ Registered {} resources:", resources_registry.len());
    for uri in resources_registry.keys() {
        println!("    - {}", uri);
    }

    let ctx = ExpansionContext {
        tools: &tools_registry,
        resources: &resources_registry,
    };

    // Step 4: Convert to protocol types
    println!("\n🔄 Step 4: Converting to Protocol Types\n");

    for (i, internal_msg) in messages.iter().enumerate() {
        println!("  Message {}:", i + 1);
        println!("    Role: {:?}", internal_msg.role);
        println!(
            "    Internal Content: {:?}",
            discriminant(&internal_msg.content)
        );

        match internal_msg.to_protocol(&ctx) {
            Ok(protocol_msg) => {
                println!("    ✓ Converted successfully");
                println!("    Protocol Role: {:?}", protocol_msg.role);

                match &protocol_msg.content {
                    MessageContent::Text { text } => {
                        println!("    Protocol Content: Text");
                        if text.len() > 100 {
                            println!("      Preview: {}...", &text[..100]);
                        } else {
                            println!("      Text: {}", text);
                        }
                    },
                    MessageContent::Image { mime_type, .. } => {
                        println!("    Protocol Content: Image ({})", mime_type);
                    },
                    MessageContent::Resource { uri, .. } => {
                        println!("    Protocol Content: Resource ({})", uri);
                    },
                }
                println!();
            },
            Err(e) => {
                println!("    ✗ Conversion failed: {}", e);
                println!();
            },
        }
    }

    // Step 5: Demonstrate handle expansion in detail
    println!("🔍 Step 5: Detailed Handle Expansion\n");

    let tool_msg = InternalPromptMessage::new(
        Role::System,
        PromptContent::ToolHandle(calculator_tool.clone()),
    );

    println!("  Original: ToolHandle(\"calculator\")");
    if let Ok(protocol) = tool_msg.to_protocol(&ctx) {
        if let MessageContent::Text { text } = protocol.content {
            println!("\n  Expanded to embedded schema:\n");
            println!("  {}", indent(&text, "  "));
        }
    }

    println!("\n📊 Summary:\n");
    println!("  Loose Mode (Strings):");
    println!("    - Text: Direct passthrough");
    println!("    - ResourceUri: Becomes MessageContent::Resource");
    println!("    - Easy migration, no type safety");
    println!();
    println!("  Strict Mode (Handles):");
    println!("    - ToolHandle: Expands to embedded schema text (LLM can read)");
    println!("    - ResourceHandle: Validates registry, becomes Resource reference");
    println!("    - Type-safe, catches missing tools/resources at build time");
    println!();
    println!("  ExpansionContext:");
    println!("    - Provides tool schemas for handle expansion");
    println!("    - Validates handles against registered tools/resources");
    println!("    - Acts as the bridge between internal and protocol types");

    println!("\n✨ Key Takeaways:");
    println!("  1. Handles are lightweight Arc<str> wrappers");
    println!("  2. ToolHandle embeds schemas so LLMs know what tools do");
    println!("  3. ResourceHandle validates URIs (resource:// or file://)");
    println!("  4. ExpansionContext acts as the registry for lookups");
    println!("  5. to_protocol() converts internal → protocol at the edge");
    println!("  6. Multi content parts are concatenated with newlines");

    println!("\n📖 Next Steps:");
    println!("  - See examples/52_workflow_server_integration.rs for full server setup");
    println!("  - See examples/53_workflow_execution.rs for runtime execution");
}

/// Get the discriminant name for content type
fn discriminant(content: &PromptContent) -> &'static str {
    match content {
        PromptContent::Text(_) => "Text",
        PromptContent::Image { .. } => "Image",
        PromptContent::ResourceUri(_) => "ResourceUri",
        PromptContent::ToolHandle(_) => "ToolHandle",
        PromptContent::ResourceHandle(_) => "ResourceHandle",
        PromptContent::Multi(_) => "Multi",
        _ => "Unknown",
    }
}

/// Indent each line of text
fn indent(text: &str, prefix: &str) -> String {
    text.lines()
        .map(|line| format!("{}{}", prefix, line))
        .collect::<Vec<_>>()
        .join("\n")
}
