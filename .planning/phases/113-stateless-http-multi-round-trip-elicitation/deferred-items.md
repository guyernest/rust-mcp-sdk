# Phase 113 — Deferred / Out-of-Scope Items

Discoveries made while executing Phase-113 plans that are **not** caused by the
current plan's changes and therefore were not auto-fixed (executor SCOPE BOUNDARY).

---

## D-113-A — Typed request structs rename `_meta` to `meta` on the wire

**Found during:** plan 02, task 3 (building the shared v2 HTTP harness)
**Severity:** HIGH — blocks a conformant v2 client from being detected as v2
**Owner:** plan 04 (HTTP-01) or plan 11 (conformance); needs a Phase-level decision

`CallToolRequest`, `GetPromptRequest` and `ReadResourceRequest` all carry
`#[serde(rename_all = "camelCase")]`, which renames the `_meta` **field**. Verified
by probing pmcp's own serialization:

```
CallToolRequest   { _meta: Some(..) }  ->  {"name":"x","arguments":{},"meta":{...}}
GetPromptRequest  { _meta: Some(..) }  ->  {"name":"p","arguments":{},"meta":{...}}
```

and by round-tripping deserialization:

```
{"name":"x","arguments":{},"_meta":{...}}  ->  _meta == None   (silently ignored)
{"name":"x","arguments":{},"meta":{...}}   ->  _meta == Some   (accepted)
```

The MCP spec spelling is `_meta`. Because Phase 112 routes the whole per-request era
signal through `extract_request_meta_value(request)` — which reads the TYPED
`req._meta` — a conformant client that sends `_meta` gets **no** era detection at
all, and its `MCP-Protocol-Version: 2026-07-28` header is then rejected as
"header claims v2 but `_meta` protocolVersion disagrees".

Note the `server/discover` ingress reads a RAW `params._meta` and therefore uses the
CORRECT spelling — the two ingress paths currently disagree.

**Why not auto-fixed here:** changing the wire spelling of `_meta` on three public
v1 request types changes v1 wire bytes for every existing client. That is a Rule-4
architectural decision, not a Rule-1/2/3 auto-fix.

**Workaround in place:** `tests/common/v2.rs::v2_body_with_caps` emits the reserved
`_meta` object under BOTH spellings, and
`tests/common_harness_smoke.rs::forward_tripwire_typed_requests_rename_meta_away_from_the_spec_spelling`
pins the current spelling so the workaround cannot silently outlive the defect.

---

## D-113-B — `tools/list` (and every non-`_meta`-bearing method) cannot be a v2 request

**Found during:** plan 02, task 3
**Severity:** HIGH for HTTP-01 (stateless v2 has no handshake)
**Owner:** plan 04

`extract_request_meta_value` enumerates exactly three `_meta`-bearing client
requests (`CallTool`, `GetPrompt`, `ReadResource`); every other variant returns
`None`. `ListToolsRequest` has no `_meta` field at all.

A stateless v2 server has no `initialize` handshake, so the per-request `_meta`
signal is the ONLY era channel — which means `tools/list`, `prompts/list`,
`resources/list`, `completion/complete` and `subscriptions/listen` currently cannot
be v2 requests. They are rejected 400 with
"MCP-Protocol-Version header claims v2 but `_meta` protocolVersion disagrees".

**Pinned by:** `tests/common_harness_smoke.rs::forward_tripwire_tools_list_cannot_be_a_v2_request`.

---

## D-113-C — Stateful (`::default()`) config still demands a session on v2

**Found during:** plan 02, task 3
**Severity:** expected — this IS requirement HTTP-01
**Owner:** plan 04

`StreamableHttpServerConfig::default()` keeps a live `session_id_generator`, so
`validate_non_init_session` rejects a v2 `tools/call` with 400 "Session ID required
for non-initialization requests". The per-request era gate that suppresses sessions
on v2 is plan 04's deliverable.

**Pinned by:** `tests/common_harness_smoke.rs::forward_tripwire_stateful_config_still_demands_a_session_on_v2`.
