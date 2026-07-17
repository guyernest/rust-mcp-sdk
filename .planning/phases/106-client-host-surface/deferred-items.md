# Phase 106 — Deferred / Flagged Items

## D-106-A: `Server::run` cannot answer its own server→client requests during a tool call

**Discovered:** 2026-07-17 (plan 106-01, Task 3)

**Severity:** Medium — blocks the high-level end-to-end demo of "a tool calls
`extra.peer().sample()` and the hosting client answers", but does NOT affect the
client host surface itself (fully delivered and tested via a raw duplex pump).

**Detail:** `Server::run`'s `spawn_message_handler` drives a single serialized
loop: `receive → handle_transport_message → (for a Request) await handle_request
inline`. When a tool handler blocks on `extra.peer().sample()` / `.list_roots()`,
the outbound request is sent, but the client's response can only be read by that
same loop — which is busy `await`ing the tool handler. The default dispatcher
built in `run()` has no timeout, so it hangs indefinitely. Confirmed empirically
(tests hung >60s) and by inspection of `src/server/mod.rs` (`spawn_message_handler`
/ `handle_request_message` await the handler inline).

**Why not fixed here:** This is a pre-existing server-side concurrency limitation.
Fixing it means spawning per-request handling in the server message loop
(ordering, cancellation, backpressure implications) — an architectural change to
`ServerCore`/`Server::run`, out of scope for this client-focused, additive plan
(Rule 4 territory).

**Impact on this plan:** The client host surface (answering inbound
sampling/elicitation/roots) is complete and proven via a raw duplex pump that
drives the server side by hand. The `s49_sampling_host` example likewise uses a
hand-rolled mock server.

**Recommended owner:** Phase 108 (`SamplingSource` / `pmcp-agent`) — that phase
builds the agent-as-server-that-hosts-sampling flow on this surface and will need
the server loop to process an inbound response while a tool handler is awaiting a
peer request. Consider spawning tool-handler invocations (or at least the
`peer.*` round-trip path) in `Server::run`.
