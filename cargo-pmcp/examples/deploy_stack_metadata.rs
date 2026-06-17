//! Example: the `deploy/lib/stack.ts` regeneration guard + config-driven
//! `[metadata]` (Phase 98, DSTK-01/02/03).
//!
//! Demonstrates the end-to-end Phase 98 workflow:
//!   1. Parse a `graph-rag`-shaped deploy.toml with a `[metadata]` block into a
//!      `DeployConfig` and show the curated `server_type` / `snapshot_baked`
//!      literals survive into config (so a regenerated `stack.ts` reproduces
//!      `mcp:serverType:'graph-rag'` + `mcp:snapshotBaked:'true'` instead of the
//!      hardcoded `'custom'`).
//!   2. Demonstrate the exists-guard: a pre-existing operator-curated
//!      `deploy/lib/stack.ts` is PRESERVED when `regenerate_stack` is false, and
//!      only OVERWRITTEN when the operator opts in via `--regenerate-stack`
//!      (`config.regenerate_stack = true`).
//!
//! Only the lib-public deployment surface
//! (`cargo_pmcp::deployment::config::{DeployConfig, MetadataConfig}`) is used.
//! The real renderer + the `write_stack_ts_guarded` helper live behind the
//! bin-only `commands::*`/`deployment::targets::*` tree the lib does not
//! re-export; this example reproduces the guard's `path.exists() && !regenerate`
//! contract (the exact predicate `write_stack_ts_guarded` enforces, and which
//! the `--regenerate-stack`/`--force` flag drives via `config.regenerate_stack`)
//! so it stays runnable from outside the crate.
//!
//! Run with:
//!     cargo run -p cargo-pmcp --example deploy_stack_metadata

use cargo_pmcp::deployment::config::{DeployConfig, MetadataConfig};

const GRAPH_RAG_TOML: &str = include_str!("fixtures/graph-rag.deploy.toml");

/// A minimal operator-curated `deploy/lib/stack.ts` carrying both metadata
/// literals, exactly as a hand-edit would look (mirrors the reported defect).
const CURATED_STACK_TS: &str = "// operator-curated stack.ts — DO NOT CLOBBER\n\
     templateOptions.metadata = {\n\
     \x20 mcp:serverType:'graph-rag',\n\
     \x20 mcp:snapshotBaked:'true',\n\
     };\n";

fn main() {
    println!("=== Phase 98 — stack.ts regeneration guard + config-driven [metadata] ===\n");

    // ---------------------------------------------------------------------
    // 1. Config-driven metadata: parse the [metadata] block.
    // ---------------------------------------------------------------------
    println!("--- 1. A graph-rag deploy.toml with a [metadata] block ---\n");
    print_indented(GRAPH_RAG_TOML);
    println!();

    let cfg: DeployConfig = toml::from_str(GRAPH_RAG_TOML)
        .expect("fixture parses — the DSTK-04 proptest would have caught a regression");

    println!("--- 2. Parsed [metadata] (config-driven stack.ts literals) ---");
    println!(
        "  server_type:    {:?}\n  snapshot_baked: {:?}\n  is_empty():     {}\n",
        cfg.metadata.server_type,
        cfg.metadata.snapshot_baked,
        cfg.metadata.is_empty(),
    );
    assert_eq!(cfg.metadata.server_type.as_deref(), Some("graph-rag"));
    assert_eq!(cfg.metadata.snapshot_baked, Some(true));

    println!("--- 3. What the regenerated stack.ts advertises ---");
    println!("  These literals are now reproducible-from-config, so a");
    println!("  `cargo pmcp deploy --regenerate-stack` reproduces them instead of");
    println!("  falling back to the hardcoded server type 'custom':");
    println!(
        "    mcp:serverType:'{}'",
        cfg.metadata.server_type.as_deref().unwrap_or("custom"),
    );
    if cfg.metadata.snapshot_baked.unwrap_or(false) {
        println!("    mcp:snapshotBaked:'true'");
    }
    println!();

    // Backward-compat: an absent [metadata] block is byte-identical (no
    // [metadata] header, no snapshotBaked literal).
    println!("--- 4. Backward-compat: a config WITHOUT [metadata] is unchanged ---");
    let mut bare = DeployConfig::default_for_server(
        "plain-server".to_string(),
        "us-west-2".to_string(),
        std::path::PathBuf::from("/tmp/phase98-example-plain"),
    );
    bare.regenerate_stack = true; // runtime-only flag, never serialised.
    let bare_toml = toml::to_string(&bare).expect("serialises");
    let has_metadata_header = bare_toml.contains("[metadata]");
    let leaks_runtime_flag = bare_toml.contains("regenerate_stack");
    println!("  emits a [metadata] header:        {has_metadata_header} (expected false)");
    println!("  leaks regenerate_stack to disk:   {leaks_runtime_flag} (expected false)");
    assert!(
        !has_metadata_header,
        "empty [metadata] must not serialise a header"
    );
    assert!(!leaks_runtime_flag, "regenerate_stack is #[serde(skip)]");
    println!();

    // ---------------------------------------------------------------------
    // 5. The exists-guard: preserve a curated stack.ts unless opted in.
    // ---------------------------------------------------------------------
    println!("--- 5. Exists-guard: a curated deploy/lib/stack.ts is preserved ---\n");
    let tmp = tempfile::tempdir().expect("create tempdir");
    let lib_dir = tmp.path().join("deploy").join("lib");
    std::fs::create_dir_all(&lib_dir).expect("create deploy/lib");
    let stack_ts = lib_dir.join("stack.ts");
    std::fs::write(&stack_ts, CURATED_STACK_TS).expect("write curated stack.ts");

    // Without the flag (regenerate_stack = false): the guard SKIPS the write.
    let preserved = guarded_write(&stack_ts, "// REGENERATED — would clobber\n", false);
    let after_no_flag = std::fs::read_to_string(&stack_ts).expect("read back");
    println!("  regenerate_stack = false:");
    println!("    wrote a new stack.ts?  {preserved} (expected false — preserved)");
    println!(
        "    file unchanged?        {} (expected true)",
        after_no_flag == CURATED_STACK_TS,
    );
    assert!(
        !preserved,
        "guard must skip the write when regenerate_stack is false"
    );
    assert_eq!(
        after_no_flag, CURATED_STACK_TS,
        "curated file must be untouched"
    );
    println!("    {STACK_TS_PRESERVED_NOTICE}\n");

    // With --regenerate-stack (regenerate_stack = true): the guard overwrites.
    let regenerated = guarded_write(&stack_ts, "// REGENERATED from config\n", true);
    let after_flag = std::fs::read_to_string(&stack_ts).expect("read back");
    println!("  regenerate_stack = true (--regenerate-stack / --force):");
    println!("    wrote a new stack.ts?  {regenerated} (expected true — overwritten)");
    println!(
        "    file regenerated?      {} (expected true)\n",
        after_flag != CURATED_STACK_TS,
    );
    assert!(
        regenerated,
        "guard must overwrite when regenerate_stack is true"
    );
    assert_ne!(
        after_flag, CURATED_STACK_TS,
        "file must be regenerated with the flag"
    );

    println!("=== Example complete ===");
}

/// One-line notice the deploy targets print when the guard preserves an existing
/// `stack.ts` (matches `deployment::config::STACK_TS_PRESERVED_NOTICE`, which is
/// `pub(crate)` and therefore not reachable from this external example).
const STACK_TS_PRESERVED_NOTICE: &str =
    "preserved existing deploy/lib/stack.ts (pass --regenerate-stack to overwrite)";

/// The exists-guard contract that `write_stack_ts_guarded` enforces on both
/// deploy targets: skip the write (returning `false`) when the file already
/// exists and the operator did not opt into regeneration; otherwise write and
/// return `true`. The real helper is `pub(crate)`; this mirror keeps the example
/// runnable from outside the crate and documents the exact predicate.
fn guarded_write(path: &std::path::Path, contents: &str, regenerate: bool) -> bool {
    if path.exists() && !regenerate {
        return false;
    }
    std::fs::write(path, contents).expect("write stack.ts");
    true
}

fn print_indented(s: &str) {
    for line in s.lines() {
        println!("  {line}");
    }
}
