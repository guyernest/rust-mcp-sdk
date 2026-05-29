# Config-Driven SQL Servers: From Scaffold to Lambda

In Chapter 3 you hand-built a database MCP server in Rust — writing a tool
handler per query, managing the connector, and recompiling on every change. That
is the right approach when your logic is bespoke. But a large class of
enterprise SQL servers are *mostly mechanical*: expose a few blessed queries,
let an agent handle the ad-hoc rest under policy, and ship it. For those, PMCP
offers a config-driven path where the server is described in a `config.toml`
instead of written in Rust — and `cargo pmcp` scaffolds, runs, and deploys it
to AWS Lambda for you.

This chapter walks the full lifecycle: **scaffold → run → customize → deploy**.

## What You'll Learn

- When a config-driven server beats a hand-coded one (and when it doesn't)
- How `cargo pmcp new --kind sql-server` scaffolds a deployable crate
- How the curated-tools / Code Mode "Pareto split" covers 100% of queries
- How `cargo pmcp deploy` ships the server to Lambda, including the secret
  posture that keeps dev credentials out of production artifacts

## Prerequisites

```bash
# The PMCP CLI
cargo install cargo-pmcp

# For the Lambda deploy at the end of this chapter:
aws sts get-caller-identity        # AWS credentials configured
cargo install cargo-lambda         # cross-compile to the Lambda runtime
npm install -g aws-cdk             # infrastructure provisioning
```

## The Pareto Split

A production SQL server faces two kinds of demand. A small set of **blessed
operations** ("list active customers", "orders for account X") deserve named,
typed, audited tools. A much larger **long tail** of ad-hoc questions cannot be
enumerated in advance. Hand-coding forces a bad trade: write a handler for every
tail query, or expose one dangerous "run any SQL" tool.

Config-driven servers split the work cleanly:

```
┌───────────────────────────────────────────────────────────────────────────┐
│                     CONFIG-DRIVEN SQL MCP SERVER                            │
├───────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   config.toml                                          schema.sql           │
│   ┌──────────────────────────┐                  ┌───────────────────────┐  │
│   │  [[tools]]               │                  │  CREATE TABLE ...      │  │
│   │   list_books             │                  │  (served as the Code   │  │
│   │   books_by_author        │                  │   Mode schema resource)│  │
│   │  ── the curated ~20% ──  │                  └───────────────────────┘  │
│   └──────────────────────────┘                            │                │
│              │                                            │                │
│              ▼                                            ▼                │
│   ┌──────────────────────────┐            ┌──────────────────────────────┐ │
│   │  Named MCP tools         │            │  Code Mode (validate_code /   │ │
│   │  (typed parameters)      │            │  execute_code) — the long-tail│ │
│   │                          │            │  ~80%, generated SQL under    │ │
│   │                          │            │  policy + approval tokens     │ │
│   └──────────────────────────┘            └──────────────────────────────┘ │
│              │                                            │                │
│              └──────────────────────┬─────────────────────┘                │
│                                     ▼                                       │
│                          pmcp-server-toolkit                               │
│                          (SQL connector + builder)                         │
└───────────────────────────────────────────────────────────────────────────┘
```

You curate the 20% as `[[tools]]`; Code Mode (Chapter 12.9 concepts) covers the
80%. The server itself is not hand-coded per query.

## Step 1: Scaffold

```bash
cargo pmcp new acme-sql --kind sql-server
cd acme-sql
```

This emits a single runnable crate:

```
acme-sql/
├── Cargo.toml          # pins pmcp-server-toolkit ["code-mode","sqlite","http"]
├── src/main.rs         # generated wiring — the only Rust, and you rarely touch it
├── config.toml         # [server] / [database] / [code_mode] + a list_books tool
├── schema.sql          # idempotent demo DDL + seed
├── deploy.toml         # deploy descriptor (human-visible)
└── .pmcp/deploy.toml   # the copy cargo pmcp deploy reads
```

The generated `src/main.rs` loads `config.toml` and `schema.sql` through
`pmcp::assets::load_string` (cwd locally, `/var/task/assets/` on Lambda) and
opens SQLite at `demo_db_path()` (`/tmp/demo.db` on Lambda). That single design
choice is why the **same compiled binary runs locally and on Lambda unchanged**.

## Step 2: Run It Locally

```bash
cargo run
# prints: PMCP_SQL_SERVER_ADDR=http://127.0.0.1:<port>
```

Connect your MCP client (or `cargo pmcp test check <addr>`) to the printed
address. You get the curated `list_books` tool plus Code Mode's `validate_code`
and `execute_code`. Ask for something uncurated and the agent writes SQL against
your schema, validated and policy-checked before it runs.

> **Local DX tip:** `cargo pmcp dev --server acme-sql` wraps `cargo run`,
> rebuilding and injecting `.env` variables for you.

## Step 3: Customize Through Config

The config uses `#[serde(deny_unknown_fields)]`, so typos fail loudly at startup
rather than silently disabling a feature — a property you want in regulated
environments. Add a curated tool and tighten the policy entirely in config:

```toml
[code_mode]
enabled = true
allow_writes = false        # default-deny: read-only
require_limit = true        # every generated query must bound its rows
max_limit = 1000
token_secret = "dev-only-insecure-secret-min-16-bytes"   # DEV ONLY
allow_inline_token_secret_for_dev = true

[[tools]]
name = "books_by_author"
description = "Books written by a given author"
sql = "SELECT id, title FROM books WHERE author = :author ORDER BY title LIMIT :limit"

[[tools.parameters]]
name = "author"
type = "string"
required = true
```

Restart — the new tool is live. No recompile. Switching backends is a config +
feature change: set `[database] type = "postgres"` (or `mysql` / `athena`) and
enable the matching `pmcp-server-toolkit` connector feature.

## Step 4: Deploy to AWS Lambda

The scaffold's `deploy.toml` targets `pmcp-run` by default. For Lambda, set the
target type (mirror the edit into `.pmcp/deploy.toml`):

```toml
[target]
type = "aws-lambda"
version = "1.0.0"

[aws]
region = "us-east-1"

[server]
name = "acme-sql"
memory_mb = 512
timeout_seconds = 30

[assets]
include = ["config.toml", "schema.sql"]
```

```bash
cargo pmcp validate deploy                  # pre-flight: IAM footgun detection, no AWS calls yet
cargo pmcp deploy --target-type aws-lambda
cargo pmcp deploy outputs                    # deployed endpoint URL
```

### What the deploy does for you

```
  config.toml ─┐                          ┌─► /var/task/assets/config.toml  (read-only)
               ├─ bundled into the zip ───┤
  schema.sql  ─┘                          └─► /var/task/assets/schema.sql    (read-only)

  runtime opens SQLite ───────────────────► /tmp/demo.db                     (writable)
```

The critical security step is automatic: the deploy path **rewrites the bundled
config's inline DEV `token_secret` to `${CODE_MODE_SECRET}`**, so the deployed
artifact never ships the dev literal. Provide `CODE_MODE_SECRET` as a Lambda
environment variable / deploy secret. Your on-disk `config.toml` is untouched —
only the bundled copy is sanitized.

Verify the live endpoint with the same conformance suite from Chapter 8:

```bash
cargo pmcp test conformance <deployed-url>
```

## When to Use This (and When Not To)

| Use config-driven | Hand-code instead (Chapter 3) |
|---|---|
| Mostly CRUD / reporting over SQL | Complex business logic per tool |
| You want non-Rust teammates to own tools | You need custom transports/middleware |
| Fast iteration on tools & schema | Non-SQL backends or bespoke connectors |
| Curated 20% + agent-driven long tail | Every operation must be explicitly coded |

For backends beyond SQL or deeply custom behavior, the hand-coded approach from
Chapter 3 remains the right tool.

## Exercise: Ship a Two-Table Server

**Goal:** scaffold, extend, and deploy a config-driven SQL server.

1. Scaffold `library-sql` with `cargo pmcp new library-sql --kind sql-server`.
2. Add a second table (`authors`) to `schema.sql` and a curated
   `[[tools]]` entry `authors_with_books` that joins `books` and `authors`.
3. Set `allow_writes = false` and `require_limit = true`; confirm via the MCP
   client that an unbounded `SELECT` generated through Code Mode is rejected.
4. Edit `deploy.toml` to target `aws-lambda`, run `cargo pmcp validate deploy`,
   and resolve any IAM warning it reports.
5. **Stretch:** deploy to Lambda, supply `CODE_MODE_SECRET` as an environment
   variable, and confirm with `cargo pmcp test conformance <url>` that the live
   server rejects a write attempt.

**Success criteria:** the curated join tool works locally; Code Mode honors the
read-only / limit policy; `validate deploy` passes; and (stretch) the deployed
endpoint passes conformance with the secret supplied via environment, not inline.

## Key Takeaways

- Config-driven servers describe a SQL MCP server in `config.toml` + `schema.sql`
  instead of hand-written Rust — curated tools for the common 20%, Code Mode for
  the long-tail 80%.
- `cargo pmcp new --kind sql-server` scaffolds a deployable crate; the same
  binary runs locally and on Lambda because assets and the DB resolve to
  runtime-appropriate paths.
- `cargo pmcp deploy --target-type aws-lambda` bundles your assets and swaps the
  dev secret for `${CODE_MODE_SECRET}` automatically — no dev credentials ship to
  production.
- Reach for the hand-coded approach (Chapter 3) when logic is bespoke or the
  backend isn't SQL.
