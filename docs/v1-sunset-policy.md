# MCP v1 (2025-11-25) Sunset Policy

## Overview

This document is the **normative** policy for how and when PMCP will stop supporting the
MCP 2025-11-25 protocol era ("v1"). It states a **condition**, not a date.

PMCP is a dual-version SDK: it speaks both MCP 2025-11-25 and the 2026-07-28 ("v2") spec,
negotiated per request. v2 is the strategic primary path. v1 remains fully supported, and
this document exists so that "fully supported" and "cheaply removable later" are both true
at the same time — and so that a future contributor can tell which is which.

## What `v1-compat` gates

`v1-compat` is a cargo feature. It is **default-on**: it is a member of both `default` and
`full`, so every existing consumer already has it and nothing changes for them.

It gates the machinery that only the 2025-11-25 era needs:

| Surface | Path | Why it is v1-only |
|---|---|---|
| `initialize` / `initialized` session lifecycle, `Mcp-Session-Id` | `src/server/streamable_http_server/v1_session.rs` | v2 is stateless — `initialize` is removed from the spec and there is no session to track |
| SSE **resumability** — `Last-Event-ID` replay and the event store backing it | `src/shared/event_store.rs` | v2 removes `Last-Event-ID`; a dropped v2 stream is re-issued as a new request rather than replayed |

The split into a dedicated `v1_session` module lands across Phase 117; the feature and the
severance proof described below are its enforcement mechanism.

## SSE framing and parsing are SHARED — do NOT gate them

This section exists to stop a well-meaning future contributor from "finishing the job".

**`src/shared/sse_parser.rs` and `src/shared/sse_optimized.rs` are NOT v1-only and must not be
feature-gated.** v2 uses Server-Sent Events too: `subscriptions/listen` is a **v2-only** method
that returns a long-lived `text/event-stream`, framed server-side and parsed client-side by the
same shared code v1 uses.

Only SSE **resumability** — the `Last-Event-ID` request header and the event store that makes
replay possible — is v1-only. Framing and parsing are shared infrastructure. Gating
`sse_parser` behind `v1-compat` would break the v2 subscription path, and the severance build
would catch it as a compile error rather than as a silent regression, but the intent should be
clear before anyone tries.

## The removal condition

Removal of v1 support is tracked as **SMPL-F1** and is:

- **Gated on public-client adoption of v2.** When the client ecosystem has moved, v1 comes out.
- **Landing in a future pmcp 3.0**, as a major-version breaking change.
- **Not scheduled.** There is **no date and no committed window** in this policy, deliberately.
  A dated window would commit the project to a deadline the ecosystem may not meet, and the
  roadmap has always described this as adoption-gated.

Until that condition is met, v1 is a supported path, not a legacy one.

## What you have to do today

**Nothing.** `v1-compat` is in `default`. An ordinary `pmcp = "2"` dependency keeps the full
2025-11-25 behavior with no code change, no feature flag, and no build change.

## How to verify severability yourself

```bash
RUSTFLAGS="-D warnings" cargo build -p pmcp --no-default-features --features full-v2
```

`full-v2` is the `full` feature list minus exactly `v1-compat`. If that command succeeds, the
crate compiles — with the real HTTP/streamable-HTTP transport present — while the v1 layer is
absent. That is the severability guarantee, as a compile-time fact rather than a claim.

Three things that can **never** prove severance, and why:

- **`--all-features`** enables `full-v2` *and* `v1-compat` at the same time. Cargo features are
  additive; they cannot be subtracted. This is also why there is no inverted `v2-only` feature —
  any crate anywhere in the dependency graph enabling a negative feature would silently strip v1
  for every other consumer in the build.
- **Workspace-wide builds** unify features across members, several of which depend on `pmcp`
  with `full`. The `-p pmcp` scope is load-bearing.
- **`--no-default-features` alone** strips `http` and `streamable-http` too, because
  `default = ["logging"]`. It would "prove" severability by never compiling the transport that
  holds the code being severed. Hence the parallel `full-v2` list.

`RUSTFLAGS="-D warnings"` is also load-bearing: without it, code stranded by the cut emits a
`dead_code` warning and the build still reports success.

`full` and `full-v2` are two enumerated lists and can drift, so
`tests/v1_severability_tripwire.rs` derives both from `Cargo.toml` at test time and fails if
they differ by anything other than `v1-compat`.

## Explicit non-commitments

This policy deliberately does **not** do any of the following, and adding them later would be a
change to this policy rather than an implementation detail:

- **No `#[deprecated]` attributes.** A `#[deprecated]` marker would emit compiler warnings at
  effectively every current user of a path that is still fully supported, and would force
  `allow()` suppressions throughout the SDK's own source.
- **No runtime warning on v1 negotiation.** Logging a warning when a client negotiates v1 would
  change v1 runtime behavior, which is exactly what the next point forbids.
- **No behavior change on the wire.** v1 request/response bytes stay identical. Feature-gating
  moves where code lives; it does not change what a v1 client observes.

## Scope of this document

This is the **normative** policy: what `v1-compat` is, what removal is conditioned on, and what a
consumer must do. It is intentionally short and precise.

The **narrative** v2 migration guide — how to opt into v2, the dual-version story, and the Tasks
extension migration — is tracked separately as DOCS-05 and links back here as the authority. It
does not restate or override anything above.
