---
spike: 005
name: multi-dialect-sql-connector
type: standard
validates: "Given a single `SqlConnector` trait + `Dialect` enum, when the toolkit is driven by ONE `config.toml` with canonical `:name` placeholders against authentic in-process mocks for Postgres ($1,$2 + information_schema), MySQL (? + information_schema), Athena (? + Glue catalog), and a real SQLite, then dialect translation, schema introspection, and dialect-aware code-mode prompt bodies all flow through the trait without per-backend specifics leaking into toolkit core."
verdict: VALIDATED
related: [003, 004]
tags: [schema-server, sql-dialect, postgres, athena, mysql, connector-trait]
---

# Spike 005: multi-dialect-sql-connector

## What This Validates

The user pushed back on spike 004's SQLite-as-reference framing: real
deployments target remote Postgres / Aurora / Athena / MySQL, each with
its own SQL dialect (placeholder syntax, schema introspection, identifier
quoting, LIMIT semantics). Spike 005 answers: **does a single
`SqlConnector` trait + `Dialect` enum cleanly accommodate this divergence,
or do dialect specifics force per-dialect connector crates with
non-uniform shapes?**

**Given** one `SqlConnector` trait with three methods (`dialect()`,
`execute(sql, named_params)`, `schema_text()`) plus a 4-variant `Dialect`
enum,
**when** the SAME `config.toml` (with canonical `:name` placeholders) is
driven through authentic in-process mocks for Postgres, MySQL, and
Athena, plus a real SQLite connector carried over from spike 004,
**then** all four dialects:
- Return identical row results for the same inputs
- See dialect-correct translated placeholder syntax (`$1` for Postgres,
  `?` for MySQL/Athena, `:name` identity for SQLite)
- Emit dialect-styled schema descriptions (information_schema for PG,
  `SHOW CREATE TABLE`/backticks for MySQL, Glue Data Catalog for Athena,
  CREATE TABLE for SQLite)
- Receive dialect-aware code-mode bootstrap prompt bodies

## Research

Per the user's correction (now captured in [[feedback_avoid_docker_pure_rust_lambda]]),
this spike does NOT use Docker or `testcontainers-rs`. PMCP's deployment
target is pure-Rust AWS Lambda binaries; Docker adds CI fragility unrelated
to the trait-design question and excludes Lambda. Authentic in-process
mocks that model each dialect's wire behavior answer the question without
requiring infrastructure.

Mocks model:
- **Postgres** — `:name` → `$1, $2, ...` translation; `information_schema`-style
  schema description; positional bind list assembly from named params
- **MySQL** — `:name` → `?` translation; `SHOW CREATE TABLE`-style schema
  with backtick identifiers and InnoDB engine markers; positional bind
  list
- **Athena (Presto)** — `:name` → `?` translation; Glue Data Catalog-style
  schema with S3 output location semantics; positional bind list
- **SQLite** — real `rusqlite` (bundled, pure-Rust); native `:name`
  placeholders so toolkit translation is identity-on-SQLite

Real wire-level integration for Postgres/MySQL/Athena uses pure-Rust
drivers (`tokio-postgres`, `sqlx`, `aws-sdk-athena`) — all Lambda-compatible.
That layer is per-connector-crate responsibility, not toolkit core, and
intentionally NOT in this spike's scope.

## How to Run

```bash
cargo run --manifest-path .planning/spikes/005-multi-dialect-sql-connector/Cargo.toml
```

## What to Expect

A six-step report:

- **Step A** — `translate_placeholders(dialect, sql)` produces:
  - Postgres: `:a, :b, :a` → `$1, $2, $3` with positional order `["a","b","a"]`
  - MySQL: `:a, :b` → `?, ?` with order `["a","b"]`
  - Athena: `:a` → `?` with order `["a"]`
  - SQLite: identity translation
- **Step B** — Same config.toml drives all four backends:
  - `get_employee_by_id(id=3)` returns 1 row ("Alan Turing") on all four
  - `list_employees_by_department(department='Research', limit_n=2)` returns
    2 rows ordered by salary DESC (Knuth 220k, Turing 210k) on all four
- **Step C** — Each mock saw native placeholder syntax in its executed SQL.
  Postgres mock observed `$1`, MySQL/Athena observed `?`, SQLite passed
  through as `:id`.
- **Step D** — `schema_text()` carries the dialect's signature shape:
  PG `information_schema`, MySQL `SHOW CREATE TABLE`/backticks, Athena
  Glue Data Catalog/S3 output location, SQLite seed blob.
- **Step E** — Code-mode prompt bodies include dialect name + the
  dialect-specific placeholder guidance (e.g. "Bind parameters using
  `$1, $2, ...` (1-indexed positional)" for Postgres).
- **Step F** — Trait surface assessment: 3 methods total; dialect-specific
  code is contained in `Dialect`'s methods + `translate_placeholders`
  helper (both in toolkit core). Per-backend crates only own I/O.

## Investigation Trail

**Initial premise.** User pushed back on SQLite as the production
reference. Real production SQL backends are remote (PG/Aurora/Athena/MySQL),
each with its own dialect, and the toolkit's `SqlConnector` trait from
spike 004 hadn't been pressure-tested against dialect divergence.

**Trait shape candidates.** Three options considered:
1. **Trait per dialect** (`PostgresConnector`, `MysqlConnector`, ...) —
   max flexibility, max fragmentation; the toolkit's tool handler would
   need to know which trait it's holding.
2. **One trait + `Dialect` enum** — middle ground; trait stays uniform,
   dialect-aware behavior pushed into enum methods + free helpers.
3. **One trait + per-dialect impls of associated types** — most "Rusty"
   but adds generics throughout the toolkit's tool handler and pushes
   complexity onto every per-tool handler.

Option 2 chosen. It keeps the trait small (3 methods), centralizes
dialect-aware code in `Dialect`'s impl + `translate_placeholders` (both
toolkit-owned), and per-backend crates only own I/O.

**Mock authenticity.** Each mock's SQL routing was deliberately simple
(string-matches against the two queries in `config.toml`) since the
spike's question is "does the trait shape hold?", not "does the SQL
engine work?". But each mock DOES capture its dialect's true behavior
for the dimensions that matter:
- Placeholder syntax (`$1` vs `?`)
- Identifier quoting in schema text (`"name"` vs backticks vs unquoted)
- Schema introspection format (information_schema vs SHOW vs Glue)
- Backend-specific config knobs (Athena's S3 output location)

**API friction discovered.** First compile failed because `SqlConnector`
wasn't in scope where I called `.execute()` on the concrete `Arc<PostgresMock>`
for step C's introspection. One-line fix: `use toolkit::SqlConnector;`
at the top of `main`. Not a DX concern for the real toolkit — users
holding `Arc<dyn SqlConnector>` would have the trait in scope naturally.

**Pure-Rust constraint paid off.** Avoiding Docker meant the spike runs
in <2s (warm cache) or ~60s (cold) and works on any dev machine. The
mocks model the dialect specifics the trait must accommodate; real
wire-level integration is a per-connector crate concern using pure-Rust
drivers that all compile to Lambda binaries.

## Results

**Verdict: ✓ VALIDATED**

All six step assertions held. The `SqlConnector` trait shape from spike
004 holds up under multi-dialect pressure.

### Key trait surface

```rust
#[async_trait]
pub trait SqlConnector: Send + Sync + 'static {
    fn dialect(&self) -> Dialect;
    async fn execute(
        &self,
        sql_with_named_placeholders: &str,
        named_params: &[(String, Value)],
    ) -> Result<Vec<Value>>;
    async fn schema_text(&self) -> Result<String>;
}

pub enum Dialect { Postgres, MySql, Athena, Sqlite }
```

### Where dialect-specific code lives

| Concern | Location | Pattern |
|---|---|---|
| Placeholder translation | `toolkit::translate_placeholders(Dialect, sql)` | Free helper. Per-backend crates call it inside their `execute` impl. |
| Placeholder guidance for prompts | `Dialect::placeholder_guidance() -> &str` | Const-fold per variant. |
| Prompt body shape | `toolkit::build_code_mode_prompt(Dialect, &str)` | Free helper. |
| Schema introspection format | `*Connector::schema_text()` impl | Per-backend. The toolkit doesn't try to normalize this — the LLM gets the dialect's native shape. |
| I/O + transport | `*Connector::execute()` impl | Per-backend (tokio-postgres / sqlx / aws-sdk-athena / rusqlite). |

### Extending to Oracle / SQL Server / DuckDB

Three steps:
1. Add a `Dialect::Oracle` (or `::SqlServer` / `::DuckDb`) variant
2. Add a translation rule in `translate_placeholders` (Oracle uses
   `:name` natively, SQL Server uses `@p1`, DuckDB uses `?`)
3. Ship a new connector crate (e.g. `crates/pmcp-toolkit-oracle/`) that
   returns the new `Dialect` from `dialect()`

**Toolkit core does not change.** This is the load-bearing
extensibility claim.

### Implications for the SDK lift

- The `SqlConnector` trait from spike 004 promotes cleanly to the public
  toolkit (`crates/pmcp-server-toolkit`) with the `Dialect` enum + the
  two free helpers (`translate_placeholders`, `build_code_mode_prompt`).
- **The first wave of per-backend crates** should be (in order):
  `pmcp-toolkit-postgres` (via `tokio-postgres`), `pmcp-toolkit-athena`
  (via `aws-sdk-athena`), `pmcp-toolkit-mysql` (via `sqlx`). All
  pure-Rust, all Lambda-suitable.
- **SQLite stays as a dev/CI/test backend** behind a `sqlite` feature
  flag on the toolkit, not as a published per-backend crate.
- Real-DB integration tests for each per-backend crate are the
  per-crate's responsibility (using pure-Rust drivers against optional
  live infrastructure), not the toolkit's CI path.

### Surprises

- **The trait is smaller than I expected.** Three methods. The temptation
  was to push dialect-specific work into method *parameters* (e.g.
  `execute(&self, translated_sql, positional_args)`). Resisting that
  kept the toolkit's tool handler dialect-agnostic — it always hands
  the connector named-placeholder SQL + named-params, and translation
  happens inside the connector's `execute` impl. That's the right
  factoring.
- **Per-dialect prompt guidance is one of the highest-value features.**
  Without it, the LLM gets the schema but doesn't know what placeholder
  format to use for ad-hoc code-mode SQL. A two-sentence dialect block
  in the prompt body fixes this entirely.

### Impact

Combined with spike 003 (proto-SDK extract recommendation) and spike 004
(toolkit thin-slice validation), the SQL portion of the toolkit lift is
de-risked. Phase 1 lift plan: `pmcp-server-toolkit` + Postgres / Athena /
MySQL per-backend crates + SQLite dev feature. GraphQL and OpenAPI
remain as Phase 2 / Phase 3 work pending their respective spikes.
