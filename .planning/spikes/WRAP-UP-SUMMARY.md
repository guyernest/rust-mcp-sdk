# Spike Wrap-Up Summary

**Dates:** 2026-05-12 (session 1), 2026-05-17 (session 2)
**Spikes processed:** 6 total (2 + 4)
**Feature areas:** 5 (Wire Protocol, DX Layer, Schema-Server Architecture, SQL Dialects, Type 2 Authoring Skills)
**Skill output:** `./.claude/skills/spike-findings-rust-mcp-sdk/`

## Processed Spikes

### Session 1 — SEP-2640 Skills support

| # | Name | Type | Verdict | Feature Area |
|---|------|------|---------|--------------|
| 001 | skills-as-resources-mapping | standard | ✓ VALIDATED (with caveats) | Wire Protocol |
| 002 | skill-ergonomics-pragmatic | standard | ✓ VALIDATED | DX Layer + Dual-Surface |

### Session 2 — Schema-server toolkit lift

| # | Name | Type | Verdict | Feature Area |
|---|------|------|---------|--------------|
| 003 | schema-server-surface-diff | standard | ⚠ PARTIAL → ✓ VALIDATED | Schema-Server Architecture |
| 004 | schema-server-thin-slice-sql | standard | ✓ VALIDATED | Schema-Server Architecture |
| 005 | multi-dialect-sql-connector | standard | ✓ VALIDATED | SQL Multi-Dialect Connectors |
| 006 | authoring-skills-server | standard | ✓ VALIDATED | Type 2 Authoring Skills |

## Key Findings

### From session 1 (Skills SEP-2640)

**Skills are wire-cheap for PMCP.** SEP-2640 layers entirely on existing
`resources/list` + `resources/read` with no new RPC methods. The PMCP
`ResourceHandler` trait is exactly the right shape.

**Two small protocol-types additions are required** (both additive):
GAP #1 — `ServerCapabilities.extensions` field (required); GAP #2 —
`Content::Resource.blob` field (optional, blocks archive distribution
only). Ship GAP #1 with the DX lift; defer GAP #2.

**The dual-surface pattern is the load-bearing design decision.** Prompt
body MUST inline the same content as the SKILL surface — pointer-style
prompts silent-fail on SEP-2640-blind hosts. PMCP ships
`bootstrap_skill_and_prompt(skill, prompt_name)` that registers both
surfaces from one `Skill` value with byte-equality asserted in-binary.

### From session 2 (Schema-server toolkit lift)

**The shared abstraction is already extracted** — `mcp-server-common`
(~2.2k LoC at `pmcp-run/built-in/shared/`) plus `pmcp-code-mode` (SDK
crate) already provide the auth/secrets/resource/prompt config layer +
HMAC token machinery + `#[derive(CodeMode)]`. All three pmcp-run backend
cores already consume both. The lift PROMOTES the proto-SDK to a
public PMCP workspace crate; it does NOT redesign or re-extract.

**A single `SchemaServer<S, C>` trait is NOT viable.** Per-backend
executors, parameter binding, and policy surfaces diverge semantically.
`code_mode.rs` LoC spread of 545 / 767 / 1560 reflects real divergence
(OpenAPI's 3× weight is AVP/Cedar + two-tier blocklist + JS sandbox,
not slop). The right shape is a public toolkit core + separate
per-backend crates.

**One `SqlConnector` trait + `Dialect` enum cleanly handles
Postgres / MySQL / Athena / SQLite.** Three methods (`dialect`,
`execute`, `schema_text`) + two free helpers (`translate_placeholders`,
`build_code_mode_prompt`) live in toolkit core. Per-backend crates own
only I/O + their dialect declaration. Adding Oracle / SQL Server /
DuckDB is a 3-step extension that does NOT touch toolkit core.

**The user-facing surface is 12 lines** (Shape C library use) or ZERO
lines (Shape A pure-config `pmcp-sql-server --config X --schema Y`
binary). The toolkit synthesizes `ToolInfo` from `[[tools]]` config
entries; the developer writes NO per-tool Rust handlers. Asserted
in-binary by spike 004 step G.

**`pmcp-config-helper` is the new Type 2 deliverable.** Spike 006
validated that an MCP server can ship SEP-2640 Skills (root SKILL.md +
references + worked examples) for `config.toml` authoring. Type 1
(build-time, in `ai-agents/`) targets coding agents writing Rust;
Type 2 (runtime, SEP-2640) targets end-users via their MCP client.
Both layers grow together with the lift.

**Pure-Rust Lambda is the deployment target.** No Docker, no
testcontainers. Per-backend crates use pure-Rust drivers
(`tokio-postgres`, `sqlx`, `aws-sdk-athena`, `rusqlite` bundled) — all
compile to Lambda binaries. Authentic in-process mocks answer
trait-design questions in spikes; real-DB integration is per-connector
crate concern.

**Two upstream DX gaps for the implementation phase to close:**
1. `pmcp::ServerBuilder` lacks `tool_arc` / `prompt_arc` (the inner
   `ServerCoreBuilder` has them) — every config-driven toolkit author
   writes a 20-line delegating wrapper shim until this is fixed.
2. `Server::handle_request` is private; external toolkit tests can
   only drive `ToolHandler::handle` directly. Either expose a public
   in-process driver or document the handler-level pattern.

## Recommended Implementation Path

### Phase 0 — Skills support (mostly already shipped)

Most of session 1's recommendations were implemented as Phase 80
(SEP-2640 Skills) — see `.planning/phases/80-sep-2640-skills-support/`.
Remaining: examples + integration tests + GAP #2 (archive distribution,
v2).

### Phase 1 — Toolkit lift (next, 1-2 weeks)

1. Upstream DX fixes: `tool_arc` + `prompt_arc` on public `ServerBuilder`.
2. `crates/pmcp-server-toolkit/` — promote `mcp-server-common`-shape.
3. SQL per-backend crates: `pmcp-toolkit-postgres`,
   `pmcp-toolkit-athena`, `pmcp-toolkit-mysql`. SQLite as feature flag.
4. `crates/pmcp-config-helper/` — Type 2 authoring Skills MCP server.
5. `pmcp-sql-server` binary (Shape A pure-config).
6. `cargo pmcp new --kind sql-server` scaffolding (Shape B).
7. Type 1 skills content updates in `ai-agents/`.

### Phase 2 — GraphQL toolkit (after Phase 1)

8. Spike GraphQL connector shape (analogous to spike 005).
9. `crates/pmcp-toolkit-graphql/`.

### Phase 3 — OpenAPI toolkit (gated by spike 007)

10. **Spike 007** (planned, not yet run): openapi-auth-policy-pluggability.
    Can ONE `PolicyEvaluator` trait serve AVP / OPA / Cedar / bespoke RBAC?
11. If verdict allows: `crates/pmcp-toolkit-openapi/`. Otherwise OpenAPI
    stays at `pmcp-run` as an advanced feature.

## Deferred for Future Spikes

- **007 openapi-auth-policy-pluggability** — gates Phase 3 OpenAPI lift.
- **008 cargo-pmcp-new-from-schema** — Shape B scaffold (low risk;
  depends on Phase 1 shipping).
- **009 schema-codemode-as-skill** — composition: code-mode bootstrap
  as SEP-2640 Skill vs current `/start_code_mode` prompt convention.
- **tasks-vertical-slice** — validate `docs/design/tasks-feature-design.md`
  architecture before scaffolding `crates/pmcp-tasks/`.
- **task-retry-expiry-gaps** — pragmatic retry/expiry policy layer
  above SEP-1686.
- **skills-describing-tasks** — composition spike for both primitives
  once Tasks ships.
