# Schema-Server Toolkit — Type 2 Authoring Skills

This blueprint specifies the `pmcp-config-helper` MCP server: a binary
the toolkit ships that exposes SEP-2640 Skills for authoring
`config.toml` files. End-users connect their MCP client (Claude
Desktop, ChatGPT MCP-aware tools, Goose, fast-agent, etc.) and chat
their way through writing a config — backend-agnostically.

This is a SEPARATE deliverable from the toolkit core and per-backend
connector crates. Distinct from the Type 1 agent skills in
`ai-agents/{claude-code,kiro}/` (which are build-time scaffolding
content for coding agents).

## Requirements

These build on top of the Skills requirements already captured in
`skills-wire-protocol.md` and `skills-dx-layer.md` — read those first.

- **The vocabulary distinction is non-negotiable.** Documentation
  must clearly distinguish:
  - **Type 1 (build-time):** coding-agent skills in `ai-agents/`
    (Claude Code, Kiro). Used by a developer who is writing PMCP code.
  - **Type 2 (runtime, SEP-2640):** skills served BY an MCP server at
    runtime. Used by an end-user via their MCP client.
- **`pmcp-config-helper` ships as a pure-Rust binary.** Same
  deployment story as the toolkit — Lambda-suitable, no Docker.
  Skill content is embedded via `include_str!` at compile time so
  the binary is self-contained.
- **Composes upstream `Skill` / `Skills` / `bootstrap_skill_and_prompt`
  without redefinition.** The Skills API is already public behind the
  `skills` feature flag (from spike 002's lift into the SDK). Type 2
  consumers use it as a library; they do not re-implement the wire
  surface.
- **The dual-surface invariant MUST hold** for every Skill the
  config-helper exposes. `Skill::as_prompt_text()` is byte-equal to
  `SKILL.md` body + concatenated reference bodies. SEP-2640-blind
  hosts fetching the bootstrap prompt get the same content
  SEP-2640-aware hosts get from `resources/read`.
- **Skill bundles use real YAML frontmatter** in their `SKILL.md`.
  `Skill::new(name, body)` parses `description:` from frontmatter
  automatically; no manual `.with_description(...)` override is needed
  for production content.
- **`resources/list` MUST include the discovery index URI**
  (`skill://index.json`) but MUST NOT enumerate `references/*.md`.
  Per SEP-2640 §9. The SDK handles this automatically via
  `Skills::into_handler()`.

## How to Build It

### Skill bundle layout

```
crates/pmcp-config-helper/
├── Cargo.toml                         # binary crate, depends on pmcp (skills feature)
├── src/main.rs                        # ~50 LoC: assemble Skill + run server
└── skills/
    ├── SKILL.md                       # root, with YAML frontmatter
    ├── references/
    │   ├── sql-pareto-tools.md
    │   ├── openapi-pareto-tools.md
    │   ├── graphql-pareto-tools.md
    │   └── code-mode-policy.md
    └── examples/
        ├── employee-directory-sql.md
        ├── stripe-api-openapi.md
        └── shopify-graphql.md
```

`include_str!` pulls each `.md` file into the compiled binary so the
Lambda artifact is hermetic.

### SKILL.md frontmatter (required)

```markdown
---
name: config-authoring
description: Help a developer design a config.toml for a PMCP schema-server toolkit deployment, applying the Pareto principle to curated tools and code-mode policy.
---

# PMCP Config Authoring

[the rest of the skill body]
```

`Skill::new("config-authoring", SKILL_BODY)` parses `description:` from
the frontmatter at construction time — no `.with_description(...)`
needed.

### Skill assembly (the load-bearing ~15 lines)

```rust
use pmcp::server::skills::{Skill, SkillReference};

const SKILL_BODY: &str = include_str!("../skills/SKILL.md");
const REF_SQL: &str = include_str!("../skills/references/sql-pareto-tools.md");
const REF_OPENAPI: &str = include_str!("../skills/references/openapi-pareto-tools.md");
const REF_GRAPHQL: &str = include_str!("../skills/references/graphql-pareto-tools.md");
const REF_CODE_MODE: &str = include_str!("../skills/references/code-mode-policy.md");
const EXAMPLE_EMP: &str = include_str!("../skills/examples/employee-directory-sql.md");

fn build_config_authoring_skill() -> Skill {
    Skill::new("config-authoring", SKILL_BODY)
        .with_reference(SkillReference::new(
            "references/sql-pareto-tools.md", "text/markdown", REF_SQL))
        .with_reference(SkillReference::new(
            "references/openapi-pareto-tools.md", "text/markdown", REF_OPENAPI))
        .with_reference(SkillReference::new(
            "references/graphql-pareto-tools.md", "text/markdown", REF_GRAPHQL))
        .with_reference(SkillReference::new(
            "references/code-mode-policy.md", "text/markdown", REF_CODE_MODE))
        .with_reference(SkillReference::new(
            "examples/employee-directory-sql.md", "text/markdown", EXAMPLE_EMP))
}
```

### Server construction (composes `bootstrap_skill_and_prompt`)

```rust
#[tokio::main]
async fn main() -> Result<()> {
    let skill = build_config_authoring_skill();
    let server = pmcp::Server::builder()
        .name("pmcp-config-helper")
        .version(env!("CARGO_PKG_VERSION"))
        .bootstrap_skill_and_prompt(skill, "start_config_authoring")
        .build()?;
    server.run_lambda().await   // or .run_stdio() for local dev
}
```

That's it. `bootstrap_skill_and_prompt` registers BOTH surfaces from
one `Skill` value — the SEP-2640 resource surface AND a byte-equal
prompt named `start_config_authoring` for SEP-2640-blind hosts.

### Skill content authoring guide

Each `references/*.md` is a curated reference for one backend kind.
Structure that worked in spike 006:

1. **Tool design heuristics** (the Pareto principle, intent-based
   parameter naming, bounded result sizes, dialect-aware authoring)
2. **TOML shape** with a representative example
3. **Backend-specific gotchas** (verb semantics for OpenAPI; root-field
   allowlist for GraphQL; LIMIT enforcement for SQL)

Each `examples/*.md` is a complete worked example showing the
schema → intents → config.toml progression for one realistic system.

### Type 1 ↔ Type 2 cross-references

The toolkit lift should also update Type 1 `ai-agents/` content with
references to Type 2 surfaces:

- `ai-agents/claude-code/mcp-developer.md` — add a section "Helping a
  user adopt the toolkit" that points Claude Code to launch
  `pmcp-config-helper` locally for the user, OR to use Type 1 content
  to scaffold a Cargo project that talks to the toolkit at runtime.
- Same for `ai-agents/kiro/`.

Two skills layers, one product story: build-time scaffolding (Type 1)
+ runtime config curation (Type 2).

## What to Avoid

- **Don't construct `SkillPromptHandler` directly from outside the SDK.**
  It's `pub(crate)` by design. Use `bootstrap_skill_and_prompt` on the
  builder, or call `Skill::as_prompt_text()` if you need the prompt
  content for testing.
- **Don't write pointer-style prompts.** A prompt body that says "Read
  the skill at `skill://config-authoring/SKILL.md` for full context"
  silent-fails on SEP-2640-blind hosts — the LLM gets a URI it cannot
  fetch. The dual-surface invariant exists to prevent this.
- **Don't ship Skill content from a filesystem read at runtime.**
  Embed via `include_str!` — Lambda binaries are static, and runtime
  filesystem reads add deployment complexity for zero benefit.
- **Don't conflate Type 1 and Type 2 in documentation.** They look
  similar (both are "skills for an AI") but target different audiences
  and run at different times. Always distinguish.
- **Don't enumerate references/*.md in `resources/list`.** SEP-2640 §9
  prohibits this. The SDK's `SkillsHandler` handles it automatically;
  do not work around it.
- **Don't try to make the Skill content interactive at the API level.**
  Skill content is static (curated Markdown). For runtime feedback
  (validate a draft config, suggest pareto tools from a schema), ship
  a separate TOOL (e.g. `validate_config_toml`) — that's the right
  layer for dynamic behavior.

## Constraints

- **`pmcp` `skills` feature flag must be enabled** in the consumer's
  Cargo.toml: `pmcp = { ..., features = ["skills"] }`.
- **`Skill::with_reference` panics on invalid relative paths** —
  empty, contains `..`, starts with `/`, contains `://`, exactly
  `"SKILL.md"`, or duplicates an existing reference. Use
  `try_with_reference` for fallible/dynamic registration.
- **`Skill::as_prompt_text()` format** is fixed by the SDK:
  - `<SKILL.md body>\n`
  - For each reference: `\n--- <relative_path> ---\n<body>\n`
  - This is the dual-surface canonical form. Reconstruction is
    deterministic (asserted by spike 006 step F).
- **The discovery index URI is `skill://index.json`** by default.
  Per the SDK's `SkillsHandler::list()`. Hosts that need it for
  navigation can fetch it; hosts that don't can ignore it.
- **MIME type for Markdown content is `text/markdown`.** Used in
  `SkillReference::new(path, mime_type, body)`.

## Origin

Synthesized from:
- **006 authoring-skills-server** — `pmcp-config-helper` MCP server
  shipping SEP-2640 Skill bundle for config authoring. Verdict:
  VALIDATED on first compile. Source files:
  `sources/006-authoring-skills-server/`.

Cross-references:
- **001 skills-as-resources-mapping** (`references/skills-wire-protocol.md`)
  — the wire-level SEP-2640 contract this server honors.
- **002 skill-ergonomics-pragmatic** (`references/skills-dx-layer.md`)
  — the upstream `Skill` / `Skills` / `bootstrap_skill_and_prompt` API
  this server consumes.
- **003 schema-server-surface-diff + 004 schema-server-thin-slice-sql**
  (`references/schema-server-architecture.md`) — the toolkit this
  authoring-skills server is a deliverable OF.
