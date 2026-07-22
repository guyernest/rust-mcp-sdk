# Requirements: PMCP SDK — Milestone v2.5 (MCP Spec 2026-07-28 v2 Support)

**Defined:** 2026-07-22
**Core Value:** One pmcp server binary transparently serves both MCP 2025-11-25 and 2026-07-28 clients via per-request negotiation — with v2 as the strategic primary path (stateless/Lambda-first, Tasks, MCP Apps) and v1 as a cleanly severable compatibility layer.

**Strategic stance (from milestone scoping):** The v2 spec validates pmcp's existing focus decisions (stateless serverless deployment, streamable HTTP over SSE, Tasks for long-running tools, MCP Apps). v2.5 uses the spec transition as a simplification opportunity: pmcp's own clients (`pmcp` Client, `pmcp-agent`) upgrade to v2, public-client adoption (ChatGPT, Claude, Gemini, Copilot) is assumed to be fast, and legacy v1 client support is architected for sunset — not dragged indefinitely.

## v1 Requirements

### Version Plumbing & Negotiation (VERS)

- [x] **VERS-01**: Server resolves a `ProtocolContext` (era, negotiated version, clientInfo, clientCapabilities) once at transport ingress and threads it through dispatch; handlers read it via typed accessors on `RequestHandlerExtra`
- [x] **VERS-02**: pmcp supports protocol version 2026-07-28 as an explicit opt-in; `LATEST_PROTOCOL_VERSION` stays pinned to 2025-11-25 and existing v1 clients negotiate exactly as before (milestone stays a 2.x minor)
- [x] **VERS-03**: v2 requests self-describe via per-request `_meta` (`io.modelcontextprotocol/protocolVersion`, `clientInfo`, `clientCapabilities`); v2 results carry `serverInfo`
- [ ] **VERS-04**: Server implements `server/discover` as a read-only projection of already-computed ServerCore capabilities
- [ ] **VERS-05**: Required headers `Mcp-Method`/`Mcp-Name` (alongside `MCP-Protocol-Version`) are enforced inbound and emitted outbound on the v2 HTTP path
- [ ] **VERS-06**: All protocol error codes live in one centralized version-gated constant table; v2 values are filled ONLY from the final 2026-07-28 schema.json (resolving the `-32002`/`-32602` conflict), and the frozen v1 `-32002` task-pending semantics stay unchanged
- [ ] **VERS-07**: All results carry the `resultType` envelope discriminator (`complete`/`input_required`/`task`); a missing `resultType` defaults to `complete` for backcompat
- [ ] **VERS-08**: The `extensions` capability map (reverse-DNS IDs) is supported in capability negotiation
- [x] **VERS-09**: W3C trace-context keys (`traceparent`/`tracestate`/`baggage`) in `_meta` are surfaced via typed accessors and propagated through dispatch

### Stateless HTTP & Multi-Round-Trip (HTTP)

- [ ] **HTTP-01**: v2 HTTP requests run with no `initialize` handshake and no `Mcp-Session-Id`, era-gated onto the existing `stateless()` branch; v1 session behavior is unchanged
- [ ] **HTTP-02**: A server handler can return `input_required` with `inputRequests` and an opaque `requestState` that is integrity-protected, principal-bound, and TTL'd
- [ ] **HTTP-03**: A client retry of the original request carrying `inputResponses` + echoed `requestState` resumes the operation correctly (multi-round-trip elicitation end-to-end)
- [ ] **HTTP-04**: v2 clients get change notifications via a `subscriptions/listen` long-lived stream (opt-ins for toolsListChanged/promptsListChanged/resourcesListChanged/resourceSubscriptions, `subscriptionId` tagging), replacing HTTP GET + `resources/subscribe`/`unsubscribe` on the v2 path
- [ ] **HTTP-05**: SSE resumability (`Last-Event-ID`) is not offered on the v2 path, and a regression test proves response JSON-RPC ids are always derived from the live request (the id-replay / discovery-cache bug class)

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

- [ ] **CLNT-01**: The pmcp `Client` can speak v2: per-request `_meta` emission, `server/discover`, required headers, no `initialize` — selected explicitly per connection
- [ ] **CLNT-02**: The pmcp `Client` fulfills MRTR `input_required` results by producing `inputResponses` — the Phase-106 host handlers (sampling/elicitation/roots) are folded into this flow on v2
- [ ] **CLNT-03**: `pmcp-agent` (including its `ToolInvoker` and task polling) works end-to-end against a v2 server
- [ ] **CLNT-04**: `mcp-tester` can exercise a v2 server (headers, discover, stateless flow) for dual-version testing

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
| VERS-04 | Phase 112 | Pending |
| VERS-05 | Phase 112 | Pending |
| VERS-06 | Phase 112 | Pending |
| VERS-07 | Phase 112 | Pending |
| VERS-08 | Phase 112 | Pending |
| VERS-09 | Phase 112 | Complete |
| HTTP-01 | Phase 113 | Pending |
| HTTP-02 | Phase 113 | Pending |
| HTTP-03 | Phase 113 | Pending |
| HTTP-04 | Phase 113 | Pending |
| HTTP-05 | Phase 113 | Pending |
| CLNT-01 | Phase 113 | Pending |
| CLNT-02 | Phase 113 | Pending |
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

**Coverage:**

- v1 requirements: 38 total
- Mapped to phases: 38 ✓
- Unmapped: 0

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
