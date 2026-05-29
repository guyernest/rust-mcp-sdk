# Config-Driven OpenAPI Servers (cargo pmcp)

The previous chapter showed how Code Mode safely executes LLM-generated code
against a contract. This chapter is the applied form of that idea for HTTP/REST
APIs: a complete, deployable OpenAPI MCP server that you describe in a
`config.toml` (plus an optional OpenAPI spec) instead of hand-writing in Rust.
You declare your backend, a handful of curated tools, and a Code Mode policy;
the `cargo pmcp` CLI scaffolds the crate, runs it locally, and deploys it — and
the long tail of operations you didn't curate is handled by Code Mode generating
scripts against your OpenAPI contract.

It is the HTTP sibling of the [Config-Driven SQL Servers](ch12-10-config-driven-sql-servers.md)
chapter: same Pareto model, same scaffold-run-deploy lifecycle, same secret
posture — with an HTTP backend and outgoing auth in place of a SQL connector and
a schema file.

## The Problem (Why Config, Not Code)

To expose an HTTP API over MCP the conventional way, you hand-write a Rust
binary: construct a `ServerBuilder`, implement a tool handler for *every*
endpoint, build and manage an HTTP client plus outgoing authentication, wire up
Code Mode policy, stand up the HTTP transport — and recompile every time a tool
changes. For an API with dozens of useful operations, most of that code is
mechanical, and the recompile loop slows iteration to a crawl.

There is a Pareto split hiding in this work. Roughly 20% of operations are
"blessed" paths worth curating as named tools with typed parameters. The other
~80% is a long tail of ad-hoc operations you cannot enumerate in advance.
Config-driven servers answer both halves:

```text
                       ┌──────────────────────────────────┐
   config.toml  ─────► │  [[tools]]  (the curated 20%)      │ ──► named MCP tools
                       └──────────────────────────────────┘
                       ┌──────────────────────────────────┐
   --spec (OpenAPI) ─► │  Code Mode  (the long-tail 80%)    │ ──► validate_code /
                       └──────────────────────────────────┘     execute_code
```

You curate the common operations as `[[tools]]`; Code Mode handles the rest by
generating scripts against the OpenAPI contract resource, validated and
policy-checked. Nothing about the server is hand-coded per operation — the parts
that vary live in config. Both halves run over the **same HTTP engine**: one
`reqwest::Client` and one outgoing-auth provider feed the single-call tools, the
script tools, and Code Mode alike.

## Two Shapes

PMCP ships two ways to run this, both built on the same `pmcp-server-toolkit`
library and the `pmcp-openapi-server` crate's `dispatch` + `build_server` seam:

| | **Shape A — the binary** | **Shape B — the scaffold** |
|---|---|---|
| What | The prebuilt `pmcp-openapi-server` binary | A crate from `cargo pmcp new --kind openapi-server` |
| Run | `pmcp-openapi-server --config c.toml [--spec s.yaml]` | `cargo run` inside the crate |
| Rust source? | None | A small generated `src/main.rs` you own |
| Best for | Zero-build point-and-serve; extending the toolkit | Building, customizing, and **deploying** |

This chapter leads with **Shape B**, because it is the path the CLI scaffolds and
the one `cargo pmcp deploy` understands end-to-end. Shape A is covered in the
`pmcp-openapi-server` crate README.

## Step 1: Scaffold

```bash
cargo pmcp new my-openapi-server --kind openapi-server
cd my-openapi-server
```

This emits a single runnable crate (not the default multi-crate workspace):

```text
my-openapi-server/
├── Cargo.toml          # pins pmcp-server-toolkit (openapi-code-mode) + pmcp-openapi-server
├── src/main.rs         # generated wiring: dispatch → build_server → serve
├── config.toml         # [server] / [backend] / [code_mode] + a single-call + a script tool
├── api.yaml            # minimal OpenAPI spec (optional at runtime — D-03)
├── deploy.toml         # deploy descriptor (human-visible)
└── .pmcp/deploy.toml   # the copy cargo pmcp deploy reads
```

The generated `src/main.rs` is the load-bearing wiring — deliberately small. It
loads `config.toml` (and `api.yaml` if present), calls `dispatch` to build the
`(HttpConnector, HttpCodeExecutor)` pair lazily, calls `build_server` to assemble
the curated tools + Code Mode, and serves over streamable HTTP. The HTTP path
has no `ServerBuilderExt` method, so the wiring lives in the `pmcp-openapi-server`
library that the scaffold depends on (the same `dispatch`/`build_server` seam this
crate's own `examples/openapi_server_min.rs` uses).

## Step 2: Run It

```bash
cargo run
# serves over streamable HTTP on 127.0.0.1:8080 by default
```

Point an MCP client at the address. You will see the curated tools from
`config.toml` plus Code Mode's `validate_code` and `execute_code` tools. Ask the
model for something you *didn't* curate and it will write a script against your
OpenAPI contract, have it validated, and execute it under the policy you
configured.

## Step 3: Customize Through Config — Two Kinds of Tools

A `[[tools]]` entry is one of two kinds (D-01), and both run over the same engine.
A **single-call** tool maps 1:1 onto one backend endpoint; a **script** tool is a
multi-call orchestration in an engine-accurate JS subset:

```toml
# single-call: one endpoint, typed parameters.
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

This is the `london-tube` (Transport for London) worked example — a single-call
status tool and a script tool that fans out for disruption detail. Tune the Code
Mode safety posture under `[code_mode]`; both files are read at startup, so
editing the config and restarting makes the new tools live with **no recompile**.

## Outgoing Authentication

The `[backend.auth] type` value selects how the server authenticates to your
backend. There are six variants — `none` plus five authenticated ones, traced to
`pmcp_server_toolkit::http::auth::AuthConfig`:

| `type`                       | Mechanism |
| ---------------------------- | --------- |
| `none`                       | No authentication (the default). |
| `api_key`                    | API key as query parameter(s) and/or header(s). |
| `bearer`                     | `Authorization: Bearer <token>`. |
| `basic`                      | `Authorization: Basic <base64(user:pass)>`. |
| `oauth2_client_credentials`  | OAuth2 client-credentials grant (fetches + caches a token). |
| `oauth_passthrough`          | Forwards the **incoming** MCP client token to the backend (SSO). |

Secrets use `${ENV_VAR}` references resolved at provider-construction time —
never inline a real token in a committed config:

```toml
[backend]
base_url = "https://api.tfl.gov.uk"

[backend.auth]
type = "api_key"
query_params = { app_key = "${TFL_APP_KEY}" }
required = false
```

The static variants ignore any inbound MCP token; only `oauth_passthrough`
forwards the per-request inbound token to the backend.

## The OpenAPI Spec Is Optional (D-03)

`--spec` (the scaffold's `api.yaml`) is **optional**. A curated-only server —
only single-call and/or script `[[tools]]` — boots with no spec at all. When you
supply a spec, it is served as the `api_schema` Code Mode resource so the agent
authors scripts against your real OpenAPI contract. If `[code_mode] enabled =
true` but no spec is supplied, the server warns and proceeds: Code Mode runs
without the contract resource rather than failing.

## Step 4: Deploy

The scaffold's `deploy.toml` defaults to the `pmcp-run` target. As with the SQL
server, change `[target] type` to `aws-lambda`, `cloud-run`, or `cloudflare`
(mirror the edit into `.pmcp/deploy.toml`), then:

```bash
cargo pmcp validate deploy        # pre-flight checks before any cloud call
cargo pmcp deploy
cargo pmcp deploy outputs         # the deployed endpoint URL
```

The deploy path bundles your assets (`config.toml`, `api.yaml`) and applies the
same secret posture as the SQL server: any inline DEV `token_secret` in the
bundled config is rewritten to a `${...}` environment reference, so the deployed
artifact never ships a dev literal. Supply the secret as a deploy environment
variable. Your on-disk config is left unchanged — only the bundled copy is
sanitized.

## What You Built

You now have an OpenAPI MCP server that:

- exposes curated, typed tools for the common 20% of operations,
- safely answers the long-tail 80% through Code Mode against your OpenAPI contract,
- authenticates to the backend through one of six outgoing-auth variants,
- is changed by editing config — not by recompiling, and
- ships without ever leaking a dev secret into the deployed artifact.

For the standalone no-Rust binary form, see the `pmcp-openapi-server` crate
README; for the Code Mode internals that make the long-tail path safe, revisit
[Chapter 12.9](ch12-9-code-mode.md); for the SQL sibling, see
[Chapter 12.10](ch12-10-config-driven-sql-servers.md).
