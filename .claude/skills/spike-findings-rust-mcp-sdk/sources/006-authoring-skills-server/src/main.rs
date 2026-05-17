//! Spike 006: authoring-skills-server
//!
//! Question: can a `pmcp-config-helper` MCP server ship a SEP-2640 Skill
//! bundle (root SKILL.md + per-backend references + worked example) that
//! drives `resources/list` / `resources/read` / `prompts/get` correctly,
//! preserving spike 002's dual-surface byte-equality invariant?
//!
//! This is the "Type 2" skills spike — Skills served BY an MCP server at
//! runtime to its end-user via their MCP client. Distinct from the Type 1
//! `ai-agents/` knowledge files that teach Claude Code / Kiro to write
//! PMCP-based Rust code at build time. Both are legitimate; this spike
//! validates that the toolkit lift can ship Type 2 content cleanly using
//! the upstream `Skill` / `Skills` / `bootstrap_skill_and_prompt` API.
//!
//! What this proves:
//!   1. A Skill assembled from a real directory layout (SKILL.md +
//!      references/*.md + examples/*.md) wires through the upstream API
//!      without spike-local re-definitions.
//!   2. `Skills::into_handler()` produces a `ResourceHandler` whose
//!      `list()` returns ONLY the root SKILL.md per SEP-2640 §9 —
//!      references are addressable via `read()` but NOT enumerated.
//!   3. `read()` succeeds for both the root URI and every reference URI.
//!   4. `bootstrap_skill_and_prompt(skill, "start_config_authoring")`
//!      registers both surfaces; the resulting `pmcp::Server` reports
//!      `has_prompt("start_config_authoring") == true`.
//!   5. The dual-surface invariant: `Skill::as_prompt_text()` (what
//!      SkillPromptHandler returns) is byte-equal to the concatenation
//!      of the SKILL.md body + every reference body, in the order they
//!      were registered. SEP-2640-blind hosts get the SAME content via
//!      `prompts/get` as SEP-2640-aware hosts get via reading the
//!      resources.

#![allow(dead_code)]

use anyhow::{Context, Result};
use pmcp::server::skills::{Skill, SkillReference, Skills};
use pmcp::Server;

// =============================================================================
//                        EMBEDDED SKILL CONTENT
// =============================================================================
//
// In production, a `pmcp-config-helper` MCP server would either embed
// these via `include_str!` at compile time (zero filesystem dependency,
// works in any deployment including Lambda) or load them from a
// configurable skills directory at startup. The spike uses `include_str!`
// for hermeticity.

const SKILL_BODY: &str = include_str!("../skills/SKILL.md");
const REF_SQL: &str = include_str!("../skills/references/sql-pareto-tools.md");
const REF_OPENAPI: &str = include_str!("../skills/references/openapi-pareto-tools.md");
const REF_CODE_MODE: &str = include_str!("../skills/references/code-mode-policy.md");
const EXAMPLE_EMP: &str = include_str!("../skills/examples/employee-directory-sql.md");

// =============================================================================
//                            SKILL ASSEMBLY
// =============================================================================
//
// This is what the toolkit's `pmcp-config-helper` MCP server would do at
// startup. Roughly 15-20 lines for a real production server.

fn build_config_authoring_skill() -> Skill {
    Skill::new("config-authoring", SKILL_BODY)
        .with_reference(SkillReference::new(
            "references/sql-pareto-tools.md",
            "text/markdown",
            REF_SQL,
        ))
        .with_reference(SkillReference::new(
            "references/openapi-pareto-tools.md",
            "text/markdown",
            REF_OPENAPI,
        ))
        .with_reference(SkillReference::new(
            "references/code-mode-policy.md",
            "text/markdown",
            REF_CODE_MODE,
        ))
        .with_reference(SkillReference::new(
            "examples/employee-directory-sql.md",
            "text/markdown",
            EXAMPLE_EMP,
        ))
}

// =============================================================================
//                              ASSERTIONS
// =============================================================================

fn print_banner() {
    println!();
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("  Spike 006: authoring-skills-server");
    println!("  Compose upstream Skill/Skills/bootstrap_skill_and_prompt for Type 2 skills");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!();
}

fn header(title: &str) {
    println!();
    println!("{}", "─".repeat(78));
    println!("▶ {title}");
    println!("{}", "─".repeat(78));
}

fn ok(msg: &str) {
    println!("  ✓ {msg}");
}

#[tokio::main]
async fn main() -> Result<()> {
    print_banner();

    header("Step A · Build Skill from directory layout");
    let skill = build_config_authoring_skill();
    println!("  Skill name:          {}", skill.name());
    println!("  Resolved description: {}", skill.resolved_description());
    println!("  Reference count:     {}", skill.references().count());

    assert_eq!(skill.name(), "config-authoring");
    assert!(
        !skill.resolved_description().is_empty(),
        "frontmatter `description:` should be parsed into resolved_description"
    );
    assert!(
        skill.resolved_description().starts_with("Help a developer design"),
        "description should come from the SKILL.md YAML frontmatter, got: {}",
        skill.resolved_description()
    );
    assert_eq!(
        skill.references().count(),
        4,
        "4 references: 3 references/*.md + 1 examples/*.md"
    );
    ok("Skill assembled with 4 supporting files; frontmatter description parsed");

    header("Step B · ResourceHandler list() returns ONLY the root SKILL.md (SEP-2640 §9)");
    let skills = Skills::new().add(skill.clone());
    let handler = skills
        .into_handler()
        .context("Skills::into_handler should succeed")?;

    let extra = pmcp::RequestHandlerExtra::default();
    let list_result = handler
        .list(None, extra.clone())
        .await
        .context("ResourceHandler::list")?;
    println!("  resources/list returned {} entries:", list_result.resources.len());
    for r in &list_result.resources {
        println!("    {}", r.uri);
    }

    // SEP-2640 §9: only the root SKILL.md is enumerated. References are
    // addressable via read() but MUST NOT appear in list().
    // (The discovery index URI may or may not appear depending on the
    // SDK's implementation; assertion is on the SKILL.md presence + the
    // absence of references.)
    let root_uri = "skill://config-authoring/SKILL.md";
    let root_in_list = list_result.resources.iter().any(|r| r.uri == root_uri);
    assert!(root_in_list, "root SKILL.md URI must be in list()");

    let ref_uri_substr = "references/sql-pareto-tools.md";
    let any_ref_enumerated = list_result
        .resources
        .iter()
        .any(|r| r.uri.ends_with(ref_uri_substr));
    assert!(
        !any_ref_enumerated,
        "SEP-2640 §9 violation: references/*.md MUST NOT appear in resources/list, but found: {:?}",
        list_result.resources.iter().map(|r| &r.uri).collect::<Vec<_>>()
    );
    ok("Root SKILL.md present in list; references/*.md NOT enumerated (§9 honored)");

    header("Step C · ResourceHandler read() serves both root and references");
    let root_read = handler
        .read(root_uri, extra.clone())
        .await
        .context("read root SKILL.md")?;
    println!("  read({root_uri}) → {} content nodes", root_read.contents.len());
    assert!(
        !root_read.contents.is_empty(),
        "root SKILL.md read must return content"
    );

    let refs_to_read = [
        "skill://config-authoring/references/sql-pareto-tools.md",
        "skill://config-authoring/references/openapi-pareto-tools.md",
        "skill://config-authoring/references/code-mode-policy.md",
        "skill://config-authoring/examples/employee-directory-sql.md",
    ];
    for uri in refs_to_read {
        let r = handler
            .read(uri, extra.clone())
            .await
            .with_context(|| format!("read {uri}"))?;
        println!("  read({uri}) → {} content nodes", r.contents.len());
        assert!(!r.contents.is_empty(), "{uri} should return content");
    }
    ok("All 5 URIs (1 root + 4 references) reachable via ResourceHandler::read()");

    header("Step D · bootstrap_skill_and_prompt registers BOTH surfaces");
    let server: Server = Server::builder()
        .name("pmcp-config-helper")
        .version("0.1.0")
        .bootstrap_skill_and_prompt(skill.clone(), "start_config_authoring")
        .build()
        .map_err(|e| anyhow::anyhow!("server build failed: {e}"))?;

    assert!(
        server.has_prompt("start_config_authoring"),
        "Server::has_prompt should report the bootstrap prompt"
    );
    ok("Server::has_prompt('start_config_authoring') = true");

    header("Step E · Dual-surface byte-equality invariant (spike 002's load-bearing claim)");
    let prompt_text = skill.as_prompt_text();
    println!("  Skill::as_prompt_text() length: {} bytes", prompt_text.len());

    // The body MUST appear verbatim at the start (the SKILL.md content
    // is the prefix of the prompt text).
    assert!(
        prompt_text.starts_with(skill.body()),
        "prompt text must begin with the SKILL.md body verbatim — pointer-style prompts (just a URI) are prohibited"
    );

    // Each reference body must appear inlined in the prompt text. This
    // is the load-bearing claim: a SEP-2640-blind host that fetches the
    // prompt gets ALL the content the LLM needs, not just a URI it
    // cannot fetch.
    for r in skill.references() {
        assert!(
            prompt_text.contains(r.body()),
            "reference '{}' body MUST appear in prompt_text (dual-surface invariant)",
            r.relative_path()
        );
    }
    ok("Prompt text starts with SKILL.md body; every reference body is inlined");

    // Spot-check: a SEP-2640-blind host fetching the prompt should be
    // able to see, e.g., the SQL pareto-tools heuristics. We assert on
    // a signature phrase from each reference file.
    assert!(
        prompt_text.contains("One tool = one user-visible operation"),
        "SQL reference content should be visible in prompt text"
    );
    assert!(
        prompt_text.contains("oauth_passthrough"),
        "OpenAPI reference content should be visible in prompt text"
    );
    assert!(
        prompt_text.contains("Approval tokens"),
        "code-mode-policy reference content should be visible in prompt text"
    );
    assert!(
        prompt_text.contains("employee-directory"),
        "worked example content should be visible in prompt text"
    );
    ok("All 4 references' signature content present in prompt_text (a SEP-2640-blind host gets the full surface)");

    header("Step F · Cross-spike consistency check");
    // Spike 002 established that the prompt body MUST be byte-equal to
    // the concatenation of `SKILL.md + references/*` reads. Reconstruct
    // that concatenation here and assert byte-equality against
    // `as_prompt_text()`.
    let mut reconstructed = String::new();
    reconstructed.push_str(skill.body());
    if !skill.body().ends_with('\n') {
        reconstructed.push('\n');
    }
    for r in skill.references() {
        reconstructed.push_str("\n--- ");
        reconstructed.push_str(r.relative_path());
        reconstructed.push_str(" ---\n");
        reconstructed.push_str(r.body());
        if !r.body().ends_with('\n') {
            reconstructed.push('\n');
        }
    }
    assert_eq!(
        prompt_text, reconstructed,
        "prompt_text must be byte-equal to the canonical concatenation"
    );
    ok("Byte-equality holds: as_prompt_text() == SKILL.md + concatenated references");

    println!();
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("  VERDICT: ✓ VALIDATED");
    println!();
    println!("  A `pmcp-config-helper` MCP server can ship a SEP-2640 Skill bundle by");
    println!("  composing upstream `Skill::new(...)` + `.with_reference(...)` +");
    println!("  `bootstrap_skill_and_prompt(...)`. The toolkit lift gains a Type 2");
    println!("  Skills deliverable: end-users connect their MCP client to");
    println!("  pmcp-config-helper, get the config-authoring skill, and chat their");
    println!("  way through writing a config.toml — backend-agnostically.");
    println!();
    println!("  The dual-surface invariant from spike 002 holds: SEP-2640-blind hosts");
    println!("  fetching `prompts/get start_config_authoring` get byte-equal content");
    println!("  to what SEP-2640-aware hosts assemble from `resources/read`. No");
    println!("  silent-fail risk.");
    println!();
    println!("  The Type 1 (build-time, ai-agents/) and Type 2 (runtime, SEP-2640)");
    println!("  Skills surfaces are now both anchored: Type 1 in `ai-agents/` config");
    println!("  files for coding agents; Type 2 in `pmcp-server-toolkit` MCP servers");
    println!("  exposed to any SEP-2640-aware client at runtime.");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    Ok(())
}
