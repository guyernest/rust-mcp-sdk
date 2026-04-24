use anyhow::{bail, Context, Result};
use std::process::Command;

use crate::deployment::DeployConfig;

/// Initialize Cloudflare Workers deployment using Scaffold + Adapter pattern
pub async fn init_cloudflare(config: &DeployConfig) -> Result<()> {
    println!("🚀 Initializing Cloudflare Workers deployment...");
    println!("   Using Scaffold + Adapter pattern for clean separation");
    println!();

    // 1. Check wrangler authentication
    let is_authenticated = check_wrangler_auth()?;

    // 2. Auto-detect the user's package (don't rely on config.server.name for init)
    let (package_name, package_path) = auto_detect_server_package(&config.project_root)?;

    println!(
        "📦 Found server package: {} ({})",
        package_name,
        package_path.display()
    );

    // 3. Create deploy/cloudflare directory structure
    let deploy_dir = config.project_root.join("deploy/cloudflare");
    std::fs::create_dir_all(&deploy_dir).context("Failed to create deploy/cloudflare directory")?;
    std::fs::create_dir_all(deploy_dir.join("src"))
        .context("Failed to create deploy/cloudflare/src directory")?;

    // 4. Create Cargo.toml for the adapter
    create_adapter_cargo_toml(
        &deploy_dir,
        &package_name,
        &package_path,
        &config.server.name,
        &config.project_root,
    )?;

    // 5. Create wrangler.toml
    create_wrangler_toml(&deploy_dir, &config.server.name)?;

    // 6. Create adapter src/lib.rs (imports user's create_server())
    create_adapter_code(&deploy_dir, &package_name, &config.server.name)?;

    // 7. Create .gitignore
    create_gitignore(&deploy_dir)?;

    println!();
    println!("✅ Cloudflare Workers deployment initialized!");
    println!();
    println!("📁 Generated files:");
    println!("   deploy/cloudflare/");
    println!("   ├── Cargo.toml       (adapter dependencies)");
    println!("   ├── wrangler.toml    (Cloudflare configuration)");
    println!("   ├── src/lib.rs       (generated adapter - DO NOT EDIT)");
    println!("   └── .gitignore");
    println!();
    println!("ℹ️  The adapter code imports your core server via:");
    println!("   use {}::build_server;", package_name);
    println!("   This expects a WASM-compatible core package with minimal dependencies.");
    println!();
    println!("Next steps:");
    if !is_authenticated {
        println!("1. Login to Cloudflare: wrangler login");
        println!("2. Deploy: cargo pmcp deploy --target cloudflare-workers");
    } else {
        println!("1. Deploy: cargo pmcp deploy --target cloudflare-workers");
    }
    println!();

    Ok(())
}

fn check_wrangler_auth() -> Result<bool> {
    print!("🔍 Checking Wrangler authentication...");
    std::io::Write::flush(&mut std::io::stdout())?;

    let output = Command::new("wrangler").args(&["whoami"]).output();

    match output {
        Ok(output) if output.status.success() => {
            println!(" ✅");
            Ok(true)
        },
        _ => {
            println!(" ⚠️");
            println!();
            println!("Wrangler not authenticated. Please run:");
            println!("  wrangler login");
            println!();
            Ok(false)
        },
    }
}

/// Auto-detect the user's MCP server package
/// Prioritizes -core packages for WASM compatibility
fn auto_detect_server_package(
    project_root: &std::path::Path,
) -> Result<(String, std::path::PathBuf)> {
    println!("🔍 Auto-detecting MCP server package...");

    // Check if project_root itself is a package (standalone project)
    if let Some(result) = try_standalone_or_workspace_detection(project_root)? {
        return Ok(result);
    }

    bail!(
        "Could not auto-detect MCP server package.\n\n\
         Please ensure:\n\
         1. Your project has a Cargo.toml with [package] section\n\
         2. For workspaces, create a core package: crates/mcp-yourapp-core/\n\
         3. The core package should export: pub fn build_server() -> pmcp::Result<pmcp::Server>\n\n\
         For multi-target deployment (Cloudflare, Lambda, etc.), we recommend:\n\
         - Core package: Business logic only (WASM-compatible)\n\
         - Transport packages: Use the core package\n\n\
         See CORE_TRANSPORT_PATTERN.md for details.\n\n\
         Searched in: {}\n\
         Run 'cargo build' to verify your project structure.",
        project_root.display()
    )
}

/// First-pass detection: standalone package at root, or workspace search.
/// Returns `Ok(None)` when root Cargo.toml doesn't exist or doesn't match
/// either shape (caller emits the bail! with troubleshooting).
fn try_standalone_or_workspace_detection(
    project_root: &std::path::Path,
) -> Result<Option<(String, std::path::PathBuf)>> {
    let root_cargo_toml = project_root.join("Cargo.toml");
    if !root_cargo_toml.exists() {
        return Ok(None);
    }

    let content =
        std::fs::read_to_string(&root_cargo_toml).context("Failed to read root Cargo.toml")?;

    if content.contains("[package]") && !content.contains("[workspace]") {
        let name = extract_package_name(&content)?;
        println!("   Found standalone package: {}", name);
        return Ok(Some((name, project_root.to_path_buf())));
    }

    if content.contains("[workspace]") {
        println!("   Detected workspace, searching for MCP server package...");
        return detect_workspace_package(project_root);
    }

    Ok(None)
}

/// Search a workspace for a -core package first, any MCP package second.
fn detect_workspace_package(
    project_root: &std::path::Path,
) -> Result<Option<(String, std::path::PathBuf)>> {
    if let Some((name, path)) = find_core_package(project_root)? {
        println!("   ✅ Found core package (WASM-compatible): {}", name);
        return Ok(Some((name, path)));
    }

    println!("   ⚠️  No -core package found, searching for any MCP server package...");

    if let Some((name, path)) = find_any_package(project_root)? {
        println!(
            "   ⚠️  Using package: {} (may have WASM compatibility issues)",
            name
        );
        println!(
            "   ℹ️  Consider splitting into core/transport packages for multi-target deployment"
        );
        return Ok(Some((name, path)));
    }

    Ok(None)
}

/// Scan a list of candidate directories for the first package whose name
/// satisfies `accept`, using `try_package_dir` to load each subdirectory.
///
/// Shared helper replacing the duplicated `for search_dir in search_dirs { ... }`
/// loop that previously lived in find_core_package and find_any_package.
fn scan_for_package(
    search_dirs: &[std::path::PathBuf],
    accept: impl Fn(&str) -> bool,
) -> Result<Option<(String, std::path::PathBuf)>> {
    for search_dir in search_dirs {
        if !search_dir.exists() || !search_dir.is_dir() {
            continue;
        }
        if let Some(found) = first_matching_package(search_dir, &accept)? {
            return Ok(Some(found));
        }
    }
    Ok(None)
}

/// Scan a single directory (non-recursive) and return the first package whose
/// name satisfies `accept`.
fn first_matching_package(
    search_dir: &std::path::Path,
    accept: &impl Fn(&str) -> bool,
) -> Result<Option<(String, std::path::PathBuf)>> {
    let Ok(entries) = std::fs::read_dir(search_dir) else {
        return Ok(None);
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if let Ok((name, pkg_path)) = try_package_dir(&path) {
            if accept(&name) {
                return Ok(Some((name, pkg_path)));
            }
        }
    }
    Ok(None)
}

/// Find a -core package (preferred for WASM compatibility)
fn find_core_package(
    project_root: &std::path::Path,
) -> Result<Option<(String, std::path::PathBuf)>> {
    let search_dirs = vec![
        project_root.join("core-workspace"), // Separate core workspace (recommended)
        project_root.join("crates"),         // crates/ directory
        project_root.join("packages"),       // packages/ directory
        project_root.to_path_buf(),          // Root level packages
    ];
    scan_for_package(&search_dirs, |name| name.ends_with("-core"))
}

/// Find any package that looks like an MCP server
fn find_any_package(
    project_root: &std::path::Path,
) -> Result<Option<(String, std::path::PathBuf)>> {
    // Check root-level first (skip the project_root itself; try_package_dir
    // already rejects workspace roots).
    let root_dirs = vec![project_root.to_path_buf()];
    if let Some(found) = scan_for_package(&root_dirs, |_| true)? {
        return Ok(Some(found));
    }

    let sub_dirs = vec![
        project_root.join("crates"),   // crates/ directory
        project_root.join("packages"), // packages/ directory
    ];
    scan_for_package(&sub_dirs, |_| true)
}

/// Try to load a package from a directory
fn try_package_dir(dir: &std::path::Path) -> Result<(String, std::path::PathBuf)> {
    let cargo_toml = dir.join("Cargo.toml");
    if !cargo_toml.exists() {
        bail!("No Cargo.toml");
    }

    let content = std::fs::read_to_string(&cargo_toml)?;
    if !content.contains("[package]") {
        bail!("No [package] section");
    }

    // Skip if it's a workspace itself
    if content.contains("[workspace]") {
        bail!("Is a workspace");
    }

    let name = extract_package_name(&content)?;
    Ok((name, dir.to_path_buf()))
}

/// Extract package name from Cargo.toml content
fn extract_package_name(cargo_toml_content: &str) -> Result<String> {
    for line in cargo_toml_content.lines() {
        if line.trim().starts_with("name") {
            if let Some(name_part) = line.split('=').nth(1) {
                let name = name_part.trim().trim_matches('"').trim_matches('\'');
                return Ok(name.to_string());
            }
        }
    }
    bail!("Could not find package name in Cargo.toml")
}

/// Create Cargo.toml for the Cloudflare adapter
fn create_adapter_cargo_toml(
    deploy_dir: &std::path::Path,
    parent_package: &str,
    package_path: &std::path::Path,
    worker_name: &str,
    project_root: &std::path::Path,
) -> Result<()> {
    print!("📝 Creating adapter Cargo.toml...");
    std::io::Write::flush(&mut std::io::stdout())?;

    // Calculate relative path from deploy/cloudflare to the package
    let relative_path = pathdiff::diff_paths(package_path, deploy_dir)
        .context("Failed to calculate relative path to package")?;
    let relative_path_str = relative_path
        .to_str()
        .context("Path contains invalid UTF-8")?;

    // Determine how to reference pmcp - check if it's available in parent's Cargo.toml
    let pmcp_dependency = detect_pmcp_dependency(project_root)?;

    let cargo_toml = format!(
        r#"[package]
name = "{}-cloudflare-adapter"
version = "0.1.0"
edition = "2021"

# Metadata for wasm-pack
[package.metadata.wasm-pack]
wasm-opt = false

[lib]
crate-type = ["cdylib"]

[dependencies]
# Import the parent MCP server package
{} = {{ path = "{}" }}

# Cloudflare Workers runtime
worker = "0.4"

# PMCP SDK with WASM support
{}

# JSON support
serde = {{ version = "1", features = ["derive"] }}
serde_json = "1"

# Better panic messages in WASM
console_error_panic_hook = "0.1"

[profile.release]
lto = true
strip = true
codegen-units = 1
opt-level = "z"

# Exclude from parent workspace (this is a separate build)
[workspace]
"#,
        worker_name, parent_package, relative_path_str, pmcp_dependency
    );

    std::fs::write(deploy_dir.join("Cargo.toml"), cargo_toml)
        .context("Failed to write Cargo.toml")?;

    println!(" ✅");
    Ok(())
}

/// Detect how to reference the pmcp dependency
fn detect_pmcp_dependency(project_root: &std::path::Path) -> Result<String> {
    // First, try to find pmcp in workspace members' Cargo.toml files
    // This handles both workspace and standalone projects

    // Check root Cargo.toml
    if let Ok(pmcp_dep) =
        try_find_pmcp_in_cargo_toml(&project_root.join("Cargo.toml"), project_root)
    {
        return Ok(pmcp_dep);
    }

    if let Some(pmcp_dep) = search_workspace_members_for_pmcp(project_root) {
        return Ok(pmcp_dep);
    }

    // Fallback: assume pmcp is in the SDK (common for examples)
    println!("   ⚠️  Could not detect pmcp dependency, using fallback path");
    Ok(String::from(
        "# Auto-detected: pmcp from SDK (adjust if needed)\n\
         pmcp = { path = \"../../../..\", default-features = false, features = [\"wasm\"] }",
    ))
}

/// Search workspace member directories (crates/, packages/) for a Cargo.toml
/// that references pmcp. Returns the rendered dependency string on first hit.
fn search_workspace_members_for_pmcp(project_root: &std::path::Path) -> Option<String> {
    for dir in &["crates", "packages"] {
        let search_dir = project_root.join(dir);
        if !search_dir.exists() || !search_dir.is_dir() {
            continue;
        }
        if let Some(pmcp_dep) = scan_dir_members_for_pmcp(&search_dir, project_root) {
            return Some(pmcp_dep);
        }
    }
    None
}

/// Scan immediate subdirectories of `search_dir` for a Cargo.toml that
/// references pmcp.
fn scan_dir_members_for_pmcp(
    search_dir: &std::path::Path,
    project_root: &std::path::Path,
) -> Option<String> {
    let entries = std::fs::read_dir(search_dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let cargo_toml = path.join("Cargo.toml");
        if let Ok(pmcp_dep) = try_find_pmcp_in_cargo_toml(&cargo_toml, project_root) {
            return Some(pmcp_dep);
        }
    }
    None
}

/// Try to find pmcp dependency in a specific Cargo.toml
fn try_find_pmcp_in_cargo_toml(
    cargo_toml: &std::path::Path,
    project_root: &std::path::Path,
) -> Result<String> {
    if !cargo_toml.exists() {
        bail!("Cargo.toml does not exist");
    }

    let content = std::fs::read_to_string(cargo_toml)?;

    // Check if this uses workspace dependencies
    if uses_workspace_pmcp(&content) {
        let root_cargo = project_root.join("Cargo.toml");
        if root_cargo.exists() && root_cargo != cargo_toml {
            return try_find_workspace_pmcp(&root_cargo, project_root);
        }
    }

    // Look for direct pmcp dependency with a path
    let pmcp_path = find_direct_pmcp_path(&content).context("pmcp not found in Cargo.toml")?;

    let cargo_dir = cargo_toml.parent().unwrap();
    let pmcp_absolute = cargo_dir.join(pmcp_path).canonicalize()?;
    render_pmcp_dep_line(&pmcp_absolute, project_root)
}

/// Returns true if the Cargo.toml references pmcp via `pmcp = { workspace = true }`.
fn uses_workspace_pmcp(content: &str) -> bool {
    content.contains("pmcp = { workspace = true }") || content.contains("pmcp = {workspace = true}")
}

/// Extract a literal path from a `pmcp = { path = "..." }` line. Returns None
/// if no pmcp line with an inline `path = "..."` is present.
fn find_direct_pmcp_path(content: &str) -> Option<&str> {
    let line = content.lines().find(|l| l.trim().starts_with("pmcp"))?;
    if !line.contains("path =") {
        return None;
    }
    let path_start = line.find("path = \"")?;
    let path_content = &line[path_start + 8..];
    let path_end = path_content.find('"')?;
    Some(&path_content[..path_end])
}

/// Render the final `pmcp = { path = "...", default-features = false, features = ["wasm"] }`
/// dependency line, with the path made relative to the generated
/// `deploy/cloudflare/` directory.
fn render_pmcp_dep_line(
    pmcp_absolute: &std::path::Path,
    project_root: &std::path::Path,
) -> Result<String> {
    let deploy_dir = project_root.join("deploy/cloudflare");
    let relative = pathdiff::diff_paths(pmcp_absolute, &deploy_dir)
        .context("Failed to calculate relative path to pmcp")?;
    let relative_str = relative.to_str().context("Invalid UTF-8 in path")?;
    Ok(format!(
        "pmcp = {{ path = \"{}\", default-features = false, features = [\"wasm\"] }}",
        relative_str
    ))
}

/// Try to find pmcp in workspace.dependencies
fn try_find_workspace_pmcp(
    root_cargo: &std::path::Path,
    project_root: &std::path::Path,
) -> Result<String> {
    let content = std::fs::read_to_string(root_cargo)?;
    let pmcp_path_str = extract_workspace_pmcp_path(&content)
        .context("pmcp path not found in [workspace.dependencies.pmcp]")?;

    let cargo_dir = root_cargo.parent().unwrap();
    let pmcp_full_path = if std::path::Path::new(&pmcp_path_str).is_absolute() {
        std::path::PathBuf::from(&pmcp_path_str)
    } else {
        cargo_dir.join(&pmcp_path_str)
    };

    let pmcp_absolute = pmcp_full_path
        .canonicalize()
        .with_context(|| format!("Failed to resolve pmcp path: {}", pmcp_path_str))?;

    render_pmcp_dep_line(&pmcp_absolute, project_root)
}

/// Parse `[workspace.dependencies.pmcp]` section of a workspace root Cargo.toml
/// and return the `path = "..."` literal. Returns None if the section is
/// missing or doesn't contain a path key.
fn extract_workspace_pmcp_path(content: &str) -> Option<String> {
    let mut in_pmcp_section = false;
    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed == "[workspace.dependencies.pmcp]" {
            in_pmcp_section = true;
            continue;
        }

        if trimmed.starts_with('[') && in_pmcp_section {
            break;
        }

        if in_pmcp_section && trimmed.starts_with("path =") {
            if let Some(path) = parse_path_literal(trimmed) {
                return Some(path);
            }
        }
    }
    None
}

/// Parse `path = "..."` from a TOML line, returning the quoted literal.
fn parse_path_literal(line: &str) -> Option<String> {
    let path_start = line.find("path = \"")?;
    let path_content = &line[path_start + 8..];
    let path_end = path_content.find('"')?;
    Some(path_content[..path_end].to_string())
}

fn create_wrangler_toml(deploy_dir: &std::path::Path, server_name: &str) -> Result<()> {
    print!("📝 Creating wrangler.toml...");
    std::io::Write::flush(&mut std::io::stdout())?;

    let wrangler_toml = format!(
        r#"name = "{}"
main = "build/worker/shim.mjs"
compatibility_date = "2024-11-20"

[dev]
port = 8787

[build]
command = "cargo install -q worker-build && worker-build --release"
"#,
        server_name
    );

    std::fs::write(deploy_dir.join("wrangler.toml"), wrangler_toml)
        .context("Failed to write wrangler.toml")?;

    println!(" ✅");
    Ok(())
}

/// Create adapter src/lib.rs that imports user's create_server()
fn create_adapter_code(
    deploy_dir: &std::path::Path,
    parent_package: &str,
    server_name: &str,
) -> Result<()> {
    print!("📝 Creating adapter code...");
    std::io::Write::flush(&mut std::io::stdout())?;

    // Convert package name from hyphens to underscores for Rust import
    let package_for_import = parent_package.replace("-", "_");

    let adapter_code = format!(
        r#"// GENERATED BY cargo-pmcp - DO NOT EDIT MANUALLY
// Regenerate with: cargo pmcp deploy init --target cloudflare-workers --regenerate
//
// This adapter imports your WASM-compatible core MCP server and wraps it for Cloudflare Workers.
// Your server logic stays in the core crate - this is just deployment scaffolding.
//
// Architecture:
// - Core package: Business logic (WASM-compatible)
// - This adapter: Cloudflare Workers entrypoint

use {}::build_calculator_server;
use worker::*;

#[event(fetch)]
async fn main(mut req: Request, _env: Env, _ctx: Context) -> Result<Response> {{
    // Set panic hook for better error messages
    console_error_panic_hook::set_once();

    // Log the request
    console_log!("Received: {{}} {{}}", req.method(), req.path());

    // Handle CORS preflight
    if req.method() == Method::Options {{
        return cors_preflight();
    }}

    // Handle GET requests with server info
    if req.method() == Method::Get {{
        return server_info();
    }}

    // Only handle POST requests for MCP protocol
    if req.method() != Method::Post {{
        return Response::error("Only GET and POST methods are supported", 405);
    }}

    // Build your WASM-compatible core server
    let server = match build_calculator_server() {{
        Ok(s) => s,
        Err(e) => {{
            console_error!("Failed to build server: {{}}", e);
            return Response::error(&format!("Server initialization failed: {{}}", e), 500);
        }}
    }};

    // Get request body
    let body = match req.text().await {{
        Ok(text) => text,
        Err(e) => {{
            console_error!("Failed to read body: {{}}", e);
            return Response::error("Failed to read request body", 400);
        }}
    }};

    // TODO: Use pmcp::adapters::cloudflare::serve() when available
    // For now, basic JSON-RPC handling
    let response_json = match handle_mcp_request(&server, &body).await {{
        Ok(json) => json,
        Err(e) => {{
            console_error!("Error handling request: {{}}", e);
            return Response::error(&format!("Error: {{}}", e), 500);
        }}
    }};

    // Return response with CORS headers
    let mut headers = Headers::new();
    headers.set("Content-Type", "application/json")?;
    headers.set("Access-Control-Allow-Origin", "*")?;

    Ok(Response::ok(response_json)?.with_headers(headers))
}}

fn cors_preflight() -> Result<Response> {{
    let mut headers = Headers::new();
    headers.set("Access-Control-Allow-Origin", "*")?;
    headers.set("Access-Control-Allow-Methods", "GET, POST, OPTIONS")?;
    headers.set("Access-Control-Allow-Headers", "Content-Type")?;
    Ok(Response::empty()?.with_headers(headers))
}}

fn server_info() -> Result<Response> {{
    let info = serde_json::json!({{
        "name": "{}",
        "version": "1.0.0",
        "protocol_version": "2024-11-05",
        "description": "MCP server running on Cloudflare Workers",
        "runtime": "cloudflare-workers",
        "capabilities": {{
            "tools": true,
            "resources": false,
            "prompts": false
        }}
    }});

    let mut headers = Headers::new();
    headers.set("Content-Type", "application/json")?;
    headers.set("Access-Control-Allow-Origin", "*")?;

    Ok(Response::ok(serde_json::to_string_pretty(&info)?)?.with_headers(headers))
}}

async fn handle_mcp_request(
    _server: &pmcp::Server,
    _body: &str,
) -> Result<String> {{
    // TODO: This is a placeholder - use pmcp::adapters::cloudflare when available
    // For now, return a simple response indicating the server is set up
    Ok(serde_json::json!({{
        "jsonrpc": "2.0",
        "id": "1",
        "result": {{
            "message": "Cloudflare Workers adapter initialized",
            "note": "Full MCP protocol support coming soon"
        }}
    }}).to_string())
}}
"#,
        package_for_import, server_name
    );

    std::fs::write(deploy_dir.join("src/lib.rs"), adapter_code)
        .context("Failed to write src/lib.rs")?;

    println!(" ✅");
    Ok(())
}

/// Create .gitignore for the adapter
fn create_gitignore(deploy_dir: &std::path::Path) -> Result<()> {
    print!("📝 Creating .gitignore...");
    std::io::Write::flush(&mut std::io::stdout())?;

    let gitignore = r#"# Build outputs
/target
/build
/pkg
*.wasm

# Worker build artifacts
/.wrangler
/worker

# Logs
*.log
"#;

    std::fs::write(deploy_dir.join(".gitignore"), gitignore)
        .context("Failed to write .gitignore")?;

    println!(" ✅");
    Ok(())
}
