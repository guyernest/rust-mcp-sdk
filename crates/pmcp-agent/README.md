# pmcp-agent

The **deploy-anywhere agent decision loop** for the PMCP SDK. `pmcp-agent` is a
pure decision loop that runs between three object-safe async effect seams,
configured from an [`AgentPackage`](../pmcp-package/README.md). It is isolated
from `pmcp` core (like `pmcp-tasks`) so the agent runtime can evolve on its own
`0.x` cadence.

> **Experimental (0.x).** The API may change between minor releases.

## The three effect seams

An agent is *just* a loop over three seams — swap any of them without touching the
loop:

| Seam | Responsibility |
|------|----------------|
| `CompletionSource` | produce the next model completion (the LLM) |
| `ToolInvoker` | run a tool call and return its result |
| `ConversationStore` | persist and replay conversation state |

The loop itself (`AgentEngine`) is pure decision logic over these seams, so the
same agent runs offline in a test, against a local model, or as a hosted server —
only the seam implementations change.

## What's in the crate

- **`seams`** — the three object-safe effect seams + shared `RetryClass`.
- **`config`** — `ResolvedAgentConfig`, the runtime-config contract resolved from
  an `AgentPackage`.
- **`iteration`** — the pure decision functions and the async `AgentEngine` that
  drives them to a terminal `RunOutcome`.
- **`sources`** — `CompletionSource` implementations (OpenAI-compatible, Anthropic,
  and a scripted fixed source).
- **`invoker`** — a tasks-aware `ToolInvoker` and connector factory.
- **`adapter`** — an agent-as-server adapter over `ServerCore`, so an agent can be
  served as a normal MCP server binary and deployed through the usual targets.
- **`trace`** — a public `EffectTrace` replay artifact.

## Features

All network-backed sources are **non-default**, so the default build is
`reqwest`-free and wasm-clean:

| Feature | Enables |
|---------|---------|
| `openai-compat` | the OpenAI-compatible HTTP `CompletionSource` (pulls `reqwest`) |
| `anthropic` | the Anthropic `CompletionSource` (pulls `reqwest`) |
| `url-connector` | a URL/streamable-HTTP tool connector |

## Getting started

The fastest way to a runnable agent is the CLI, which scaffolds a crate that
depends on `pmcp-agent` and drives this loop:

```bash
cargo pmcp agent new my-agent      # scaffold an AgentPackage + runner
cargo pmcp agent dev --source fixed # run the loop offline
```

See [`cargo pmcp agent`](../../cargo-pmcp/docs/commands/agent.md) for the full CLI
workflow, and the `examples/` in this crate for direct library usage.

## Known limitations (0.x)

This is an experimental `0.x` crate. Two capabilities are on the `0.2` roadmap and
are **not** wired in `0.1`:

- **Provider-native opaque blocks are not preserved across turns.** The Anthropic
  source drops unknown response blocks, so an agent that uses Anthropic *extended
  thinking* (or Gemini `thought_signature`, or provider cache markers) will not
  replay those blocks on the following turn. A first-class carrier for
  provider-native extras on the completion result is planned for `0.2`.
- **Durable-host seams are not final.** An iteration gate, step identity on seam
  calls, a task-yield outcome, and conditional (fenced) stores — the hooks a
  durable Lambda host needs to fully drive the loop — are being co-designed for
  `0.2`.

What *does* work in `0.1`: the pure loop with crash-safe checkpointing and
retry-as-data; tool discovery (the loop advertises the connector's `tools/list`
schemas to the model, filtered by the package's tool selection); and provider
token-usage accounting (mapped into the run's cumulative token budget).

## See also

- [`pmcp-package`](../pmcp-package/README.md) — the `AgentPackage` format this loop is configured from
- [`pmcp-team-servers`](../pmcp-team-servers/README.md) — compose agents into a team
- [`pmcp`](../../README.md) — the core MCP SDK
