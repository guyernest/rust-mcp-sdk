---
created: 2026-05-18T04:06:41.159Z
title: Ship SQLite-from-config example as Phase 86 Shape B/C dogfood
area: tooling
phase: 86
files:
  - cargo-pmcp/src/templates/sqlite_explorer.rs:1
  - cargo-pmcp/src/commands/add.rs:79
  - cargo-pmcp/src/commands/add.rs:112
  - pmcp-run/built-in/sql-api/servers/open-images/config.toml
---

## Problem

The existing `cargo pmcp add --template sqlite-explorer` ships a 526-line
hand-coded Rust template (`cargo-pmcp/src/templates/sqlite_explorer.rs`)
that uses `TypedTool::new(...)` + `rusqlite::Connection` for three tools
(`execute_query`, `list_tables`, `get_sample_rows`) plus workflow prompts
via `WorkflowBuilder`. This contradicts v2.2's value prop ("build
production-grade SQL MCP servers from configuration + schema files alone
— no Rust required") for the SQLite case.

We have a reference config-driven SQL server (open-images, Athena-backed)
at `pmcp-run/built-in/sql-api/servers/open-images/config.toml` (394 lines)
that demonstrates the target shape: `[server]`, `[metadata]`, `[database]`,
`[[database.tables]]`, `[code_mode]`, `[[tools]]` with `sql = """ ... :param ..."""`
named bindings, `[[prompts]]` with `include_resources`.

A SQLite analog would be ~50 lines of TOML driving `pmcp-sql-server`
(Phase 85) with `code_mode.enabled = true`, replacing the 526-line Rust
crate while still working on local dev + Lambda + Cloud Run via the same
`pmcp::assets` path resolution.

## Solution

Ship three deliverables across Phase 86 (depends on Phase 84 SQL connectors
+ Phase 85 `pmcp-sql-server` binary landing first):

1. **Shape B scaffold:** `cargo pmcp add --template sqlite-explorer-config`
   that drops a `config.toml` + 15-line `main.rs` (Shape C wiring) +
   `Cargo.toml` pinning `pmcp-server-toolkit` + `pmcp-sql-server` +
   `pmcp-sql-sqlite`. Lives alongside the existing `--template sqlite-explorer`
   (Rust escape-hatch) so users can pick TOML-driven vs Rust-driven.

2. **Shape C example:** `examples/sqlite_from_config.rs` — the ≤15-line
   dogfood proving config-only deploy: `pmcp_sql_server::run("config.toml").await`.

3. **Wire-up in `cargo-pmcp/src/commands/add.rs`:**
   - `add.rs:79 print_template_details()` — new branch for
     `"sqlite-explorer-config"` → new printer
   - `add.rs:112 print_sqlite_explorer_template_details()` — sibling
     `print_sqlite_explorer_config_template_details()`
   - `add.rs:174 print_try_it_out()` — new branch with `cargo run` + a
     sample query against the seeded DB

### TOML sketch (verified shape from open-images reference)

```toml
[server]
id = "sqlite-explorer"
name = "SQLite Explorer"
type = "sql-api"
version = "1.0.0"

[metadata]
display_name = "SQLite Explorer"
tags = ["sqlite", "sql", "demo"]
visibility = "public"

[database]
type = "sqlite"                  # Phase 84 — new Dialect variant
path = "${SQLITE_DB_PATH}"
read_only = true
query_timeout_ms = 5000

[database.pool]
max_connections = 1              # SQLite single-writer; honest cap for demo
connection_timeout_seconds = 5

[code_mode]
enabled = true                   # ← v2.2 selling point
allow_writes = false
allow_deletes = false
allow_ddl = false
require_limit = true
max_limit = 100
auto_approve_levels = ["low"]
token_ttl_seconds = 300
token_secret = "${CODE_MODE_SECRET}"

[[tools]]
name = "list_tables"
description = "List all user tables in the database."
sql = "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name"

[[tools]]
name = "get_sample_rows"
description = "Sample the first N rows from a table."
sql = "SELECT * FROM :table LIMIT :limit"   # see open issue 1 below

[[prompts]]
name = "start_code_mode"
description = "Load context for ad-hoc SQL exploration."
include_resources = ["docs://sqlite-explorer/schema", "code-mode://instructions", "code-mode://policies"]
```

### Three design inputs this sketch surfaces (forward to dependent phases)

1. **Phase 84 — `SqlConnector` identifier substitution.** `:table`
   cannot bind through SQL params (drivers bind values, not identifiers).
   The hand-coded template solves this with `JsonSchema`-validated string
   interpolation. The `SqlConnector` trait needs an explicit
   identifier-substitution path distinct from value binding, with a
   safe-identifier validator (e.g., `[A-Za-z_][A-Za-z0-9_]*` + reserved-word
   reject). Athena dodges this by enumerating tables in static SQL.

2. **Phase 85 — `[database.seed]` block.** The hand-coded template ships
   a sample DB via `pmcp::assets`. The config-driven version needs either
   `[database.seed] path = "..."` (relative to template root, packaged
   `.db` or `.sql` bootstrap) or rely entirely on user-supplied
   `${SQLITE_DB_PATH}` (better for the docs-driven demo flow).

3. **Phase 86 — two templates, not one.** Don't remove
   `--template sqlite-explorer` when shipping `--template sqlite-explorer-config`.
   They serve different users: Rust-driven (customizable, escape-hatch)
   vs TOML-driven (the v2.2 default). `add.rs` needs both printers.

### Cross-references

- Phase 86 ROADMAP block: `.planning/ROADMAP.md:1440` (Shapes B/C/D scope)
- Phase 84 dependency: SQL connectors + Dialect enum (SqlConnector trait)
- Phase 85 dependency: `pmcp-sql-server` binary that loads `config.toml`
- Reference shape: `pmcp-run/built-in/sql-api/servers/open-images/config.toml`
- Existing template: `cargo-pmcp/src/templates/sqlite_explorer.rs` (526 lines)
- Captured during: Phase 82 /simplify pass

## Resolution (2026-05-30) — SUPERSEDED + pointable example shipped

The literal `add --template sqlite-explorer-config` deliverable was SUPERSEDED by
Phase 86's generalized `cargo pmcp new --kind sql-server` (config-driven,
SQLite-capable, covers all SQL dialects) plus the toolkit Shape-C example
`crates/pmcp-server-toolkit/examples/sql_server_http.rs`. Building the one-off
template now would duplicate that generalized path.

The one genuinely-missing piece was a *pointable* SQLite config (the SQL analog of
the london-tube / contoso OpenAPI examples). The v2.9.0 release prep added a
runnable example that ships with the crate (only `tests/` is excluded):

- `crates/pmcp-sql-server/examples/sqlite-explorer.toml` (Shape-A config: `[database]
  type=sqlite` + curated `list_books` / `books_by_author` tools + Code Mode policy)
- `crates/pmcp-sql-server/examples/sqlite-explorer.sql` (idempotent seed, dual-purpose
  as the `--schema` Code Mode resource)
- Run recipe added to `crates/pmcp-sql-server/README.md`

Verified end-to-end: `sqlite3` seed + the real `pmcp-sql-server` binary boots and
serves against the example (`streamable-HTTP server listening`). Closed during the
v2.9.0 release prep.
