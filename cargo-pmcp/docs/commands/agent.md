# cargo pmcp agent

Scaffold and run deploy-anywhere agents (an agent is an MCP *client* with a loop).

## Usage

```
cargo pmcp agent <SUBCOMMAND>
```

## Description

An agent is a `pmcp-agent` loop that drives a completion source (an LLM) against a
set of tools. `cargo pmcp agent` is the on-ramp for building and running one — it
mirrors the server story (`new` to scaffold, `dev` to run locally), so the same
muscle memory carries over.

- `agent new` scaffolds a compilable agent crate: an
  [`AgentPackage`](../../../crates/pmcp-package/README.md) manifest
  (`agent.package.json`), a manifest-driven runner, the full dependency set, and an
  in-scaffold version-pin tripwire test against `pmcp-agent`.
- `agent dev` runs the loop locally against an OpenAI-compatible endpoint, a
  sampling host, or an offline fixed source.

An agent deploys through the existing target adapters — an agent-as-server is just a
server binary, so `cargo pmcp deploy` applies once you wrap it.

## Subcommands

| Subcommand | Description |
|------------|-------------|
| `new` | Scaffold a new agent package (AgentPackage manifest + runnable crate) |
| `dev` | Run an agent loop against a completion source |

---

## agent new

Scaffold a runnable agent crate at `./<name>` (or `<path>/<name>`). The emitted
crate compiles as-is: it loads its own `agent.package.json`, drives `AgentEngine`,
and ships a `tests/pin.rs` tripwire that fails if the generated `pmcp-agent` pin
drifts.

```
cargo pmcp agent new <NAME> [--path <DIR>] [--force]
```

### Options

| Option | Description |
|--------|-------------|
| `<NAME>` | Name of the agent package to scaffold (positional, required; validated as a Cargo crate name) |
| `--path <DIR>` | Parent directory to create the package in (defaults to the current dir, so the package lands at `./<name>`) |
| `--force` | Overwrite an existing **non-empty** destination directory. A symlinked or file destination is always refused |

### Example

```
$ cargo pmcp agent new demo_agent
✓ Agent package created successfully!

🚀 Next Steps (deploy-anywhere agent):
  1. Enter your package:
     cd demo_agent
  2. Run it (drives the agent loop; edit agent.package.json to point at your model):
     cargo run
  3. Verify the pin tripwire stays green:
     cargo test --test pin
```

---

## agent dev

Run the agent loop locally. `--source` selects where completions come from:

- `openai-compat` (default) — an OpenAI-compatible HTTP endpoint (defaults to local
  Ollama at `http://localhost:11434/v1`).
- `sampling` — serve the agent as an `AgentServer` over stdio; an MCP host supplies
  the LLM via `sampling/createMessage`.
- `fixed` — a scripted, offline source (no network) for smoke tests and CI.

The agent definition is **loaded** from `--package`, else `./agent.package.json`
(connecting `agent new` → `agent dev`), else a built-in demo — never a hardcoded
fixture.

```
cargo pmcp agent dev [--source <openai-compat|sampling|fixed>] [OPTIONS]
```

### Options

| Option | Description |
|--------|-------------|
| `--source <KIND>` | `openai-compat` (default), `sampling`, or `fixed` |
| `--package <PATH>` | Agent package to run (defaults to `./agent.package.json`, else a built-in demo) |
| `--endpoint <URL>` | Endpoint URL (openai-compat only; defaults to local Ollama) |
| `--model <MODEL>` | Model id passed to the source (openai-compat only; default `llama3.2`) |
| `--api-key-env <VAR>` | Environment variable holding the API key (openai-compat only; never passed on argv) |
| `--allow-insecure-http` | Allow a plain-HTTP (non-TLS) endpoint (openai-compat only). Remote `http://` is blocked by default |

### Examples

```
# Offline smoke test — no network, no LLM:
cargo pmcp agent dev --source fixed

# Against a local Ollama model (the default endpoint):
cargo pmcp agent dev --model llama3.2

# Against a remote OpenAI-compatible endpoint with a key from the environment:
cargo pmcp agent dev --endpoint https://api.example.com/v1 --api-key-env MY_API_KEY

# Serve as a sampling-hosted MCP server (the host provides the LLM):
cargo pmcp agent dev --source sampling
```

## See also

- [`pmcp-agent`](../../../crates/pmcp-agent/README.md) — the agent-loop crate the scaffold and runner build on
- [`cargo pmcp team`](team.md) — run a small team of agents plus the reference servers
- [`cargo pmcp package`](package.md) — inspect a packaged agent locally
