---
gsd_state_version: 1.0
milestone: v2.5
milestone_name: MCP Spec 2026-07-28
status: executing
stopped_at: Completed 113-07-PLAN.md
last_updated: "2026-07-25T21:28:44.075Z"
last_activity: 2026-07-25
progress:
  total_phases: 71
  completed_phases: 57
  total_plans: 294
  completed_plans: 290
  percent: 80
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-07-22) · .planning/ROADMAP.md (v2.5 milestone, Phases 112-119) · .planning/REQUIREMENTS.md (38 v1 reqs, 38/38 mapped) · .planning/research/SUMMARY.md (v2.5 research, HIGH confidence)

**Core value:** One pmcp server binary transparently serves both MCP 2025-11-25 and 2026-07-28 clients via per-request negotiation — v2 as the strategic primary path (stateless/Lambda-first, Tasks, MCP Apps), v1 as a cleanly severable compatibility layer. The whole milestone stays additive (2.x minor).
**Current focus:** Phase 113 — stateless-http-multi-round-trip-elicitation

## Current Position

Phase: 113 (stateless-http-multi-round-trip-elicitation) — EXECUTING
Plan: 10 of 13
Status: Ready to execute
Last activity: 2026-07-25

## v2.5 Phase Plan (8 phases, 38 requirements)

| Phase | Name | Goal | Reqs | Depends on |
|-------|------|------|------|------------|
| 112 | Version Plumbing Spine | `ProtocolContext` resolved once at ingress + threaded through dispatch; v2 opt-in (LATEST stays 2025-11-25); discover/extensions/headers/`resultType`/trace-context/error-code table | VERS-01..09 (9) | none (keystone) |
| 113 | Stateless HTTP + MRTR | Handshake-free/session-free v2 on the `stateless()` branch; MRTR end-to-end; `subscriptions/listen`; no SSE resumability + id-replay test; pmcp `Client` speaks v2 | HTTP-01..05, CLNT-01, CLNT-02 (7) | 112 |
| 114 | Tasks Extension Migration | extensions-map negotiation, `tasks/update`, `tasks/list` era-gated off on v2, `resultType:"task"`, fail-closed owner-binding; backends unchanged | TASK-01..06 (6) | 112 (+113 identity pattern) |
| 115 | JSON Schema 2020-12 + Caching | jsonschema 0.48 Draft 2020-12 pinned; any-JSON `structuredContent` on v2; additive `ttlMs`/`cacheScope` | SCHM-01..03 (3) | 112 (parallel) |
| 116 | Auth Hardening SEPs | RFC 9207 `iss` (strict v2/lenient v1), DCR `application_type`, issuer-keyed creds + 3 clarifications; no new crates | AUTH-01..03 (3) | 112 (parallel) |
| 117 | Agents, Tester & v1 Severability | `pmcp-agent` + `mcp-tester` on v2; v1 machinery severable + sunset policy; v2 path de-baggaged | CLNT-03, CLNT-04, SMPL-01, SMPL-02 (4) | 113, 114 |
| 118 | Conformance | official `@modelcontextprotocol/conformance` in CI over HTTP; Phase-109 Rust harness gains v2 fixtures (v1 green); deprecated caps verified under v2 | CONF-01..03 (3) | 112-117 |
| 119 | Documentation — Three Shapes + v2 Migration | Agents & Teams three-shapes (carried from v2.4 P111); v2 migration guide + dual-version story; runnable stateless-v2 + v2-client examples | DOCS-04..06 (3) | 112-118 |

**Execution order:** 112 first and alone → {113, 115, 116} parallelize once the spine lands → 114 sequenced close after 113 (shared stateless-identity/owner-binding pattern) → 117 (needs 113 Client + 114 Tasks) → 118 conformance (validates the union) → 119 docs.

**Final-spec checkpoint (2026-07-28, six days out):** wire-exact work (error-code values, `requestState` shape, caching-hint field names) sequenced after final publication. VERS-06 error-code table is structure-first, values-from-final-schema.json only. Open verification item (research): the `-32002`→`-32602` rename direction MUST be re-verified against the final schema before touching the frozen `-32002` task-pending code — cross-cuts Phases 112 and 114.

**Dependency/zero-deps note (research HIGH confidence):** no new runtime crates — only `jsonschema` 0.46→0.48 for Draft 2020-12; Node.js LTS 22.x is CI-only for the conformance suite. Milestone stays additive (2.x minor); `cargo semver-checks`/`cargo public-api` should gate every phase, not just the last (Pitfall 5 — accidental 3.0).

## Accumulated Context

### Roadmap Evolution

- v2.5 milestone roadmap created (2026-07-22): 8 phases (112-119) map the 38 v1 requirements along the research-corroborated dependency spine — version-plumbing keystone (112) first and alone, stateless HTTP + MRTR (113), Tasks-as-extension (114), parallel JSON Schema (115) and Auth (116), agents/tester + v1 severability (117), conformance (118), docs (119). 100% coverage, no orphans, no duplicates. v2.4 Phase 111 docs folded into v2.5 DOCS-04 (Phase 119). Continues numbering after v2.4's Phases 106-111 (Phase 111 never executed).
- v2.4 milestone roadmap created (2026-07-17): 6 phases (106-111) map 1:1 to the approved design doc's §4 phases A-F along the compliance→contracts→agent→teams→CLI→docs spine; all 31 v1 requirements mapped (100% coverage, no orphans).

### Decisions

Decisions are logged in PROJECT.md Key Decisions table. Decisions framing this milestone (from design §6 recommendations, approved):

- Boundary razor: contracts + reference implementations in the open SDK; operation + scale stay on pmcp.run.
- Crate name `pmcp-agent` (not `pmcp-agents`); one `pmcp-team-servers` crate with per-server feature flags (not four crates).
- `pmcp-package` adopted into this repo first, published 0.1.0 from here (source: `~/Development/mcp/sdk/pmcp-run/crates/pmcp-package` — import + publish-hygiene, not a rewrite); caret `"0.1"` dep, not `=0.1.0`.
- Legacy inverted sampling kept and documented as the "LLM-server pattern" (no breaking change / no deprecation).
- Sampling-first, not sampling-only: `SamplingSource` (zero-dep) first-class; `OpenAiCompatSource` + `AnthropicSource` feature-gated; three sources maximum, the trait is the extension point.
- The trait seams double as durability seams — the loop stays pure/replay-safe (mirrors the 2.13.0 `poll_decision` non-determinism-inside-the-step design).
- Team-tool contracts as provable-contracts YAML (house convention), namespaced provisional PMCP extensions.
- [Phase ?]: 109-00: guard/namespaced state travels as _meta (locked D-14 route A), carried as raw JSON on RequestHandlerExtra; not smuggled in tool arguments
- [Phase ?]: 109-00: per-request handler fields wired in BOTH core.rs and server/mod.rs dispatch sites (+ wasm mirror parity)
- [Phase ?]: 109-01: derive_attachment realizes D-05/D-06/D-07; built_in demoted to deduped opt-ins; counts snapshotted at entry
- [Phase ?]: 109-01: MemberId identity IS the ComponentRef (name@version); PackageResolver + MemberTaskForwarding seams landed atomically; contract rev'd to v1.1.0 with io.modelcontextprotocol/related-task
- [Phase ?]: team-fs: fs__complete_task lives in the server layer (custom ToolHandler with ToolOutput::Result under RELATED_TASK_META_KEY), NOT the TeamFsBackend trait — task completion is protocol behavior, not storage
- [Phase ?]: team-fs local backend explicitly REJECTS symlink components (documented dev-backend TOCTOU stance); percent-encoded file:// URLs via a tested helper, not format!
- [Phase ?]: 109-04: approval-mcp splits observable lifecycle (InMemoryTaskStore) from approval-domain state (ApprovalRepository); service-owned resolution from any client (D-10)
- [Phase ?]: 109-04: double-resolve REJECTED via AlreadyResolved (first writer verdict preserved); decision validated against original option set under one mutex
- [Phase ?]: 109-08: team-servers binding drift enforced via deterministic source-resolution gate (comply-bindings-check); mandated pmat comply check --path . runs as informational report because pmat comply is holistic + cache-driven on this repo (D-07 alignment)
- [Phase ?]: 109-08: subprocess smoke test drives spawned bins via a bespoke ChildStdioTransport reusing SDK stdio framing (SDK ships no child-bound transport); handshake otherwise 100% pmcp::Client
- [Phase 110]: 110-01: cargo-pmcp foundation wired — agent/team/package command groups + 3 workspace deps (pmcp-agent openai-compat, pmcp-team-servers runtime+http→member-llm, pmcp-package caret 0.1); handlers stubbed via actionable bail! for disjoint Wave-2 fills; version 0.18.0
- [Phase 110]: 110-01: package capture uses a capture-local --target (not GlobalFlags); Package kept OUT of is_target_consuming so it never clobbers PMCP_TARGET/AWS env
- [Phase ?]: 110-02: cargo pmcp agent new scaffolds a COMPILABLE agent crate — manifest built from the real AgentPackage struct (round-trip guaranteed), a manifest-driven runner that LOADS agent.package.json + resolve_agent, full deps, and an in-scaffold tests/pin.rs; two-level pin tripwire (D-05) + validate_crate_name promoted to pub(crate) (D-01a)
- [Phase ?]: 110-03: cargo pmcp agent dev (CLI-02) wired for --source openai-compat|sampling|fixed (clap ValueEnum); loads a real AgentPackage (--package/./agent.package.json/built-in demo); correct pmcp-agent contract — Decode at source construction → --allow-insecure-http bail, non-Completed RunOutcome → --endpoint/--source fixed bail
- [Phase ?]: 110-03: run_fixed_source is a lib-safe leaf (no clap/GlobalFlags) mounted into the lib target as cargo_pmcp::agent_run via a #[path] seam (commands::* is bin-only), reused by the CLI fixed arm and the 110-06 example
- [Phase 110]: 110-04: cargo pmcp team dev (CLI-03) — default transcript delegates composition to TeamRuntime (D-02, no hand-rolled spin-up); --serve reuses the shipped team-mcp binary recipe (build_team_mcp_server + serve_streamable_http on 127.0.0.1:<port>, NOT TeamRuntime, no upstream change); --llm wraps a validated OpenAiCompatSource in the exported FixedSourceFactory (correct sync/infallible factory shape, not a custom fallible factory)
- [Phase 110]: 110-04: behavioral tests characterize the composable primitives directly (commands::* is bin-only, so the bail! stub is unreachable from an integration test) — transcript + ephemeral-port --serve tools/list + mockito-endpoint --llm smoke, all offline/loopback
- [Phase ?]: test decision xyz
- [Phase ?]: 110-06: agent example drives the PRODUCTION run_fixed_source seam, not a re-implemented AgentEngine loop (Codex 110-06 HIGH)
- [Phase ?]: 110-06: fuzz_package_kind targets the RAW-bytes untrusted manifest-parse boundary, the real package show seam
- [Phase ?]: 110-06: the three lib seams are #[doc(hidden)] internal support surface for examples/fuzz, not stable API (Codex 110-06 MEDIUM)
- [Phase 112]: 112-01: v2 reached only via opt-in accept-list; LATEST stays 2025-11-25, 2026-07-28 NOT in SUPPORTED (Pitfall 1); protocol_era classifies only exact 2026-07-28 as V2, unknown->V1
- [Phase 112]: 112-01: TraceContext::from_meta bounds W3C values at 8192 (over-bound traceparent->None, tracestate/baggage dropped); values documented RAW/UNVALIDATED/untrusted; proptest + fuzz target added (T-112-09)
- [Phase 112]: 112-01: semver tooling pinned (cargo-semver-checks 0.49.0, cargo-public-api 0.52.0); baseline pmcp 2.17.0; authoritative check-release MINOR assertion deferred to Plan 07/08
- [Phase ?]: [Phase 112]: 112-02: protocol_context + era/protocol_version/client_info/client_capabilities/trace_context accessors added ONLY to native RequestHandlerExtra (src/server/cancellation.rs); wasm32 zero-field stub + orphan shared/cancellation.rs untouched
- [Phase ?]: [Phase 112]: 112-02: trace_context() is a method over existing request_meta (no new field, VERS-09 keys in _meta); identity accessors rustdoc'd self-reported/not-for-authz (T-112-02 accept-documented); purely additive, wasm build green
- [Phase ?]: [Phase 112]: 112-03: error::ErrorCode's 11 consts delegate to new error_codes:: table (Self(error_codes::NAME)) — centralizes ~210 call sites, names/values unchanged (semver minor); per-name consistency test is the drift guard
- [Phase ?]: [Phase 112]: 112-03: both -32002 meanings kept by name (V1_TASK_PENDING frozen vs UNSUPPORTED_CAPABILITY), never reconciled; v2 codes structurally omitted (zero SATD), finalization tracked in planning
- [Phase ?]: [Phase 112]: 112-03: server/discover routed via crate-private InternalClientRequest + classify_internal_method BEFORE public-enum conversion; NO public ClientRequest/Request variant (Codex HIGH #4); Plan 05 wires it
- [Phase 112]: 112-06: v2 HTTP header gate CONSUMES Plan 04's resolved ProtocolContext era (resolved once in the HTTP layer, threaded into new pub(crate) Server::handle_request_with_context) — never a second raw-header era read (Pitfall 2 / D-11); seam lives on high-level Server since the HTTP path dispatches through it, not ServerCore
- [Phase 112]: 112-06: full header/_meta matrix as cog-25-safe pure classifier, fail-closed on every conflict cell; strict all-three-headers reject (D-05) + Mcp-Method/Mcp-Name body cross-check (D-06); outbound emission on success AND error non-panicking; new errors from error_codes:: (VERS-06); gate runs BEFORE legacy validate_protocol_version; v1/non-opted-in zero enforcement (D-04)
- [Phase ?]: [Phase 112]: 112-07: dispatch layer (core.rs/mod.rs/task_dispatch.rs) + jsonrpc.rs production error-emission sites migrated to error_codes:: constants — centralized table is now the ACTUAL wire source of truth (closes checker Blocker 1); name-for-value swaps only, wire bytes unchanged; frozen -32002->V1_TASK_PENDING / -32601->METHOD_NOT_FOUND byte-identical, locking test untouched+green
- [Phase ?]: [Phase 112]: 112-07: repo-wide VERS-06 audit — batch.rs/parallel_batch.rs production literals migrated here (Rule 2, owned by no plan); only Plan 08 streamable_http_server.rs (25) + non-compiled orphan src/wasi.rs remain (recorded)
- [Phase ?]: [Phase 112]: 112-08: streamable-HTTP transport's 25 production error-code literals migrated to error_codes:: (name-for-value swap, wire bytes identical); file now carries zero bare -32xxx; value oracle lives in Plan 03 error_codes.rs consistency tests
- [Phase ?]: [Phase 112]: 112-08: repo-wide VERS-06 audit closed — no production protocol-error EMISSION literal outside the centralized table across compiled src/; remaining are the table, #[cfg(test)] oracle, Plan-03-owned ProtocolErrorCode enum discriminants, and non-compiled orphan src/wasi.rs
- [Phase ?]: [Phase 112]: 112-08: authoritative phase-end gate GREEN — cargo semver-checks vs 2.17.0 no breaking change (no major, no enum_variant_added; 223 pass); make quality-gate passed (pmat comply advisories informational per D-07)
- [Phase ?]: [Phase 112]: 112-09: per-request _meta/ProtocolContext spine generalized from tools/call-only to GetPrompt + ReadResource at both native dispatch sites (core.rs + mod.rs); era()/client_info()/trace_context() now live inside prompt & resource handlers (Gap B closed)
- [Phase ?]: [Phase 112]: 112-09: HTTP header gate resolves resources/read logical name method-awarely from params.uri (review finding #2 / Gap C closed); a standards-shaped v2 resources/read accepted not rejected 400; no synthetic params.name fallback
- [Phase 112]: 112-10: server/discover made LIVE in production on the HTTP transport (Gap A closed, VERS-04/SC#3) via classify-then-continue — a crate-LOCAL HttpIngress::{Public,Discover} in BOTH POST parse entrypoints; TransportMessage public variants untouched so semver stays MINOR (223 checks pass)
- [Phase 112]: 112-10: discover CONTINUES through the SAME pipeline (session → run_v2_header_gate_raw running the SAME classify_v2_request matrix → legacy-version → auth → dispatch → event store → per-path assembly); NOT an early return — auth-provider 401 + response-middleware e2e prove no bypass (findings #1/#3/#4)
- [Phase 112]: 112-10: discover projection consolidated into ONE shared build_discover_response free fn (ServerCore wrappers dispatch_internal_client_request/handle_discover DELETED, no #[allow(dead_code)] remains); v1/non-opted-in discover → -32601@200 with original id (deliberate benign D-10 change from pre-112 PARSE_ERROR 400, documented in code)
- [Phase ?]: 113-01: spec verdict held PENDING (no schema/2026-07-28); the three v2 transport error codes landed ONLY under a written ## Recorded Exception naming developer/date/source-commit, with a binding plan-12 re-verification whose failure mode is phase-reopening, not advisory
- [Phase ?]: 113-01: DRIFT-1 adjudicated — Phase-112 D-05 stays LOCKED (Mcp-Name required on EVERY v2 request) despite the draft transport spec requiring it only for tools/call|resources/read|prompts/get; plan 04 keeps the rule, plan 11 marks affected conformance header scenarios KNOWN-FAILING rather than loosening the fail-closed gate
- [Phase ?]: 113-01: ring 0.17 + zeroize 1.8 promoted to explicit optional deps under streamable-http with zeroize default=[alloc] ON / derive OFF; zero-new-crates proven as a MEASURED lockfile package-name delta (728->728 byte-identical) plus cargo tree -p pmcp cleanliness, never an absolute count against the workspace-shared lockfile
- [Phase ?]: 113-01: plan 11 builds its conformance scenario manifest from 113-SPEC-RECHECK.md Section B (23 sep-2322 check ids / 14 classes @ pin a8651182), NOT the 113-RESEARCH.md table which omits 4 ids and misreports a class name as a check id
- [Phase ?]: 113-02: MRTR wire adapter lands as ONE module (src/types/mrtr.rs) with fail-loud extract (Result, absent != invalid), stale-clearing splice, kind-directed InputResponse::decode_for, whitelist-canonicalized salient_param_digest; parsing/plumbing pub(crate), only authoring/result types pub
- [Phase ?]: 113-02: ElicitRequestParams gets hand-written serde impls -- mode-optional on deserialize (v2 implicit form), byte-identical mode-tagged serialize (v1); semver-checks 223/223, no bump required
- [Phase ?]: 113-02: three pre-existing v2 blockers surfaced and pinned by FORWARD TRIPWIRE tests, not comments -- typed requests rename _meta->meta on the wire (a conformant v2 client is never detected as v2), tools/list carries no _meta so cannot be a v2 request, stateful config still demands a session on v2; all owned by plan 04
- [Phase ?]: 113-03: requestState codec is SERVER-instance-owned (Arc on Server + ServerCore), resolved exactly once at build() — no process-global; builder key/ttl beat env, and two differently-keyed servers coexist in one process (regression-tested)
- [Phase ?]: 113-03: MALFORMED PMCP_REQUEST_STATE_KEY fails the server BUILD (T-113-17); D-04's warn-and-degrade fallback covers the UNSET case only
- [Phase ?]: 113-03: Verdict not Result — UnknownKey (re-elicit) can never collapse into AuthFailed (JSON-RPC error); Expired carries the DECRYPTED continuation so round survives (T-113-49)
- [Phase ?]: 113-03: key-id collisions try EVERY matching accepting entry -> AuthFailed, never a false Ok and never a misleading UnknownKey; proven via cfg(test) forced-id constructors
- [Phase ?]: 113-03: env reads route through a cfg(test) thread-local seam (ENV_LOCK alone is insufficient — cargo test --lib is in-process parallel and from_env now runs inside ServerBuilder::build)
- [Phase ?]: [Phase 113]: 113-04: HTTP-01 landed as ONE sessions_active(state, era) predicate over the server-wide config, not a transport fork; the v2 header gate MOVED above session resolution in both POST entrypoints because the era must be known before the first session decision
- [Phase ?]: [Phase 113]: 113-04: D-113-A resolved with serde rename=_meta + alias=meta (conformant egress, backward-compatible ingress); D-113-B added optional _meta to the five list-shaped request types and widened extract_request_meta_value — absent _meta emits no key so v1 wire bytes are unchanged
- [Phase ?]: [Phase 113]: 113-04: v2 status mapping is CODE-driven not call-site-driven (plan 09's -32021 is emitted by dispatch, never the gate) and runs at the RAW level for unknown methods, recovering the original id from the body bytes for 404+-32601
- [Phase ?]: [Phase 113]: 113-04: BLOCKER D-113-D — the D-113-B field additions fail cargo semver-checks constructible_struct_adds_field, so pmcp now requires a MAJOR bump against the ROADMAP's additive-2.x scope; wire bytes unaffected; three options recorded in deferred-items.md for a phase-level decision
- [Phase ?]: [Phase 113]: 113-04: D-113-D RESOLVED by owner option 3 — the five _meta field additions were REVERTED and D-113-B re-resolved by reading params._meta off the RAW body at HTTP ingress (resolve_raw_meta_protocol_context + raw_params_meta), which covers every method with ZERO public API change; semver-checks back to 223/223 pass, no update required, milestone stays additive 2.x
- [Phase ?]: [Phase 113]: 113-04: the typed and raw v2 gates COLLAPSED into one — there is now a single era-detection path on the HTTP transport reading the spec-spelled _meta from the raw body, closing the plan-02 'two ingress paths disagree' defect; the typed extract_request_meta_value survives only for the non-HTTP transports that have no raw bytes, and both readers agree on spelling via D-113-A
- [Phase ?]: [Phase 113]: 113-04: ACCEPTED COST (do not re-litigate in plans 06/09/10) — handlers reach the per-request _meta through the ProtocolContext-derived RequestHandlerExtra accessors, NOT through a typed _meta field on a list-request struct; adding such a field to a constructible pub struct is a MAJOR semver break
- [Phase ?]: [Phase 113]: 113-05: the client mode seam is THREE defaulted Transport methods — set_negotiated_protocol_version + supports_negotiated_protocol_version + send_raw; the third exists because neither params._meta on list-shaped methods nor server/discover can travel through the typed TransportMessage::Request without a MAJOR semver break (D-113-D / Phase-112 D-10)
- [Phase ?]: [Phase 113]: 113-05: on v2 the CLIENT assembles and sends the RAW JSON-RPC frame (splice_v2_meta then send_raw) so every method carries the reserved _meta era signal with zero public API change; v1 still sends the typed message and is byte-identical
- [Phase ?]: [Phase 113]: 113-05: with_protocol_version returns Result<Self> (build() cannot become fallible) and validates against SUPPORTED_PROTOCOL_VERSIONS UNION 2026-07-28 — the v2 constant is deliberately absent from that table (Phase-112 Pitfall 1)
- [Phase ?]: [Phase 113]: 113-05: the transport v2 era is a PRIVATE latch written only by the client seam, never derived from protocol_version — process_response_headers overwrites that field from the server, so a rogue echo of MCP-Protocol-Version: 2026-07-28 would otherwise flip a v1 client into v2 mode and break its session
- [Phase ?]: [Phase 113]: 113-05: server_discover takes &mut self and STORES its projection (that is what re-arms era-aware assert_capability); it is never called implicitly and never used to CHOOSE an era (D-08)
- [Phase 113]: 113-07: the two MRTR client errors ride the EXISTING Error::Protocol variant discriminated by a stable data.pmcpError marker — pmcp::Error is not #[non_exhaustive], so a new variant is a MAJOR break; rustdoc'd so nobody "fixes" them into variants
- [Phase 113]: 113-07: the EXISTING call_tool/get_prompt/read_resource now return Err(input_required_unfulfilled) on v2 instead of deserializing an input_required into a silently EMPTY CallToolResult (content is #[serde(default)]); the additive *_mrtr siblings return MrtrOutcome::InputRequired as a value
- [Phase 113]: 113-07: the MRTR fold PREFLIGHTS every requested kind before invoking anything and routes each entry through the SAME host helpers the v1 dispatch uses, so on_sampling_approval and on_sampling_result_review apply identically on v2 (T-113-57); all-or-nothing, every refusal tracing::warn!-logged with the entry key
- [Phase 113]: 113-07: a WithTools-only sampling handler answers an MRTR entry via project_with_tools_to_legacy — an inputResponses value is spec-typed as CreateMessageResult, while the v1 host response still carries the full CreateMessageResultWithTools (one pipeline, two renderers)
- [Phase 113]: 113-07: D-113-E fixed — a v2 non-2xx whose body is a strict JSON-RPC 2.0 error envelope is fed through the normal response channel (so error.code is readable); v1 gated out by the transport v2_mode latch and byte-identical
- [Phase 113]: 113-07: a missing or non-input_required resultType is TERMINAL, so Phase 114's "task" composes with the MRTR loop without touching it; rounds are counted per LOGICAL round and the resend always uses a fresh id plus splice_mrtr_params (stale-key-free)
- [Phase 113]: 113-08: the resumability era gate is INDEPENDENT of the session gate — before this plan a v2 request reached no event store only INCIDENTALLY, via the session gate's zeroed response_session_id
- [Phase 113]: 113-08: envelope_for_live_request(payload, live_id) is the ONE direct-response constructor on the HTTP transport — payload and id are separate arguments, so a cached envelope's stale id is structurally unconstructible
- [Phase 113]: 113-08: the event store is type-erased on the crate-private ServerState (EventStoreHandle = Arc<dyn EventStore>), NOT on the public config field — widening that public field would be a MAJOR semver break (D-113-D)
- [Phase 113]: 113-08: FOUND AND FIXED a real cross-caller bug — build_response selected its SSE destination stream from the RAW INBOUND Mcp-Session-Id, so a v2 POST naming a v1 caller's open session had its response delivered into THAT caller's stream (T-113-07) and written to the event store on the way (T-113-29/30); now gated on sessions_on
- [Phase 113]: 113-09: reserved envelope fields are SERVER-OWNED — resultType/_meta serverInfo OVERWRITTEN, requestState/inputRequests REMOVED unless this egress minted them, dev.pmcp/mrtr removed always; entry().or_insert replaced for the enumerated set only, every other handler _meta key survives
- [Phase 113]: 113-09: a handler signal on v1 or a non-eligible v2 method now FAILS LOUDLY with INTERNAL_ERROR instead of emitting a mangled complete result; strip_mrtr_signal returns a THREE-state outcome so a malformed reserved payload cannot degrade into "no signal"
- [Phase 113]: 113-09: the declared-client-capability precheck is submode-aware (form vs URL elicitation, tool-augmented sampling) and runs BEFORE any minting, proven structurally by running it with codec:None so a mint attempt would fail differently; -32021 payload is a ClientCapabilities OBJECT, all-or-nothing
- [Phase 113]: 113-09: serverInfo moved to result._meta["io.modelcontextprotocol/serverInfo"]; a TOP-LEVEL serverInfo is deliberately NOT owned because it is a real schema field of ServerDiscoverResult/InitializeResult, so server/discover carries both
- [Phase 113]: 113-09: two plan verification commands matched ZERO tests and passed vacuously; each suite is now nested in a module named after the production symbol (mod mrtr_egress, mod inject_v2_result_envelope) so the filters select 21 and 16 tests

### Pending Todos

None yet.

### Blockers/Concerns

yet. (Research flags per phase to be surfaced during `/gsd:plan-phase`.)

- ~~113-02 finding D-113-A (HIGH, owned by plan 04)~~ — RESOLVED in 113-04 (`47eaad68`): the three typed request structs are pinned with `#[serde(rename = "_meta", alias = "meta")]`, so egress is spec-conformant and ingress still accepts pre-113 pmcp peers. The forward tripwire was inverted into the permanent regression guard `typed_requests_use_the_spec_meta_spelling`.
- ~~113-04 finding D-113-D (HIGH, phase-level decision)~~ — RESOLVED: the owner chose option 3. The five `_meta` field additions were reverted (`b2cc87fe`) and D-113-B re-resolved by reading `params._meta` off the RAW body at HTTP ingress (`f6735c03`), which needs zero public API change. `cargo semver-checks check-release --baseline-version 2.17.0 -p pmcp` now reports `223 checks: 223 pass, 30 skip / Summary no semver update required`, so the milestone stays additive (2.x minor) and plan 12's semver gate is clear.

No open blockers.

## Deferred Items

Items deferred by design for this milestone (design §7 / REQUIREMENTS v2):

| Category | Item | Status | Deferred At |
|----------|------|--------|-------------|
| Deploy | AgentCore deploy adapter (`cargo pmcp deploy` target) | Deferred (DEFER-01) | v2.4 scope |
| Sources | Additional `CompletionSource` impls beyond the three shipped | Deferred (DEFER-02) | v2.4 scope |
| Memory | Scaled team-memory backends (embeddings/vector stores) in the open SDK | Deferred (DEFER-03) | v2.4 scope |
| Platform | pmcp.run adopting the loop/traits (companion §8 note) | Deferred (DEFER-04) | not SDK work |

## Shipped Milestones

| Version | Name | Phases | Date |
|---------|------|--------|------|
| v1.0 | MCP Tasks Foundation | 1-3 | 2026-02-22 |
| v1.1 | Task-Prompt Bridge | 4-8 | 2026-02-23 |
| v1.2 | Pluggable Storage Backends | 9-13 | 2026-02-24 |
| v1.3 | MCP Apps Developer Experience | 14-19 | 2026-02-26 |
| v1.4 | Book & Course Update | 20-24 | 2026-02-28 |
| v2.0 | Protocol Modernization | 54-59 | — |
| v2.2 | Configuration-Only MCP Servers (SQL + OpenAPI) | 82-90.2 | substantially shipped |
| v2.3 | Excel-as-Configuration MCP Servers + Tasks DX arc | 91-96, 101-105 | 2026-07-05 |

## Session Continuity

Last session: 2026-07-25T21:28:10.932Z
Stopped at: Completed 113-07-PLAN.md
Resume file: None

## Performance Metrics

| Phase | Plan | Duration | Notes |
|-------|------|----------|-------|
| (v2.4 phases not yet planned) | — | — | — |
| Phase 109 P00 | 25min | 2 tasks | 7 files |
| Phase 109 P01 | 10min | 4 tasks | 38 files |
| Phase 109 P02 | 35min | 2 tasks | 5 files |
| Phase 109 P03 | 30min | 2 tasks | 5 files |
| Phase 109 P04 | 25min | 2 tasks | 4 files |
| Phase 109 P05 | 55min | 3 tasks | 8 files |
| Phase 109 P06 | 40min | 2 tasks | 3 files |
| Phase 109 P07 | 45min | 2 tasks | 35 files |
| Phase 109 P08 | 95min | 4 tasks | 7 files |
| Phase 110 P01 | 44min | 3 tasks | 12 files |
| Phase 110 P02 | 38min | 3 tasks | 8 files |
| Phase 110 P03 | 30min | 2 tasks | 5 files |
| Phase 110 P04 | 40min | 2 tasks | 2 files |
| Phase 110 P05 | 12min | 3 tasks | 8 files |
| Phase 110 P06 | 20min | 3 tasks | 5 files |
| Phase 112 P01 | 11min | 2 tasks | 6 files |
| Phase 112 P02 | 6min | 2 tasks | 1 files |
| Phase 112 P03 | 5min | 2 tasks | 3 files |
| Phase 112 P04 | 30 | 2 tasks | 4 files |
| Phase 112 P5 | 35 | 2 tasks | 4 files |
| Phase 112 P06 | 22min | 2 tasks | 4 files |
| Phase 112 P07 | 12min | 2 tasks | 6 files |
| Phase 112 P08 | 11min | 1 tasks | 1 files |
| Phase 112 P9 | 40 | 3 tasks | 4 files |
| Phase 112 P10 | 50 | 3 tasks | 5 files |
| Phase 113 P01 | 28min | 3 tasks | 3 files |
| Phase 113 P02 | 42min | 3 tasks | 7 files |
| Phase 113 P03 | 78min | 3 tasks tasks | 7 files files |
| Phase 113 P04 | 165min | 5 tasks | 10 files |
| Phase 113 P05 | 105min | 3 tasks | 6 files |
| Phase 113 P06 | 95min | 3 tasks | 7 files |
| Phase 113 P07 | 41min | 3 tasks tasks | 6 files files |
| Phase 113 P08 | 25min | 2 tasks | 2 files |
| Phase 113 P09 | 118min | 3 tasks | 7 files |
