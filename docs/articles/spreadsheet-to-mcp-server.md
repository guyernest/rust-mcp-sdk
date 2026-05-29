# From a Spreadsheet to a Secure MCP Server in 10 Minutes

Most companies don't start with a database. They start with a spreadsheet — a
workbook someone maintains by hand, with a couple of tabs that quietly run part
of the business. A *Customers* sheet. A *Quotes* sheet. It works, until someone
asks: *"can the AI assistant answer questions about this?"*

The usual answer is a project: stand up a database, write an API, write a tool
handler for every question anyone might ask, and host it somewhere. That's weeks
of work to expose data you already have.

This article shows a shorter path. We'll take a real two-tab spreadsheet, import
it into SQLite, and put a production-grade MCP server in front of it — **without
writing a single line of query code**. The result is not a mechanical "run any
SQL" wrapper: it's a well-designed server with curated, typed tools for the
common questions and a governed Code Mode path for everything else, built on the
[PMCP SDK](https://github.com/paiml/rust-mcp-sdk) so it inherits Rust's safety
and performance and can deploy to serverless clouds.

All the files below live in
[`examples/spreadsheet-to-mcp/`](../../examples/spreadsheet-to-mcp/), and every
command here was run as written.

## What we're building

```text
  workbook.xlsx                     pmcp-sql-server
  ┌───────────────┐   export    ┌──────────────────────────────┐
  │ Customers tab │ ─────────►  │  curated tools (the ~20%)     │
  │ Quotes    tab │   CSV       │   • list_customers            │ ──► MCP client
  └───────────────┘             │   • quotes_for_customer       │     (Claude, etc.)
         │                      │                               │
         │  sqlite3 import      │  Code Mode (the long-tail 80%)│
         ▼                      │   • validate_code             │
   company.db  ──────────────►  │   • execute_code              │
   (typed tables)               └──────────────────────────────┘
                                   config.toml + schema.sql
```

You provide two inputs — a `config.toml` and a `schema.sql` — and run one binary.
No Rust, no recompile to change a tool or the schema.

## The spreadsheet

Imagine `acme-sales.xlsx` with two tabs:

**Customers**

| id | company_name | contact_name | email | country | created_at |
|----|--------------|--------------|-------|---------|------------|
| 1 | Northwind Traders | Ana Ruiz | ana@northwind.example | US | 2024-01-15 |
| 2 | Globex Industrial | Tom Becker | tom@globex.example | DE | 2024-02-03 |
| … | | | | | |

**Quotes**

| id | customer_id | amount | currency | status | issued_date | valid_until |
|----|-------------|--------|----------|--------|-------------|-------------|
| 1001 | 1 | 12500.00 | USD | accepted | 2024-03-01 | 2024-03-31 |
| 1003 | 2 | 32000.00 | EUR | accepted | 2024-03-15 | 2024-04-15 |
| … | | | | | | |

Ordinary stuff: a row per customer, a row per quote, a `customer_id` linking the
two.

## Step 1 — Export each sheet to CSV

A spreadsheet exports the *active* sheet, so you do this once per tab.

- **Excel:** open the *Customers* tab → **File → Save As** (or **Export**) →
  choose **CSV (Comma delimited)** → save as `customers.csv`. Repeat on the
  *Quotes* tab → `quotes.csv`.
- **Google Sheets:** select a tab → **File → Download → Comma-separated values
  (.csv)**. Repeat per tab.

You now have two plain files with a header row each:

```text
customers.csv      quotes.csv
─────────────      ──────────
id,company_name…   id,customer_id,amount…
1,Northwind…       1001,1,12500.00…
```

## Step 2 — Load into SQLite, with real types

This is the step that decides whether you end up with a *well-designed* server
or a mechanical one. If you let SQLite infer everything, every column becomes
`TEXT` — and `amount > 10000` or `SUM(amount)` quietly do the wrong thing on
text. So we define the tables *first*, with explicit types, then import.

`schema.sql` does double duty (we'll reuse it in Step 3 as the model's schema
reference):

```sql
CREATE TABLE customers (
    id           INTEGER PRIMARY KEY,
    company_name TEXT    NOT NULL,
    contact_name TEXT    NOT NULL,
    email        TEXT    NOT NULL,
    country      TEXT    NOT NULL,   -- ISO 3166 alpha-2
    created_at   TEXT    NOT NULL    -- ISO-8601 date (YYYY-MM-DD)
);

CREATE TABLE quotes (
    id          INTEGER PRIMARY KEY,
    customer_id INTEGER NOT NULL REFERENCES customers(id),
    amount      REAL    NOT NULL,
    currency    TEXT    NOT NULL DEFAULT 'USD',
    status      TEXT    NOT NULL,    -- draft | sent | accepted | rejected | expired
    issued_date TEXT    NOT NULL,
    valid_until TEXT
);

CREATE INDEX idx_quotes_customer ON quotes(customer_id);
CREATE INDEX idx_quotes_status   ON quotes(status);
```

Create the database and import the CSVs (skipping each header row):

```bash
rm -f company.db
sqlite3 company.db < schema.sql
sqlite3 company.db ".mode csv" \
  ".import --skip 1 customers.csv customers" \
  ".import --skip 1 quotes.csv quotes"
```

Verify it landed — and landed *typed*:

```bash
$ sqlite3 company.db "SELECT 'customers', COUNT(*) FROM customers
                      UNION ALL SELECT 'quotes', COUNT(*) FROM quotes;"
customers|8
quotes|15

$ sqlite3 company.db "SELECT typeof(id), typeof(amount), typeof(issued_date)
                      FROM quotes LIMIT 1;"
integer|real|text
```

`amount` is `real`, not `text` — so aggregates and comparisons are correct. A
quick business question already answers itself:

```bash
$ sqlite3 -header -column company.db \
  "SELECT c.company_name, ROUND(SUM(q.amount),2) AS won
     FROM customers c JOIN quotes q ON q.customer_id = c.id
    WHERE q.status = 'accepted'
    GROUP BY c.id ORDER BY won DESC LIMIT 5;"
company_name        won
------------------  --------
Globex Industrial   32000.0
Wayne Components    28900.0
Soylent Foods       18250.75
Northwind Traders   12500.0
Umbrella Logistics  9900.0
```

That JOIN+SUM is exactly the kind of query we'll let the AI write for itself in a
moment — safely.

## Step 3 — Describe the server in `config.toml`

Here's the part that replaces a codebase. `pmcp-sql-server` is a prebuilt binary
from the PMCP SDK: you point it at a `config.toml` and a schema file, and it
serves a complete MCP server. The config declares the backend, a Code Mode
policy, and the handful of tools worth curating.

```toml
[server]
name = "Acme Sales MCP"
version = "0.1.0"
description = "Customers and quotes imported from the sales spreadsheet"

[database]
# The binary opens this existing SQLite file (the one we built in Step 2).
type = "sqlite"
file_path = "company.db"

[[database.tables]]
name = "customers"
description = "Companies we sell to"

[[database.tables]]
name = "quotes"
description = "Sales quotes issued to customers"

[code_mode]
enabled = true
allow_writes = false          # read-only: this is reference data
allow_deletes = false
allow_ddl = false
require_limit = true          # every generated query must bound its rows
max_limit = 1000
sensitive_columns = ["email"] # surfaced as policy guidance to the model
# DEV ONLY (see the deployment note). Production uses a secrets reference.
token_secret = "dev-only-insecure-secret-min-16-bytes"
allow_inline_token_secret_for_dev = true

# ---- Curated tools: the ~20% of questions worth blessing -------------------

[[tools]]
name = "list_customers"
description = "List customers ordered by company name"
sql = "SELECT id, company_name, contact_name, country FROM customers ORDER BY company_name LIMIT :limit"

[[tools.parameters]]
name = "limit"
type = "integer"
required = false
default = 50

[[tools]]
name = "quotes_for_customer"
description = "List quotes for a given customer, newest first"
sql = "SELECT id, amount, currency, status, issued_date FROM quotes WHERE customer_id = :customer_id ORDER BY issued_date DESC LIMIT :limit"

[[tools.parameters]]
name = "customer_id"
type = "integer"
required = true

[[tools.parameters]]
name = "limit"
type = "integer"
required = false
default = 50

# ---- Schema resource + Code Mode prompt ------------------------------------

[[resources]]
uri = "docs://acme/schema"
name = "Database Schema"
description = "DDL for the customers and quotes tables"
# content is filled at startup from the --schema file.

[[prompts]]
name = "start_code_mode"
description = "Load schema + policy context for Code Mode SQL generation"
include_resources = [
    "docs://acme/schema",
    "code-mode://instructions",
    "code-mode://policies",
]
```

Two design decisions are worth calling out, because they're the difference
between a thoughtful API and a dumb passthrough:

- **You curate the common 20%.** `list_customers` and `quotes_for_customer` are
  named, typed, documented tools — the questions you *know* people ask. They
  read like an API, not like raw SQL.
- **Code Mode handles the long-tail 80% — under policy.** "Total accepted quote
  value per country this quarter" isn't worth a hand-written tool, but you can't
  enumerate every such question either. Code Mode lets the model *generate* SQL
  against the schema, but every statement is validated against the policy above
  (`allow_writes = false`, `require_limit = true`, a `max_limit`, sensitive
  columns flagged) and bound to an approval token before it runs. The model gets
  flexibility; you keep the guardrails.

## Step 4 — Run it

```bash
# build the binary once (from the repo root)
cargo build -p pmcp-sql-server

# serve (run from the example directory so file_path resolves)
cd examples/spreadsheet-to-mcp
../../target/debug/pmcp-sql-server --config config.toml --schema schema.sql
```

The server loads the config, opens `company.db`, assembles the tools + Code Mode
+ schema resource, and serves over streamable HTTP:

```text
INFO pmcp_sql_server: streamable-HTTP server listening bound=127.0.0.1:8080
```

Point any MCP client at that address — Claude Desktop, an IDE MCP integration,
or the SDK's own tester (`cargo pmcp test conformance http://127.0.0.1:8080`).
An `initialize` handshake confirms the server identity and what it offers:

```json
{
  "result": {
    "protocolVersion": "2025-06-18",
    "capabilities": {
      "tools": { "listChanged": false },
      "prompts": { "listChanged": false },
      "resources": { "subscribe": false, "listChanged": false }
    },
    "serverInfo": { "name": "Acme Sales MCP", "version": "0.1.0" }
  }
}
```

`tools/list` returns four tools: your two curated ones —
`list_customers`, `quotes_for_customer` — plus Code Mode's `validate_code` and
`execute_code`.

## Step 5 — What the assistant can now do

With the server connected, an assistant can:

- **Call a curated tool directly:** "show me Globex's quotes" → `quotes_for_customer(customer_id = 2)` returns the typed rows.
- **Answer anything else through Code Mode:** "which country has the highest
  total of accepted quotes?" The model loads the `start_code_mode` prompt (schema
  + policy), writes a `GROUP BY` query, submits it to `validate_code` — which
  checks it's read-only and bounded — gets an approval token, and runs it via
  `execute_code`. A request to `DELETE` or `UPDATE` is rejected by policy before
  it ever touches the database.

You wrote zero query handlers, yet the server is neither leaky nor limited.

## Why this isn't a "mechanical wrapper"

It would be easy to expose one `run_sql(query)` tool and call it a day. That's
the mechanical wrapper — unsafe, untyped, and impossible for a model to use well.
What we built instead is shaped:

- **Typed schema** → correct comparisons and aggregates, and a schema the model
  can reason about.
- **Curated tools** → a small, legible API for the common path.
- **Code Mode under policy** → flexible long-tail access with real guardrails,
  not an open SQL socket.
- **Built on PMCP/Rust** → memory-safe, fast, and deployable to serverless
  targets — the same binary that runs on your laptop runs in the cloud.

The spreadsheet is still the source of truth your team edits. Re-export, re-run
the two `sqlite3` import lines, and restart — the server reflects the new data.
No migration, no schema drift, no redeploy of code.

## Deploying it (a note)

Running locally is the whole story for an internal tool. When you want it hosted,
the PMCP CLI (`cargo pmcp`) handles deployment to AWS Lambda and other targets —
see the [Config-Driven SQL Server guide](../../cargo-pmcp/README.md#config-driven-sql-server-new---kind-sql-server).
One honest design choice to make first, because it depends on your data:

- **Reference data that changes occasionally** (a product catalog, a pricing
  sheet, this quotes export): bake it into the deployment. Dump the imported
  database to SQL with `sqlite3 company.db .dump` and ship that as the schema the
  server bootstraps on startup — the data travels with the binary, perfect for
  read-mostly snapshots. (Use `CREATE TABLE IF NOT EXISTS` / `INSERT OR IGNORE`
  so repeated cold starts stay idempotent.)
- **Live, mutable data**: don't put a system of record in a bundled SQLite file.
  Point `[database]` at a hosted database instead —
  `type = "postgres"` with a `url` — and the *same* config-driven server serves
  it. SQLite is the right tool for the spreadsheet snapshot; Postgres is the
  right tool when the data keeps changing.

Either way, the deploy path swaps the inline dev `token_secret` for a real
secrets reference automatically, so no development credential ships to
production.

## Takeaways

- A spreadsheet → CSV → `sqlite3` import gives you a typed, queryable database in
  three commands.
- `pmcp-sql-server` turns that database into a secure MCP server from a
  `config.toml` + schema — curated tools for the common path, governed Code Mode
  for the rest, zero query code.
- Define **types** and **curated tools** to get a well-designed server, not a raw
  SQL passthrough.
- Keep editing the spreadsheet; re-import to refresh. Deploy when ready, choosing
  bundled SQLite for reference snapshots or hosted Postgres for live data.

The companion files are in
[`examples/spreadsheet-to-mcp/`](../../examples/spreadsheet-to-mcp/). For the
build-and-deploy CLI workflow, see the
[Config-Driven SQL Server guide](../../cargo-pmcp/README.md#config-driven-sql-server-new---kind-sql-server)
and the *Config-Driven SQL Servers* chapters in the PMCP book and course.
