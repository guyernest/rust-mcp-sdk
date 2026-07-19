# pmcp-team-servers

**Dev-grade reference implementations** of the four team-server tool surfaces
contracted in `contracts/team-servers-v1.yaml`, plus the in-process runtime that
composes them into a small team.

> **Experimental (0.x).** This is the SDK's *reference* team stack — contracts +
> dev-grade implementations, not a scaled production backend. Scaled
> team-memory / approval backends live on the platform (design DEFER-03).

## The four reference servers

| Server | Tools | Role |
|--------|-------|------|
| `team-fs` | `fs__*` | a local-directory filesystem workspace (draft → review) |
| `mem-mcp` | `mem__*` | an in-memory, BM25-searchable team memory |
| `approval-mcp` | `resolve_approval`, `get_approval`, `team_approval__ask_<member>` | human sign-off |
| `team-mcp` | `team_mcp__<member>` | member dispatch, forwarding to member agents under depth + ancestor-cycle guards |

Each server lives behind its own cargo feature (all on by `default`), so a
deployment can build a single-server binary via
`--no-default-features --features <server>`.

## Composition runtime

Beyond the individual servers, the crate ships the runtime that wires member
agents to the four servers from a [`TeamPackage`](../pmcp-package/README.md) —
the same primitive the `cargo pmcp team dev` verb is a thin CLI over. Members are
[`pmcp-agent`](../pmcp-agent/README.md) loops; the runtime composes them in one
process over in-memory transports (or serves `team-mcp` over HTTP).

## Features

| Feature | Enables |
|---------|---------|
| `team-fs` / `mem-mcp` / `approval-mcp` / `team-mcp` | each reference server (all on by `default`) |
| `conformance` | the conformance test harness against the v1 contracts |
| `runtime` | the in-process composition runtime |
| `member-llm` | back members with a real model (`pmcp-agent/openai-compat`) |
| `http` | serve `team-mcp` over streamable HTTP (implies `member-llm`) |
| `webhook` | outbound webhook notifications (pulls `reqwest`) |

The default publish build (no `http`/`webhook`) is `reqwest`-free and wasm-clean.

## Getting started

```bash
# Run the built-in doc-review team in one process and print its transcript:
cargo pmcp team dev

# Serve team-mcp over HTTP for an MCP client:
cargo pmcp team dev --serve --port 8080
```

See [`cargo pmcp team`](../../cargo-pmcp/docs/commands/team.md) for the full CLI
workflow, and `examples/doc_review_team.rs` for direct library usage.

## See also

- [`pmcp-agent`](../pmcp-agent/README.md) — the member agent loop
- [`pmcp-package`](../pmcp-package/README.md) — the `TeamPackage` format the runtime composes from
- [`pmcp`](../../README.md) — the core MCP SDK
