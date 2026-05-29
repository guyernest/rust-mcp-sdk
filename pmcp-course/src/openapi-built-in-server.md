# Config-Driven OpenAPI Servers: From Scaffold to Deploy

Earlier you hand-built MCP servers in Rust — writing a tool handler per
operation, managing the client, and recompiling on every change. That is the
right approach when your logic is bespoke. But a large class of API-backed MCP
servers are *mostly mechanical*: expose a few blessed endpoints, let an agent
handle the ad-hoc rest under policy, and ship it. For those, PMCP offers a
config-driven path where the server is described in a `config.toml` (plus an
optional OpenAPI spec) instead of written in Rust — and `cargo pmcp` scaffolds,
runs, and deploys it for you.

This is the HTTP sibling of the
[Config-Driven SQL Servers](./part3-deployment/ch08-5-config-driven-sql-server.md)
chapter. If you have done that one, this will feel familiar: same lifecycle,
same Pareto model, same secret posture — with an HTTP backend and outgoing auth
in place of a SQL connector and schema.

This chapter walks the full lifecycle: **scaffold → run → customize → deploy**.

## What You'll Learn

- When a config-driven OpenAPI server beats a hand-coded one (and when it doesn't)
- How `cargo pmcp new --kind openapi-server` scaffolds a deployable crate
- The two kinds of tools — single-call and script — and the Code Mode long tail
- The six outgoing-auth variants and the `${ENV}` secret discipline
- How `cargo pmcp deploy` ships the server, keeping dev secrets out of artifacts

## Prerequisites

```bash
# The PMCP CLI
cargo install cargo-pmcp
```

## The Pareto Split

An API-backed MCP server faces two kinds of demand. A small set of **blessed
operations** ("get all line statuses", "fetch order X") deserve named, typed,
audited tools. A much larger **long tail** of ad-hoc questions cannot be
enumerated in advance. Hand-coding forces a bad trade: write a handler for every
tail operation, or expose one dangerous "call any endpoint" tool.

Config-driven servers split the work cleanly:

```text
┌───────────────────────────────────────────────────────────────────────────┐
│                   CONFIG-DRIVEN OPENAPI MCP SERVER                          │
├───────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│   config.toml                                          api.yaml (--spec)    │
│   ┌──────────────────────────┐                  ┌───────────────────────┐  │
│   │  [[tools]]               │                  │  OpenAPI contract      │  │
│   │   get-tube-status        │                  │  (served as the Code   │  │
│   │   disrupted-lines (script)│                 │   Mode api_schema)     │  │
│   │  ── the curated ~20% ──  │                  └───────────────────────┘  │
│   └──────────────────────────┘                            │                │
│              │                                            │                │
│              ▼                                            ▼                │
│   ┌──────────────────────────┐            ┌──────────────────────────────┐ │
│   │  Named MCP tools         │            │  Code Mode (validate_code /   │ │
│   │  (single-call + script)  │            │  execute_code) — the long-tail│ │
│   │                          │            │  ~80%, scripts under policy   │ │
│   └──────────────────────────┘            └──────────────────────────────┘ │
│              │                                            │                │
│              └──────────────────────┬─────────────────────┘                │
│                                     ▼                                       │
│                   one HTTP engine (reqwest::Client + outgoing auth)         │
└───────────────────────────────────────────────────────────────────────────┘
```

You curate the 20% as `[[tools]]`; Code Mode covers the 80%. Crucially, the
single-call tools, the script tools, and Code Mode all share **one HTTP engine** —
one client and one outgoing-auth provider.

## Step 1: Scaffold

```bash
cargo pmcp new tube-api --kind openapi-server
cd tube-api
```

This emits a single runnable crate:

```text
tube-api/
├── Cargo.toml          # pins pmcp-server-toolkit (openapi-code-mode) + pmcp-openapi-server
├── src/main.rs         # generated wiring — the only Rust, and you rarely touch it
├── config.toml         # [server] / [backend] / [code_mode] + a single-call + a script tool
├── api.yaml            # minimal OpenAPI spec (optional at runtime)
├── deploy.toml         # deploy descriptor (human-visible)
└── .pmcp/deploy.toml   # the copy cargo pmcp deploy reads
```

The generated `src/main.rs` loads the config (and `api.yaml` if present), calls
`dispatch` to build the connector + Code Mode executor pair, calls `build_server`
to assemble the tools, and serves over streamable HTTP. The HTTP wiring lives in
the `pmcp-openapi-server` library the scaffold depends on — the same seam this
crate's own example uses.

## Step 2: Run It Locally

```bash
cargo run
# serves over streamable HTTP on 127.0.0.1:8080 by default
```

Connect your MCP client (or `cargo pmcp test check <addr>`) to the address. You
get the curated tools plus Code Mode's `validate_code` and `execute_code`. Ask
for something uncurated and the agent writes a script against your OpenAPI
contract, validated and policy-checked before it runs.

## Step 3: Two Kinds of Tools

The most important new idea over the SQL server is that `[[tools]]` comes in two
kinds (D-01). A **single-call** tool maps onto one endpoint; a **script** tool
orchestrates several calls in an engine-accurate JS subset. The `london-tube`
worked example shows both:

```toml
# single-call: one endpoint, no parameters.
[[tools]]
name = "get-tube-status"
description = "Get the current status of all London Underground lines."
path = "/Line/Mode/tube/Status"
method = "GET"

# script: fetch statuses, filter to disrupted lines, fan out for per-line detail.
[[tools]]
name = "disrupted-lines-with-detail"
description = "List currently-disrupted tube lines with per-line disruption detail."
script = """
const statuses = await api.get('/Line/Mode/tube/Status');
const disrupted = statuses.filter(line => line.lineStatuses.some(s => s.statusSeverity < 10));
const out = [];
for (const line of disrupted) {
  const detail = await api.get(`/Line/${line.id}/Disruption`);
  out.push({ line: line.name, detail: detail, max: args.maxLines });
}
return { count: out.length, lines: out };
"""

[[tools.parameters]]
name = "maxLines"
type = "integer"
required = false
default = 5
```

The script subset is deliberately small: `await api.get(...)` with string or
template-literal paths, normal JS filtering/looping, `args.<name>` for declared
parameters, and a final `return`. Edit and restart — the new tools are live with
**no recompile**.

## Step 4: Authenticate to the Backend

The `[backend.auth] type` value picks the outgoing-auth variant. There are six —
`none` plus five authenticated ones:

| `type`                       | Mechanism |
| ---------------------------- | --------- |
| `none`                       | No authentication (the default). |
| `api_key`                    | API key as query parameter(s) and/or header(s). |
| `bearer`                     | `Authorization: Bearer <token>`. |
| `basic`                      | `Authorization: Basic <base64(user:pass)>`. |
| `oauth2_client_credentials`  | OAuth2 client-credentials grant. |
| `oauth_passthrough`          | Forwards the **incoming** MCP client token (SSO). |

Always reference secrets via `${ENV_VAR}` — never inline a real token:

```toml
[backend]
base_url = "https://api.tfl.gov.uk"

[backend.auth]
type = "api_key"
query_params = { app_key = "${TFL_APP_KEY}" }
required = false
```

An unset `required = false` reference is omitted rather than sent as the literal
`${...}` placeholder. The static variants ignore any inbound MCP token; only
`oauth_passthrough` forwards it.

## The Spec Is Optional (D-03)

`--spec` (the scaffold's `api.yaml`) is optional. Curated-only servers boot with
no spec. When supplied, the spec becomes the Code Mode `api_schema` resource. If
Code Mode is enabled but no spec is present, the server warns and proceeds —
Code Mode runs without the contract rather than failing.

## Step 5: Deploy

The scaffold's `deploy.toml` targets `pmcp-run` by default. Change `[target]
type` to `aws-lambda`, `cloud-run`, or `cloudflare` (mirror the edit into
`.pmcp/deploy.toml`), then:

```bash
cargo pmcp validate deploy        # pre-flight checks, no cloud calls yet
cargo pmcp deploy
cargo pmcp deploy outputs         # deployed endpoint URL
```

The deploy bundles `config.toml` + `api.yaml` and applies the same secret posture
as the SQL server: any inline DEV `token_secret` in the bundled config is
rewritten to a `${...}` reference so the deployed artifact never ships a dev
literal. Supply the secret as a deploy environment variable; your on-disk config
is left untouched.

## When to Use This (and When Not To)

| Use config-driven OpenAPI | Hand-code instead |
|---|---|
| Mostly proxying / orchestrating REST endpoints | Complex bespoke logic per tool |
| You want non-Rust teammates to own tools | Custom transports / middleware |
| Fast iteration on tools | Non-HTTP backends |
| Curated 20% + agent-driven long tail | Every operation must be explicitly coded |

## Exercise: Ship a Two-Tool OpenAPI Server

**Goal:** scaffold, extend, and deploy a config-driven OpenAPI server.

1. Scaffold `weather-api` with `cargo pmcp new weather-api --kind openapi-server`.
2. Point `[backend]` at a public weather API and add a single-call `[[tools]]`
   entry for its current-conditions endpoint with a typed `city` parameter.
3. Add a script tool that fetches conditions for several cities and returns the
   warmest, reading the cities from `args`.
4. Configure `[backend.auth] type = "api_key"` with the key as a `${WEATHER_KEY}`
   query-param reference; confirm an unset key is omitted, not sent literally.
5. **Stretch:** set `[target] type = "aws-lambda"`, run `cargo pmcp validate
   deploy`, deploy, and supply `WEATHER_KEY` as an environment variable.

**Success criteria:** the single-call tool returns live conditions; the script
tool aggregates across cities; the secret is referenced via `${...}`, never
inlined; and (stretch) the deployed endpoint authenticates with the env-supplied
key.

## Key Takeaways

- Config-driven OpenAPI servers describe an HTTP MCP server in `config.toml`
  (+ optional OpenAPI spec) instead of hand-written Rust — curated tools for the
  common 20%, Code Mode for the long-tail 80%.
- Tools come in two kinds: single-call (`path`/`method`) and script
  (multi-call JS orchestration); both run over one shared HTTP engine.
- Outgoing auth has six variants (`none` + `api_key`/`bearer`/`basic`/
  `oauth2_client_credentials`/`oauth_passthrough`); always reference secrets via
  `${ENV}`.
- `cargo pmcp new --kind openapi-server` scaffolds a deployable crate; `cargo
  pmcp deploy` bundles assets and swaps dev secrets for `${...}` references.
- The OpenAPI spec is optional — curated-only servers boot without it (D-03).
