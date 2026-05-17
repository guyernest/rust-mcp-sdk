# Skills — DX Layer + Dual-Surface Pattern

How PMCP exposes SEP-2640 Skills to server authors. Builder ergonomics,
multi-file directory model, and the byte-equal-by-construction
dual-surface pattern that keeps SKILL ↔ PROMPT in lockstep.

## Requirements

These are non-negotiable for any real implementation:

- **DX must mirror existing builder patterns** (`.tool(name, handler)` /
  `.resources(handler)`). New methods: `.skill(Skill)`, `.skills(Skills)`,
  and `.bootstrap_skill_and_prompt(Skill, prompt_name)`.
- **Skills must support the SEP-2640 directory model** — a skill is a
  `SKILL.md` plus zero-or-more supporting files (`references/schema.graphql`,
  `examples.md`, etc.). Supporting files are readable via `resources/read`
  but **MUST NOT** be enumerated in `resources/list` or the discovery index
  (SEP-2640 §9).
- **Composition with existing `.resources(custom)` is required.** Server
  authors with pre-existing resource handlers must not have to give them
  up to ship skills. URI-prefix routing (`skill://` → skills handler;
  everything else → user's handler) is the validated pattern.
- **Duplicate-URI rejection at build time.** `Skills::into_handler()` should
  return `Result` and surface duplicates rather than silently overwrite.
- **Dual-surface rule:** when a skill carries instructions an LLM should
  also be able to load via a prompt (for hosts that don't yet support
  SEP-2640), the prompt body MUST inline the same content — it must NOT
  redirect to the skill URI. A pointer-style prompt body silent-fails on
  SEP-2640-blind hosts (the LLM gets a literal string mentioning a URI but
  the host has no mechanism to fetch it). PMCP must ship a
  `bootstrap_skill_and_prompt(...)` method that registers both surfaces
  from one `Skill` value so they cannot drift.
- **Skills are a general primitive, not a code-mode delivery mechanism.**
  The canonical examples must include three tiers (hello-world, refunds,
  code-mode) so framing positions code-mode as one valuable application,
  not the only or main use case. The hello-world and refunds examples
  enable cross-SDK developer-experience comparison (they appear in TS SDK,
  gemini-cli, fast-agent, goose, codex reference implementations).

## How to Build It

### Step 1 — Define the user-facing types

Behind a `skills` feature flag on the `pmcp` crate. Lift the reference
implementation from `sources/002-skill-ergonomics-pragmatic/src/main.rs`
near-verbatim:

```rust
pub struct SkillReference {
    relative_path: String,  // "references/schema.graphql"
    mime_type: String,      // "application/graphql"
    body: String,
}

pub struct Skill {
    name: String,                       // from frontmatter, equals final URI segment
    body: String,                       // SKILL.md content with frontmatter
    path: Option<String>,               // override for namespaced paths
    description: Option<String>,        // override; defaults to frontmatter
    references: Vec<SkillReference>,    // supporting files
}

pub struct Skills {
    skills: Vec<Skill>,
}
```

Builder methods: `Skill::new(name, body)`, `.with_path(...)`,
`.with_description(...)`, `.with_reference(SkillReference)`.

### Step 2 — `Skills::into_handler()` flattens to two URI maps

```rust
impl Skills {
    pub fn into_handler(self) -> Result<Arc<dyn ResourceHandler>, BuildError> {
        let mut skill_md: HashMap<String, Skill> = HashMap::new();
        let mut references: HashMap<String, (String, String)> = HashMap::new();
        let mut duplicates = Vec::new();
        for skill in self.skills {
            for r in &skill.references {
                references.insert(
                    skill.reference_uri(&r.relative_path),
                    (r.mime_type.clone(), r.body.clone()),
                );
            }
            if skill_md.insert(skill.skill_md_uri(), skill).is_some() {
                duplicates.push(skill_md.keys().last().unwrap().clone());
            }
        }
        if !duplicates.is_empty() {
            return Err(BuildError::DuplicateSkills(duplicates));
        }
        Ok(Arc::new(SkillsHandler { skill_md, references }))
    }
}
```

The internal `SkillsHandler::list()` returns SKILL.md entries + the index.
`read()` checks `skill_md` first, then `references`, then errors with
`METHOD_NOT_FOUND`. Critically, `list()` and the discovery index **never**
include reference entries — per SEP-2640 §9.

### Step 3 — Builder integration with `ServerCoreBuilder`

Add three methods in `src/server/builder.rs`:

```rust
pub fn skill(self, skill: Skill) -> Self {
    self.skills(Skills::new().add(skill))
}

pub fn skills(mut self, skills: Skills) -> Self {
    let handler = skills.into_handler().expect("duplicate skills");
    // Compose with any pre-existing .resources(...) handler via URI prefix.
    match self.resources.take() {
        Some(existing) => self.resources = Some(Arc::new(ComposedResources {
            skills: handler,
            other: existing,
        })),
        None => self.resources = Some(handler),
    }
    // Set capabilities — use `extensions` once GAP #1 lands.
    self.capabilities.resources.get_or_insert_with(Default::default);
    let mut ext = self.capabilities.extensions.clone().unwrap_or_default();
    ext.insert("io.modelcontextprotocol/skills".to_string(), json!({}));
    self.capabilities.extensions = Some(ext);
    self
}

pub fn bootstrap_skill_and_prompt(
    mut self,
    skill: Skill,
    prompt_name: impl Into<String>,
) -> Self {
    // Register skill (SKILL surface).
    let skill_for_prompt = skill.clone();
    self = self.skill(skill);
    // Register prompt (PROMPT surface) whose body is byte-equal to the skill.
    self.prompt(prompt_name, SkillPromptHandler::new(skill_for_prompt))
}
```

### Step 4 — The dual-surface invariant

```rust
impl Skill {
    pub fn as_prompt_text(&self) -> String {
        let mut out = String::new();
        out.push_str(&self.body);
        if !self.body.ends_with('\n') { out.push('\n'); }
        for r in &self.references {
            out.push_str("\n--- ");
            out.push_str(&r.relative_path);
            out.push_str(" ---\n");
            out.push_str(&r.body);
            if !r.body.ends_with('\n') { out.push('\n'); }
        }
        out
    }
}
```

The `SkillPromptHandler` returns a `GetPromptResult` whose single
`PromptMessage` has `content.text == skill.as_prompt_text()`. The
SKILL surface (concatenation of `read()` results for SKILL.md + each
reference URI, in order, with the same labelled rules) is byte-equal
to this. Spike 002 STEP 5 asserts this in-binary. The real integration
test MUST reproduce that assertion.

### Step 5 — Code-mode integration (worked example)

The advanced tier demonstrates that Skills compose with another PMCP
feature without modifying that feature:

```rust
let code_mode_skill = Skill::new("code-mode", include_str!("skills/code-mode/SKILL.md"))
    .with_reference(SkillReference::new(
        "references/schema.graphql", "application/graphql",
        include_str!("skills/code-mode/references/schema.graphql")))
    .with_reference(SkillReference::new(
        "references/examples.md", "text/markdown",
        include_str!("skills/code-mode/references/examples.md")))
    .with_reference(SkillReference::new(
        "references/policies.md", "text/markdown",
        include_str!("skills/code-mode/references/policies.md")));

let builder = pmcp::Server::builder()
    // existing code-mode tools (unchanged):
    .pipe(|b| code_mode_server.register_code_mode_tools(b).unwrap())
    // skill + parallel prompt from the same data:
    .bootstrap_skill_and_prompt(code_mode_skill, "start_code_mode");
```

The `validate_code` / `execute_code` tools are unchanged. The HMAC
approval-token security model is unchanged. Only the bootstrap layer
moves from a hand-rolled prompt + scattered resources into a single
SEP-2640 skill that ALSO exposes a prompt fallback.

### Step 6 — Ship examples + integration test

Per PMCP convention (paired `sNN_` / `cNN_` examples + ALWAYS-required
example mandate from `CLAUDE.md`):

| File | Purpose | Approx size |
|---|---|---|
| `examples/s38_server_skills.rs` | Registers hello-world + refunds + code-mode skills via the builder. Demonstrates `bootstrap_skill_and_prompt` for code-mode. Mounts alongside existing code-mode tools. | ~150 lines |
| `examples/c38_client_skills.rs` | Walks both host flows: (a) SEP-2640 host enumerates skills via `skill://index.json` and reads lazily; (b) older host calls `prompts/get start_code_mode` and gets everything eagerly. Same end-state context. | ~50 lines |
| `tests/skills_integration.rs` | In-process server + client. Exercises all four SEP-2640 endpoints (list, read SKILL.md, read reference, read index). Asserts `as_prompt_text()` byte-equals concatenated `read()` content. | ~80 lines |

## What to Avoid

- **Naïve prompt-as-pointer.** A prompt body that says "read
  `skill://code-mode/SKILL.md` and follow its instructions" silent-fails on
  SEP-2640-blind hosts — the LLM gets a literal URI string but the host has
  no mechanism to fetch it. Always inline the content. This is the most
  important lesson from spike 002 iteration 8.
- **Letting the SKILL and PROMPT surfaces drift.** If you register them as
  two independent things (two function calls with two separate content
  inputs), someone will edit one and forget the other. The reference
  implementation derives both from one `Skill` value and asserts byte-
  equality in tests. Replicate this invariant.
- **Enumerating supporting files in `resources/list`.** SEP-2640 §9 is
  explicit: only SKILL.md entries + the index are listed. The
  `SkillsHandler::list()` must filter — don't lazily emit every URI in the
  flattened maps.
- **A parallel `SkillHandler` trait.** Tempting because it makes the API
  feel "first-class". But it duplicates `ResourceHandler` surface for zero
  protocol benefit (SEP-2640 has no new methods) and breaks composition
  with pre-existing resource handlers. `Skill` is data; `Skills` is a
  registry; `ResourceHandler` is the protocol-level interface. Keep them
  layered.
- **Per-skill capability advertisement.** Add the
  `extensions["io.modelcontextprotocol/skills"] = {}` flag once at the
  server level, not per skill. The flag declares "this server supports
  the skills convention" — it doesn't enumerate skills.
- **Skipping the cross-SDK comparison examples.** Spike 002 framing
  decision: keep hello-world and refunds even after introducing code-mode.
  These two skills appear in other reference implementations; dropping
  them would make PMCP DX harder to compare and learn from. Code-mode is
  the *advanced* tier, not the canonical demo.

## Constraints

- **Feature flag.** Ship the DX layer behind a `skills` feature flag on
  the `pmcp` crate so users who don't want SEP-2640 don't pull the
  transitive surface.
- **GAP #1 dependency.** The wire-correct capability advertisement
  requires the `extensions` field on `ServerCapabilities`. Until that
  lands, the DX layer's `declare_capability` uses `experimental` as a
  one-line workaround. Migrating the call site is the only change once
  GAP #1 ships.
- **Frontmatter parsing is intentionally minimal.** The reference
  implementation parses `name:` and `description:` only — that's all
  the SEP requires. Don't grow a full YAML parser.
- **Token cost framing.** Per design discussion: the bootstrap context
  (schema + examples + policies) is amortized across many code-mode
  uses. Don't optimize for "minimum SKILL.md size" — optimize for
  "complete enough to generate correct queries on the first try."
  Caching the bootstrap response is a host concern.
- **Backward-compat for hosts.** The dual-surface pattern means servers
  publishing skills today work on hosts that don't yet implement SEP-2640.
  When SEP-2640 ships in the host ecosystem, capable hosts switch to the
  skill surface; the prompt remains as a fallback. No server-side change
  required.

## Origin

Synthesized from spike: 002 (skill-ergonomics-pragmatic, VALIDATED).
Source files available in: `sources/002-skill-ergonomics-pragmatic/`.

The spike's `src/main.rs` is the reference implementation — the `Skill`,
`SkillReference`, `Skills`, `SkillsHandler`, and `ComposedResources` types
should be lifted near-verbatim into the real implementation. The in-binary
assertions (especially STEP 5's byte-equality check on the dual-surface
invariant) translate directly into the integration test.

The README's "Investigation Trail" (iterations 1–9) documents how the design
arrived at the dual-surface pattern, including the iteration 8 catch where
prompt-as-pointer was rejected as a silent-fail mode. Read it before
implementing — it captures why the design looks the way it does.
