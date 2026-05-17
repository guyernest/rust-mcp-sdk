---
spike: 003
name: schema-server-surface-diff
type: standard
validates: "Given the three pmcp-run built-in core crates (sql/graphql/openapi), when their config schemas, runtime traits, code-mode handlers, and shared deps are structurally diffed, then either a shared SDK-level abstraction is visible with a concrete shape — or shown to be illusory."
verdict: PARTIAL
related: [004]
tags: [schema-server, structural-diff, builtin-servers, sdk-lift]
---

# Spike 003: schema-server-surface-diff

## What This Validates

**Given** the three pmcp-run built-in core crates
(`mcp-sql-server-core`, `mcp-graphql-server-core`, `mcp-openapi-server-core`)
that build MCP servers from a schema file + TOML configuration,
**when** their config schemas, runtime traits, code-mode handlers, and
shared dependencies are structurally diffed,
**then** either a shared SDK-level abstraction becomes visible with a
concrete shape — or it is shown to be illusory, killing the "lift to PMCP
SDK" question before it consumes more design effort.

This is the **risk-first kill-switch** for Idea 2 in the MANIFEST. If the
three cores genuinely don't share a useful shape, the rest of the
investigation (cargo-pmcp scaffolding, proc-macros, per-backend lifts)
becomes moot and we keep this functionality at pmcp-run.

## Research

The pmcp-run repo at `~/Development/mcp/sdk/pmcp-run/built-in/` was
catalogued by three structural-map agents reading each core crate in
parallel. Their outputs (config surface, connectors, tool execution, code
mode, schema layer, server bootstrap, public API, divergent axes) were
synthesized into the comparison tables in `ANALYSIS.md`.

The spike binary `src/main.rs` re-derives every claim via direct source
scanning (file existence, dependency parsing, substring presence,
top-level LoC accounting) and asserts each one with `assert!` so a future
refactor at pmcp-run cannot quietly invalidate the verdict.

No external libraries beyond `anyhow` and the path-dep on `pmcp` are
introduced — the spike is intentionally an analysis tool, not a runtime
demo. (Spike 004 is the runtime demo.)

### Approaches considered

| Approach | Tool | Pros | Cons | Status |
|----------|------|------|------|--------|
| `syn` AST parsing of each core crate | `syn` 2.0 | Precise, structural | 30 min spike turns into half-day; adds dep | rejected |
| Grep + regex over source | bash | Fast, simple | Fragile vs naming changes | rejected |
| Substring presence + dep-name parse + LoC | std-only | Cheap, asserts only what matters | Misses subtle drift | **chosen** |
| Read the three READMEs | n/a | Zero code | Documentation drifts from source | rejected |

## How to Run

From the PMCP SDK workspace root:

```bash
cargo run --manifest-path .planning/spikes/003-schema-server-surface-diff/Cargo.toml
```

Path-dep on `pmcp` keeps the spike + SDK in lock-step per CONVENTIONS.md.

## What to Expect

A multi-section structured report covering:

- **Step A** — locate sources at the four pmcp-run/built-in paths.
- **Step B** — assert that `mcp-server-common` exists and the three cores
  already depend on it (the **proto-SDK-already-extracted** finding).
- **Step C** — assert shared top-level shape: `from_toml`, `into_pmcp_server`,
  `run_lambda`, the same six per-crate `.rs` files at the top level.
- **Step D** — assert that the *divergent* axes are real: SQL has a
  multi-impl `DatabaseConnector` trait while GraphQL/OpenAPI have
  concrete reqwest-wrapped clients; `code_mode.rs` LoC spread > 2×;
  OpenAPI imports AVP/Cedar policy types the others don't.
- **Step E** — verdict block with LoC accounting and recommended shape
  for the SDK lift.

Every `assert!` either holds (spike succeeds) or fires loudly with the
exact mismatch.

## Investigation Trail

**Initial premise.** The user proposed lifting "schema-driven MCP server"
functionality from pmcp-run into the PMCP SDK as one of: SDK crates,
cargo-pmcp commands, or proc-macros. The decomposition put 003 first as
the kill-switch: are the three cores actually unifiable?

**First read.** Skimmed the three core crates' `lib.rs` files. All three
re-export a `*Config`, a server bootstrap type, a code-mode handler, and
`run_lambda`. Surface symmetry looked promising — *maybe* too good.

**Concern: code_mode.rs LoC spread.** `wc -l` on the three `code_mode.rs`
files gave 545 / 767 / **1560**. A 3× spread on the central "long tail"
component is a strong signal that the three crates have semantically
different policy surfaces, not just stylistic differences. This warranted
deeper investigation.

**Three parallel structural-map agents.** Dispatched Explore agents to
catalog each core crate's config types, connectors, tool execution, code
mode internals, public API, and explicitly the non-generalizable surface.
~750-900 words each. (Outputs are in the conversation; the synthesis is in
`ANALYSIS.md`.)

**The reframing discovery.** The agent outputs surfaced repeated references
to `mcp-server-common`. A check at `pmcp-run/built-in/shared/` confirmed it
exists as a real, ~2.2k LoC crate already extracting `AuthProvider`,
`SecretsProvider`, `StaticResourceHandler`, `StaticPromptHandler`, shared
`ResourceConfig` / `PromptConfig`, and `CODE_MODE_PROMPT_NAME`. **The
proto-SDK already exists.** The question reframes from "should we extract
an abstraction" to "should the already-extracted abstraction live in the
PMCP SDK or stay locked at pmcp-run/built-in/shared".

**Likewise `pmcp-code-mode`.** This is an SDK crate (separate from `pmcp`
but in the same family). It already owns the HMAC token machinery,
`TokenSecret`, `JsCodeExecutor`, and the `#[derive(CodeMode)]` macro. The
tool names `validate_code` / `execute_code` come from this crate, not from
the per-backend cores. Another piece of "already shared" infrastructure.

**Divergence assessment.** The genuinely-divergent axes are:
1. Backend executor — SQL needs a multi-impl trait (SQLite ≠ Athena ≠ etc),
   GraphQL/OpenAPI don't (both are HTTP over reqwest).
2. Parameter binding — `:name` vs `$var` vs `{name}+verb` — genuinely
   different in the source language.
3. Policy surface — OpenAPI's AVP/Cedar integration is real substance
   (3× LoC reflects this).

A single `SchemaServer<S, C>` trait fitting all three is not viable.
A *shared toolkit crate* feeding three independent per-backend crates
already works in production.

**Spike binary.** Wrote a std-only structural scanner that asserts every
claim above. Includes LoC-spread assertions, dep-name presence checks,
and key-symbol substring checks. ~370 LoC, single file. Fails loudly if
pmcp-run drifts.

## Results

**Verdict: PARTIAL** — reframed as **VALIDATED with a specific shape.**

| Finding | Evidence | Implication |
|---------|----------|-------------|
| Shared abstraction already exists | `mcp-server-common` 2.2k LoC + `pmcp-code-mode` SDK crate; all three cores depend on both | Lift, don't design |
| `Config::from_toml` is uniform | All three cores have this signature | Toolkit can expose a generic helper |
| `into_pmcp_server` / `build` shape is uniform | All three yield `pmcp::Result<pmcp::Server>` | Server bootstrap is consistent |
| Single `SchemaServer<S, C>` trait is **not** viable | Backend executor / param binding / policy diverge semantically | Per-backend crates remain separate |
| `code_mode.rs` LoC spread is 545 / 767 / 1560 | Direct `wc -l` | Real semantic divergence, not slop |
| OpenAPI imports AVP/Cedar | `PolicyEvaluator` references in source | Policy substance differs across cores |

### Recommended SDK shape (validated further in spike 004)

1. **Promote `mcp-server-common` to a workspace crate under `crates/`.**
   Candidate names: `pmcp-server-toolkit` or `pmcp-builtin`. Public,
   stable, on crates.io.
2. **Keep per-backend crates at pmcp-run for now.** Replace their path-dep
   on `shared/mcp-server-common` with a versioned crates.io dep on the
   new toolkit. Independent release cadence becomes possible.
3. **Add `cargo-pmcp new --kind <sql|graphql|openapi>-server`** as the
   developer-facing entry point. Scaffolds a starter Cargo.toml + main.rs
   + config.toml stub.
4. **Defer `#[pmcp::sql_server]` proc-macro.** The toolkit being public is
   the prerequisite. Macros are sugar on top, not the lift itself.

### Surprises

- The most useful finding wasn't a feasibility result — it was the
  *reframing*. "Should we extract" → "what to do with what's already
  extracted". The spike paid for itself in clarifying the question.
- The OpenAPI core's 1560-line `code_mode.rs` is **not** a smell. It's
  AVP/Cedar policy integration that the others don't have. This is the
  warning against premature unification — a "shared" trait would have to
  either omit policy (useless for OpenAPI) or include it (overweight for
  SQL/GraphQL).

### Impact on remaining spikes

- **Spike 004 reshapes slightly.** Instead of "implement a `SchemaServer`
  trait", 004 builds a thin slice of the *toolkit* — config helpers +
  resource/prompt handlers + a SQLite reference backend — sized to prove
  the lift works end-to-end with a tiny `main.rs` user surface.
- **Spikes 005/006/007 (deferred this session)** are NOT invalidated.
  Comparison spike 005a/005b makes sense once the toolkit lift is real.
  `cargo-pmcp new --kind` (006) is more attractive than before. The
  code-mode-as-Skill spike (007) connects directly to validated work
  from spikes 001/002.
