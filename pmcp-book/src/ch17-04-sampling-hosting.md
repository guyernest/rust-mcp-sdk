# Sampling & Hosting

MCP sampling runs in **two opposite directions**, and pmcp supports both without
ambiguity. Because the two patterns share the word "sampling" — and each has a
trait that is a `SamplingHandler` in spirit — it is easy to confuse them. This
page pins down which direction each one runs and which real public trait answers
the request, so you never wire the wrong side.

The short version:

- **Spec host sampling** — a *server asks the client*. The client is the host and
  answers with a `pmcp::client::host::HostSamplingHandler`.
- **LLM-server pattern** — a *client asks a server*. The server answers with a
  server-side `pmcp::SamplingHandler`.

## Spec host sampling (server asks the client)

This is the MCP-spec direction added in Phase 106: an MCP **server** issues a
`sampling/createMessage` request *to the client*, and the client — acting as the
host that owns the model access — produces the completion. The same hosting
surface also answers `elicitation/create` and `roots/list`.

The client registers its host handlers when it is built, via `ClientBuilder`:

- `on_sampling(handler)` — takes a `pmcp::client::host::HostSamplingHandler`
  whose `handle_create_message(params) -> Result<CreateMessageResult>` produces
  the completion (including `tools` / `tool_choice` and `tool_use` / `tool_result`
  content per MCP 2025-11-25).
- `on_elicitation(handler)` — a `pmcp::client::host::HostElicitationHandler`
  answering `elicitation/create`.
- `on_roots(provider)` — a `RootsProvider` closure answering `roots/list`.

### The preflight approval gate

Server-asks-client sampling is human-in-the-loop-ready. A **preflight approval
hook**, registered with `on_sampling_approval(..)`, runs *before the LLM does*:
it inspects the request and returns an `ApprovalDecision`
(`Allow`, or `Deny(reason)`). A `Deny` stops the sampling call before any model
is invoked — the default, when no hook is registered, is to allow. An optional
`on_sampling_result_review(..)` hook can additionally deny *after* generation,
suppressing a completion that has already been produced. This is the seam that
consoles and approval UIs plug into; this phase delivers the gate, not a UI.

```text
server --- sampling/createMessage --> client
                                       │  on_sampling_approval  (deny before LLM)
                                       │  HostSamplingHandler    (produce completion)
                                       │  on_sampling_result_review (deny after LLM)
server <-- CreateMessageResult ------- client
```

### Current limitation (nested flow only)

Today the client answers these inbound server→client requests only **while one of
its own requests is in flight** — a nested flow. A pure *idle* host (a client that
sits waiting and answers server sampling with no outbound request of its own
pending) needs a background receive loop, which is deferred. Relatedly, the
high-level `Server::run` loop cannot yet answer a `peer.sample()` it issues during
a tool call (it serializes request handling and would deadlock); that server-loop
fix is tracked for Phase 108. Neither limitation affects the client host surface
itself, which is fully delivered and tested.

## LLM-server pattern (client asks a server)

This is the **legacy, inverted** direction — kept and **not deprecated**. Here a
**client** calls `Client::create_message`, sending `sampling/createMessage` *to a
server*, and the *server* answers by running the model. The answering trait is
the server-side `pmcp::SamplingHandler` (the same trait is also reachable at its
module path, `pmcp::server::SamplingHandler`).

> Note the path: it is `pmcp::SamplingHandler` /
> `pmcp::server::SamplingHandler`. Those are the only two public paths for this
> trait — do not reach for any internal-module variant.

Because the caller here is the client and the answerer is the server, this is the
mirror image of spec host sampling. Nothing about this pattern's behavior changed
in Phase 106; only its documentation, which now names it the "LLM-server pattern"
to keep it distinct from spec sampling.

## Contrasting the two directions

| | Direction | Caller entry point | Answering trait | Use case |
|---|---|---|---|---|
| **Spec host sampling** | server → client | server issues `sampling/createMessage`; client built with `ClientBuilder::on_sampling(..)` | `pmcp::client::host::HostSamplingHandler` | The client hosts model access; a server (or agent) delegates completions to it, optionally gated by an approval hook. |
| **LLM-server pattern** | client → server | `Client::create_message` | `pmcp::SamplingHandler` (a.k.a. `pmcp::server::SamplingHandler`) | A server owns the LLM; clients ask it to sample. The original, still-supported inverted path. |

## Try it

A runnable end-to-end host is available as an example:

```bash
cargo run --example s49_sampling_host
```

It stands up a client with a registered `HostSamplingHandler` and drives a
server that asks it to sample, exercising the server→client direction.

---

This page focuses on **direction and hosting semantics** — which side asks and
which real trait answers. The complete "Sampling & Hosting" chapter, with full
worked examples and the approval-console walkthrough, lands with the Phase 111
docs pass.
