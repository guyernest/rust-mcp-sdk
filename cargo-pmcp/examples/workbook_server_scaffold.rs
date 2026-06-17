//! Example: scaffolding a Shape B governed-Excel workbook server (WBCL-05).
//!
//! Demonstrates the `cargo pmcp new --kind workbook-server` payload WITHOUT
//! booting a server: it drives the PUBLIC LIB SEAM
//! ([`cargo_pmcp::templates_workbook_server::generate`]) into a temp dir and
//! prints the generated file tree. The example reaches the emitter through the
//! lib-public `templates_workbook_server` seam (mounted in `lib.rs` via
//! `#[path]`), NOT the bin-only `templates::*` module — so it actually compiles
//! in the lib target.
//!
//! This satisfies the CLAUDE.md ALWAYS EXAMPLE requirement for WBCL-05.
//!
//! Run with:
//!     cargo run -p cargo-pmcp --example workbook_server_scaffold

use std::path::Path;

fn main() -> anyhow::Result<()> {
    // An isolated, auto-cleaned scratch dir for the scaffold output.
    let tmp = tempfile::tempdir()?;
    let crate_dir = tmp.path().join("my_workbook_server");
    std::fs::create_dir_all(crate_dir.join("src"))?;

    // Drive the SAME emitter the `--kind workbook-server` command uses, via the
    // narrow public lib seam.
    cargo_pmcp::templates_workbook_server::generate(&crate_dir, "my_workbook_server")?;

    println!(
        "Scaffolded a Shape B workbook server crate at: {}",
        crate_dir.display()
    );
    println!("Generated file tree:");
    print_tree(&crate_dir, &crate_dir);

    println!(
        "\nNext: `cd` into the crate and `cargo run` — it serves the five workbook \
         tools over streamable HTTP (prints PMCP_WORKBOOK_SERVER_ADDR=...)."
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
