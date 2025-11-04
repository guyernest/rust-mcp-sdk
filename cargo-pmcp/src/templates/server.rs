//! Server template generator
//!
//! Generates server crates based on templates.

use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::Path;

/// Generate server crates (mcp-{name}-core and {name}-server)
pub fn generate(name: &str, template: &str) -> Result<()> {
    match template {
        "calculator" => generate_calculator(name),
        "minimal" => generate_minimal(name),
        "complete" | "complete-calculator" => generate_complete(name),
        "sqlite-explorer" => generate_sqlite_explorer(name),
        _ => anyhow::bail!(
            "Unknown template: {}. Available templates: calculator, minimal, complete-calculator, sqlite-explorer",
            template
        ),
    }
}

fn generate_calculator(name: &str) -> Result<()> {
    let crates_dir = Path::new("crates");
    let core_name = format!("mcp-{}-core", name);
    let server_name = format!("{}-server", name);

    // Create core crate with calculator template
    generate_core_crate_calculator(&crates_dir.join(&core_name), name)?;
    println!(
        "  {} Created {} (calculator template)",
        "✓".green(),
        core_name.bright_yellow()
    );

    // Create server binary crate
    generate_server_crate(&crates_dir.join(&server_name), name)?;
    println!("  {} Created {}", "✓".green(), server_name.bright_yellow());

    // Create scenarios
    generate_scenarios(name)?;
    println!("  {} Created test scenarios", "✓".green());

    // Update workspace Cargo.toml
    update_workspace_members(&core_name, &server_name)?;
    println!("  {} Updated workspace members", "✓".green());

    Ok(())
}

fn generate_minimal(name: &str) -> Result<()> {
    let crates_dir = Path::new("crates");
    let core_name = format!("mcp-{}-core", name);
    let server_name = format!("{}-server", name);

    // Create core crate
    generate_core_crate(&crates_dir.join(&core_name), name)?;
    println!("  {} Created {}", "✓".green(), core_name.bright_yellow());

    // Create server binary crate
    generate_server_crate(&crates_dir.join(&server_name), name)?;
    println!("  {} Created {}", "✓".green(), server_name.bright_yellow());

    // Create scenarios
    generate_scenarios(name)?;
    println!("  {} Created test scenarios", "✓".green());

    // Update workspace Cargo.toml
    update_workspace_members(&core_name, &server_name)?;
    println!("  {} Updated workspace members", "✓".green());

    Ok(())
}

fn generate_complete(name: &str) -> Result<()> {
    let crates_dir = Path::new("crates");
    let core_name = format!("mcp-{}-core", name);
    let server_name = format!("{}-server", name);

    // Create core crate with complete template
    generate_core_crate_complete(&crates_dir.join(&core_name), name)?;
    println!(
        "  {} Created {} (complete template)",
        "✓".green(),
        core_name.bright_yellow()
    );

    // Create server binary crate (same as minimal)
    generate_server_crate(&crates_dir.join(&server_name), name)?;
    println!("  {} Created {}", "✓".green(), server_name.bright_yellow());

    // Create scenarios
    generate_scenarios(name)?;
    println!("  {} Created test scenarios", "✓".green());

    // Update workspace Cargo.toml
    update_workspace_members(&core_name, &server_name)?;
    println!("  {} Updated workspace members", "✓".green());

    Ok(())
}

fn generate_sqlite_explorer(name: &str) -> Result<()> {
    let crates_dir = Path::new("crates");
    let core_name = format!("mcp-{}-core", name);
    let server_name = format!("{}-server", name);

    // Create core crate with sqlite-explorer template
    generate_core_crate_sqlite(&crates_dir.join(&core_name), name)?;
    println!(
        "  {} Created {} (sqlite-explorer template)",
        "✓".green(),
        core_name.bright_yellow()
    );

    // Create server binary crate (same as minimal)
    generate_server_crate(&crates_dir.join(&server_name), name)?;
    println!("  {} Created {}", "✓".green(), server_name.bright_yellow());

    // Create scenarios
    generate_scenarios(name)?;
    println!("  {} Created test scenarios", "✓".green());

    // Update workspace Cargo.toml
    update_workspace_members(&core_name, &server_name)?;
    println!("  {} Updated workspace members", "✓".green());

    // Create chinook.db placeholder or instructions
    create_chinook_placeholder()?;
    println!("  {} Created database placeholder", "✓".green());

    Ok(())
}

fn generate_core_crate(core_dir: &Path, name: &str) -> Result<()> {
    fs::create_dir_all(core_dir).context("Failed to create core directory")?;
    fs::create_dir_all(core_dir.join("src")).context("Failed to create core src directory")?;

    // Generate Cargo.toml
    let cargo_toml = format!(
        r#"[package]
name = "mcp-{}-core"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true

[dependencies]
pmcp = {{ workspace = true }}
serde = {{ workspace = true }}
serde_json = {{ workspace = true }}
schemars = {{ workspace = true }}
anyhow = {{ workspace = true }}
thiserror = {{ workspace = true }}
validator = {{ workspace = true }}

[dev-dependencies]
tokio = {{ workspace = true }}
"#,
        name
    );

    fs::write(core_dir.join("Cargo.toml"), cargo_toml)
        .context("Failed to create core Cargo.toml")?;

    // Generate lib.rs
    let lib_rs = generate_core_lib_rs(name);
    fs::write(core_dir.join("src/lib.rs"), lib_rs).context("Failed to create core lib.rs")?;

    // Generate types.rs
    let types_rs = generate_types_rs(name);
    fs::write(core_dir.join("src/types.rs"), types_rs).context("Failed to create types.rs")?;

    // Generate resources
    generate_resources(core_dir, name)?;

    Ok(())
}

fn generate_core_lib_rs(name: &str) -> String {
    let capitalized = capitalize(name);
    format!(
        r#"//! {} MCP Server Core
//!
//! This crate contains the business logic for the {} server.

mod types;

use pmcp::{{Server, TypedTool}};
use pmcp::types::capabilities::ServerCapabilities;
use serde_json::json;
use validator::Validate;
use types::*;

/// Build the {} server
pub fn build_{}_server() -> pmcp::Result<Server> {{
    Server::builder()
        .name("{}")
        .version("1.0.0")
        .capabilities(ServerCapabilities::tools_only())
        .tool(
            "add",
            TypedTool::new("add", |input: AddInput, _extra| {{
                Box::pin(async move {{
                    // Validate using validator crate
                    input.validate()
                        .map_err(|e| pmcp::Error::validation(format!("Validation failed: {{}}", e)))?;

                    // Perform calculation
                    let result = input.a + input.b;

                    Ok(json!({{
                        "result": result,
                        "operation": format!("{{}} + {{}} = {{}}", input.a, input.b, result)
                    }}))
                }})
            }})
            .with_description("Add two numbers together with range validation"),
        )
        .build()
}}

#[cfg(test)]
mod tests {{
    use super::*;

    #[tokio::test]
    async fn test_server_builds() {{
        let server = build_{}_server();
        assert!(server.is_ok());
    }}

    #[tokio::test]
    async fn test_add_validation() {{
        let input = AddInput {{ a: 5.0, b: 3.0 }};
        assert!(input.validate().is_ok());

        // Test out of range
        let input = AddInput {{ a: 2000000.0, b: 3.0 }};
        assert!(input.validate().is_err());
    }}

    #[tokio::test]
    async fn test_add_logic() {{
        let input = AddInput {{ a: 5.0, b: 3.0 }};
        assert_eq!(input.a + input.b, 8.0);

        let input = AddInput {{ a: -5.0, b: 3.0 }};
        assert_eq!(input.a + input.b, -2.0);
    }}
}}
"#,
        capitalized, name, capitalized, name, name, name
    )
}

fn generate_types_rs(_name: &str) -> String {
    r#"//! Type definitions with validation

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use validator::Validate;

/// Input parameters for the add operation
///
/// This demonstrates type safety with automatic schema generation and validation:
/// - `schemars::JsonSchema` automatically generates detailed JSON schema for MCP clients
/// - `validator::Validate` provides runtime validation with custom constraints
/// - `serde` handles JSON serialization/deserialization
/// - `deny_unknown_fields` rejects any extra fields for security
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[schemars(deny_unknown_fields)]
pub struct AddInput {
    /// First number to add (must be between -1,000,000 and 1,000,000)
    #[validate(range(min = -1000000.0, max = 1000000.0))]
    #[schemars(description = "First number in the addition operation")]
    pub a: f64,

    /// Second number to add (must be between -1,000,000 and 1,000,000)
    #[validate(range(min = -1000000.0, max = 1000000.0))]
    #[schemars(description = "Second number in the addition operation")]
    pub b: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_input() {
        let input = AddInput { a: 5.0, b: 3.0 };
        assert!(input.validate().is_ok());
    }

    #[test]
    fn test_out_of_range() {
        let input = AddInput {
            a: 2000000.0,
            b: 3.0,
        };
        assert!(input.validate().is_err());
    }

    #[test]
    fn test_negative_numbers() {
        let input = AddInput { a: -5.0, b: -3.0 };
        assert!(input.validate().is_ok());
    }
}
"#
    .to_string()
}

fn generate_resources(core_dir: &Path, name: &str) -> Result<()> {
    let resources_dir = core_dir.join("resources");
    fs::create_dir_all(&resources_dir).context("Failed to create resources directory")?;

    let capitalized = capitalize(name);
    let guide_md = format!(
        r#"# {} Server Guide

## Overview

The {} server provides tools for mathematical operations.

## Available Tools

### add

Adds two numbers together with validation.

**Parameters:**
- `a` (number): First number (range: -1,000,000 to 1,000,000)
- `b` (number): Second number (range: -1,000,000 to 1,000,000)

**Example:**
```json
{{
  "a": 5,
  "b": 3
}}
```

**Response:**
```json
{{
  "result": 8,
  "operation": "5 + 3 = 8"
}}
```

## Validation

All inputs are validated:
- Numeric ranges prevent overflow/DoS attacks
- Unknown fields are rejected
- Type safety enforced at compile time

## Error Handling

The server returns structured errors:
- Validation errors include field-level details
- Type errors indicate schema mismatches
- Server errors are logged with request IDs
"#,
        capitalized, name
    );

    fs::write(resources_dir.join("guide.md"), guide_md).context("Failed to create guide.md")?;

    Ok(())
}

fn generate_core_crate_calculator(core_dir: &Path, _name: &str) -> Result<()> {
    fs::create_dir_all(core_dir).context("Failed to create core directory")?;
    fs::create_dir_all(core_dir.join("src")).context("Failed to create core src directory")?;

    // Generate Cargo.toml
    let cargo_toml = r#"[package]
name = "mcp-calculator-core"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true

[dependencies]
pmcp = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
schemars = { workspace = true }

[dev-dependencies]
tokio = { workspace = true }
"#;

    fs::write(core_dir.join("Cargo.toml"), cargo_toml)
        .context("Failed to create core Cargo.toml")?;

    // Use the calculator template
    fs::write(
        core_dir.join("src/lib.rs"),
        super::calculator::CALCULATOR_LIB,
    )
    .context("Failed to create calculator lib.rs")?;

    Ok(())
}

fn generate_core_crate_complete(core_dir: &Path, _name: &str) -> Result<()> {
    fs::create_dir_all(core_dir).context("Failed to create core directory")?;
    fs::create_dir_all(core_dir.join("src")).context("Failed to create core src directory")?;

    // Generate Cargo.toml (same as minimal)
    let cargo_toml = r#"[package]
name = "mcp-calculator-core"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true

[dependencies]
pmcp = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
schemars = { workspace = true }
anyhow = { workspace = true }
thiserror = { workspace = true }
validator = { workspace = true }

[dev-dependencies]
tokio = { workspace = true }
"#;

    fs::write(core_dir.join("Cargo.toml"), cargo_toml)
        .context("Failed to create core Cargo.toml")?;

    // Use the complete calculator template
    fs::write(
        core_dir.join("src/lib.rs"),
        super::complete_calculator::COMPLETE_CALCULATOR_LIB,
    )
    .context("Failed to create complete calculator lib.rs")?;

    Ok(())
}

fn generate_core_crate_sqlite(core_dir: &Path, name: &str) -> Result<()> {
    fs::create_dir_all(core_dir).context("Failed to create core directory")?;
    fs::create_dir_all(core_dir.join("src")).context("Failed to create core src directory")?;

    // Generate Cargo.toml with rusqlite dependency
    let cargo_toml = format!(
        r#"[package]
name = "mcp-{}-core"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true

[dependencies]
pmcp = {{ workspace = true }}
serde = {{ workspace = true }}
serde_json = {{ workspace = true }}
schemars = {{ workspace = true }}
anyhow = {{ workspace = true }}
thiserror = {{ workspace = true }}
rusqlite = {{ version = "0.32", features = ["bundled"] }}

[dev-dependencies]
tokio = {{ workspace = true }}
"#,
        name
    );

    fs::write(core_dir.join("Cargo.toml"), cargo_toml)
        .context("Failed to create core Cargo.toml")?;

    // Use the sqlite explorer template with parameterized server name
    let template = super::sqlite_explorer::SQLITE_EXPLORER_LIB
        .replace("build_sqlite_server", &format!("build_{}_server", name));

    fs::write(core_dir.join("src/lib.rs"), template)
        .context("Failed to create sqlite explorer lib.rs")?;

    Ok(())
}

fn create_chinook_placeholder() -> Result<()> {
    let readme = r#"# Database Setup

This server requires the Chinook sample database.

## Quick Setup

Download the chinook database:

```bash
curl -L https://github.com/lerocha/chinook-database/raw/master/ChinookDatabase/DataSources/Chinook_Sqlite.sqlite -o chinook.db
```

Or manually:
1. Visit https://github.com/lerocha/chinook-database
2. Download `Chinook_Sqlite.sqlite`
3. Rename to `chinook.db` and place in workspace root

## About Chinook Database

Chinook is a sample database representing a digital media store:
- 11 tables (customers, invoices, tracks, albums, artists, etc.)
- 59 customers across 24 countries
- 3,503 tracks by 275 artists
- Real-world data for testing MCP servers

License: MIT (included with Chinook)
"#;

    fs::write("DATABASE.md", readme).context("Failed to create DATABASE.md")?;

    Ok(())
}

fn generate_server_crate(server_dir: &Path, name: &str) -> Result<()> {
    fs::create_dir_all(server_dir).context("Failed to create server directory")?;
    fs::create_dir_all(server_dir.join("src")).context("Failed to create server src directory")?;

    // Generate Cargo.toml
    let cargo_toml = format!(
        r#"[package]
name = "{}-server"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true

[[bin]]
name = "{}-server"
path = "src/main.rs"

[dependencies]
mcp-{}-core = {{ path = "../mcp-{}-core" }}
server-common = {{ path = "../server-common" }}
tokio = {{ workspace = true }}
anyhow = {{ workspace = true }}
"#,
        name, name, name, name
    );

    fs::write(server_dir.join("Cargo.toml"), cargo_toml)
        .context("Failed to create server Cargo.toml")?;

    // Generate main.rs (6 lines)
    let main_rs = format!(
        r#"//! {} Server Binary

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {{
    let server = mcp_{}_core::build_{}_server()?;
    server_common::run_http(server).await
}}
"#,
        capitalize(name),
        name.replace("-", "_"),
        name.replace("-", "_")
    );

    fs::write(server_dir.join("src/main.rs"), main_rs)
        .context("Failed to create server main.rs")?;

    Ok(())
}

fn generate_scenarios(name: &str) -> Result<()> {
    let scenarios_dir = Path::new("scenarios").join(name);
    fs::create_dir_all(&scenarios_dir).context("Failed to create scenarios directory")?;

    // Create a simple README explaining how to use scenarios
    let readme = r#"# Test Scenarios

This directory contains test scenarios for your MCP server.

## Quick Start

```bash
# 1. Start your server in another terminal
cargo pmcp dev --server NAME

# 2. Generate test scenarios
cargo pmcp test --server NAME --generate-scenarios

# 3. Run tests
cargo pmcp test --server NAME --detailed
```

## ⚠️ Known Limitation: Tool Assertion Paths

**You will see assertion failures for tool calls in the generated scenarios.** This is expected behavior in Phase 1.

### Why Tool Assertions Fail

MCP wraps tool responses in a nested structure:

```json
{
  "result": {
    "content": [{
      "type": "text",
      "text": "{\"result\":357,\"operation\":\"123 + 234 = 357\"}"
    }]
  }
}
```

The generated scenarios assert on `result` directly, but the actual value is in `content[0].text` (as a JSON string).

### What To Do

**Option 1: Use `contains` assertions (Recommended for Phase 1)**

Edit your `generated.yaml` and change tool assertions to use `contains`:

```yaml
- name: 'Test tool: add (123 + 234 = 357)'
  operation:
    type: tool_call
    tool: add
    arguments:
      a: 123
      b: 234
  continue_on_failure: false  # Change to false once fixed
  assertions:
    - type: success
    - type: contains
      path: "content[0].text"
      value: "357"  # Just check result is in the response
```

**Option 2: Accept the limitation for now**

The scenarios are marked with `continue_on_failure: true`, so tests will pass overall even if individual tool assertions fail. This allows you to validate server connectivity and basic functionality while the assertion system is improved.

## Generating Scenarios

```bash
cargo pmcp test --server NAME --generate-scenarios
```

This will:
1. Discover all tools, prompts, and resources from your running server
2. Generate smart test cases with meaningful values (e.g., add(123, 234) = 357)
3. Create assertions to verify expected results
4. Save scenarios to `generated.yaml`

## Running Tests

```bash
# Run all scenarios
cargo pmcp test --server NAME

# Run with detailed output (see what's failing)
cargo pmcp test --server NAME --detailed
```

## Customizing Scenarios

Edit the generated `generated.yaml` file to:
- Add more test cases
- Customize test values
- Fix assertion paths (see above)
- Test edge cases and error conditions
- Add validation for error scenarios

## Scenario Format

See https://docs.example.com/mcp-tester for full documentation on scenario format.
"#;

    fs::write(scenarios_dir.join("README.md"), readme).context("Failed to create README")?;

    Ok(())
}

fn update_workspace_members(core_name: &str, server_name: &str) -> Result<()> {
    let cargo_toml_path = Path::new("Cargo.toml");
    let content =
        fs::read_to_string(cargo_toml_path).context("Failed to read workspace Cargo.toml")?;

    // Add members to workspace
    let new_content = content.replace(
        "# Add server crates here via: cargo pmcp add server <name>",
        &format!(
            "\"crates/{}\",\n    \"crates/{}\",\n    # Add server crates here via: cargo pmcp add server <name>",
            core_name, server_name
        ),
    );

    fs::write(cargo_toml_path, new_content).context("Failed to update workspace Cargo.toml")?;

    Ok(())
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capitalize() {
        assert_eq!(capitalize("calculator"), "Calculator");
        assert_eq!(capitalize("myserver"), "Myserver");
        assert_eq!(capitalize(""), "");
    }
}
