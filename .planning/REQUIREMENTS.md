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
> **HTTP-09 was the exception: it was `[ ]`, not `[~]`** — a genuine open gap rather than a
> publication-gated one, which is why it did not clear on 2026-07-28. It was **closed on the
> merits by Phase 113.1** and now reads `[x]`; the hold below applies to the remaining `[~]`
> requirements only.
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

- [x] **HTTP-09**: Every peer-controlled read on the v2 transport path is memory-bounded. Closure is **enumerable, not narrative**: a tripwire test asserts that no unbounded whole-body read (`.collect()`, `read_to_end`) and no unbounded accumulation over peer-supplied bytes exists in `src/shared/`, `src/client/subscriptions.rs`, or `src/server/streamable_http_server.rs` outside an explicit reviewed allowlist, and that no scan over peer-chosen input is worse than O(n).

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

> **Status marker `[~]` — implemented, gated on the final schema. Booked by Phase 114 plan 114-18
> (2026-08-01) under **D-18**.** All six TASK requirements are implemented and green at Phase-114
> HEAD, and none is marked complete. The completion gate is
> [`114-SPEC-RECHECK.md`](phases/114-tasks-extension-migration/114-SPEC-RECHECK.md) — read it before
> flipping anything here. Its `## Verdict` is **`PENDING`**.
>
> **All six flip together, never individually**, and only on a `PUBLISHED-CONFIRMED` landing of that
> record's `## Procedure` step 4. Splitting the wire-exact TASK-02/04 from the schema-independent
> TASK-01/03/05/06 was considered during discussion and **not** chosen.
>
> **The remaining trigger is now a ONE-repository check.** Re-measured with the prescribed `gh api`
> form on **2026-08-01T00:09:19Z**: `modelcontextprotocol/modelcontextprotocol` has published
> `schema/2026-07-28/` (condition **met**), while `modelcontextprotocol/ext-tasks` still carries
> `schema/draft/` and `specification/draft/` only, with **0 tags and 0 releases** (condition **NOT
> met**). Under the DQ6 both-repositories trigger that is a **partial publication**, which the
> record's `## Third Outcome Policy` rule 5 defines as **`STILL-ABSENT`** — so the hold stays
> engaged. **Watch `ext-tasks`; nothing else is outstanding.**

- [~] **TASK-01**: Tasks are negotiated on v2 via the extensions map (`io.modelcontextprotocol/tasks`); v1 `experimental.tasks` negotiation continues to work — *implemented; pending final schema*
- [~] **TASK-02**: A client can feed input into a running task via `tasks/update` — *implemented; pending final schema*
- [~] **TASK-03**: `tasks/list` (and blocking `tasks/result` semantics per final spec) are era-gated off on v2 while remaining fully functional for v1 consumers — *implemented; pending final schema*
- [~] **TASK-04**: v2 task-augmented results use `resultType:"task"` with `CreateTaskResult{taskId,status,ttlMs,pollIntervalMs}`, and the v1 5-state machine maps deterministically to the v2 status enum (`working|input_required|completed|failed|cancelled`) — *implemented; pending final schema*
- [~] **TASK-05**: On v2, task owner binding requires OAuth `sub` or a stable per-request identity and fails closed when absent (no session-id fallback); a security test proves no cross-caller task visibility — *implemented; pending final schema*
- [~] **TASK-06**: The `TaskStore` trait, state machine, and DynamoDB/Redis/in-memory backends survive unchanged — the migration is a wire-API reshape behind the `TaskRouter` boundary, not a storage rewrite — *implemented; pending final schema*

> **⚠ TASK-05's "fails closed" is narrower than it reads, and the booking carries the
> qualification rather than absorbing it.** `114-SPEC-RECHECK.md` § *⚠ Known INTERNAL wording gap —
> TASK-05 "fails closed" vs D-07 row 3* obliges this booking to say so. **"Fails closed" applies to
> **auth-configured deployments**** — a server that has an auth provider and receives a caller with
> no subject is refused `-32003`. On a server with **no auth provider at all**, D-07 row 3
> deliberately maps every anonymous caller onto one `ANONYMOUS_PRINCIPAL` (`""`) bucket, so v2 tasks
> there run in a **single shared bucket by design**: a development / stdio affordance, **not**
> per-caller isolation. D-07 is a **LOCKED** decision, implemented verbatim by 114-09, and this row
> does not reopen it. It is independently bounded on the production backends —
> `TaskSecurityConfig::default()` sets `allow_anonymous: false`, so `GenericTaskStore` refuses that
> bucket unless an operator opts in. **The no-cross-caller-visibility half is proven, not asserted:**
> `tests/v2_tasks_security.rs` (114-15) closes all three v2 `tasks/*` methods to a cross-caller over
> a real socket, with the refusals indistinguishable from an absent id on both code and message, and
> `114-15-SUMMARY.md` § *BLOCKING: TASK-05 security defect* records **NONE FOUND**. The named future
> closure is the configurable proxy-header / claim-based identity source, which is **deferred, not
> scheduled**.

> **⚠ TASK-04's `resultType:"task"` is conformant-by-extension, and that is a judgement this booking
> makes explicitly rather than absorbing.** Measured 2026-08-01 against the **published** core
> `schema/2026-07-28/schema.ts`: `Result.resultType` is **required** (`resultType: ResultType`, with
> *"Servers implementing this protocol version MUST include this field"*) and
> `ResultType = "complete" | "input_required" | string`. `"task"` is **not** a named upstream value;
> it is admissible only through the open `| string` tail — and the `io.modelcontextprotocol/tasks`
> extension is precisely what names it (`schema.ts:228-229`, *"The resultType field MUST be set to
> `\"task\"`"*). **Verdict: conformant-by-extension, NOT prospective drift** — an extension supplying
> a value through a deliberately open union is the mechanism working as designed. It nevertheless
> stays under the D-18 hold, because the sentence that mandates `"task"` lives in the unpublished
> `ext-tasks` draft. **One correction to the 2026-07-29 advance observation:** that run recorded
> Phase 112's absent-`resultType`-means-`complete` decoding as *"a tolerance, not the contract"*. The
> published core states the opposite — a client **MUST** treat an absent field as `"complete"` when
> the server implements an earlier protocol version — so pmcp's decoding **is** the contract.

### JSON Schema 2020-12 & Caching Hints (SCHM)

- [x] **SCHM-01**: Schema validation runs Draft 2020-12 explicitly pinned (jsonschema 0.49, no `$schema` auto-detect), staying wasm-clean and SEP-2106-compliant (no external `$ref` dereference)

> **CLOSED AGAIN 2026-08-02 — re-booked on POST-FIX measured evidence covering the COLLIDING-NAME
> case, by the `115-14` + `115-15` gap-closure pair (round 2).** Both blocks below are kept
> VERBATIM. The one immediately following is `115-13`'s closure record; the one after it is the
> original downgrade. Neither is deleted — the sequence *is* the finding. (The downgrade block's
> heading word is deliberately not repeated in THIS block either, so the `grep -c` count of that word
> over this file stays at 1 — the check that proves the record was amended rather than removed.)
>
> **`115-13`'s `[x]` was premature — for the SECOND time on this requirement.** Its evidence table
> was accurate for the cases it MEASURED (an embedded resource filed under a `$defs` entry named
> `Inner`), but its conclusion generalized past them. `115-VERIFICATION.md` falsified it by renaming
> a single `$defs` key. This is ledger `D-115-G` — a requirement booked ahead of the evidence that
> would falsify it — recurring in a narrower form on the very requirement `D-115-G` was filed about,
> and naming that plainly is more useful to a future reader than the marker itself.
>
> **The residual defect.** `115-12`'s recursive walk was POSITION-BLIND: it tested
> `DATA_ONLY_KEYWORDS` (`const`/`enum`/`default`/`examples`) against EVERY object key. But the keys
> of `properties` / `patternProperties` / `$defs` / `definitions` / `dependentSchemas` are
> AUTHOR-CHOSEN NAMES, never keywords. An `$id`-bearing embedded schema resource filed under a
> `$defs` entry an author had NAMED `default` was therefore visited by NEITHER walker, and its legacy
> `$schema` survived the v2 pin — the same vacuous-validator bypass, moved sideways instead of down.
>
> **The shipped fix (`115-14`, commits `f8692f1d` / `07bfdd52` / `2bf4d637`).** A
> `SUBSCHEMA_MAP_KEYWORDS` constant consulted FIRST in the member dispatch of BOTH walkers, making it
> a three-way decision: a member in that list whose value is an object → recurse into every VALUE,
> never keyword-filtering the map's own keys; the same key with a non-object (malformed) value →
> ordinary walk, so no coverage is lost; otherwise → the `DATA_ONLY_KEYWORDS` skip, unchanged. Both
> signatures stay byte-identical to `contracts/binding.yaml`. The dispatch was extracted into
> `first_legacy_dialect_in_member` / `pin_dialect_in_member` only AFTER measuring that inline it put
> `pin_dialect_in_place` at cognitive 24 against `pmat quality-gate`'s threshold of 23; no `#[allow]`
> was used.
>
> **The measurement**, through the same seam (`output_validation::fuzz_support`, `jsonschema`
> 0.49.2), instance `{"n": "NOT-AN-INTEGER"}`:
>
> | Document | Before 115-14 | After 115-14 |
> |---|---|---|
> | `$defs.Inner` (control) | `(Conforms, Violates)`, `rewritten=true` | `(Conforms, Violates)`, `rewritten=true` |
> | **`$defs.default`** | **`(Conforms, Conforms)`, `rewritten=false`** | **`(Conforms, Violates)`, `rewritten=true`** |
> | `$defs.const` / `.enum` / `.examples` | as `$defs.default` | enforced, as the control |
> | `properties.{const,enum,default,examples}` | not rewritten | `Cow::Owned`, `/properties/<name>/$schema == 2020-12` |
>
> `rewritten=false` is the part with teeth: the normalizer returned `Cow::Borrowed`, so no
> `tracing::warn!` fired either and the author got NO signal. The `properties`-position row is fenced
> STRUCTURALLY, deliberately — `jsonschema` 0.49.2 still enforces `type` there against the DEFECTIVE
> code, so a behavioural assertion would have been a fence that can never fire.
>
> **The fences, by name, with counts and gate visibility:**
>
> | Fence | Where | Count / state |
> |---|---|---|
> | `v2_pin_still_enforces_an_embedded_resource_named_like_a_data_keyword` | `mod tests`, feature `validation` — **gate-visible** | in the **18** `output_validation::tests` (17 + 1) |
> | `normalization_cases()` (f) `$defs.default` and (g) `properties.examples` | same | flow through the structural and idempotence fences automatically; `normalization_cases()` returns 7 |
> | `property_normalization_does_not_depend_on_a_subschema_map_key_name` | `tests/property_tests.rs`, `--features "full fuzzing"` | **20** vs **18** under `--features full`; generator now DRAWS the colliding names — **58 of 256** cases drew one together with an embedded non-2020-12 dialect, all 12 container×name combinations hit |
> | fuzz **invariant 6** `assert_normalization_is_invariant_under_rename` | `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs` | derived from the spec, not restated from the crate's lists |
> | seed `14_defs_named_default` | `fuzz/corpus/fuzz_schema_draft_pin/` | **14** committed seeds; `-runs=0` replay exit 0 (15 996 runs); `-max_total_time=300` → **3 697 874** runs, exit 0, artifacts dir EMPTY |
>
> **THREE negative controls OBSERVED this round — because an unfired fence is not evidence.** That is
> the standard `115-VERIFICATION.md` applied when it refused to inherit the SUMMARYs' conclusions.
>
> 1. `115-14` Task 1, against the position-blind body: **16 passed / 2 failed**, exactly the two
>    predicted — `v2_pin_still_enforces_an_embedded_resource_named_like_a_data_keyword` (`BYPASS
>    ($defs.const): the v2 Draft 2020-12 pin accepted a STRING where the embedded schema resource
>    declares integer`) and `normalize_schema_dialect_changes_only_dollar_schema_keys` (borrow/own,
>    `left: false, right: true`).
> 2. `115-15` Task 1, with the position-blind member filter restored:
>    `property_normalization_does_not_depend_on_a_subschema_map_key_name` FAILED — *"RENAME
>    INVARIANCE VIOLATED at `/$defs/const` vs `/$defs/__rename_probe__`"* — with a shrunk
>    counterexample whose entry name is one of the four colliding literals.
> 3. `115-15` Task 2, same revert: seed `14_defs_named_default` exits **1** naming invariant 5. And
>    the decisive one — with BOTH restated copies of the rule ALSO made blind (so invariants 2 and 5
>    pass vacuously, exactly as they did pre-`115-14`), that seed still exits **1**, naming
>    **invariant 6**. That is the direct proof that invariant 6 is the instrument for a defect in the
>    shared rule.
>
> All were restored from `shasum -a 256 -c`-verified snapshots and re-run clean; `git status --short
> src/` empty afterwards.
>
> **The STRUCTURAL finding, and what was done about it.** All three fences that existed before this
> round RESTATED the code's own traversal rule: the unit postcondition called the crate's own blind
> DETECTOR, the property generator hard-coded the definition name `"Inner"` so its space could not
> draw a colliding one, and fuzz invariant 5's collector re-implemented the same filter while its
> module doc called the scan *"TOTAL — no skip condition"* and *"INDEPENDENT"*. Independent in
> IMPLEMENTATION only, never in RULE — and a rule defect is exactly what that cannot catch. It was
> MEASURED: for both `$defs.default` and `properties.examples`, `owned=false` (nothing rewritten) yet
> `first_legacy_dialect(&normalized) == None` PASSED. A postcondition satisfied vacuously by the
> defect it was written to catch.
>
> The repair is a metamorphic relation DERIVED from a JSON Schema 2020-12 fact instead of restated
> from pmcp's source: the keys of the five subschema-map keywords are author-chosen names with no
> keyword semantics under the core and applicator vocabularies, therefore **normalizing an entry must
> not depend on the name it is filed under**. It consults no keyword list at all, it fires on the
> shipped defect immediately, and it would equally catch a future rule defect that special-cases some
> other name or gains a sixth data-only keyword without gaining the position exception. It exists in
> both generators, and both were observed to fire. Invariant 5's two false doc claims are corrected
> in place.
>
> **The whole-phase gate, run over the fixed tree BEFORE this marker was written (`115-15` Task 3).**
> `/usr/bin/make quality-gate` exit **0** — **5054 passed / 0 failed / 81 ignored across 309 `test
> result:` lines**. `pmat quality-gate --fail-on-violation --checks complexity` exit **0**, **0
> violations**, so both reshaped walkers stay under the gate with no `#[allow]`. SCHM-02/SCHM-03
> re-run unregressed at exactly the counts `115-VERIFICATION.md` measured: **78/78** across
> `structured_tool_output` 20, `v2_caching_hints` 19, `v1_lists_golden` 7, `v2_schema_tripwires` 13,
> `v2_core_schema_facts` 8, `vendored_schema_provenance` 6, `phase115_contract_bindings` 5. No
> `Cargo.toml` / `Cargo.lock` anywhere in the `115-14`+`115-15` closure diff, and **0** new `pub fn` /
> `pub struct` / `pub enum` lines under `src/` — the milestone's additive 2.x-minor posture holds
> without a `cargo public-api` run.
>
> **Provenance.** This closure is **option (a)** of `115-VERIFICATION.md` § *Human Verification
> Required* — "accept a further closure plan implementing position-aware traversal" — and NOT option
> (b), an override. The owner's `115-10` sign-off (Guy Ernest, 2026-08-01, commit `496da96b`)
> predates `115-REVIEW.md` and is expressly **not** read as covering CR-01; nothing here relies on
> it. Re-verification is `/gsd:verify-phase 115`'s job and this block is the evidence it should
> score. The marker was written AFTER every command above had run and every count had matched —
> which, on this requirement, is the whole point.

> **CLOSED 2026-08-01 — re-booked `[~]` → `[x]` on POST-FIX measured evidence, by the `115-12` +
> `115-13` gap-closure pair.** The downgrade block immediately below is kept VERBATIM: it is the
> honest record of a booking that was wrong, and `/gsd:verify-phase 115` will be re-run against it.
> Nothing in it is deleted; this block states what changed. (Its heading word is deliberately not
> repeated here, so the `grep -c` count of that word over this file stays at its pre-closure value
> of 1 — the check that proves the record was amended rather than removed.)
>
> **The defect.** `normalize_schema_dialect` rewrote the ROOT `$schema` only. Under Draft 2020-12 a
> `$schema` is legal at the root of any EMBEDDED SCHEMA RESOURCE — a subschema carrying `$id` — and
> `jsonschema` 0.49.2 honours it there, so a legacy declaration on such a resource survived the pin,
> resolved an EMPTY vocabulary set and produced an accept-everything sub-validator: the
> vacuous-validator bypass the pin exists to close, moved one level down.
>
> **The shipped fix (`115-12`, commits `fdf236c8` / `a9af3a5d` / `60cda794`).** The signature is
> unchanged (`fn normalize_schema_dialect(schema: &Value) -> Cow<'_, Value>`, byte-identical to
> `contracts/binding.yaml`) and the `Cow::Borrowed` zero-allocation path survives. The body is now a
> detector (`first_legacy_dialect`) / rewriter (`pin_dialect_in_place`) pair implementing ONE
> traversal rule stated once in rustdoc, under two guards that are load-bearing, not cosmetic:
> a `$schema` is a declaration **only when its value is a `Value::String`** (the code review's own
> fix sketch used `map.contains_key("$schema")`, which would have replaced a `properties` subschema
> named `$schema` with a string and made the document uncompilable), and the walk **never descends
> into `const` / `enum` / `default` / `examples`** (a `$schema` there is instance DATA, and
> rewriting it changes which instances conform). The `expect` the old body carried is gone, replaced
> by the checkable postcondition `first_legacy_dialect(&owned) == None`.
>
> **The three-row measurement, RE-RUN post-fix** through the same seam the review and the verifier
> used (`output_validation::fuzz_support::validate_bytes`, `jsonschema` 0.49.2), schema =
> `properties.n → $ref "#/$defs/Inner"` with `$defs.Inner` carrying `$id` + `$schema: draft-07` +
> `type: integer`, instance `{"n": "NOT-AN-INTEGER"}`:
>
> | Case | Before 115-12 | After 115-12 |
> |---|---|---|
> | embedded-legacy-resource | `(Conforms, Conforms)` | `(Conforms, Violates)` |
> | control-no-nested-schema | `(Violates, Violates)` | `(Violates, Violates)` |
> | **root-draft07 + embedded** | `(Violates, Conforms)` | **`(Violates, Violates)`** |
>
> Row 3 is the clause this requirement's text turns on, and it now reads `(Violates, Violates)` —
> v2 is no longer weaker than v1. Row 1's v1 column deliberately stays `Conforms`: D-01 freezes the
> v1 arm at `jsonschema::validator_for`, whose auto-detect still honours the embedded declaration.
> That is the freeze working, and `v2_pin_still_enforces_an_embedded_legacy_resource` asserts it
> stays put.
>
> **The fences, by name and count** — the point being that the defect shipped past a green gate
> because all three of its would-be fences either excluded the shape structurally or sat behind a
> feature the gate does not enable:
>
> | Fence | Where | Count / state |
> |---|---|---|
> | `v2_pin_still_enforces_an_embedded_legacy_resource` | `mod tests`, feature `validation` — **gate-visible** | in the 17 `output_validation::tests` |
> | `normalize_schema_dialect_leaves_a_dollar_schema_that_is_data_alone` | same | guards the string-valued rule |
> | `normalization_cases()` case (e) | same | the `$id`-bearing document, `expected_owned == true` |
> | `property_schema_normalization_is_idempotent_and_surgical` | `tests/property_tests.rs`, `--features "full fuzzing"` | **19** vs **18** under `--features full`; generator now EMITS `$id`-bearing embedded resources — **100 of 256** generated cases carried an embedded non-2020-12 declaration |
> | fuzz **invariant 5** `assert_no_legacy_dialect_survives` | `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs` | TOTAL, no skip; walk implemented INDEPENDENTLY of the crate's own detector |
> | seeds `12_embedded_legacy_resource`, `13_embedded_resource_no_dialect` | `fuzz/corpus/fuzz_schema_draft_pin/` | **13** committed seeds; `-runs=0` replay exit 0; `-max_total_time=300` → **3 951 202** runs, exit 0, artifacts dir EMPTY |
>
> **Negative controls, OBSERVED — because an unfired fence is not evidence.** That is the standard
> `115-VERIFICATION.md` applied when it refused to inherit the SUMMARYs' conclusions, and it is
> applied to the closure too. Against a deliberately reverted (root-only) `pin_dialect_in_place`:
> `115-12` observed **15 passed / 2 failed** with the behavioural and the borrow/own fences both
> firing; `115-13` Task 1 observed the property test FAIL with the dialect-purity message on
> `{"$defs":{"Inner":{"$id":…,"$schema":"…draft-04…","type":"integer"}}}`; `115-13` Task 2 observed
> seed `12_embedded_legacy_resource` trip invariant 5 with **exit 77**. All three were restored and
> re-run clean.
>
> **The whole-phase gate, run once over the fixed tree (`115-13` Task 3).** `make quality-gate`
> exit **0** — **5052 passed / 0 failed / 81 ignored across 309 `test result:` lines**, 0 non-`ok.`
> lines. `pmat quality-gate --fail-on-violation --checks complexity` (the PR-blocking CI check
> `make quality-gate` does NOT cover) → **PASSED, 0 violations**, so the three reshaped functions
> stay under cognitive 25 with no `#[allow]`. SCHM-02/SCHM-03's suites re-run unregressed at exactly
> the counts `115-VERIFICATION.md` measured: **78/78** across `structured_tool_output` 20,
> `v2_caching_hints` 19, `v1_lists_golden` 7, `v2_schema_tripwires` 13, `v2_core_schema_facts` 8,
> `vendored_schema_provenance` 6, `phase115_contract_bindings` 5. No `Cargo.toml` / `Cargo.lock` in
> the closure diff (no supply-chain review triggered) and **0** new `pub fn` / `pub struct` /
> `pub enum` lines under `src/` (the milestone's additive 2.x-minor posture is preserved).
>
> The first gate run FAILED (exit 2) on a `clippy::similar_names` error `115-12` introduced —
> `row3` beside `rows` — which `115-12`'s own `cargo clippy --all-targets --features full -D
> warnings` did not see because `similar_names` is pedantic and only `make lint` enables that group.
> Fixed by renaming, not by an `#[allow]` (commit `cab8937a`). It is recorded here rather than
> absorbed because it is the measured instance of CLAUDE.md § *Why `make quality-gate` (not
> individual cargo commands)*.
>
> **Provenance of this booking.** This closure was executed as **option (a)** of
> `115-VERIFICATION.md` § *Human Verification Required* — "accept a closure plan implementing the
> recursive-normalization fix" — and NOT as option (b), an override. The owner's `115-10` sign-off
> (Guy Ernest, 2026-08-01, commit `496da96b`) **predates `115-REVIEW.md`** and is therefore **not**
> being read as covering CR-01; nothing in this block relies on it. Re-verification is
> `/gsd:verify-phase 115`'s job and this block is the evidence it should score, not a substitute
> for it. Ledger entry `D-115-G` — a requirement flipped before its evidence existed — is the
> process defect this re-booking was written to avoid repeating.

> **REOPENED 2026-08-01 — booking downgraded `[x]` → `[~]` after verification.** *(Superseded by the
> CLOSED block above; amended, not deleted, by `115-13`.)* The `[x]`
> below was written by `115-10` Task 3 immediately after owner sign-off, which predates
> `115-REVIEW.md`. `115-VERIFICATION.md` (status `gaps_found`, 3/4) then measured that the
> "no `$schema` auto-detect" clause **does not hold**: `normalize_schema_dialect`
> (`src/server/output_validation.rs:146-165`) rewrites only the ROOT `$schema`, so a legacy
> dialect declaration on an embedded schema resource (a subschema carrying `$id`) survives
> the pin and yields the vacuous accept-everything validator the pin exists to prevent.
> Reproduced independently twice via `output_validation::fuzz_support::validate_bytes`,
> including `root-draft07 + embedded (v1,v2) = (Violates, Conforms)` — v2 validating
> **weaker** than v1. All three defensive layers structurally exclude the shape
> (`normalization_cases()`, `arb_schema_document()`, `is_dialect_neutral`), which is why a
> green gate and 660k fuzz runs did not reach it. The version text is also corrected here:
> 0.49 shipped, not the 0.48 originally named. SCHM-02 and SCHM-03 are unaffected and remain
> `[x]` — both were re-measured against the codebase during verification. Gap closure is
> tracked in `115-VERIFICATION.md`.

> **Booked `[x]`, NOT `[~]`, and the distinction is deliberate.** Phase 114's D-18 hold exists
> because its wire values come from an unpublished `draft/` directory in an Experimental
> repository. **Phase 115's do not.** Its values come from the **published** core schema for
> protocol version `2026-07-28`, vendored at `schema/vendored/core-2026-07-28/` from
> `modelcontextprotocol/modelcontextprotocol` at pinned commit
> `271ecc9accafdd9b83a3c869fa67c22953b2af80` — a **versioned** upstream directory, not `draft/`.
> Both files are digest-fenced by `tests/vendored_schema_provenance.rs` (SHA-256 **and** git blob
> SHA-1, cross-checked against the GitHub contents API at the pin), and the wire facts are
> **re-derived from those bytes at runtime** by `tests/v2_core_schema_facts.rs`. Decision **D-15**
> states the target plainly: *"Phase 115 has NO publication hold and must not inherit a `[~]`
> booking from Phase 114 by habit."* The contingency D-15 kept available (the Phase-113 HTTP-04
> split) **did not fire**. Booking `[~]` here would be exactly the habit D-15 named.
>
> **Measured evidence** *(as of `115-10`. Two rows moved in the gap closure and are superseded by
> the CLOSED block above: `binary(property_tests)` is now **18 / 19**, not 17 / 18, and the corpus
> carries **13** committed seeds, not 12. Kept as written — this table is what `115-10` measured.)*
> (all re-run by `115-10` at phase close, by binary name, because
> `make validate-always`'s three ALWAYS targets are fail-open — see `deferred-items.md` entries
> `U`/`V`/`W`):
>
> | Evidence | Count |
> |---|---|
> | `binary(vendored_schema_provenance)` | 6 tests |
> | `binary(v2_core_schema_facts)` | 8 tests |
> | `binary(v2_schema_tripwires)` | 13 tests (SEP-2106 over cargo's DECLARED **and** RESOLVED graphs) |
> | `--lib -E 'test(/output_validation::/)'` | 15 tests |
> | `binary(property_tests)` | 17 (`--features full`) / 18 (`--features "full fuzzing"`) |
> | `fuzz_schema_draft_pin` | corpus replay of 12 committed seeds exit 0; a 60 s session ran **660,271** executions and left `fuzz/artifacts/fuzz_schema_draft_pin/` **EMPTY** |
>
> **The judgement this booking MAKES rather than absorbs.** *"Draft 2020-12 explicitly pinned"* is
> satisfied by **normalize-then-compile**, not by the naive pin — because the naive pin was
> **MEASURED to be a silent validation BYPASS**. `jsonschema`'s `draft202012::new` sets the keyword
> set, but a document declaring a legacy meta-schema still resolves its *vocabularies* from that
> declaration, and under 2020-12 vocabulary semantics a draft-07 declaration yields an EMPTY
> vocabulary set — a validator that accepts **every** instance. Measured across `jsonschema`
> 0.46.10 / 0.47.0 / 0.48.0 / 0.48.5 / 0.49.2, and `draft202012::meta::is_valid` returns `true` for
> such a document, so there is no library-side detector. The pin is therefore implemented as
> `normalize_schema_dialect` (pure, idempotent, `Cow`-returning, ~~root `$schema` only~~ —
> **CORRECTED by `115-12`: EVERY string-valued `$schema` at every depth; see the CLOSED block
> above**) followed by
> `compile_2020_12`, fenced by a draft-07 test **whose negative control was observed to fire** —
> see `115-03-SUMMARY.md`. `compile_for_era` keeps v1's `jsonschema::validator_for` auto-detect
> **verbatim** (D-01 freeze) and is the only auto-detect entry point left in the module.
>
> **The wasm-clean half is proven by an explicit command, because the gate does not prove it.**
> `make wasm-build` (`Makefile:59-62`) passes only `--features wasm` and therefore **never compiles
> `jsonschema` at all**. The evidence is
> `cargo build --target wasm32-unknown-unknown --no-default-features --features "wasm,validation"`
> — **exit 0** at phase close. `make wasm-build` also exits 0, but on its own it is not evidence
> for this requirement (ledger entry `X`).
>
> **SEP-2106** (no external `$ref` dereference) is fenced against **both** of cargo's dependency
> graphs via `cargo metadata` — the declared graph and the feature-resolved graph — rather than by
> scanning `Cargo.toml` as text, so a renamed or table-style dependency and graph-wide feature
> unification are all caught. Remote-ref resolution stays disabled: an external `$ref` must fail to
> **compile**, with zero I/O.
>
> **DEVIATION — shipped `jsonschema = "0.49"`, not the literal `0.48` in this requirement's text.**
> 0.48.0–0.48.2 carry packaging defects fixed in 0.48.3–0.48.5, and 0.49 is additive-only over
> 0.48. An exact `=0.49.2` pin was **DECLINED**: pinning an exact version in a published *library*
> crate propagates the constraint to every downstream consumer. The residual — `Cargo.lock` is
> gitignored, so the bump has no reviewable lockfile diff — is recorded as ledger entry `4`.

- [x] **SCHM-02**: On v2, `structuredContent` accepts any JSON value (scalar/array/null/object); v1-negotiated tools keep the existing object-shaped behavior

> **Booked `[x]` on the same published-artifact evidence as SCHM-01** — the shape claim is
> re-derived from `schema/vendored/core-2026-07-28/` at pinned commit
> `271ecc9accafdd9b83a3c869fa67c22953b2af80`, where `CallToolResult.structuredContent` is declared
> `structuredContent?: unknown` — *"any JSON value (object, array, string, number, boolean, or
> null)"*. **Not `[~]`:** there is no publication hold on this value.
>
> **Measured evidence:** `binary(structured_tool_output)` — **20 tests**, covering scalar, array,
> string, boolean and explicit-`null` payloads across **both** native dispatchers. Public API:
> `CallToolResult::structured_value(Value) -> Self` (the additive widening sibling;
> `CallToolResult::structured` keeps its exact signature and object-shaped intent under the D-06
> freeze). `s52_v2_caching_hints` prints `"structuredContent":42` on a live v2 wire and
> `"structuredContent":null` for a present-null payload.
>
> **Finding 6 held, and this booking states it rather than absorbing it: THERE WAS NO OBJECT-ONLY
> GUARD IN pmcp TO REMOVE.** The v1 constraint lived in v1 *spec text*, never in pmcp code — the
> field has always been `Option<Value>` and neither native dispatcher shape-checks the handler's
> value on the way out. So pmcp **already emitted** non-object `structuredContent` on v1, which is
> more permissive than v1's own spec allows. **Decision D-05 FREEZES that over-permissiveness
> rather than correcting it**, because tightening v1 to reject scalars would itself be a v1 wire
> change. `tests/structured_tool_output.rs` fences the v1 half on both dispatchers precisely so a
> later "correctness" tightening fails loudly.
>
> **The v2 claim is proven with an IN-BAND era witness.** The pre-review version of these tests
> would have run as **v1** while asserting v2 behaviour — a green suite proving nothing. The
> landed tests assert on the in-band `resultType` field arriving in the same response, so a test
> that silently negotiated v1 fails instead of passing.
>
> **KNOWN LIMITATION, accepted not hidden:** a present `structuredContent: null` does not survive a
> typed re-read. The server is correct on the wire (asserted twice); serde's default `Option<T>`
> deserializer collapses JSON `null` onto `None` on the way back in, so `CallToolResult`'s own
> `Deserialize` cannot distinguish "null" from "absent". Pre-existing on both eras, fenced by
> `present_null_structured_content_does_not_survive_a_typed_reread`, and booked as ledger entry `L`.

- [x] **SCHM-03**: The five list/read results carry `ttlMs`/`cacheScope` caching hints (additive fields)

> **Booked `[x]` on published evidence.** `CacheableResult` **is in the published core schema** —
> `schema/vendored/core-2026-07-28/` at pinned commit
> `271ecc9accafdd9b83a3c869fa67c22953b2af80`, digest-fenced by
> `tests/vendored_schema_provenance.rs`, with the contract re-derived from those bytes at runtime
> by `tests/v2_core_schema_facts.rs`. That test also measured `ttlMs` as
> `{"type": "integer", "minimum": 0}` — integrality and non-negativity are **contract**, which is
> why the Rust mapping is `u64` and not `f64`. **Not `[~]`:** D-15's contingency did not fire.
>
> **Measured evidence:**
>
> | Evidence | Count |
> |---|---|
> | `binary(v2_caching_hints)` | **19 tests** — six methods × two eras × both native dispatchers |
> | `binary(v1_lists_golden)` | 7 tests — pre-change raw-byte goldens with a leak guard **proven to fire** |
> | `binary(v2_schema_tripwires)` | 13 tests — D-12 single-projection, the wasm call site, the middleware ordering |
> | `--lib -E 'test(/types::caching/)'` | 15 tests |
> | `--lib -E 'test(/inject_v2_result_envelope/)'` | 26 tests |
> | `s52_v2_caching_hints` | exit 0: `ttlMs`/`cacheScope` present on the v2 responses, **actively stripped** on the v1 one |
>
> **DEVIATION — SIX result types carry the hints, not the FIVE in this requirement's text.**
> `DiscoverResult extends CacheableResult` in the pinned published schema, alongside
> `ListToolsResult`, `ListResourcesResult`, `ListResourceTemplatesResult`, `ReadResourceResult` and
> `ListPromptsResult`. `server/discover` is therefore included: excluding it would have shipped a
> knowingly non-conformant **first call** for every v2 client, and including it is *cheaper*,
> because `ServerDiscoverResult` already routes through the same `inject_v2_result_envelope`
> chokepoint. The projection is a single shared, **cfg-free** `project_caching_hints` wired into
> **all three** dispatchers including the wasm one — closing a v1 leak the cross-AI review found.
>
> **SCOPE BOUND, asserted at a named test rather than left implicit.**
> `extract_request_meta_value` (`src/server/core.rs`) reads the typed `_meta` era signal from only
> `CallTool`, `GetPrompt` and `ReadResource`, so **four of the six methods cannot reach `Era::V2`
> through in-process `ServerCore` dispatch at all**. Their v2 evidence is therefore over **HTTP**,
> where the era arrives on the transport, and the bound is pinned by a named test so it can neither
> widen nor persist unnoticed (ledger entry `Q`).
>
> **Two further limitations this booking names rather than buries.** (1) Only **two** of the six —
> `ListResourcesResult` and `ReadResourceResult` — are settable by a `ResourceHandler`; the other
> four, including `resources/templates/list`, always emit the SDK default (`ttlMs: 0`,
> `cacheScope: "private"`) on v2, because `ResourceHandler` declares only `read` and `list` (ledger
> entry `P`). (2) Response middleware runs **after** the projection, so it can still remove or
> forge the keys; not reordered, because that would change what middleware observes about Phase
> 114's envelope — documented, tested and fenced instead (ledger entry `R`).

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
| HTTP-09 | Phase 113.1 | **Met** — bounded-read tripwire green with an EMPTY `WHOLE_BODY_ALLOWLIST`, plus two falsifiable O(n) guards on `SseParser::feed` |
| CLNT-01 | Phase 113 | Implemented — pending final schema |
| CLNT-02 | Phase 113 | Implemented — pending final schema |
| CLNT-05 | Phase 113 | Implemented — pending final schema |
| TASK-01 | Phase 114 | Implemented — pending final schema |
| TASK-02 | Phase 114 | Implemented — pending final schema |
| TASK-03 | Phase 114 | Implemented — pending final schema |
| TASK-04 | Phase 114 | Implemented — pending final schema |
| TASK-05 | Phase 114 | Implemented — pending final schema (see the TASK-05 scope qualification above) |
| TASK-06 | Phase 114 | Implemented — pending final schema |
| SCHM-01 | Phase 115 | Complete — gap closed in two rounds: 115-12 + 115-13 (recursive `$schema` pin; `root-draft07 + embedded` now `(Violates, Violates)`) then 115-14 + 115-15 (POSITION-AWARE traversal — `SUBSCHEMA_MAP_KEYWORDS`, so a keyword deny-list is never tested against a key in NAME position; `$defs.default` now `(Conforms, Violates)`, `rewritten=true`; rename-invariance fences in both generators, derived from the spec rather than restated from the crate's keyword lists) |
| SCHM-02 | Phase 115 | Complete |
| SCHM-03 | Phase 115 | Complete |
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
| `[~]` / Implemented — pending final schema | Code shipped and green, but the requirement's own SPEC-RECHECK gate has not landed `PUBLISHED-CONFIRMED`. **Two different gates are in play — check which one owns the row before flipping it.** HTTP-0x / CLNT-0x are gated by `113-SPEC-RECHECK.md`; **TASK-01..06 are gated by `114-SPEC-RECHECK.md`, whose DQ6 trigger requires a versioned schema directory in BOTH `modelcontextprotocol/modelcontextprotocol` AND `modelcontextprotocol/ext-tasks`.** As of 2026-08-01 only the core half has published, so the TASK rows stay held. |
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
