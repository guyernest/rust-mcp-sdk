# Schema-Server Toolkit — Multi-Dialect SQL Connectors

This blueprint specifies the `SqlConnector` trait + `Dialect` enum + the
two free helpers that make ONE `config.toml` drive Postgres / MySQL /
Athena / SQLite without per-backend specifics leaking into toolkit core.

Builds on top of `schema-server-architecture.md` — read that first for
the broader toolkit shape.

## Requirements

- **Single `SqlConnector` trait, three methods.** `dialect()`,
  `execute(sql_with_named_placeholders, named_params)`,
  `schema_text()`. Anything bigger ossifies per-backend assumptions.
- **`:name` is the canonical user-facing placeholder syntax.** Users
  write `WHERE id = :id` in their `config.toml` regardless of target
  backend. The connector translates at execute time.
- **`Dialect` enum lives in the toolkit core**, NOT in per-backend
  crates. Adding a new dialect is a 3-step extension that does NOT
  touch the toolkit's trait or tool-handler code.
- **Production targets are remote** — Postgres / Aurora / Athena /
  MySQL. SQLite is dev/CI only, behind a `sqlite` feature flag on the
  toolkit. Per-backend crates use pure-Rust drivers compatible with
  AWS Lambda (no Docker, no system libs).
- **`schema_text()` output is dialect-styled, not normalized.** The
  LLM gets the native introspection shape per dialect
  (information_schema / SHOW CREATE TABLE / Glue Data Catalog) — no
  toolkit-level homogenization that would obscure dialect-specific
  details the LLM needs.
- **Code-mode prompt body is dialect-aware.** Each prompt includes the
  dialect name + dialect-specific placeholder guidance so the LLM
  knows what placeholder syntax to use for ad-hoc `execute_code` SQL.

## How to Build It

### The trait + enum (toolkit-owned)

```rust
// crates/pmcp-server-toolkit/src/sql/mod.rs
use async_trait::async_trait;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    Postgres,
    MySql,
    Athena,
    Sqlite,
    // To add a new one: extend here + add a translation rule below +
    // ship a per-backend crate that returns it from dialect().
}

impl Dialect {
    pub fn name(self) -> &'static str { /* "Postgres" / "MySQL" / ... */ }

    pub fn placeholder_guidance(self) -> &'static str {
        match self {
            Dialect::Postgres => "Bind parameters using `$1`, `$2`, ... (1-indexed positional).",
            Dialect::MySql    => "Bind parameters using `?` (positional, in declared order).",
            Dialect::Athena   => "Bind parameters using `?` (positional, Presto-style).",
            Dialect::Sqlite   => "Bind parameters using `:name` (named) or `?` (positional).",
        }
    }
}

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
```

### The translation helper (toolkit-owned)

```rust
/// Translate the toolkit's canonical `:name` placeholders into the
/// dialect's native format. Returns (translated_sql, positional_order).
/// Per-backend crates call this inside their `execute` impl.
pub fn translate_placeholders(
    dialect: Dialect,
    sql: &str,
) -> (String, Vec<String>) {
    let mut out = String::with_capacity(sql.len());
    let mut order = Vec::new();
    let mut chars = sql.chars().peekable();
    let mut pg_index: usize = 0;

    while let Some(c) = chars.next() {
        if c == ':' {
            let mut name = String::new();
            while let Some(&nc) = chars.peek() {
                if nc.is_alphanumeric() || nc == '_' { name.push(nc); chars.next(); }
                else { break; }
            }
            if name.is_empty() { out.push(':'); continue; }
            order.push(name.clone());
            match dialect {
                Dialect::Postgres => { pg_index += 1; write!(out, "${pg_index}").unwrap(); }
                Dialect::MySql | Dialect::Athena => out.push('?'),
                Dialect::Sqlite => write!(out, ":{name}").unwrap(),
            }
        } else { out.push(c); }
    }
    (out, order)
}
```

### Per-backend connector pattern (Postgres example)

Each per-backend crate is small. The Postgres crate looks like:

```rust
// crates/pmcp-toolkit-postgres/src/lib.rs
use pmcp_server_toolkit::sql::{Dialect, SqlConnector, translate_placeholders};
use tokio_postgres::Client;

pub struct PostgresConnector { client: Client }

impl PostgresConnector {
    pub async fn connect(url: &str) -> Result<Self> {
        // tokio-postgres setup; spawn the connection in tokio::spawn
    }
}

#[async_trait]
impl SqlConnector for PostgresConnector {
    fn dialect(&self) -> Dialect { Dialect::Postgres }

    async fn execute(&self, sql: &str, params: &[(String, Value)])
        -> Result<Vec<Value>>
    {
        let (translated, order) = translate_placeholders(Dialect::Postgres, sql);
        // Bind positional from `order` against `params` (name lookup)
        let positional: Vec<&(dyn ToSql + Sync)> = order
            .iter()
            .map(|n| params.iter().find(|(name, _)| name == n)
                          .map(|(_, v)| v as &(dyn ToSql + Sync))
                          .unwrap_or(&Null))
            .collect();
        // Execute via tokio-postgres
        let rows = self.client.query(&translated, &positional).await?;
        // Convert pg rows → serde_json::Value rows
    }

    async fn schema_text(&self) -> Result<String> {
        // SELECT table_name, column_name, data_type FROM information_schema.columns
        // WHERE table_schema = 'public' (or configured) — reconstruct CREATE TABLE-ish output
    }
}
```

### The 3-step extension protocol

Adding Oracle / SQL Server / DuckDB (or any future dialect):

1. **Extend the enum** in toolkit core (one line):
   ```rust
   pub enum Dialect { ..., Oracle, SqlServer, DuckDb }
   ```
2. **Add a translation rule** in `translate_placeholders` (one match arm):
   ```rust
   Dialect::Oracle => write!(out, ":{name}").unwrap(),  // Oracle is :name native
   Dialect::SqlServer => { sql_server_index += 1; write!(out, "@p{sql_server_index}").unwrap(); }
   Dialect::DuckDb => out.push('?'),
   ```
3. **Add `Dialect::placeholder_guidance` arms** + ship a new
   `pmcp-toolkit-oracle` (or `-sqlserver` or `-duckdb`) crate.

**Toolkit core does NOT change** for new backends. This is the
load-bearing extensibility claim, asserted in-binary by spike 005's
trait surface assessment (Step F).

### Mock pattern for tests

Per CONVENTIONS.md, dialect-aware mocks for unit/integration tests are
fine. Each mock authentically models its dialect's wire shape (the
placeholder format it observes, the schema_text shape it emits) so
trait-level tests stay portable. Spike 005's `postgres_mock`,
`mysql_mock`, `athena_mock` are reference implementations.

Real-DB integration tests are the per-connector crate's responsibility
(opt-in CI job with credentials), NOT the toolkit's default test path.
No Docker / testcontainers — that is misaligned with the pure-Rust
Lambda deployment model.

## What to Avoid

- **Don't make `translate_placeholders` a trait method.** It's a free
  helper for a reason — every connector calls it the same way. Putting
  it on the trait invites per-backend overrides that introduce subtle
  drift.
- **Don't normalize `schema_text()` output across dialects.** The LLM
  benefits from seeing the native introspection format. Forcing
  everything into a synthetic "tables: ..." shape hides dialect
  specifics the LLM needs to write dialect-correct ad-hoc SQL.
- **Don't use Docker / testcontainers in CI.** Pure-Rust drivers
  compile to Lambda binaries; Docker doesn't. The trait-design
  question is answered by authentic in-process mocks. Real-DB
  integration is per-connector-crate concern.
- **Don't expose dialect-specific config knobs on the toolkit's
  generic `SchemaServerConfig`.** Each per-backend crate parses its
  own `[database]` section shape. The toolkit only knows
  `[[tools]]` and `[code_mode]`.
- **Don't try to handle every SQL dialect's quirks at translation
  time.** `translate_placeholders` only handles placeholder syntax.
  LIMIT vs ROWNUM, quoting rules, function-name differences — those
  are the user's problem when writing per-deployment SQL, with help
  from the dialect-aware code-mode prompt.
- **Don't share connection pools across dialects.** Each connector
  owns its own pool (`tokio-postgres::Pool`, `sqlx::Pool<MySql>`,
  Athena's stateless HTTP). The toolkit's `Arc<dyn SqlConnector>`
  abstracts away the pool implementation.

## Constraints

- **Phase 1 per-backend crates:** `pmcp-toolkit-postgres`,
  `pmcp-toolkit-athena`, `pmcp-toolkit-mysql`. All pure-Rust,
  Lambda-suitable.
- **SQLite ships as a feature flag on the toolkit**, not a separate
  crate. Dev/CI only — no production deployments expected.
- **Tokio runtime required.** All connector traits are `async_trait`.
- **`Arc<dyn SqlConnector>` is the canonical shared connector type.**
  The per-tool handler holds `Arc<C: SqlConnector>` for monomorphized
  calls; the in-process test harness can hold `Arc<dyn SqlConnector>`.
- **`pmcp::Error::validation(msg)` and `pmcp::Error::internal(msg)`**
  are the right error constructors when surfacing connector errors
  into the toolkit's per-tool handler (matching spike 004's pattern).

## Origin

Synthesized from:
- **005 multi-dialect-sql-connector** — `SqlConnector` trait + `Dialect`
  enum + `translate_placeholders` + per-dialect schema introspection
  shapes. Verdict: VALIDATED. Source files:
  `sources/005-multi-dialect-sql-connector/`.

Cross-references:
- See `schema-server-architecture.md` for the broader toolkit shape
  this connector trait fits into.
- See `feedback_avoid_docker_pure_rust_lambda` memory for the rationale
  on pure-Rust drivers + mock-only spike tests.
