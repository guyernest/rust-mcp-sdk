# Requirements: PMCP SDK — Milestone v2.5 (MCP Spec 2026-07-28 v2 Support)

**Defined:** 2026-07-22
**Core Value:** One pmcp server binary transparently serves both MCP 2025-11-25 and 2026-07-28 clients via per-request negotiation — with v2 as the strategic primary path (stateless/Lambda-first, Tasks, MCP Apps) and v1 as a cleanly severable compatibility layer.

**Strategic stance (from milestone scoping):** The v2 spec validates pmcp's existing focus decisions (stateless serverless deployment, streamable HTTP over SSE, Tasks for long-running tools, MCP Apps). v2.5 uses the spec transition as a simplification opportunity: pmcp's own clients (`pmcp` Client, `pmcp-agent`) upgrade to v2, public-client adoption (ChatGPT, Claude, Gemini, Copilot) is assumed to be fast, and legacy v1 client support is architected for sunset — not dragged indefinitely.

## v1 Requirements

### Version Plumbing & Negotiation (VERS)

- [x] **VERS-01**: Server resolves a `ProtocolContext` (era, negotiated version, clientInfo, clientCapabilities) once at transport ingress and threads it through dispatch; handlers read it via typed accessors on `RequestHandlerExtra`
- [x] **VERS-02**: pmcp supports protocol version 2026-07-28 as an explicit opt-in; `LATEST_PROTOCOL_VERSION` stays pinned to 2025-11-25 and existing v1 clients negotiate exactly as before (milestone stays a 2.x minor)
- [x] **VERS-03**: v2 requests self-describe via per-request `_meta` (`io.modelcontextprotocol/protocolVersion`, `clientInfo`, `clientCapabilities`); v2 results carry `serverInfo`
- [x] **VERS-04**: Server implements `server/discover` as a read-only projection of already-computed ServerCore capabilities
- [x] **VERS-05**: Required headers `Mcp-Method`/`Mcp-Name` (alongside `MCP-Protocol-Version`) are enforced inbound and emitted outbound on the v2 HTTP path
- [x] **VERS-06**: All protocol error codes live in one centralized version-gated constant table; v2 values are filled ONLY from the final 2026-07-28 schema.json (resolving the `-32002`/`-32602` conflict), and the frozen v1 `-32002` task-pending semantics stay unchanged
- [x] **VERS-07**: All results carry the `resultType` envelope discriminator (`complete`/`input_required`/`task`); a missing `resultType` defaults to `complete` for backcompat
- [x] **VERS-08**: The `extensions` capability map (reverse-DNS IDs) is supported in capability negotiation
- [x] **VERS-09**: W3C trace-context keys (`traceparent`/`tracestate`/`baggage`) in `_meta` are surfaced via typed accessors and propagated through dispatch

### Stateless HTTP & Multi-Round-Trip (HTTP)

> **Status marker `[~]` — implemented, gated on the final schema.** Every `[~]` HTTP-0x and CLNT-0x
> requirement below is **implemented and green** at Phase-113 HEAD, but none is marked complete.
> **HTTP-09 is the exception: it is `[ ]`, not `[~]`** — it is a genuine open gap, not a
> publication-gated one, and it does not clear on 2026-07-28.
> `113-SPEC-RECHECK.md`'s `## Verdict` is still `PENDING`: as re-verified on 2026-07-26 there is
> no `schema/2026-07-28` directory upstream, so the wire constants Phase 113 landed
> (`-32020`/`-32021`/`-32022`) are **pre-final values held under a written developer exception**.
> The exception's re-verification obligation is binding and a mismatch is a phase-reopening
> event. Re-run the checkpoint on or after 2026-07-28 and flip these to `[x]` only then.

- [~] **HTTP-01**: v2 HTTP requests run with no `initialize` handshake and no `Mcp-Session-Id`, era-gated onto the existing `stateless()` branch; v1 session behavior is unchanged — *implemented; pending final schema*
- [~] **HTTP-02**: A server handler can return `input_required` with `inputRequests` and an opaque `requestState` that is integrity-protected, principal-bound, and TTL'd — *implemented; pending final schema*
- [~] **HTTP-03**: A client retry of the original request carrying `inputResponses` + echoed `requestState` resumes the operation correctly (multi-round-trip elicitation end-to-end) — *implemented; pending final schema*
- [~] **HTTP-04**: On the v2 path, `resources/subscribe`/`unsubscribe` are removed and change notifications are instead delivered over a `subscriptions/listen` long-lived stream — *implemented; pending final schema*
- [~] **HTTP-05**: SSE resumability (`Last-Event-ID`) is not offered on the v2 path, and a regression test proves response JSON-RPC ids are always derived from the live request (the id-replay / discovery-cache bug class) — *implemented; pending final schema*
- [~] **HTTP-06**: The HTTP GET stream endpoint is not served on the v2 path (transport-level removal, distinct from HTTP-04's method-level removal) — *implemented; pending final schema*
- [~] **HTTP-07**: The `subscriptions/listen` stream's frame protocol: `notifications/subscriptions/acknowledged` is the mandatory first frame, and every notification **delivered on a subscription stream** carries `io.modelcontextprotocol/subscriptionId` tagging (the key is REQUIRED on `SubscriptionsListenResultMeta` but OPTIONAL on `NotificationMetaObject` — it is absent for notifications not delivered via a subscription, so this is a stream-path obligation, not a universal type requirement) — *implemented; pending final schema*

> **⚠ HTTP-07 rests on the least-settled part of the spec.** Both its obligations are **post-RC
> additions**: at tag `2026-07-28-RC`, `grep -c subscriptionId` = 0, and the acknowledgement
> docblock was descriptive with **no MUST**. They landed via PRs #2889/#2953 (June 17/23) and open
> **PR #3006 still targets this exact surface**. This is the highest-drift-risk requirement in the
> phase — see `113-SPEC-RECHECK-ADDENDUM-2026-07-26.md` Finding 9.
- [~] **HTTP-08**: Subscription delivery is opt-in and self-consistent: the four capability opt-ins (`toolsListChanged`/`promptsListChanged`/`resourcesListChanged`/`resourceSubscriptions`) gate the stream; a server advertising none may answer `subscriptions/listen` with method-not-found and remain conformant **per the conformance suite's SKIPPED grading and the spec's generic method-not-found rule** (the spec says nothing about this for `subscriptions/listen` specifically); a tripwire test enforces that advertising any subscription capability obliges serving the stream — **this advertise-implies-serve rule is CONFORMANCE-SUITE POLICY, not spec: it comes from `conformance/src/scenarios/server/stateless.ts:988-1015`, and no spec sentence creates it** — *implemented; pending final schema*

> **⚠ HTTP-08 is gated on a source the schema re-check cannot see.** Its predicate lives in the
> **conformance repo**, not the schema — `subscriptions.mdx` contains no capability-gating rule and
> `ServerCapabilities` has no `subscriptions` capability. `113-SPEC-RECHECK.md` pins only a schema
> sha, so drift in `advertisesSubscriptions` is undetectable by the current gate. The gate needs a
> second arm pinning a conformance-repo sha (currently `a865118206d4d8cc8dbc5f5201607839281d0c3b`).
- [ ] **HTTP-09**: Every peer-controlled read on the v2 transport path is memory-bounded. Closure is **enumerable, not narrative**: a tripwire test asserts that no unbounded whole-body read (`.collect()`, `read_to_end`) and no unbounded accumulation over peer-supplied bytes exists in `src/shared/`, `src/client/subscriptions.rs`, or `src/server/streamable_http_server.rs` outside an explicit reviewed allowlist, and that no scan over peer-chosen input is worse than O(n). — *NOT met; see below*

> **Why HTTP-09 exists.** The "memory-bounded long-lived stream" criterion was a *derived* success
> criterion of the old HTTP-04 — it appeared in no requirement text, so it had no enumerable
> closure condition. It reopened three times (plans 113-14/15/16, 113-17/20, then the 2026-07-26
> full-phase review), each round capping the specific sites that round's findings named while the
> next review found another unnamed site: a 4th uncapped `collect()` in `rejection_error`, an
> uncapped `HttpTransport::send_request`, and an O(n²) `take_utf8_prefix` sitting *upstream* of
> every bound the phase had added. Those three are fixed (commit `5f045086`), but the requirement
> is stated as an **invariant with a mechanical check** so the next review cannot miss a site by
> omission. It stays `[ ]` until that tripwire test exists — the fixes alone do not satisfy it.

#### Positioning & known limitations carried out of the old HTTP-04

These two clauses were embedded in the pre-split HTTP-04. Neither is a requirement — neither has
a pass/fail closure condition — so both are recorded here as standing context rather than as
checkboxes a verifier can fail on.

- **D-11 positioning.** Polling over the Tasks mechanism remains pmcp's RECOMMENDED enterprise
  mechanism, documented as a pmcp extension and explicitly **not** a conformant substitute for the
  `subscriptions/listen` stream. Verifiable only as a documentation claim; belongs to DOCS-05.
- **Deployment limitation (plan 113-10).** The `ListenRegistry` is instance-local, so advertising a
  subscription capability behind a non-sticky load balancer under-delivers notifications. A
  build-time `tracing::warn!` names this but does not prevent it. This is a known limitation, not
  an obligation — it is satisfied by being documented, not by being fixed.

### Tasks Extension Migration (TASK)

- [ ] **TASK-01**: Tasks are negotiated on v2 via the extensions map (`io.modelcontextprotocol/tasks`); v1 `experimental.tasks` negotiation continues to work
- [ ] **TASK-02**: A client can feed input into a running task via `tasks/update`
- [ ] **TASK-03**: `tasks/list` (and blocking `tasks/result` semantics per final spec) are era-gated off on v2 while remaining fully functional for v1 consumers
- [ ] **TASK-04**: v2 task-augmented results use `resultType:"task"` with `CreateTaskResult{taskId,status,ttlMs,pollIntervalMs}`, and the v1 5-state machine maps deterministically to the v2 status enum (`working|input_required|completed|failed|cancelled`)
- [ ] **TASK-05**: On v2, task owner binding requires OAuth `sub` or a stable per-request identity and fails closed when absent (no session-id fallback); a security test proves no cross-caller task visibility
- [ ] **TASK-06**: The `TaskStore` trait, state machine, and DynamoDB/Redis/in-memory backends survive unchanged — the migration is a wire-API reshape behind the `TaskRouter` boundary, not a storage rewrite

### JSON Schema 2020-12 & Caching Hints (SCHM)

- [ ] **SCHM-01**: Schema validation runs Draft 2020-12 explicitly pinned (jsonschema 0.48, no `$schema` auto-detect), staying wasm-clean and SEP-2106-compliant (no external `$ref` dereference)
- [ ] **SCHM-02**: On v2, `structuredContent` accepts any JSON value (scalar/array/null/object); v1-negotiated tools keep the existing object-shaped behavior
- [ ] **SCHM-03**: The five list/read results carry `ttlMs`/`cacheScope` caching hints (additive fields)

### Auth Hardening (AUTH)

- [ ] **AUTH-01**: OAuth callback validates RFC 9207 `iss` (strict on v2, lenient on v1 to protect existing deployments)
- [ ] **AUTH-02**: Dynamic client registration sends/accepts `application_type`
- [ ] **AUTH-03**: The remaining auth-hardening SEPs (issuer-keyed credential storage + the three clarifications) are applied without breaking existing v1 OAuth deployments (Lambda `oauth_passthrough`, documented proxy exceptions)

### Client & Agents on v2 (CLNT)

- [~] **CLNT-01**: The pmcp `Client` can speak v2: per-request `_meta` emission, `server/discover`, required headers, no `initialize` — selected explicitly per connection — *implemented; pending final schema*
- [~] **CLNT-02**: The pmcp `Client` fulfills MRTR `input_required` results by producing `inputResponses` — the Phase-106 host handlers (sampling/elicitation/roots) are folded into this flow on v2 — *implemented; pending final schema*
- [ ] **CLNT-03**: `pmcp-agent` (including its `ToolInvoker` and task polling) works end-to-end against a v2 server
- [ ] **CLNT-04**: `mcp-tester` can exercise a v2 server (headers, discover, stateless flow) for dual-version testing
- [~] **CLNT-05**: The pmcp `Client` exposes `subscriptions_listen` returning a typed `SubscriptionStream` of notifications, and the retired `subscribe_resource`/`unsubscribe_resource` methods fail fast with a typed `retired_on_v2` error on v2 (client half of HTTP-04/07/08) — *implemented; pending final schema*

### Simplification & v1 Sunset (SMPL)

- [ ] **SMPL-01**: v1-only machinery (initialize/session lifecycle, SSE resumability) is isolated behind a clearly severable era-gated layer with a documented legacy-support sunset policy — removal in a future major is a deletion, not a refactor
- [ ] **SMPL-02**: The v2 code path carries no session/SSE-resumability baggage, and a simplification pass removes code the v2 model obsoletes wherever v1 compatibility permits

### Conformance (CONF)

- [ ] **CONF-01**: The official `@modelcontextprotocol/conformance` suite (pinned to a commit, re-pinned after the final spec) runs in CI against a dual-version pmcp server example over real HTTP
- [ ] **CONF-02**: The Phase-109 Rust conformance harness gains v2 fixtures while v1 fixtures stay green (dual conformance, verified with a dev-dependency-free build to avoid feature-unification false-greens)
- [ ] **CONF-03**: Deprecated Roots/Sampling/Logging capabilities remain fully functional under v2 negotiation (advisory-only deprecation, 12-month window)

### Docs in Three Shapes (DOCS — continues v2.4 numbering)

- [ ] **DOCS-04**: Agents & Teams documented in three shapes (pmcp-book chapters, runnable examples, README/course), cargo-pmcp-first — carried from v2.4 Phase 111
- [ ] **DOCS-05**: v2 migration guide + dual-version documentation: how to opt into v2, the dual-version story, Tasks extension migration, and the legacy sunset policy
- [ ] **DOCS-06**: Runnable v2 examples: a stateless (Lambda-style) v2 server and a v2 client/agent example

### Unassigned — Awaiting Phase Assignment (UNAS)

In-milestone requirements surfaced after roadmap creation. **These are NOT deferred to a later
milestone** — they belong to v2.5 but have no phase yet. Assign them during the next
`/gsd:plan-phase` pass.

- [ ] **UNAS-01**: SEP-2243 `x-mcp-header` / `Mcp-Param-{Name}` support — the v2 transport spec says clients **MUST** support `x-mcp-header` mirroring, and the header-mismatch validation table covers `Mcp-Param-*` alongside `Mcp-Method`/`Mcp-Name`. **No current requirement covers it**: not VERS-05 (which scopes only `Mcp-Method`/`Mcp-Name`), not HTTP-01..05, not CLNT-01. Surfaced by 113-RESEARCH.md assumption A8 and Open Question 4, both of which explicitly resolved *not* to absorb it into Phase 113 — no Phase-113 plan implements `Mcp-Param-{Name}` mirroring. It is **closest to CLNT-01's header work** (the client's outbound required-header emission) and would most naturally extend the server-side `classify_v2_request` matrix that Phase 112 landed. **UNASSIGNED — do not fold this into a phase without an explicit scoping decision.**

## v2.6 Requirements — AI-Package Portability (Phases 120-124)

Defined 2026-07-27. Scoped against `pmcp-package` 0.1.0 and `pmcp-openapi-server` 0.1.0 as they
stand, and against two milestone-scoping decisions: attestation is **pmcp.run-issued** (so the SDK
carries and verifies, and adds **no crypto dependency**) and **GraphQL mediates import** (so the CLI
adds **no registry client**). Both decisions put the critical path in the pmcp.run backend, which is
why PKGX-01/02 are contract-first.

### Package Portability (PKG)

- [ ] **PKG-01**: A server with **no bespoke binary** can be packed. Vendor media types carry the server's own `config.toml` and its OpenAPI spec as layers, so a Shape A config-only server (`pmcp-openapi-server`) has a complete package identity. Today `pack_server` requires `bootstrap: &[u8]` and neither file has a layer type.
- [ ] **PKG-02**: The binary is **dual-mode** — embedded (bootstrap bytes, for a new server or new version) or referenced (`BinaryRef { digest, media_type }` resolved in the target environment, for a server already deployed there). Both modes are required; `BinaryRef` already has the right shape but nothing resolves it.
- [ ] **PKG-03**: What is **baked** versus what is a **slot** is decided and documented. Working split: the OpenAPI spec is baked (it defines the tool surface — change it and it is a different package); endpoint, credentials and auth mode are slots filled at unpack.
- [ ] **PKG-04**: A package round-trips between environments with **tool-list parity** as the asserted property: pack in A → unpack in B → `detect_deviation` names exactly the slots B must fill → fill them → the served tool list matches A. Asserted on behaviour via the existing `parity_replay.rs`, never on manifest structure, so it survives the manifest refactors this milestone expects.

### Package Exchange (PKGX — contract-first, backend-dependent)

- [ ] **PKGX-01**: A package carries a **pmcp.run-issued attestation** and can be verified against pmcp.run's identity on import. The SDK provides carriage and verification only — no signing, no crypto dependency. (`digest::verify` is and remains an integrity check, not a signature check.) In-repo half is a vendored contract plus an offline blocking contract test.
- [ ] **PKGX-02**: `cargo pmcp package pack | unpack | export | import`, resolving environments through `configure`'s existing resolver and reusing the working `deployment/targets/pmcp_run/{graphql,auth}.rs` seam rather than a second API path. `pack`/`unpack` are local and land immediately; `export`/`import` are contract-first against the platform's import contract.

### Release Hygiene (PKGR)

- [ ] **PKGR-01**: `pmcp-openapi-server` is added to CLAUDE.md's publish order. It is absent today (zero occurrences) and would silently not publish, unlike its siblings `pmcp-sql-server` and `pmcp-workbook-server`.

> **⚠ PKGX-01 and PKGX-02 cannot fully close inside this repo.** Both need pmcp.run backend work —
> package import and attestation issuance — that was not confirmed as scheduled. They are written so
> the in-repo half is completable and offline-verifiable; promote them to blocking and add the live
> E2E leg once the backend is scheduled.

## Future Requirements

Deferred to a later milestone. Tracked but not in the current roadmap.

### Deferred from v2.5 scoping

- **VERS-F1**: `server/discover` as a client-side STDIO backcompat probe (safe downgrade detection) — deferred by explicit scoping choice
- **APPS-F1**: MCP Apps alignment to its official-extension form (gives the Phase 45 rework a fixed target) — needs its own scoping pass
- **SMPL-F1**: Actual v1 (2025-11-25) support removal — a future pmcp 3.0, gated on public-client v2 adoption; v2.5 only makes it cheaply severable (SMPL-01)
- **CLI-F1**: cargo-pmcp scaffolds defaulting new projects to v2-first configuration

## Out of Scope

Explicitly excluded. Documented to prevent scope creep.

| Feature | Reason |
|---------|--------|
| Hard cutover to v2 (dropping 2025-11-25) | Ecosystem still overwhelmingly v1; final spec publishes 2026-07-28. Dual-version now, sunset later per SMPL-01 policy. |
| Hard-coding new `-3202x`/`-32602` error codes before the final schema | RC error-code allocation renumbered post-RC and conflicts with frozen pmcp codes — VERS-06 fills values from final schema.json only. |
| Rewriting `pmcp-tasks` for the extension | TaskStore/backends/CAS/security model all survive; only the wire API reshapes (TASK-06). |
| Removing Roots/Sampling/Logging | Deprecated, not removed — 12-month advisory window; zero work beyond CONF-03 runtime verification. |
| SSE resumability on the v2 path | v2 removes `Last-Event-ID`; retrofitting fights the stateless model. Re-issue as new request. |
| Per-connection list caching / stateful load balancing | v2 requires list endpoints not vary per connection; `ttlMs`/`cacheScope` is the spec-blessed alternative. |
| Adding `oauth2`/`openidconnect` crates | Duplicates the hand-rolled flow, pulls reqwest, breaks wasm-clean posture — auth SEPs land as source changes. |

## Traceability

Which phases cover which requirements. Updated during roadmap creation.

| Requirement | Phase | Status |
|-------------|-------|--------|
| VERS-01 | Phase 112 | Complete |
| VERS-02 | Phase 112 | Complete |
| VERS-03 | Phase 112 | Complete |
| VERS-04 | Phase 112 | Complete |
| VERS-05 | Phase 112 | Complete |
| VERS-06 | Phase 112 | Complete |
| VERS-07 | Phase 112 | Complete |
| VERS-08 | Phase 112 | Complete |
| VERS-09 | Phase 112 | Complete |
| HTTP-01 | Phase 113 | Implemented — pending final schema |
| HTTP-02 | Phase 113 | Implemented — pending final schema |
| HTTP-03 | Phase 113 | Implemented — pending final schema |
| HTTP-04 | Phase 113 | Implemented — pending final schema |
| HTTP-05 | Phase 113 | Implemented — pending final schema |
| HTTP-06 | Phase 113 | Implemented — pending final schema |
| HTTP-07 | Phase 113 | Implemented — pending final schema |
| HTTP-08 | Phase 113 | Implemented — pending final schema |
| HTTP-09 | Phase 113 | **NOT met** — needs the bounded-read tripwire test |
| CLNT-01 | Phase 113 | Implemented — pending final schema |
| CLNT-02 | Phase 113 | Implemented — pending final schema |
| CLNT-05 | Phase 113 | Implemented — pending final schema |
| TASK-01 | Phase 114 | Pending |
| TASK-02 | Phase 114 | Pending |
| TASK-03 | Phase 114 | Pending |
| TASK-04 | Phase 114 | Pending |
| TASK-05 | Phase 114 | Pending |
| TASK-06 | Phase 114 | Pending |
| SCHM-01 | Phase 115 | Pending |
| SCHM-02 | Phase 115 | Pending |
| SCHM-03 | Phase 115 | Pending |
| AUTH-01 | Phase 116 | Pending |
| AUTH-02 | Phase 116 | Pending |
| AUTH-03 | Phase 116 | Pending |
| CLNT-03 | Phase 117 | Pending |
| CLNT-04 | Phase 117 | Pending |
| SMPL-01 | Phase 117 | Pending |
| SMPL-02 | Phase 117 | Pending |
| CONF-01 | Phase 118 | Pending |
| CONF-02 | Phase 118 | Pending |
| CONF-03 | Phase 118 | Pending |
| DOCS-04 | Phase 119 | Pending |
| DOCS-05 | Phase 119 | Pending |
| DOCS-06 | Phase 119 | Pending |
| UNAS-01 | **unassigned** | Awaiting phase assignment |

**Coverage:**

- v1 requirements: 38 total
- Mapped to phases: 38 ✓
- Unmapped: 0
- **Added after roadmap creation: 1 (UNAS-01, SEP-2243 `x-mcp-header`) — UNMAPPED, needs a phase**
- Running total: 39 requirements, 38 mapped, **1 unmapped**

**Status-marker legend:**

| Marker | Meaning |
|--------|---------|
| `[x]` / Complete | Shipped and verified |
| `[~]` / Implemented — pending final schema | Code shipped and green, but the completion gate (`113-SPEC-RECHECK.md` `## Verdict` == `PUBLISHED-*`) has not passed. Re-run on or after 2026-07-28. |
| `[ ]` / Pending | Not started |

**Phase map (8 phases, 112-119):**

- Phase 112 Version Plumbing Spine — VERS-01..09 (9)
- Phase 113 Stateless HTTP + MRTR — HTTP-01..05, CLNT-01, CLNT-02 (7)
- Phase 114 Tasks Extension Migration — TASK-01..06 (6)
- Phase 115 JSON Schema 2020-12 + Caching Hints — SCHM-01..03 (3)
- Phase 116 Auth Hardening SEPs — AUTH-01..03 (3)
- Phase 117 Agents, Tester & v1 Severability — CLNT-03, CLNT-04, SMPL-01, SMPL-02 (4)
- Phase 118 Conformance — CONF-01..03 (3)
- Phase 119 Documentation — DOCS-04..06 (3)

---
*Requirements defined: 2026-07-22*
*Last updated: 2026-07-22 — traceability populated by v2.5 roadmap (Phases 112-119, 38/38 mapped)*
