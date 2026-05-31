---
phase: quick-260528-oam
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - crates/pmcp-sql-server/README.md
autonomous: true
requirements:
  - DOC-README-SQL-SERVER
must_haves:
  truths:
    - "A reader understands pmcp-sql-server is a standalone binary that serves a production MCP server from a config.toml + schema file with no Rust written"
    - "A reader understands the improvement: config-only vs hand-writing a Rust MCP server with the SDK"
    - "A reader can build the binary and run it against the four supported backends (sqlite/postgres/mysql/athena)"
    - "A reader can copy a minimal config.toml and a CLI invocation that match the real crate (--config / --schema / --http)"
    - "A reader knows how to select a backend and how to opt out of unused connectors at build time"
  artifacts:
    - path: "crates/pmcp-sql-server/README.md"
      provides: "Crate README grounded in the real CLI, config schema, and backend set"
      min_lines: 80
      contains: "pmcp-sql-server"
  key_links:
    - from: "README CLI section"
      to: "crates/pmcp-sql-server/src/cli.rs"
      via: "flags --config/--schema/--http reflect Args exactly"
      pattern: "--config"
    - from: "README backends section"
      to: "crates/pmcp-sql-server/src/dispatch.rs"
      via: "sqlite/postgres/mysql/athena + per-backend required fields"
      pattern: "sqlite, postgres, mysql, athena"
    - from: "README config example"
      to: "crates/pmcp-sql-server/examples/sql_server_min.rs"
      via: "minimal config derived from the real inline example / reference-config.toml"
      pattern: "\\[database\\]"
---

<objective>
Write `crates/pmcp-sql-server/README.md` for the Shape A pure-config SQL MCP server binary.

Purpose: This crate is the v2.2 "Configuration-Only MCP Servers" payoff — the runnable binary on top of `pmcp-server-toolkit`. It currently ships with NO README. Downstream consumers (and crates.io) need a README that (1) explains the improvement over hand-writing an MCP server in Rust, and (2) shows exactly how to build, configure, and run it.

Output: A single new file `crates/pmcp-sql-server/README.md`, grounded in the real crate source (CLI flags, config schema, supported backends), matching the voice/structure of the sibling `crates/pmcp-server-toolkit/README.md`.
</objective>

<execution_context>
@/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.claude/get-shit-done/workflows/execute-plan.md
@/Users/guy/Development/mcp/sdk/rust-mcp-sdk/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
Read these to ground every claim in the README. Do NOT invent flags, fields, or backends — reflect exactly what these files contain.

@crates/pmcp-sql-server/Cargo.toml
@crates/pmcp-sql-server/src/cli.rs
@crates/pmcp-sql-server/src/lib.rs
@crates/pmcp-sql-server/src/dispatch.rs
@crates/pmcp-sql-server/src/main.rs
@crates/pmcp-sql-server/examples/sql_server_min.rs
@crates/pmcp-sql-server/tests/fixtures/reference-config.toml
@crates/pmcp-server-toolkit/README.md

<interfaces>
<!-- Ground truth extracted from the crate. Use these exactly; do not invent. -->

CLI surface (src/cli.rs — `Args`):
  --config <PATH>   required   server config.toml (server + [[tools]] + [database] + [code_mode] + [[resources]] + [[prompts]])
  --schema <PATH>   required   code-mode schema resource file (DDL text served as the schema resource / code-mode prompt input)
  --http <ADDR>     optional   bind address for the streamable-HTTP transport; default "127.0.0.1:8080"
  Binary name: `pmcp-sql-server`. Thin tokio::main shim delegating to `pmcp_sql_server::run`.

Pipeline (src/lib.rs — `run`):
  read --config + --schema → dispatch connector for [database] type → build_server (tools + code-mode + resources + prompts, --schema merged into the schema resource) → serve over streamable HTTP (SDK Tower/axum adapter; stateful; AllowedOrigins::localhost() default; loopback default matching --http).
  STATUS: fully implemented — `run()` is the complete pipeline. NOT a placeholder/WIP. (Do not label it WIP.)

Backends (src/dispatch.rs — `[database] type` → connector):
  "sqlite"   → SqliteConnector — requires `file_path` OR `database` (`:memory:` or a path)
  "postgres" → PostgresConnector — requires `url`
  "mysql"    → MysqlConnector — requires `url`
  "athena"   → AthenaConnector — requires `workgroup`; region from AWS_REGION / AWS_DEFAULT_REGION (default us-east-1); optional output_location, database
  Unknown type → error "supported: sqlite, postgres, mysql, athena". A type whose feature was compiled out → FeatureMissing error with "rebuild with --features <name>".

Cargo features (Cargo.toml):
  default = ["sqlite", "postgres", "mysql", "athena"]   (published binary serves any backend from config alone)
  Lean single-backend build: `--no-default-features --features sqlite`
  Crate version 0.1.0. Depends on pmcp + pmcp-server-toolkit (code-mode + sqlite) + optional per-backend connector crates.

Minimal config shape (examples/sql_server_min.rs CONFIG + reference-config.toml):
  [server] name/version/type
  [database] type + per-backend fields
  [code_mode] enabled, allow_writes, token_secret (supports "${ENV_VAR}" interpolation, e.g. ${CODE_MODE_SECRET})
  [[tools]] name/description/sql + [[tools.parameters]] (the curated "~20%" tools)
  [[resources]] / [[prompts]] (optional; reference-config shows schema/examples/learnings resources + a start_code_mode prompt)
  Pareto model: ~20% curated [[tools]], ~80% of operations via Code Mode SQL generation against the schema resource.

Design context link (relative from crates/pmcp-sql-server/README.md):
  ../../.claude/skills/spike-findings-rust-mcp-sdk/SKILL.md
</interfaces>
</context>

<tasks>

<task type="auto">
  <name>Task 1: Write the pmcp-sql-server README grounded in crate source</name>
  <files>crates/pmcp-sql-server/README.md</files>
  <action>
Create `crates/pmcp-sql-server/README.md`. Match the voice and section structure of `crates/pmcp-server-toolkit/README.md` (title, one-line summary, **Status:** line, "What this crate is", "What this crate is NOT", a Quickstart, and a "Design context" link). Use these sections, all grounded in the <interfaces> block above — derive nothing that is not in the real source:

1. **Title + one-liner.** `# pmcp-sql-server` and the crate's own description: a Shape A pure-config SQL MCP server binary — point it at a config.toml + schema and serve a production MCP server with no Rust required.

2. **Status line.** `**Status:** 0.1.0 — early access.` (version from Cargo.toml). Keep tone consistent with the toolkit README's status line. Do NOT call the pipeline WIP — `run()` is fully implemented.

3. **The improvement (why this exists).** Contrast the two approaches explicitly:
   - Status quo: to expose a SQL database over MCP you hand-write a Rust binary against the SDK — wire a `ServerBuilder`, implement tool handlers, manage the connector, wire code-mode, set up the transport, and recompile for every schema/tool change.
   - With pmcp-sql-server: you write a `config.toml` (declaring `[[tools]]`, `[database]`, `[code_mode]`, resources/prompts) plus a schema file, and run one binary. No Rust, no recompile to change tools or schema. Mention the Pareto model: ~20% curated `[[tools]]` cover common operations, ~80% of the long tail is handled by Code Mode generating SQL against the schema resource.
   Note that pmcp-sql-server is the runnable binary built on top of the `pmcp-server-toolkit` library (reference the sibling crate).

4. **What this crate is NOT.** Grounded, honest boundaries: it is the binary, not the library (that's `pmcp-server-toolkit`); it is not a DynamoDB/NoSQL toolkit (SQL backends only); it does not invent a SQL dialect — you supply the schema DDL and the backend URL/path.

5. **Supported backends.** A short table: backend (`type` value) | connector | required `[database]` fields. Reflect dispatch.rs exactly:
   - sqlite | SqliteConnector | `file_path` or `database` (`:memory:` or a path)
   - postgres | PostgresConnector | `url`
   - mysql | MysqlConnector | `url`
   - athena | AthenaConnector | `workgroup` (region from `AWS_REGION`/`AWS_DEFAULT_REGION`, default `us-east-1`; optional `output_location`, `database`)
   State that all four are compiled in by default, and a lean single-backend build uses `--no-default-features --features <backend>`; naming a backend whose feature was compiled out yields a clear "rebuild with --features <name>" error.

6. **Quickstart / how to use.** Four sub-parts:
   a. **Build/install.** Show `cargo build -p pmcp-sql-server --release` (and note `cargo install --path crates/pmcp-sql-server` is available). Use ```bash fences.
   b. **A minimal config.toml.** Provide a small, real config derived from `examples/sql_server_min.rs` CONFIG (a `[server]`, a `[database] type = "sqlite"` with `file_path`, `[code_mode]` with `token_secret = "${CODE_MODE_SECRET}"`, and one `[[tools]]` + `[[tools.parameters]]`). Use a ```toml fence. Note that the full reference config lives at `tests/fixtures/reference-config.toml` (Chinook demo).
   c. **Run it.** `pmcp-sql-server --config config.toml --schema schema.ddl` and the `--http` override (default `127.0.0.1:8080`). Mention `RUST_LOG` controls log verbosity (run() inits a tracing_subscriber EnvFilter). Use ```bash fences.
   d. **Selecting a backend.** Explain it is the `[database] type` value in the config, plus the build-feature note from section 5.

7. **Design context.** Link `[../../.claude/skills/spike-findings-rust-mcp-sdk/SKILL.md](../../.claude/skills/spike-findings-rust-mcp-sdk/SKILL.md)` and the `.planning/phases/85-*` design log, mirroring the toolkit README's closing section.

CRITICAL fencing rule (so `cargo test --doc` is unaffected): use ```toml for config, ```bash for shell, ```text for any plain output. Do NOT use a bare ```rust fence. If you show any Rust at all, it must be ```rust,ignore — but prefer NOT including Rust, since this crate is consumed as a binary + config, not as a library API.

Keep it focused and skimmable (~80–140 lines). Every flag, field, and backend MUST trace to the real source files in <context>; invent nothing.
  </action>
  <verify>
    <automated>test -f crates/pmcp-sql-server/README.md && grep -q "pmcp-sql-server" crates/pmcp-sql-server/README.md && grep -q -- "--config" crates/pmcp-sql-server/README.md && grep -q -- "--schema" crates/pmcp-sql-server/README.md && grep -Eq "sqlite.*postgres.*mysql.*athena|sqlite|postgres|mysql|athena" crates/pmcp-sql-server/README.md && ! grep -q '```rust$' crates/pmcp-sql-server/README.md && echo OK</automated>
  </verify>
  <done>
README.md exists at crates/pmcp-sql-server/README.md with: a Status line, an "improvement vs hand-written Rust server" contrast, a "What this is NOT" section, a backends table covering sqlite/postgres/mysql/athena with required fields, a Quickstart (build + minimal toml config + CLI run with --config/--schema/--http + backend selection), and a Design context link. All flags/fields/backends match the real crate source. No bare ```rust fences (only ```toml/```bash/```text, or ```rust,ignore).
  </done>
</task>

<task type="auto">
  <name>Task 2: Verify README does not break doctests and the example still builds</name>
  <files>crates/pmcp-sql-server/README.md</files>
  <action>
Confirm the new README does not introduce a compiled doctest and that the crate's runnable example (the "how to use in library form" demonstration the README references) still builds. The README is not wired as a doctest source unless a ```rust fence is present, so the main risk is an accidental rust fence — re-confirm none exist.

Run the verify commands. If the doctest run surfaces a failure traceable to the README (a stray ```rust fence), fix the fence to ```toml/```bash/```text (or ```rust,ignore) and re-run. Do not modify crate source code — only the README.
  </action>
  <verify>
    <automated>cd /Users/guy/Development/mcp/sdk/rust-mcp-sdk && ! grep -nE '^```rust$' crates/pmcp-sql-server/README.md && cargo build -p pmcp-sql-server --example sql_server_min --no-default-features --features sqlite 2>&1 | tail -5</automated>
  </verify>
  <done>
No bare ```rust fence in README.md. The `sql_server_min` example compiles (`cargo build -p pmcp-sql-server --example sql_server_min --no-default-features --features sqlite` succeeds), confirming the surrounding crate is unaffected by the doc change.
  </done>
</task>

</tasks>

<verification>
- README exists and is grounded: `--config`, `--schema`, `--http` flags present and correct; default `127.0.0.1:8080` stated; all four backends (sqlite/postgres/mysql/athena) listed with correct required fields.
- The "improvement" section contrasts pure-config against hand-writing a Rust MCP server.
- The "how to use" section includes build, a minimal real config.toml, the CLI invocation, and backend selection.
- Voice/structure matches `crates/pmcp-server-toolkit/README.md` (Status line, What this is / is NOT, Quickstart, Design context).
- No bare ```rust fence (doctest-safe).
</verification>

<success_criteria>
- `crates/pmcp-sql-server/README.md` exists, ~80–140 lines, grounded entirely in real crate source.
- A new reader can build and run the binary against any of the four backends from the README alone.
- The improvement-over-status-quo narrative is explicit and accurate.
- `cargo build -p pmcp-sql-server --example sql_server_min --no-default-features --features sqlite` still succeeds (crate unaffected).
</success_criteria>

<output>
After completion, create `.planning/quick/260528-oam-create-a-readme-for-the-new-pmcp-sql-ser/260528-oam-SUMMARY.md`
</output>
