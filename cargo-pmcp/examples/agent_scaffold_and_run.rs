//! Example: scaffold an agent project (`agent new`) and drive the offline
//! fixed-source loop (`agent dev --source fixed`) end-to-end (CLI-01 / CLI-02).
//!
//! Two halves, both fully offline (no network, no LLM, no sockets):
//!
//! 1. **Scaffold** — drive the PUBLIC LIB SEAM
//!    ([`cargo_pmcp::templates_agent::generate`]) into an auto-cleaned temp dir
//!    and print the generated file tree. This is the SAME emitter the
//!    `cargo pmcp agent new` command uses.
//! 2. **Run** — drive the PRODUCTION fixed-source runner
//!    ([`cargo_pmcp::agent_run::run_fixed_source`]) — the SAME path the
//!    `cargo pmcp agent dev --source fixed` CLI arm calls (Codex 110-06 HIGH: no
//!    re-implemented `AgentEngine` loop here) — to a terminal
//!    [`RunOutcome`](pmcp_agent::RunOutcome) and print it.
//!
//! Both halves reach production code through the narrow `#[doc(hidden)]` public
//! lib `#[path]` seams (`templates_agent`, `agent_run`), NOT the bin-only
//! `templates::*` / `commands::*` module trees — so this example compiles in the
//! lib target and exercises the REAL CLI code paths.
//!
//! This satisfies the CLAUDE.md ALWAYS EXAMPLE requirement for CLI-01/CLI-02.
//!
//! Run with:
//!     cargo run -p cargo-pmcp --example agent_scaffold_and_run

use std::path::Path;

use pmcp_agent::ResolvedAgentConfig;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // --- Half 1: scaffold a runnable agent crate (the `agent new` payload) ----
    let tmp = tempfile::tempdir()?;
    let crate_dir = tmp.path().join("demo_agent");
    std::fs::create_dir_all(crate_dir.join("src"))?;

    // Drive the SAME emitter the `agent new` command uses, via the narrow public
    // lib seam.
    cargo_pmcp::templates_agent::generate(&crate_dir, "demo_agent")?;

    println!("Scaffolded an agent crate at: {}", crate_dir.display());
    println!("Generated file tree:");
    print_tree(&crate_dir, &crate_dir);

    // --- Half 2: run the offline fixed-source loop (the `agent dev` payload) ---
    // Build a resolved config (the s50 constructor: instructions, model,
    // max_tokens, max_iterations) and call the PRODUCTION runner seam — the SAME
    // path `agent dev --source fixed` uses (NOT a re-implemented AgentEngine loop).
    let config = ResolvedAgentConfig::new(
        "You are a concise, helpful assistant. Use tools when helpful.",
        "demo-model",
        100_000,
        5,
    );

    println!("\nDriving the production fixed-source runner offline (agent dev --source fixed)…");
    let outcome = cargo_pmcp::agent_run::run_fixed_source(config).await;
    println!("→ terminal RunOutcome: {outcome:?}");

    println!(
        "\nNext: `cd` into the scaffolded crate, edit `agent.package.json`, and \
         `cargo run` — the generated runner drives the same loop against a real \
         OpenAI-compatible endpoint (Ollama by default)."
    );
    Ok(())
}

/// Print every file under `root`, relative to `base`, sorted for deterministic
/// output.
fn print_tree(root: &Path, base: &Path) {
    let mut files: Vec<String> = walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(std::result::Result::ok)
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| {
            e.path()
                .strip_prefix(base)
                .ok()
                .map(|p| p.to_string_lossy().replace('\\', "/"))
        })
        .collect();
    files.sort();
    for f in files {
        println!("  {f}");
    }
}
