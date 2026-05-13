//! # Server example: SEP-2640 Skills (Phase 80)
//!
//! Demonstrates the three-tier skill registration pattern + the dual-surface
//! bootstrap. Skills live under `examples/skills/` and are embedded via
//! `include_str!` at compile time.
//!
//! **Why this example uses `Server::builder()` (not `ServerCoreBuilder`):**
//! `pmcp::Server::builder()` is the public, documented entry point for
//! constructing servers in PMCP. The skill API (`.skill`, `.skills`,
//! `.try_skills`, `.bootstrap_skill_and_prompt`) is wired onto BOTH
//! `ServerBuilder` (returned by `Server::builder()`) AND `ServerCoreBuilder`
//! (per 80-REVIEWS.md Fix 2 / Codex C3) so this example demonstrates the
//! recommended path.
//!
//! Run with: `cargo run --example s44_server_skills --features skills,full`
//!
//! What this example prints:
//! 1. The registered SKILL.md URIs (discovery-listable resources).
//! 2. The auto-synthesized `skill://index.json` discovery URI.
//! 3. The fact that `bootstrap_skill_and_prompt(...)` registers BOTH a
//!    skill AND a parallel prompt from one `Skill` value.
//! 4. The byte length of the dual-surface text (skill body + references).
//!
//! Note on SEP-2640 §9: reference URIs (e.g.
//! `skill://code-mode/references/schema.graphql`) are addressable via
//! `resources/read` but MUST NOT appear in `resources/list` or the
//! discovery index. They are intentionally absent from the printed URI
//! list below — this is the spec-required "readable but not listable"
//! behavior, locked by the integration test in `tests/skills_integration.rs`.
//!
//! Pair with `c10_client_skills.rs` to see the full client-side flow.

use pmcp::server::skills::{Skill, SkillReference};

// Compiled-in skill bodies (matches spike 002 reference impl byte-for-byte
// — see `.planning/spikes/002-skill-ergonomics-pragmatic/src/main.rs`
// lines 391-512).
const HELLO_WORLD: &str = include_str!("skills/hello-world/SKILL.md");
const REFUNDS: &str = include_str!("skills/refunds/SKILL.md");
const CODE_MODE: &str = include_str!("skills/code-mode/SKILL.md");
const CODE_MODE_SCHEMA: &str = include_str!("skills/code-mode/references/schema.graphql");
const CODE_MODE_EXAMPLES: &str = include_str!("skills/code-mode/references/examples.md");
const CODE_MODE_POLICIES: &str = include_str!("skills/code-mode/references/policies.md");

fn build_code_mode_skill() -> Skill {
    Skill::new("code-mode", CODE_MODE)
        .with_reference(SkillReference::new(
            "references/schema.graphql",
            "application/graphql",
            CODE_MODE_SCHEMA,
        ))
        .with_reference(SkillReference::new(
            "references/examples.md",
            "text/markdown",
            CODE_MODE_EXAMPLES,
        ))
        .with_reference(SkillReference::new(
            "references/policies.md",
            "text/markdown",
            CODE_MODE_POLICIES,
        ))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let code_mode_skill = build_code_mode_skill();

    // pmcp::Server::builder() returns ServerBuilder (see src/server/mod.rs:637).
    // The skill API is wired on BOTH ServerBuilder AND ServerCoreBuilder per
    // 80-REVIEWS.md Fix 2; we use the public path here so the example
    // mirrors how a real server author would write this code.
    //
    // Tier 1: hello-world (trivial, default path = skill://hello-world/SKILL.md).
    // Tier 2: refunds (path-overridden — demonstrates skill://acme/billing/refunds/SKILL.md).
    // Tier 3: code-mode (multi-file + dual-surface bootstrap).
    //
    // The chained `.skill().skill().bootstrap_skill_and_prompt(...)` sequence
    // exercises the accumulator pattern from 80-REVIEWS.md Fix 1: every
    // call accumulates into a single pending registry, finalized once at
    // .build() time. There is no per-call wrapper nesting.
    let _server = pmcp::Server::builder()
        .name("skills-demo")
        .version("0.1.0")
        .skill(Skill::new("hello-world", HELLO_WORLD))
        .skill(Skill::new("refunds", REFUNDS).with_path("acme/billing/refunds"))
        .bootstrap_skill_and_prompt(code_mode_skill.clone(), "start_code_mode")
        .build()?;

    // For the full code-mode tool wiring (validate_code + execute_code tools),
    // see examples/s41_code_mode_graphql.rs. Inlining it here would add ~50
    // lines of unrelated GraphQL scaffolding; the cross-reference keeps this
    // example focused on the skill registration story.

    println!("Skills demo server built successfully via pmcp::Server::builder().");
    println!();
    println!("Registered SKILL.md URIs (discovery-listable):");
    println!("  skill://hello-world/SKILL.md");
    println!("  skill://acme/billing/refunds/SKILL.md");
    println!("  skill://code-mode/SKILL.md");
    println!("Also auto-synthesized: skill://index.json");
    println!();
    println!("Code-mode dual-surface registered as:");
    println!("  Skill at:  skill://code-mode/SKILL.md (+ 3 references)");
    println!("  Prompt at: start_code_mode");
    println!();
    println!(
        "Dual-surface text length: {} bytes",
        code_mode_skill.as_prompt_text().len()
    );
    println!();
    println!("Pair this with: cargo run --example c10_client_skills --features skills,full");

    Ok(())
}
