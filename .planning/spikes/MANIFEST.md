# Spike Manifest

## Ideas

### Idea 1: SEP-2640 Skills support [VALIDATED — see spikes 001, 002]

Review the experimental MCP Skills extension (SEP-2640, "Skills Over MCP") and
evaluate how the PMCP SDK can support it pragmatically. The PMCP SDK's
"batteries included" stance means: if there is a credible new primitive in the
spec, we want server authors to reach for it via a small, ergonomic API rather
than hand-roll the wire format. The companion question for Tasks (SEP-1686) is
deferred to a later session — the in-repo `docs/design/tasks-feature-design.md`
already covers that primitive in depth, and the user prioritized Skills first
because the spec is genuinely new and the DX shape is unknown.

### Idea 2: Schema-driven configuration-only MCP servers

The `pmcp-run` service ships three "built-in server" core crates
(`mcp-sql-server-core`, `mcp-graphql-server-core`, `mcp-openapi-server-core`)
that let server authors build a complete MCP server from a *schema file*
(OpenAPI / SQL DDL / GraphQL SDL) plus a *configuration file* (TOML). The
config defines the pareto ratio of curated tools (each with a backend-specific
script or query), while the rest of the schema flows through a code-mode layer
for the long tail. The shape is intentional — it's not a 1:1 schema → tools
auto-conversion, it's a curated DX. **Question:** should this capability be
lifted into the PMCP SDK (so anyone outside pmcp.run can build such servers),
exposed via `cargo-pmcp` scaffolding commands, expressed via Rust macros, or
left as a pmcp.run-only advanced feature? Risk-first: prove (or disprove)
that a shared abstraction is real (003), then validate the smallest viable
SDK lift (004).

## Requirements

(Updated as spikes progress. Non-negotiable for the real build.)

- **Text-mode SKILL.md serving must work without a new `SkillHandler` trait.**
  Spike 001 confirmed that `ResourceHandler` is sufficient. The DX layer in
  spike 002 must be sugar over `ResourceHandler`, not a parallel trait.
- **`ServerCapabilities` needs an `extensions: Option<HashMap<String, Value>>`
  field** (parallel to `experimental`) to declare SEP-2640 support wire-correctly.
  This is a one-line additive change to `src/types/capabilities.rs:51` and is
  independent of any DX work.
- **Archive distribution (SEP-2640 §4, `application/gzip` + base64 blob) is
  out of scope for v1.** PMCP `Content::Resource` has no `blob` field today;
  the SEP marks archive distribution as optional. Ship text-mode skills first.
- **Skill registration must compose with `.resources(...)`** — spike 002
  proved URI-prefix routing inside the builder is the right composition
  pattern. The `Skill` / `Skills` DX must not require the user to give up
  their existing resource handler.
- **`Skills::add()` should error on duplicate URIs** at `.build()` time
  rather than silently overwriting. Spike 002 surfaced silent overwrite as
  the only meaningful UX paper-cut in the DX design.
- **Skills must support the SEP-2640 directory model** — a skill is a
  `SKILL.md` plus zero-or-more supporting files (`references/schema.graphql`,
  `examples.md`, etc.). Per SEP-2640 §9, supporting files are addressable via
  `resources/read` but MUST NOT be enumerated in `resources/list` or the
  discovery index. Spike 002 validated this layout end-to-end.
- **Dual-surface rule.** When a skill carries instructions an LLM should also
  be able to load via a prompt (for hosts that don't yet support SEP-2640),
  the prompt surface MUST inline the same content — it must NOT redirect to
  the skill URI. A pointer-style prompt body silent-fails on SEP-2640-blind
  hosts: the LLM gets a literal string mentioning a URI but the host has no
  mechanism to fetch it. PMCP should ship a `bootstrap_skill_and_prompt(...)`
  builder method that registers both surfaces from one `Skill` value so they
  cannot drift. Spike 002 asserts byte-equality between the surfaces in-binary.
- **Skills are a general primitive, not a code-mode delivery mechanism.** The
  canonical demo includes three tiers (hello-world, refunds, code-mode) so
  the example surface matches other SEP-2640 reference implementations and
  the framing positions code-mode as one valuable application — not the only
  or main use case.

### From Idea 2 (Schema-driven config servers):

- **The shared abstraction is already extracted.** `mcp-server-common`
  (~2.2k LoC at `pmcp-run/built-in/shared/`) plus `pmcp-code-mode` (SDK
  crate) already provide `AuthProvider`, `SecretsProvider`,
  `StaticResourceHandler`, `StaticPromptHandler`, `CODE_MODE_PROMPT_NAME`,
  HMAC token machinery, and the `#[derive(CodeMode)]` macro. All three
  backend cores consume both. Spike 003 confirmed this structurally.
- **No single `SchemaServer<S, C>` trait.** The per-backend executor,
  parameter binding, and policy surface diverge semantically:
  `:name`→`?` vs `$var` vs `{name}`+verb; LIMIT enforcement vs root-field
  allowlist vs HTTP-verb policy + AVP/Cedar. The `code_mode.rs` LoC spread
  (545 / 767 / 1560) reflects real divergence, not implementation slop.
  Any "lift to SDK" must NOT try to unify the per-backend executors behind
  one trait.
- **Promote `mcp-server-common` to `crates/`** (candidate name
  `pmcp-server-toolkit` or `pmcp-builtin`) as the actionable SDK lift.
  Public, stable, on crates.io. Per-backend crates (sql/graphql/openapi)
  stay at pmcp-run but replace their path-dep on `shared/` with a
  versioned crates.io dep on the new toolkit — unblocking independent
  release cadence.
- **`cargo-pmcp new --kind <sql|graphql|openapi>-server` is the right
  developer-facing layer** for this idea. It scaffolds a starter
  `Cargo.toml` + `main.rs` + `config.toml` against the public toolkit
  plus a chosen backend crate. No new runtime abstraction required.
- **`#[pmcp::sql_server]` / `#[pmcp::openapi_server]` proc-macros are
  deferred.** The toolkit being public on crates.io is the prerequisite
  — without it, the macro would expand to types nobody can depend on.
- **Public `ServerBuilder` needs `tool_arc` + `prompt_arc`.** Spike 004
  hit this directly — the user-facing `pmcp::ServerBuilder`
  (`src/server/mod.rs:1741`) only exposes `.tool(name, impl ToolHandler)`
  and `.prompt(name, impl PromptHandler)` (by value), forcing every
  config-driven toolkit author to write a 20-line delegating-wrapper
  shim to share an `Arc<Handler>` between the builder and an
  in-process handler map. `ServerCoreBuilder` (`src/server/builder.rs:203`)
  already has the arc variants; lift them to the public builder.
- **Per-backend connector trait MUST expose `schema_text()`** (or
  equivalent). The code-mode bootstrap prompt body needs a schema
  description to seed the LLM with the long-tail surface; spike 004
  surfaced this naturally when wiring the prompt handler. SQLite impl
  can hand back the seed schema blob; production impls introspect
  `sqlite_master`-style metadata.
- **`Server::handle_request` is private** — external toolkit code
  cannot drive a built `pmcp::Server` in-process. Either expose a
  public `in_process` driver, or document handler-level testing
  (the pattern spike 002 + spike 004 both used) as the recommended
  way to test a config-driven toolkit.

## Spikes

| # | Name | Type | Validates | Verdict | Tags |
|---|------|------|-----------|---------|------|
| 001 | skills-as-resources-mapping | standard | A PMCP server can publish a SEP-2640 Skill via existing `resources/*` primitives, and a representative client can discover + load it with no protocol-extension code. | ✓ VALIDATED (with caveats) | skills, sep-2640, resources, wire-protocol |
| 002 | skill-ergonomics-pragmatic | standard | A PMCP server author can register a skill with a `register_skill(...)` builder (or `#[pmcp::skill]` macro) that mirrors the `register_tool_typed` DX — no hand-rolling URIs, mime types, or naming. | ✓ VALIDATED | skills, dx, batteries-included, builder |
| 003 | schema-server-surface-diff | standard | Given the three pmcp-run built-in core crates (sql/graphql/openapi), when their config schemas, runtime traits, code-mode handlers, and shared deps are structurally diffed, then either a shared SDK-level abstraction is visible with a concrete shape — or shown to be illusory. | ⚠ PARTIAL → reframed ✓ VALIDATED (shared abstraction already extracted at `mcp-server-common` + `pmcp-code-mode`; single per-backend trait NOT viable; lift mcp-server-common to crates/) | schema-server, structural-diff, builtin-servers |
| 004 | schema-server-thin-slice-sql | standard | Given the shared abstraction surfaced by 003, when a minimal SDK-level schema-server primitive is implemented with a SQLite reference connector, then a tiny `config.toml` + `schema.sql` + ~15-line `main.rs` produces a runnable MCP server end-to-end, validating tools/list, tools/call, and the code-mode bootstrap surface. | ✓ VALIDATED (user-facing surface 12 LoC; toolkit slice 346 LoC; SQLite backend 110 LoC; 0 per-tool Rust handlers; 2 DX gaps surfaced upstream) | schema-server, sdk-lift, sqlite, dx |

**Deferred (not in this session):**

- `tasks-vertical-slice` — validate the existing `tasks-feature-design.md`
  architecture compiles + runs end-to-end before scaffolding `crates/pmcp-tasks/`.
- `task-retry-expiry-gaps` — pragmatic retry/expiry policy layer above SEP-1686.
- `skills-describing-tasks` — composition of the two primitives in an agent
  workflow.
