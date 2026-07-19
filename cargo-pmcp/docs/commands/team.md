# cargo pmcp team

Run an in-process small team of agents plus the reference team servers, locally.

## Usage

```
cargo pmcp team <SUBCOMMAND>
```

## Description

`team dev` composes a small team in **one process** — member agents wired to the
four [`pmcp-team-servers`](../../../crates/pmcp-team-servers/README.md) reference
servers with dev-grade backends:

| Server | Role |
|--------|------|
| `team-fs` | a shared workspace filesystem (draft → review) |
| `approval-mcp` | human sign-off (ask / resolve) |
| `mem-mcp` | shared team memory |
| `team-mcp` | agent-facing member dispatch |

The team is wired from a [`TeamPackage`](../../../crates/pmcp-package/README.md) —
`--package` (+ `--data-dir`) or the built-in two-member doc-review fixture (the
locked default). Composition is delegated entirely to the `pmcp-team-servers`
runtime; `team dev` is a thin CLI over it, not a re-implementation.

## Subcommands

| Subcommand | Description |
|------------|-------------|
| `dev` | Run an in-process small team (member agents + the four reference servers) |

---

## team dev

Three behaviors:

- **default** — compose the team over in-memory transports with an offline source
  and print a labeled transcript of the 7-step doc-review flow (draft → publish →
  ask reviewer → record verdict → read → remember → dispatch), then tear down
  cleanly. Fully offline and deterministic — no network, no LLM, no sockets.
- **`--serve`** — expose `team-mcp` over HTTP on `127.0.0.1:<port>` (via the shipped
  `team-mcp` serve recipe), running until Ctrl-C. Point any MCP client at it.
- **`--llm <endpoint>`** — swap the offline source for an OpenAI-compatible source so
  members are backed by a real model.

```
cargo pmcp team dev [--serve] [--llm <URL>] [OPTIONS]
```

### Options

| Option | Description |
|--------|-------------|
| `--package <PATH>` | Team package to run (defaults to the built-in doc-review fixture) |
| `--data-dir <DIR>` | Directory for team-fs / mem-mcp state and member `AgentPackage`s |
| `--serve` | Serve team-mcp over HTTP instead of running the in-process transcript |
| `--port <PORT>` | Port for the HTTP serve path (default `8080`) |
| `--llm <URL>` | LLM endpoint for members backed by a real model (swaps the offline source) |
| `--model <MODEL>` | Model id passed to the LLM source (`--llm` only; default `llama3.2`) |
| `--llm-api-key-env <VAR>` | Environment variable holding the LLM API key (`--llm` only) |
| `--allow-insecure-http` | Allow a plain-HTTP (non-TLS) LLM endpoint (`--llm` only) |

### Examples

```
# Offline doc-review transcript (the built-in fixture):
cargo pmcp team dev

# Serve team-mcp over HTTP for an MCP client to drive:
cargo pmcp team dev --serve --port 8080

# Back the members with a real model:
cargo pmcp team dev --llm https://api.example.com/v1 --llm-api-key-env MY_API_KEY

# Run your own team package with its member data:
cargo pmcp team dev --package ./team.package.json --data-dir ./team-mcp-data
```

## See also

- [`pmcp-team-servers`](../../../crates/pmcp-team-servers/README.md) — the four reference servers and the composition runtime
- [`cargo pmcp agent`](agent.md) — scaffold and run the member agents
- [`cargo pmcp package`](package.md) — inspect a team package locally
