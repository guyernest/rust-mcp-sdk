# Phase 113 — Deferred / Out-of-Scope Items

Discoveries made while executing Phase-113 plans that are **not** caused by the
current plan's changes and therefore were not auto-fixed (executor SCOPE BOUNDARY).

---

## D-113-A — Typed request structs rename `_meta` to `meta` on the wire

**Found during:** plan 02, task 3 (building the shared v2 HTTP harness)
**Severity:** HIGH — blocks a conformant v2 client from being detected as v2
**Owner:** plan 04 (HTTP-01)
**Status:** ✅ **RESOLVED** in plan 04, commit `47eaad68` (test `73a24cf1`)

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
`req._meta` — a conformant client that sent `_meta` got **no** era detection at
all, and its `MCP-Protocol-Version: 2026-07-28` header was then rejected as
"header claims v2 but `_meta` protocolVersion disagrees".

**Resolution (owner decision at plan 04): `rename` + `alias`.** Each of the three
fields now carries `#[serde(rename = "_meta", alias = "meta", skip_serializing_if =
"Option::is_none", default)]` — conformant on egress, backward-compatible with
pre-113 pmcp peers on ingress. The repo already used this idiom at
`src/types/tools.rs:219` and `:556`; the three request types were simply missing it.

**Workaround retired.** `tests/common/v2.rs` no longer emits a dual spelling (it
exports `REQUEST_META_KEY = "_meta"`), and the tripwire
`forward_tripwire_typed_requests_rename_meta_away_from_the_spec_spelling` was
INVERTED into the permanent regression guard
`tests/common_harness_smoke.rs::typed_requests_use_the_spec_meta_spelling`, which
proves both halves (egress `_meta`, ingress accepts both spellings).

**Cross-version note for plan 12 / release:** on egress pmcp now emits `_meta`. A
pmcp **server** older than this change accepts only `meta`, so a new pmcp client
talking to a ≤2.17 pmcp server loses the per-request `_meta` signal (progress
token, task id, namespaced keys). The alias fixes the ingress direction only.
That old server was already non-conformant with the spec spelling.

---

## D-113-B — `tools/list` (and every non-`_meta`-bearing method) cannot be a v2 request

**Found during:** plan 02, task 3
**Severity:** HIGH for HTTP-01 (stateless v2 has no handshake)
**Owner:** plan 04
**Status:** ✅ **RESOLVED** in plan 04, commit `47eaad68` (test `73a24cf1`)

`extract_request_meta_value` enumerated exactly three `_meta`-bearing client
requests (`CallTool`, `GetPrompt`, `ReadResource`); every other variant returned
`None`. `ListToolsRequest` had no `_meta` field at all.

A stateless v2 server has no `initialize` handshake, so the per-request `_meta`
signal is the ONLY era channel — which meant `tools/list`, `prompts/list`,
`resources/list`, `completion/complete` and `subscriptions/listen` could not be
v2 requests. They were rejected 400 with
"MCP-Protocol-Version header claims v2 but `_meta` protocolVersion disagrees".

**Resolution (owner decision at plan 04):** an optional
`_meta: Option<RequestMeta>` field (same serde attributes as D-113-A) was added to
`ListToolsRequest`, `ListPromptsRequest`, `ListResourcesRequest`,
`ListResourceTemplatesRequest` and `CompleteRequest`, and
`extract_request_meta_value` was widened to read all five. `ListResourceTemplatesRequest`
was included beyond the enumerated list because omitting it would have left
`resources/templates/list` as the sole remaining un-v2-able list method.

The tripwire `forward_tripwire_tools_list_cannot_be_a_v2_request` is now
`tests/common_harness_smoke.rs::tools_list_is_a_valid_v2_request` (asserts 200),
and `tests/v2_stateless_http.rs::v2_nameless_method_empty_mcp_name_accepted`
exercises the same path live.

**Plan 10 obligation:** `subscriptions/listen` lands as a new `ClientRequest`
variant. `extract_request_meta_value`'s match is wildcard-free, so the new variant
is a COMPILE ERROR there until it is classified — it must be classified as
`_meta`-bearing.

---

## D-113-C — Stateful (`::default()`) config still demands a session on v2

**Found during:** plan 02, task 3
**Severity:** expected — this IS requirement HTTP-01
**Owner:** plan 04
**Status:** ✅ **RESOLVED** in plan 04, commit `2baf265f` (test `2d16e23b`)

`StreamableHttpServerConfig::default()` keeps a live `session_id_generator`, so
`validate_non_init_session` rejected a v2 `tools/call` with 400 "Session ID required
for non-initialization requests". Plan 04 introduced the single
`sessions_active(state, era)` predicate and routed every session decision site
through it, making the ERA rather than the build-time config the decider.

The tripwire `forward_tripwire_stateful_config_still_demands_a_session_on_v2` is
now `tests/common_harness_smoke.rs::stateful_config_runs_v2_session_free`, and
`tests/v2_stateless_http.rs` carries fifteen live-HTTP assertions against a
`Default::default()` server.

---

## D-113-D — D-113-B's field additions require a MAJOR semver bump

**Found during:** plan 04 (measured immediately after the D-113-B fix landed)
**Severity:** HIGH — the v2.5 milestone is scoped as additive (2.x minor)
**Owner:** **needs a phase-level decision**; plan 12 owns the authoritative
`cargo semver-checks` gate and cannot pass it as-is
**Status:** OPEN

Measured, not inferred:

```
$ cargo semver-checks check-release --baseline-version 2.17.0 -p pmcp
Checked 223 checks: 222 pass, 1 fail, 0 warn, 30 skip

--- failure constructible_struct_adds_field: externally-constructible struct adds field ---
  field ListToolsRequest._meta              (src/types/tools.rs)
  field ListPromptsRequest._meta            (src/types/prompts.rs)
  field ListResourcesRequest._meta          (src/types/resources.rs)
  field ListResourceTemplatesRequest._meta  (src/types/resources.rs)
  field CompleteRequest._meta               (src/types/protocol/mod.rs)

Summary semver requires new major version: 1 major and 0 minor checks failed
```

The D-113-A half is clean — it is serde attributes only, and `CallToolRequest` is
already `#[non_exhaustive]`. Only the five D-113-B structs are flagged: they are
`pub` with all-`pub` fields and NOT `#[non_exhaustive]`, so a downstream
`ListToolsRequest { cursor: None }` stops compiling.

**The WIRE is unaffected.** Every added field is `Option`, `default`ed and
`skip_serializing_if = "Option::is_none"`, so an absent `_meta` emits no key and
v1 bytes are byte-identical (pinned by
`src/types/protocol/mod.rs::absent_meta_emits_no_key_on_any_request_type`). This is
purely a Rust-API source-compatibility break.

There is no way to add a field to a constructible struct without this break —
`#[non_exhaustive]` and a private field are both flagged major by the same tool.

**Options for the phase owner:**

1. **Accept the major bump** and ship this milestone as 3.0. Contradicts the
   ROADMAP's "milestone stays additive (2.x minor)" and research Pitfall 10
   ("accidental 3.0").
2. **Mark the five structs `#[non_exhaustive]` as well** while taking the break,
   so this is the LAST time a `_meta`-style addition breaks these types. Same
   major bump, better long-run shape. (Costs a mechanical update to
   `tests/protocol_invariants.rs`, `crates/pmcp-openapi-server`, and
   `examples/wasm-client`, all of which already needed `_meta: None` added.)
3. **Revert the five field additions and resolve D-113-B from the RAW body
   instead** — the HTTP layer already has the body bytes and already resolves a
   raw `params._meta` for `server/discover`
   (`Server::resolve_discover_protocol_context`). Generalizing that read to every
   method makes all methods v2-able with ZERO public API change, at the cost of
   handlers no longer being able to read `_meta` off the typed list-request struct
   (the `ProtocolContext`-derived `RequestHandlerExtra` accessors still work).

The 15 live tests in `tests/v2_stateless_http.rs` and the harness tripwires pass
under option 1 and 2 unchanged; option 3 would need `extract_request_meta_value`'s
new arms reverted and the raw path wired instead.
