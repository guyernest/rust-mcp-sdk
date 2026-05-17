---
spike: 004
name: schema-server-thin-slice-sql
type: standard
validates: "Given the shared abstraction surfaced by 003, when a minimal SDK-level schema-server primitive is implemented with a SQLite reference connector, then a tiny `config.toml` + `schema.sql` + ~15-line `main.rs` produces a runnable MCP server end-to-end, validating tools/list, tools/call, and the code-mode bootstrap surface."
verdict: VALIDATED
related: [003]
tags: [schema-server, sdk-lift, sqlite, dx, toolkit]
---

# Spike 004: schema-server-thin-slice-sql

## What This Validates

**Given** the shared abstraction surfaced by spike 003 (a toolkit layer
that owns config loading + resource/prompt handlers + a per-backend
`*Connector` trait, with per-backend executors staying separate),
**when** a minimal inline slice of that toolkit is implemented with a
SQLite reference connector,
**then** a tiny `config.toml` + `schema.sql` + ~15-line `main.rs`
produces a runnable PMCP server end-to-end, where:

- TOML config deserializes into typed `*Config` structs cleanly
- Each `[[tools]]` entry becomes a real `pmcp` tool with synthesized
  `ToolInfo` (name, description, JSON Schema for inputs)
- Parameter binding (`:name` placeholders) works against the real DB
- Missing required parameters produce a clean validation error
- The code-mode bootstrap prompt is registered with the canonical
  `"start_code_mode"` name and includes the schema in its body
- The developer writes ZERO per-tool Rust handlers

## Research

Built on spike 003's verdict. The Research section of spike 003
catalogued the shape of `mcp-server-common` (~2.2k LoC, the proto-SDK
already extracted at `pmcp-run/built-in/shared/`) and the per-backend
divergence between SQL, GraphQL, and OpenAPI cores. Spike 004 takes the
"toolkit + per-backend trait" shape from that analysis as its working
hypothesis and tries to refute it by building a minimum viable instance.

PMCP API surfaces consulted before coding:

- `pmcp::Server::builder()` returns `pmcp::ServerBuilder` (at
  `src/server/mod.rs:1741`, NOT the `ServerCoreBuilder` from
  `src/server/builder.rs`). This was a non-obvious trap — see the
  Investigation Trail for the friction it caused.
- `pmcp::server::ToolHandler` (`src/server/mod.rs:224`) requires
  `async fn handle(args, extra) -> Result<Value>` plus an optional
  `fn metadata() -> Option<ToolInfo>`.
- `pmcp::server::PromptHandler` (`src/server/mod.rs:238`) is the same
  shape for prompts.
- Non-exhaustive types (`CallToolResult`, `GetPromptResult`,
  `PromptMessage`, `PromptInfo`) require named constructors per
  CONVENTIONS.md (`CallToolResult::new(content)`,
  `PromptInfo::new(name).with_description(...)`, etc).

No external libraries beyond CONVENTIONS baseline + `toml` + `rusqlite`
(bundled feature). `rusqlite` is the ONE new dep the spike pulls in —
justified because the question is specifically whether the toolkit
composes with a real DB backend, not a mock.

### Approaches considered

| Approach | Variant | Pros | Cons | Status |
|----------|---------|------|------|--------|
| Real DB (rusqlite, bundled) | chosen | Proves end-to-end binding | Adds dep | **chosen** |
| Mock connector (`Vec<Row>`) | considered | Zero deps | Misses parameter-binding semantics — the very thing under test | rejected |
| Full toolkit crate (`crates/pmcp-server-toolkit/`) | considered | Realistic | Premature — the spike's job is to PROVE the shape, not ship it | rejected |
| Use `Server::handle_request` for wire-level testing | attempted | True end-to-end | Method is private (no `pub`); only test-mod can call it | rejected |
| Handler-level in-process assertions | chosen | Matches spike 002 pattern; reaches everything that matters | Skips JSON-RPC dispatch (already covered by pmcp's own tests) | **chosen** |

## How to Run

From the PMCP SDK workspace root:

```bash
cargo run --manifest-path .planning/spikes/004-schema-server-thin-slice-sql/Cargo.toml
```

First run pulls `rusqlite` + `toml` (~1 minute warm compile, ~3 minutes
cold). Subsequent runs are <1 second.

## What to Expect

A seven-step report:

- **Step A** — config.toml parses, schema.sql seeds the in-memory DB,
  `SchemaServer::new(...).into_pmcp_server_with_handlers()` succeeds.
- **Step B** — exactly 2 handlers registered (matching 2 `[[tools]]`
  entries). `ToolInfo` for `get_employee_by_id` has the expected name,
  description, and a JSON Schema with `id: integer` marked required.
- **Step C** — `Server::has_tool()` reports both tools as present and
  rejects an unregistered name.
- **Step D** — `get_employee_by_id(id=3)` returns exactly one row
  ("Alan Turing"); `list_employees_by_department(department='Research')`
  returns 4 rows ordered by salary descending (Knuth 220k first).
  Wire-format JSON payload printed.
- **Step E** — `get_employee_by_id({})` (missing required `id`) returns
  a clean `pmcp::Error::Validation("missing required parameter id")`.
- **Step F** — `Server::has_prompt("start_code_mode")` is true; the
  prompt body includes `CREATE TABLE employees` (so the LLM gets the
  long-tail surface in one fetch).
- **Step G** — `run_user_facing_main` is **12** meaningful lines of
  code (target: ≤15). The toolkit slice is 346 LoC; the SQLite backend
  is 110 LoC; everything else is assertions.

## Investigation Trail

**Initial premise.** Spike 003 reframed the original "lift schema-driven
MCP server support to PMCP SDK" question as "promote the already-extracted
proto-SDK (`mcp-server-common`) to `crates/`". Spike 004 takes that
recommendation and tries to refute it with a minimum viable inline
toolkit slice that actually compiles and runs.

**API surface discovery 1: two `ServerBuilder`s.** First compile attempt
used `builder.tool_arc(name, Arc<dyn ToolHandler>)` and
`builder.prompt_arc(name, Arc<dyn PromptHandler>)`. Compiler said no.
Investigation revealed there are TWO `ServerBuilder` types in the
codebase:

- `pmcp::server::builder::ServerCoreBuilder` at `src/server/builder.rs`
  — has `tool_arc` (line 203) and a corresponding `prompt_arc` for
  resource/prompt arc registration.
- `pmcp::ServerBuilder` at `src/server/mod.rs:1741` — what
  `pmcp::Server::builder()` returns. Has `tool(name, handler: impl
  ToolHandler + 'static)` only.

This is **public-API DX feedback for the real lift**: the user-facing
`ServerBuilder` should mirror the `ServerCoreBuilder`'s arc-registration
methods. The toolkit needs to register handlers that the spike binary
ALSO holds for in-process testing — `tool_arc(name, Arc<dyn ToolHandler>)`
would let one Arc serve both. Without it, the spike uses a delegating
wrapper (`ToolHandlerArc(Arc<SqlToolHandler>)` impl `ToolHandler` →
delegates to the inner Arc). It works, but it's a 20-line shim every
toolkit author would re-write. Logged as a DX requirement in the
MANIFEST.

**API surface discovery 2: non-exhaustive types.** First handler
implementation tried `CallToolResult { content: ..., is_error: false,
... }` struct-literal. Compiler said no — `#[non_exhaustive]` per
CONVENTIONS.md. Switched to `CallToolResult::new(content)` (which sets
`is_error: false` by default) and equivalent constructors for
`GetPromptResult::new`, `PromptMessage::user`, and
`PromptInfo::new(name).with_description(...)`. This was already
documented in CONVENTIONS.md (Tools & Libraries section) — the spike
just confirmed CONVENTIONS got it right.

**API surface discovery 3: `Server::handle_request` is private.** Tried
to drive `tools/list` and `tools/call` via the in-process
`server.handle_request(id, request, None)` path that the SDK's own
test-mod uses (test at `src/server/mod.rs:3705`). Method is `async fn`
without `pub`, so external code cannot call it. Fell back to driving
the handlers directly via `pmcp::server::ToolHandler::handle(handler,
args, extra).await` — same pattern spike 002 used for skills. Works,
and covers what matters (handler logic, parameter binding, output
shape). The wire-level dispatch is already covered by pmcp's existing
internal tests so re-testing it from a spike would be redundant.

**SQLite parameter binding.** `rusqlite::Statement::parameter_index(":name")`
returns the 1-based position for a named placeholder. The toolkit
iterates declared parameters in `ToolDecl.parameters` order, calls
`raw_bind_parameter(idx, sql_value)` for each, and gracefully skips
parameters whose names don't appear in the SQL. Parameters not present
in `args` are bound as NULL if optional, or produce a validation error
if required (`pmcp::Error::validation("missing required parameter id")`).
This matches the semantics `mcp-sql-server-core/src/tools.rs:89-115` uses
in production, modulo prepared-statement caching (a future-toolkit
optimization, not a correctness concern).

**User-facing surface measurement.** The spike binary contains a small
LoC counter that reads its own source at runtime, extracts the
`run_user_facing_main` function body, strips comments and blank lines,
and asserts the count is ≤15. Final number: **12 meaningful lines**.
The "no per-tool Rust handlers" claim from the spike's `validates`
clause is asserted, not just claimed.

**Surprise: the schema_text method.** I added `SqlConnector::schema_text()`
to the trait so the connector can emit a string for the code-mode
bootstrap prompt body. This wasn't in the original sketch — it
surfaced as a need when wiring the prompt handler. The SQLite reference
impl just hands back the schema blob it was seeded with; a production
impl would introspect `sqlite_master`. Worth promoting to a CONVENTION:
**every per-backend connector trait MUST expose a method that yields a
schema description suitable for prompt-body inclusion**, since the
code-mode primitive depends on it.

## Results

**Verdict: ✓ VALIDATED**

All 7 step assertions held. The smallest viable toolkit slice runs
end-to-end against real SQLite. The user-facing surface is 12 lines of
code, no per-tool Rust handlers. The recommendation from spike 003
holds: lift `mcp-server-common`-shape to `crates/pmcp-server-toolkit/`
plus a per-backend connector trait + reference impl.

### Key metrics

| Metric | Value | Notes |
|--------|-------|-------|
| User-facing `main.rs` LoC | 12 | Target was ≤15 |
| Toolkit slice LoC | 346 | Would be larger in production (output_schema, widgets, output transforms, prepared statement caching, telemetry) |
| SQLite backend LoC | 110 | Minimal; production impl adds pooling, retries, schema introspection |
| Number of per-tool Rust handlers user writes | 0 | The "config-driven" claim is real |
| External crate additions vs spike 001/002 baseline | 2 (`toml`, `rusqlite`) | `rusqlite` justified by the question; `toml` is a leaf utility |

### Refined SDK lift shape (for the implementation phase)

1. **`crates/pmcp-server-toolkit/`** (or `pmcp-builtin/`)
   - `SchemaServerConfig`, `ServerSection`, `ToolDecl`, `ParamDecl`,
     `CodeModeSection` deserializers
   - `Config::from_toml(&str)` + `from_file(&Path)`
   - `SqlConnector` trait + `SqlToolHandler<C: SqlConnector>` per-tool
     handler that synthesizes `ToolInfo` from `ToolDecl`
   - `CodeModePrompt<C: SqlConnector>` (uses `connector.schema_text()`)
   - `SchemaServer<C>` top-level type with `.into_pmcp_server()` and
     `.into_pmcp_builder()` for the "custom-handlers-too" escape hatch
   - Re-export `mcp-server-common`'s auth/secrets/resource/prompt
     helpers under the toolkit's roof (the proto-SDK is what gets
     wrapped, not rewritten)
2. **`crates/pmcp-toolkit-sqlite/`** OR a feature flag `sqlite` on the
   toolkit crate
   - `SqliteConnector` impl, `bundled` rusqlite feature
3. **`ServerBuilder::tool_arc(name, Arc<dyn ToolHandler>)` and
   `prompt_arc(name, Arc<dyn PromptHandler>)`** added to the public
   `pmcp::ServerBuilder` at `src/server/mod.rs:1741`. These are needed
   because shared `Arc<Handler>` is the natural shape for
   config-driven toolkits.
4. **`cargo pmcp new --kind sql-server`** scaffolds:
   - `Cargo.toml` depending on `pmcp` + `pmcp-server-toolkit` + the
     chosen backend crate (or feature)
   - `src/main.rs` — the same 12-line shape this spike validated
   - `config.toml` stub with 1-2 example `[[tools]]` entries
   - `schema.sql` placeholder

### Surprises

- **`Server::handle_request` is private**, so external code (or future
  toolkit authors) cannot drive a built server in-process. The handler-
  level workaround works for testing but couples test code to the
  toolkit's internal handler types. The SDK might want to expose an
  `in_process` driver for this case (or document the handler-level
  pattern as the recommended way).
- **`tool_arc` / `prompt_arc` missing from the public `ServerBuilder`**
  forced a 20-line delegating-wrapper shim. This will hit every
  toolkit author. Worth fixing as a one-line upstream addition.
- **`schema_text` on the connector trait** wasn't in the initial
  sketch but emerged naturally when wiring the code-mode prompt. The
  toolkit's connector trait should require it from day one.

### Impact

- **Spike 003's recommendation is reinforced.** The thin-slice runs and
  the user-facing surface is tiny. The "lift `mcp-server-common` to
  `crates/`" plan is viable.
- **Two DX gaps for the implementation phase to close**: arc-based
  handler registration on the public `ServerBuilder`, and either an
  in-process driver or documented handler-level testing pattern.
- **Spike 005 (DX comparison: macro vs config-only)** still makes sense
  as a follow-up. The config-only path is now proven; whether a macro
  on top is *worth* the build-time complexity is a separate question.
- **Spike 006 (`cargo-pmcp new --kind X`)** is now obvious — the
  scaffold template is whatever this spike has, with placeholders.
- **Spike 007 (code-mode-as-Skill)** can re-use the same skeleton,
  swapping the code-mode prompt for a `Skill` + `bootstrap_skill_and_prompt`
  call. The composition with spikes 001/002 work is straightforward.
