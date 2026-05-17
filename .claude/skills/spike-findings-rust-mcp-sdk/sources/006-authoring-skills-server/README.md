---
spike: 006
name: authoring-skills-server
type: standard
validates: "Given spike 002's upstream `Skill` / `Skills` / `bootstrap_skill_and_prompt` machinery and the `pmcp` `skills` feature flag, when a `pmcp-config-helper` MCP server ships a Skill bundle (root SKILL.md + per-backend references + one worked example) for authoring `config.toml` files, then `resources/list` reports only the root SKILL.md per SEP-2640 §9, `resources/read` serves both the root and supporting references, and `prompts/get` against the dual-surface bootstrap prompt body is byte-equal to the root SKILL.md content."
verdict: VALIDATED
related: [001, 002]
tags: [skills, sep-2640, dual-surface, config-authoring, type-2-skills]
---

# Spike 006: authoring-skills-server

## What This Validates

The user clarified a vocabulary collision in earlier discussion: **two
different things both called "skills" in this project:**

- **Type 1: Build-time agent skills** — existing in `ai-agents/` (Claude
  Code, Kiro). Teach a coding agent to scaffold + generate Rust code
  for a PMCP server.
- **Type 2: Runtime SEP-2640 skills** — what spikes 001/002 validated.
  Content served BY an MCP server to its end-user's MCP client at
  runtime.

Spike 006 validates the **Type 2** half of the authoring-skills story
for the toolkit lift: a `pmcp-config-helper` MCP server can ship a
SEP-2640 Skill bundle that walks the user through authoring their
`config.toml`, composing upstream `Skill` / `Skills` /
`bootstrap_skill_and_prompt` without any spike-local re-definitions.

**Given** the upstream Skills machinery in `pmcp` (feature `skills`,
defined in `src/server/skills.rs` + `src/server/mod.rs:2687-2743` per
the spike 002 lift),
**when** a Skill is assembled from a real directory layout (SKILL.md +
references/*.md + examples/*.md) and registered via
`bootstrap_skill_and_prompt(skill, "start_config_authoring")` on a
`pmcp::Server`,
**then** the SEP-2640 §9 listing invariant + spike 002's dual-surface
byte-equality invariant both hold.

## Research

This spike is a **composition** spike — it consumes upstream APIs and
content artifacts rather than re-implementing primitives. The Skills
machinery from spikes 001/002 is already public in `pmcp` behind the
`skills` feature flag; spike 006 just exercises it with realistic Type
2 content.

Skills bundle authored for the spike:

```
skills/
├── SKILL.md                                  (root, with YAML frontmatter)
├── references/
│   ├── sql-pareto-tools.md
│   ├── openapi-pareto-tools.md
│   └── code-mode-policy.md
└── examples/
    └── employee-directory-sql.md
```

This is the production content shape the toolkit's `pmcp-config-helper`
MCP server would ship — embedded via `include_str!` so the resulting
Lambda binary is self-contained (no filesystem reads at runtime).

## How to Run

```bash
cargo run --manifest-path .planning/spikes/006-authoring-skills-server/Cargo.toml
```

## What to Expect

A six-step report:

- **Step A** — Skill assembled with 4 references; `Skill::name() ==
  "config-authoring"`; `resolved_description()` parsed from YAML
  frontmatter and starts with "Help a developer design".
- **Step B** — `Skills::new().add(skill).into_handler()` returns a
  `ResourceHandler` whose `list()` includes the root SKILL.md URI plus
  the discovery index (`skill://index.json`) but does NOT enumerate
  any `references/*.md` URIs — SEP-2640 §9 honored.
- **Step C** — `read()` succeeds for all 5 URIs (1 root + 4 references).
- **Step D** — `bootstrap_skill_and_prompt(skill, "start_config_authoring")`
  registers the prompt; `server.has_prompt("start_config_authoring")` is
  true.
- **Step E** — `Skill::as_prompt_text()` starts with the SKILL.md body
  verbatim AND inlines every reference body (no pointer-style references).
  Spot-checks confirm signature content from each reference file is
  present in the prompt text (e.g. "One tool = one user-visible
  operation", "oauth_passthrough", "Approval tokens", "employee-directory").
- **Step F** — Byte-equality with a hand-reconstructed concatenation of
  `SKILL.md + references` confirms spike 002's load-bearing invariant.

## Investigation Trail

**Initial premise.** The user's earlier "we have skills focused on
helping coding agents like Claude Code build PMCP servers" comment
revealed I'd been conflating two different Skills concepts. After the
clarification, spike 006 was scoped specifically to validate the Type 2
half (runtime SEP-2640 skills served by an MCP server), since spikes
001/002 had already validated the SDK's underlying Skills machinery but
hadn't built a server whose Skills surface is the *product* itself.

**Discovery: `SkillPromptHandler` is `pub(crate)`.** Initial design tried
to construct a `SkillPromptHandler` directly to drive `prompts/get` from
the spike. The type isn't public — by design, since users register via
`bootstrap_skill_and_prompt` which constructs it internally. The
workaround is to use the public `Skill::as_prompt_text()` method which
returns the exact same content `SkillPromptHandler` would emit. This is
sufficient because the spike's job is to validate the dual-surface
invariant (the content shape), not to re-test the SDK's PromptHandler
wiring (already covered by pmcp's internal tests).

**Discovery: discovery index URI in `list()`.** The SDK's
`SkillsHandler::list()` returns TWO entries by default: the root SKILL.md
URI AND a `skill://index.json` discovery index. SEP-2640 §9 prohibits
references from appearing in `list()` but permits the discovery index.
The spike's assertion logic correctly handles this by asserting on
*reference absence* rather than *list-size equality* — which matches
spike 001's findings about the discovery index being part of the
listing surface.

**Pleasant surprise: YAML frontmatter parsing works.** `Skill::new(name,
body)` parses `description:` from the body's YAML frontmatter at
construction time. The spike's SKILL.md uses real frontmatter and the
resolved description flows through `resolved_description()` as expected.
No manual `.with_description(...)` override needed for production
content.

**Pleasant surprise: spike came together cleanly, first try.** The
upstream Skills API surface (validated end-to-end by spike 002) is
ergonomic enough for Type 2 use that this spike compiled and all 6
assertions passed on the first run. No DX-gap findings to surface.

## Results

**Verdict: ✓ VALIDATED**

All six step assertions held. The toolkit lift gains a third deliverable
beyond `pmcp-server-toolkit` + per-backend crates:

### New deliverable: `pmcp-config-helper` MCP server

A binary that ships SEP-2640 Skills for authoring `config.toml` files.
The end-user's MCP client (Claude Desktop, ChatGPT MCP-aware tools,
Goose, fast-agent, etc.) connects to this server and gets the
`config-authoring` skill. The user chats their way through writing a
config — backend-agnostically — with the LLM consuming the per-backend
references (`sql-pareto-tools.md`, `openapi-pareto-tools.md`,
`code-mode-policy.md`) as appropriate.

### Two-axis Skills coverage

| Audience | Surface | Where it lives |
|---|---|---|
| **Developer building a server** (build-time) | Type 1 agent skills | `ai-agents/{claude-code,kiro}/` — coding agent reads, generates Rust |
| **End-user of a deployed server** (runtime) | Type 2 SEP-2640 skills | Inside `pmcp-config-helper` MCP server — served via `resources/*` + dual-surface prompt fallback |

Both layers grow together with the toolkit lift:
- Type 1 gains content teaching agents how to scaffold + extend toolkit
  deployments (`cargo pmcp new --kind sql-server`, custom-handler
  patterns, dialect selection)
- Type 2 ships as a real MCP server binary the user runs locally

### Surprises

- **No friction in the upstream API.** The spike compiled on first
  attempt and all assertions held. Spike 002's API design has aged well.
- **The discovery index (`skill://index.json`) in `list()` is the
  right shape.** It means hosts that don't speak `skill://` URIs can
  still discover the skill bundle's structure via a single read.

### Future extensions (deferred)

- Multi-skill bundles (one skill per backend kind, separate top-level
  Skills for `sql-server`, `graphql-server`, `openapi-server`). Trivial
  via `Skills::new().add(...).add(...).add(...)`.
- Interactive flows — the toolkit could expose tools like
  `validate_config_toml` and `suggest_pareto_tools_from_schema` that
  the LLM calls during the chat. These augment the static Skill content
  with runtime feedback. Out of scope for the lift spike; would be a
  Phase 2 enhancement.

### Impact

Combined with spike 005 (multi-dialect SQL trait), Phase 1 of the
toolkit lift now has THREE de-risked deliverables:
1. `pmcp-server-toolkit` core (proto-SDK extract per spike 003)
2. Per-backend crates with multi-dialect support per spike 005
3. `pmcp-config-helper` MCP server with Type 2 authoring Skills per
   spike 006

Type 1 agent skills (`ai-agents/`) updates are content authoring against
existing infrastructure — not spike-required.
