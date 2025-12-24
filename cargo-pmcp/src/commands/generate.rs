//! Code generation commands
//!
//! - `foundation`: Generate typed client for a foundation MCP server
//!
//! Requires the `codegen` feature to be enabled.

#![cfg(feature = "codegen")]

use anyhow::{Context, Result};
use clap::Subcommand;
use console::style;
use pmcp_composition::codegen::{generate_client, ServerSchema};
use std::path::Path;

#[derive(Subcommand)]
pub enum GenerateCommand {
    /// Generate typed client for a foundation MCP server
    Foundation {
        /// Schema file (JSON)
        schema: String,

        /// Output directory (default: src/foundations/)
        #[arg(short, long, default_value = "src/foundations")]
        output: String,

        /// Module name override (default: derived from server_id)
        #[arg(short, long)]
        module: Option<String>,
    },
}

impl GenerateCommand {
    pub fn execute(self) -> Result<()> {
        match self {
            GenerateCommand::Foundation {
                schema,
                output,
                module,
            } => foundation(&schema, &output, module),
        }
    }
}

/// Generate typed client for a foundation MCP server
fn foundation(schema_path: &str, output_dir: &str, module_override: Option<String>) -> Result<()> {
    println!(
        "{} Generating foundation client from {}",
        style("->").cyan().bold(),
        style(schema_path).yellow()
    );

    // Read schema file
    let content = std::fs::read_to_string(schema_path)
        .with_context(|| format!("Failed to read schema file: {}", schema_path))?;

    // Parse schema
    let schema: ServerSchema =
        serde_json::from_str(&content).with_context(|| "Failed to parse schema JSON")?;

    // Determine module name
    let module_name = module_override.unwrap_or_else(|| to_snake_case(&schema.server_id));

    println!(
        "  {} Server: {} ({})",
        style("*").dim(),
        style(&schema.name).bold(),
        &schema.server_id
    );
    println!(
        "  {} Tools: {}, Resources: {}, Prompts: {}",
        style("*").dim(),
        schema.tools.len(),
        schema.resources.len(),
        schema.prompts.len()
    );

    // Generate client code
    println!("  {} Generating typed client...", style("*").dim());
    let code =
        generate_client(&schema).map_err(|e| anyhow::anyhow!("Code generation failed: {}", e))?;

    // Create output directory
    std::fs::create_dir_all(output_dir)
        .with_context(|| format!("Failed to create directory: {}", output_dir))?;

    // Write client file
    let client_path = Path::new(output_dir).join(format!("{}.rs", module_name));
    std::fs::write(&client_path, &code)
        .with_context(|| format!("Failed to write client to {}", client_path.display()))?;

    // Update or create mod.rs
    let mod_path = Path::new(output_dir).join("mod.rs");
    update_mod_file(&mod_path, &module_name)?;

    println!();
    println!(
        "{} Generated client: {}",
        style("OK").green().bold(),
        style(client_path.display()).cyan()
    );

    // Show generated API
    println!();
    println!("Generated API:");
    let client_name = to_pascal_case(&schema.server_id) + "Client";
    println!(
        "  {} {}::{}",
        style("struct").magenta(),
        style(&module_name).cyan(),
        style(&client_name).green()
    );

    for tool in &schema.tools {
        let method_name = to_snake_case(&tool.name);
        println!(
            "    {} .{}()",
            style("fn").magenta(),
            style(method_name).yellow()
        );
    }

    for resource in &schema.resources {
        let method_name = format!("read_{}", to_snake_case(&resource.name));
        println!(
            "    {} .{}()",
            style("fn").magenta(),
            style(method_name).yellow()
        );
    }

    for prompt in &schema.prompts {
        let method_name = format!("prompt_{}", to_snake_case(&prompt.name));
        println!(
            "    {} .{}()",
            style("fn").magenta(),
            style(method_name).yellow()
        );
    }

    println!();
    println!("Usage:");
    println!(
        "  {}",
        style(format!(
            "use crate::foundations::{}::{}Client;",
            module_name,
            to_pascal_case(&schema.server_id)
        ))
        .yellow()
    );
    println!();
    println!(
        "  {}",
        style(format!(
            "let {} = {}Client::new(&composition);",
            module_name.chars().next().unwrap_or('c'),
            to_pascal_case(&schema.server_id)
        ))
        .yellow()
    );
    println!(
        "  {}",
        style(format!(
            "let result = {}.{}(...).await?;",
            module_name.chars().next().unwrap_or('c'),
            schema
                .tools
                .first()
                .map(|t| to_snake_case(&t.name))
                .unwrap_or_else(|| "method".to_string())
        ))
        .yellow()
    );

    Ok(())
}

/// Update mod.rs to include the new module
fn update_mod_file(mod_path: &Path, module_name: &str) -> Result<()> {
    let mod_line = format!("pub mod {};", module_name);

    if mod_path.exists() {
        // Read existing content
        let content = std::fs::read_to_string(mod_path)
            .with_context(|| format!("Failed to read {}", mod_path.display()))?;

        // Check if module already declared
        if content.contains(&mod_line) {
            return Ok(());
        }

        // Append module declaration
        let new_content = format!("{}\n{}", content.trim(), mod_line);
        std::fs::write(mod_path, new_content)
            .with_context(|| format!("Failed to update {}", mod_path.display()))?;
    } else {
        // Create new mod.rs
        let content = format!("//! Generated foundation server clients\n\n{}\n", mod_line);
        std::fs::write(mod_path, content)
            .with_context(|| format!("Failed to create {}", mod_path.display()))?;
    }

    Ok(())
}

fn to_pascal_case(s: &str) -> String {
    s.split(|c: char| c == '_' || c == '-' || c == ' ')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().chain(chars).collect(),
            }
        })
        .collect()
}

fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            result.push('_');
        }
        result.push(c.to_lowercase().next().unwrap_or(c));
    }
    // Replace dashes and spaces with underscores, then collapse multiple underscores
    let normalized = result.replace('-', "_").replace(' ', "_");
    let mut collapsed = String::new();
    let mut prev_underscore = false;
    for c in normalized.chars() {
        if c == '_' {
            if !prev_underscore {
                collapsed.push(c);
            }
            prev_underscore = true;
        } else {
            collapsed.push(c);
            prev_underscore = false;
        }
    }
    collapsed
}
