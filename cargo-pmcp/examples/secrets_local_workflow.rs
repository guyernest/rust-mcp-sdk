//! Example: Local Secrets Management Workflow
//!
//! This example demonstrates how to use the local secrets provider
//! for development workflows with cargo-pmcp.
//!
//! Run with: cargo run -p cargo-pmcp --example secrets_local_workflow

use std::path::PathBuf;

/// Demonstrates the local secrets workflow for MCP server development.
///
/// In a typical workflow:
/// 1. Set secrets locally for development
/// 2. Secrets are stored in `.pmcp/secrets/{server-id}/`
/// 3. Files are protected with mode 0600
/// 4. `.gitignore` is automatically managed
fn main() {
    println!("=== Local Secrets Management Workflow ===\n");

    // Show example CLI commands for local secrets management
    println!("1. List available providers:");
    println!("   cargo pmcp secret providers\n");

    println!("2. Set a secret interactively (recommended):");
    println!("   cargo pmcp secret set chess/ANTHROPIC_API_KEY --prompt\n");

    println!("3. Set a secret from environment variable:");
    println!("   cargo pmcp secret set chess/ANTHROPIC_API_KEY --env ANTHROPIC_API_KEY\n");

    println!("4. Generate a random secret:");
    println!("   cargo pmcp secret set my-server/SESSION_SECRET --generate --length 64\n");

    println!("5. List all secrets:");
    println!("   cargo pmcp secret list\n");

    println!("6. List secrets for a specific server:");
    println!("   cargo pmcp secret list --server chess\n");

    println!("7. Get a secret value (outputs to terminal with warning):");
    println!("   cargo pmcp secret get chess/ANTHROPIC_API_KEY\n");

    println!("8. Get a secret to a file (secure):");
    println!("   cargo pmcp secret get chess/ANTHROPIC_API_KEY --output .env.local\n");

    println!("9. Delete a secret:");
    println!("   cargo pmcp secret delete chess/ANTHROPIC_API_KEY\n");

    println!("10. Force delete without confirmation:");
    println!("    cargo pmcp secret delete chess/ANTHROPIC_API_KEY --force\n");

    // Show where secrets are stored
    println!("=== Storage Location ===\n");
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("~"));
    let secrets_dir = home.join(".pmcp").join("secrets");
    println!("Secrets are stored in: {}", secrets_dir.display());
    println!("\nDirectory structure:");
    println!("  .pmcp/");
    println!("  └── secrets/");
    println!("      ├── .gitignore    # Auto-created, contains '*'");
    println!("      ├── chess/");
    println!("      │   └── ANTHROPIC_API_KEY");
    println!("      └── my-server/");
    println!("          └── SESSION_SECRET\n");

    // Security notes
    println!("=== Security Notes ===\n");
    println!("- All secret files are created with mode 0600 (owner read/write only)");
    println!("- The .gitignore ensures secrets are never committed");
    println!("- Use --prompt for interactive input (hidden)");
    println!("- Avoid --value flag as it exposes secrets in shell history");
    println!("- Use --output to write secrets to files instead of terminal\n");

    // Example pmcp.toml configuration
    println!("=== Using Secrets in pmcp.toml ===\n");
    println!("Reference secrets in your pmcp.toml configuration:");
    println!();
    println!("  [server]");
    println!("  name = \"chess\"");
    println!();
    println!("  [secrets]");
    println!("  ANTHROPIC_API_KEY = \"{{{{ secret:chess/ANTHROPIC_API_KEY }}}}\"");
    println!("  DATABASE_URL = \"{{{{ secret:chess/DATABASE_URL }}}}\"");
    println!();

    println!("=== Complete! ===");
    println!("\nFor more information, run: cargo pmcp secret --help");
}
