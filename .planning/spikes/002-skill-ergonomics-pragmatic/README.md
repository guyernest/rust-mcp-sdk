---
spike: 002
name: skill-ergonomics-pragmatic
type: standard
validates: "Given the wire shape from 001, when a PMCP server author wants to register skills (including multi-file directory-model skills), they can do so with a Skill / Skills builder that mirrors register_tool_typed DX, and the same Skill data can be exposed via parallel SKILL and PROMPT surfaces that are byte-equal by construction."
verdict: VALIDATED
related: [001]
tags: [skills, dx, batteries-included, builder, dual-surface, code-mode]
---

# Spike 002: Skill Ergonomics — Pragmatic DX Layer

## What This Validates

**Given** spike 001 confirmed the SEP-2640 wire mapping works through PMCP's
existing `ResourceHandler` trait,

**when** a PMCP server author wants to register one or more skills — including
multi-file directory-model skills with supporting reference files,

**then** they should be able to do so via a `Skill` / `SkillReference` /
`Skills` builder that:

1. Mirrors the ergonomic shape of PMCP's existing `.tool(...)` / `.resources(...)`
   builder methods.
2. Auto-generates the `skill://` URI, the `skill://index.json` discovery
   resource, and the SEP-2640 capability declaration.
3. Supports the SEP-2640 directory model: `SKILL.md` + arbitrary supporting
   files under the same skill path.
4. Composes cleanly with a pre-existing `.resources(custom)` handler.
5. Produces a wire form byte-for-byte identical to the hand-rolled spike 001
   server.
6. **Exposes the same skill content via a parallel PROMPT surface** for hosts
   that don't yet support SEP-2640. The skill and the prompt are derived from
   the same `Skill` data, so they cannot drift.

## Research

No new external research — this spike builds on spike 001's findings, PMCP's
existing builder patterns in `src/server/builder.rs`, and a design discussion
that surfaced the **dual-surface requirement** described below.

The pattern to mirror:
- `.tool(name, handler)` — stores handler, auto-sets `capabilities.tools`.
- `.resources(handler)` — stores handler, auto-sets `capabilities.resources`.

The skill equivalent should:
- `.skill(Skill)` or `.skills(Skills)` — stores skills + sets
  `capabilities.experimental["io.modelcontextprotocol/skills"]`
  (workaround until GAP #1 from spike 001 lands) and `capabilities.resources`.
- `.bootstrap_skill_and_prompt(skill, prompt_name)` — registers the same
  skill data under BOTH surfaces (see "Dual-surface pattern" below).

## How to Run

```bash
cargo run --manifest-path .planning/spikes/002-skill-ergonomics-pragmatic/Cargo.toml
```

The binary prints a labelled transcript:

1. **STEP 1** — the user-facing builder code that registers three skills
   (hello-world, refunds, code-mode).
2. **STEP 2** — `resources/list` output proving wire parity with spike 001
   *and* proving supporting files are NOT enumerated in `list()` per
   SEP-2640 §9.
3. **STEP 3** — composition with a pre-existing `CompanyDocsHandler` via
   URI-prefix routing.
4. **STEP 4** — supporting-file read: `resources/read` on
   `skill://code-mode/references/schema.graphql` returns the GraphQL schema;
   a missing reference URI errors cleanly.
5. **STEP 5** — **dual-surface parity check**: the SKILL surface (concatenated
   `read()` results for SKILL.md + every reference) and the PROMPT surface
   (`Skill::as_prompt_text()`) are asserted byte-equal.
6. **STEP 6** — edge-case probe for duplicate URIs (silent overwrite is
   wrong UX; the real impl should error).

All correctness claims are backed by in-binary `assert!` calls so regressions
fail loud.

## The Three Skill Tiers

The demo registers three skills representing different complexity levels —
all using the same `Skill::new(...)` API:

| Tier | Skill | Purpose |
|---|---|---|
| 1. Onboarding | `skill://hello-world/SKILL.md` | The trivial case. Mirrors the canonical "hello world" example in other SEP-2640 reference implementations (TS SDK, gemini-cli, fast-agent, goose, codex). Lets developers compare PMCP DX side-by-side with other SDKs. |
| 2. Realistic | `skill://acme/billing/refunds/SKILL.md` | Canonical SEP-2640 example. Demonstrates namespaced paths (the SKILL.md lives under a multi-segment path) and a realistic business workflow. Also used in other reference implementations — included for cross-SDK comparison. |
| 3. Advanced | `skill://code-mode/SKILL.md` + 3 reference files | Demonstrates the multi-file directory model AND the dual-surface pattern. The skill bootstraps PMCP's existing **code-mode** feature: tells the LLM how to use `validate_code` / `execute_code`, points at the schema reference, the canonical query patterns, and the policies. |

Tier 3 is deliberately positioned as "Skills is a general primitive — code-mode
is just one valuable application of it," not as "code-mode is the main use case."

## The Dual-Surface Pattern

The most important design decision in this spike. Surfaced from a design
discussion: a PMCP server that publishes a skill should ALSO publish a parallel
MCP prompt that carries the same content, for hosts that don't yet understand
SEP-2640.

### Why redirection doesn't work

A naïve design has the prompt body be a one-liner pointing at the skill URI:

> "Read `skill://code-mode/SKILL.md` and follow its instructions."

This fails silently on SEP-2640-blind hosts. The LLM receives a literal string
mentioning a URI but the host has no mechanism to fetch it; the LLM may not
know to call `resources/read` itself; even if it does, the host has to surface
the result. Brittle and unobservable.

### What works: same data, two surfaces

Both the skill and the prompt are derived from the same `Skill` value:

```
┌────────────────────────────────────────┐
│  Source of truth: a single Skill value │
│  - recipe.md  (SKILL.md body)          │
│  - schema.graphql                      │
│  - examples.md                         │
│  - policies.md                         │
└──────────┬─────────────────────────────┘
           │
   ┌───────┴───────────────┐
   │                       │
   ▼                       ▼
SKILL surface          PROMPT surface
(SEP-2640)             (every MCP host)
   │                       │
   ▼                       ▼
skill://code-mode/     /start_code_mode
  SKILL.md             prompts/get response:
  references/            ├── message: recipe.md content
    schema.graphql       ├── message: schema.graphql inlined
    examples.md          ├── message: examples.md inlined
    policies.md          └── message: policies.md inlined

  Host calls               Host calls
  resources/read           prompts/get
  per file as needed       (one round-trip, all pre-loaded)

  SEP-2640 hosts: ✓        Every MCP host: ✓
```

`Skill::as_prompt_text()` (the spike's reference implementation) builds the
prompt body by concatenating the SKILL.md plus each reference file in order,
with labelled rules between them. STEP 5 of the spike asserts that this prompt
text is byte-equal to what an SEP-2640 host would assemble by reading each
URI individually.

### Why this matters

- **No drift.** A future edit to the recipe or any reference updates both
  surfaces automatically. There is no way to update one and forget the other.
- **Universal reachability.** Every MCP host can `prompts/get start_code_mode`
  today, even without SEP-2640 support.
- **Spec-aligned discovery for capable hosts.** SEP-2640 hosts get the
  lazy-load model with discovery via `skill://index.json`.
- **Backward-compatible.** When SEP-2640 ships in the host ecosystem, servers
  don't need to change anything; capable hosts just start using the skill
  surface instead of the prompt.

## Integration With Code-Mode (Worked Example)

`pmcp-code-mode` registers two tools — `validate_code` and `execute_code` —
that enable LLM-generated GraphQL/SQL/JS queries to be cryptographically
validated and executed. The LLM needs schema + examples + policy context
*before* it can usefully generate code. Today that context is loaded via a
`/start_code_mode` prompt convention.

A SEP-2640 skill is exactly that bootstrap recipe formalized:

```rust
let code_mode_skill = Skill::new("code-mode", CODE_MODE_SKILL_MD)
    .with_reference(SkillReference::new(
        "references/schema.graphql", "application/graphql", schema))
    .with_reference(SkillReference::new(
        "references/examples.md", "text/markdown", examples))
    .with_reference(SkillReference::new(
        "references/policies.md", "text/markdown", policies));

// Real PMCP builder method (to be added):
builder
    .skill(code_mode_skill.clone())                          // SEP-2640 surface
    .prompt("start_code_mode", code_mode_skill.as_prompt());  // host fallback
```

Or, in one call:

```rust
builder.bootstrap_skill_and_prompt(code_mode_skill, "start_code_mode")
```

The `validate_code` / `execute_code` tools themselves don't change. Skills only
replace the *bootstrap* layer — the security model (HMAC-signed approval tokens
binding code to user/session/expiry) is untouched.

### Why this is a high-value integration

- **Token-efficient overall.** Code-mode's whole point is that a single
  `execute_code` call replaces N tool calls' worth of intermediate token
  processing. The bootstrap context (schema + examples + policies) is
  amortized across many code-mode uses, so the upfront load is cheap.
- **Agent-driven vs. user-driven discovery.** Today, code-mode requires the
  user to know to type `/start_code_mode`. With skills, the agent enumerates
  `skill://index.json`, sees `code-mode`, and can decide on its own that the
  user's request needs code-mode rather than a curated tool.
- **Multi-language servers.** A server with both GraphQL and SQL code-mode
  becomes two skills (`skill://code-mode-graphql`, `skill://code-mode-sql`)
  rather than two slash commands.

## Investigation Trail

### Iteration 1 — straight builder
Built `Skill` / `Skills` with a single-file model and a HashMap registry.
Verified wire output matched spike 001 exactly.

### Iteration 2 — frontmatter description fallback
Added auto-parsing of `description:` from SKILL.md frontmatter so users
don't have to specify it twice.

### Iteration 3 — namespaced paths
Added `Skill::with_path()` for nested skill paths like
`skill://acme/billing/refunds/SKILL.md`.

### Iteration 4 — composition probe
Built `ComposedResources` to validate that skills compose with pre-existing
`.resources(custom)` handlers via URI-prefix routing. Pattern belongs inside
the builder in the real impl.

### Iteration 5 — duplicate-URI edge case
Surfaced that silent overwrite on duplicate URIs is wrong UX. The real impl
should reject duplicates at `Skills::into_handler()` time and return `Result`.

### Iteration 6 — three-tier sample skills
Added hello-world (trivial) + refunds (canonical cross-SDK example) + code-mode
(advanced, multi-file). Driven by the realization that Skills are a general
primitive, not a code-mode delivery mechanism — the demo should reflect that.

### Iteration 7 — multi-file directory model
Extended `Skill` to carry `Vec<SkillReference>` for supporting files. The
internal `SkillsHandler` now flattens skills + references into two URI maps:
SKILL.md entries (listed) and reference entries (readable but not listed).
This matches SEP-2640 §9: the discovery index lists SKILL.md URIs only;
supporting files are addressable via `resources/read` but not enumerated.

### Iteration 8 — dual-surface refinement (the design discovery)
The original design had the `/start_code_mode` prompt point at the skill URI.
Caught during review: this silent-fails on SEP-2640-blind hosts because the
LLM has no host-supported mechanism to fetch the URI. Redesigned:
- The prompt body INLINES the same content the skill exposes lazily.
- `Skill::as_prompt_text()` derives the prompt body from the same data the
  SKILL surface reads from.
- STEP 5 of the spike binary asserts byte-equality between the two surfaces
  so drift is impossible.

This is the most important design decision in the spike — it's why the
real impl should provide `bootstrap_skill_and_prompt(...)` as a single
call rather than asking users to register both surfaces independently.

### Iteration 9 — verdict review
All Given/When/Then properties hold. The dual-surface invariant is the
new load-bearing claim; it's asserted in-binary, not just documented.

## Results

**Verdict: VALIDATED.**

### What works
- `Skill::new(name, body)` + optional `.with_path(...)` /
  `.with_description(...)` / `.with_reference(...)` is sufficient to express
  every spike-001 case and the multi-file SEP-2640 directory model in
  ~5 lines per skill.
- `Skills::new().add(...)` registry + `.into_handler()` produces an
  `Arc<dyn ResourceHandler>` ready for the existing `.resources_arc(...)`.
- `Skills::declare_capability(&mut caps)` is the one place that knows about
  the `experimental` workaround; when GAP #1 from spike 001 lands, this
  single function changes.
- `ComposedResources` demonstrates that skills + pre-existing resources
  compose cleanly via URI-prefix routing.
- Synthesized wire output is byte-for-byte identical to spike 001 for the
  SKILL.md + index path.
- Supporting files (SEP-2640 §4 directory model) round-trip via
  `resources/read` and are correctly excluded from `list()` and the
  discovery index per §9.
- **`Skill::as_prompt_text()` produces content byte-equal to the SKILL
  surface.** The spike's STEP 5 asserts this — both surfaces cannot drift
  because both are derived from the same `Skill` value.

### Open design questions (deferred to the real-impl phase)

| # | Question | Working preference |
|---|---|---|
| Q1 | How should `Skills::into_handler()` handle duplicate SKILL.md URIs? | Return `Result`, list duplicates. Silent overwrite is too surprising. |
| Q2 | `register_skill(...)` accepting `&Path` to read SKILL.md from disk? | v2 — couples with a future `#[pmcp::skill]` macro for compile-time validation. v1: `include_str!(...)` is sufficient. |
| Q3 | Where should this code live? | Inside `pmcp` core, gated behind a `skills` feature flag. |
| Q4 | Should `#[pmcp::skill]` macro be a follow-on spike? | Yes if compile-time SKILL.md validation is wanted. Not blocking. |
| Q5 | How should `bootstrap_skill_and_prompt(...)` shape its prompt response? | Spike emits a single `user` message with the concatenated body. Real impl may want one message per file (recipe + each reference) — design call during impl, doesn't change the byte-equal invariant. |

### Recommended path to production

1. Land **GAP #1** from spike 001 (add `extensions: Option<HashMap<...>>`
   to `ServerCapabilities`). One-line additive change.
2. Add `Skill` / `SkillReference` / `Skills` to `pmcp` behind a `skills`
   feature flag. Reference implementation in this spike.
3. Add `.skill(Skill)` / `.skills(Skills)` to `ServerCoreBuilder` with
   internal composition over any pre-existing `.resources(...)` handler.
4. Add `.bootstrap_skill_and_prompt(skill, prompt_name)` that registers
   the skill AND a corresponding prompt whose handler returns the
   inlined content (single source of truth, two surfaces).
5. Make `into_handler()` reject duplicate URIs (Q1).
6. Ship the example pair:
   - `examples/s38_server_skills.rs` — registers all three tier skills
     (hello-world, refunds, code-mode) AND a `start_code_mode` prompt
     bound to the code-mode skill. Demonstrates the full DX.
   - `examples/c38_client_skills.rs` — walks both flows side-by-side:
     SEP-2640 host enumerates skills lazily; older host invokes the
     prompt and gets everything eagerly. Same end-state LLM context.
7. Integration test asserting both surfaces produce byte-equal content
   for a representative multi-file skill (the dual-surface invariant).
8. Defer archive distribution (GAP #2 from spike 001) and the
   `#[pmcp::skill]` macro to v2.

Approximately a 1–2 day implementation given the spike contains the
working reference.
