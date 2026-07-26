# Phase 113 — Spec Re-Check, Conformance Pin & Contract-First Environment Record

**Produced by:** Plan 113-01, Task 1 (enforcing spec checkpoint)
**Run date (UTC):** 2026-07-25
**Purpose:** This is the ENFORCING gate that decides whether Phase 113 may land v2 wire
constants. It carries four independent records: the schema verdict (Section A), the
conformance-suite pin (Section B), the contract-first / PDMT / PMAT environment state
(Section C), and the cross-plan `Mcp-Name` header rule (Section D).

**No source file was modified by the task that produced this record.**

---

## Section A — Schema Verdict

### A.1 Upstream schema directory listing

Command:

```
gh api repos/modelcontextprotocol/modelcontextprotocol/contents/schema --jq '.[].name'
```

Literal output:

```
2024-11-05
2025-03-26
2025-06-18
2025-11-25
draft
```

**There is NO `schema/2026-07-28` directory.** The final spec publishes 2026-07-28; today is
2026-07-25. Per the task's rule, the `draft` schema was used and this section is `PENDING`.

### A.2 Schema path and commit used

| Field | Value |
|-------|-------|
| Schema path used | `schema/draft/schema.ts` (NOT `schema/2026-07-28/schema.ts` — does not exist) |
| Last-commit sha | `71e306956a4959c9655e5036be215d41986596e6` |
| Last-commit date | 2026-07-16T02:16:04Z |
| Last-commit subject | `feat(schema): add optional serverInfo response metadata and make clientInfo optional (#3002)` |
| File size at that commit | 3184 lines |

Command used to pin the commit:

```
gh api "repos/modelcontextprotocol/modelcontextprotocol/commits?path=schema/draft/schema.ts&per_page=1"
```

### A.3 Token-by-token findings

All thirteen mandated tokens were grepped against the downloaded `schema/draft/schema.ts`.

| # | Token | Status | Declaration / evidence |
|---|-------|--------|------------------------|
| 1 | `inputRequests` | **FOUND** (3 occurrences) | `InputRequiredResult.inputRequests?: InputRequests` (line 588); doc: "Requests issued by the server that must be complete before the client can retry the original request." |
| 2 | `inputResponses` | **FOUND** (1) | `InputResponseRequestParams.inputResponses?: InputResponses` (line 605) |
| 3 | `requestState` | **FOUND** (3) | `InputRequiredResult.requestState?: string` (line 594) AND `InputResponseRequestParams.requestState?: string` (line 608) |
| 4 | `resultType` | **FOUND** (2) | `resultType: ResultType` (line 234) — **required** field on `Result`; line 232: "…the client MUST treat the absent field as `\"complete\"`" |
| 5 | `input_required` | **FOUND** (2) | `export type ResultType = "complete" \| "input_required" \| string;` (line 216) |
| 6 | `InputResponseRequestParams` | **FOUND** (4) | declared line 600; extended by `ReadResourceRequestParams` (1206), `GetPromptRequestParams` (1589), `CallToolRequestParams` (1850) — confirming MRTR applies to exactly `resources/read`, `prompts/get`, `tools/call` |
| 7 | `SubscriptionsListenRequest` | **FOUND** (15) | `export interface SubscriptionsListenRequest extends JSONRPCRequest { method: "subscriptions/listen"; params: SubscriptionsListenRequestParams; }` (line 1314) |
| 8 | `notifications/subscriptions/acknowledged` | **FOUND** (4) | `SubscriptionsAcknowledgedNotification.method` (line ~1384) |
| 9 | `io.modelcontextprotocol/serverInfo` | **FOUND** (1) | `ResultMetaObject["io.modelcontextprotocol/serverInfo"]?: Implementation` (line 157) — **inside result `_meta`**, corroborating Pitfall 6 |
| 10 | `io.modelcontextprotocol/subscriptionId` | **FOUND** (3) | `SubscriptionsListenResultMeta["io.modelcontextprotocol/subscriptionId"]: RequestId` (line ~1335) — REQUIRED (not optional) on that meta object |
| 11 | `-32020` | **FOUND** (3) | `export const HEADER_MISMATCH = -32020;` (line 434) |
| 12 | `-32021` | **FOUND** (2) | `export const MISSING_REQUIRED_CLIENT_CAPABILITY = -32021;` (line 442) |
| 13 | `-32022` | **FOUND** (2) | `export const UNSUPPORTED_PROTOCOL_VERSION = -32022;` (line 450) |

**MISSING count: 0.** Every mandated token is present in the draft.

### A.4 Error-code block — verbatim

The draft schema names the three constants with **exactly the identifiers plan 113-01 Task 3
specifies for the Rust side**, which is a strong (but pre-final) corroboration:

```typescript
// schema/draft/schema.ts:407-450
/*
 * MCP error codes.
 * ...
 * - `-32000` to `-32019`: implementation-defined. ...
 * - `-32020` to `-32099`: reserved for error codes defined by the MCP
 *   specification. ...
 *
 * Codes defined by earlier protocol versions remain reserved and are never
 * reused: `-32002` (resource not found, 2025-11-25 and earlier; replaced by
 * `-32602`) and `-32042` (URL elicitation required, 2025-11-25 only).
 */
export const HEADER_MISMATCH = -32020;
export const MISSING_REQUIRED_CLIENT_CAPABILITY = -32021;
export const UNSUPPORTED_PROTOCOL_VERSION = -32022;
```

HTTP-status mappings and `error.data` payload shapes, transcribed from the same block:

| Constant | Value | HTTP status (spec MUST) | `error.data` shape |
|----------|-------|-------------------------|--------------------|
| `HEADER_MISMATCH` | `-32020` | `400 Bad Request` | none declared |
| `UNSUPPORTED_PROTOCOL_VERSION` | `-32022` | `400 Bad Request` | `{ supported: string[]; requested: string }` |
| `MISSING_REQUIRED_CLIENT_CAPABILITY` | `-32021` | `400 Bad Request` | `{ requiredCapabilities: ClientCapabilities }` — an **object**, never an array |

**`-32002` open item RESOLVED by the draft:** the rename `-32002` → `-32602` targets
*resource not found*, NOT task-pending. pmcp's proprietary `V1_TASK_PENDING` squat on `-32002`
is unaffected and must stay frozen. (Confirms Phase 112's decision to keep both `-32002`
meanings by name.)

### A.5 MRTR type shapes — verbatim

```typescript
// schema/draft/schema.ts:536-608
export type InputRequest  = CreateMessageRequest | ListRootsRequest | ElicitRequest;
export type InputResponse = CreateMessageResult | ListRootsResult | ElicitResult;

export interface InputRequests  { [key: string]: InputRequest;  }
export interface InputResponses { [key: string]: InputResponse; }

export interface InputRequiredResult extends Result {
  inputRequests?: InputRequests;
  requestState?: string;
}

export interface InputResponseRequestParams extends RequestParams {
  inputResponses?: InputResponses;
  requestState?: string;
}
```

Doc-comment obligations captured verbatim from the draft:

- `InputRequiredResult`: "At least one of `inputRequests` or `requestState` MUST be present."
- `InputRequiredResult.requestState`: "The client must treat this as an opaque blob; it must
  not interpret it in any way."
- `InputResponseRequestParams.inputResponses`: "For each key in the response's inputRequests
  field, the same key must appear here with the associated response."

`inputResponses`/`requestState` (client→server) are **top-level `params` fields**, sibling to
`name`/`arguments`/`uri` — confirmed structurally by `CallToolRequestParams extends
InputResponseRequestParams` (line 1850), `GetPromptRequestParams extends
InputResponseRequestParams` (1589), `ReadResourceRequestParams extends ResourceRequestParams,
InputResponseRequestParams` (1206). **They are NOT in `_meta`.**

### A.6 Plan-10 lock — exact declared shapes

Plan 10 must derive its Rust types from THIS record, not from prose.

**`SubscriptionFilter`** (all four fields optional):

```typescript
// schema/draft/schema.ts:1270-1288
export interface SubscriptionFilter {
  toolsListChanged?: boolean;
  promptsListChanged?: boolean;
  resourcesListChanged?: boolean;
  resourceSubscriptions?: string[];   // "Replaces the former `resources/subscribe` RPC."
}
```

`resourceSubscriptions` is **`string[]` (an array of resource URIs), optional** — not a map,
not a bool.

**The acknowledged notification's `notifications` wrapper** — the field is **REQUIRED**
(no `?`) and its type is `SubscriptionFilter`:

```typescript
// schema/draft/schema.ts:1358-1366
export interface SubscriptionsAcknowledgedNotificationParams extends NotificationParams {
  /**
   * The subset of requested notification types the server agreed to honor.
   * Only includes notification types the server actually supports; if the
   * client requested an unsupported type (e.g., `promptsListChanged` when
   * the server has no prompts), it is omitted from this set.
   */
  notifications: SubscriptionFilter;
}
```

The request side uses the same required-field shape:

```typescript
// schema/draft/schema.ts:1295-1302
export interface SubscriptionsListenRequestParams extends RequestParams {
  notifications: SubscriptionFilter;   // REQUIRED
}
```

**Ordering MUST, verbatim:** "This notification MUST be the first message the server sends
carrying the subscription's ID in `io.modelcontextprotocol/subscriptionId`. The server MUST NOT
send any notification on the subscription before acknowledging it."

**Graceful teardown:** `SubscriptionsListenResult extends Result { _meta:
SubscriptionsListenResultMeta }` where `_meta` is REQUIRED and carries a REQUIRED
`"io.modelcontextprotocol/subscriptionId": RequestId` whose value "is the JSON-RPC ID of the
`subscriptions/listen` request that opened the stream (and equals this response's `id`)."

### A.7 `Mcp-Name` presence question (feeds Section D)

`schema/draft/schema.ts` does **not** specify HTTP headers at all — the only mention is
line 71, a note that a `_meta` value "MUST match the `MCP-Protocol-Version`" header. The
header contract lives in the companion transport doc, which was fetched separately:
`docs/specification/draft/basic/transports/streamable-http.mdx` (739 lines).

Its **Standard Request Headers** table, verbatim:

```
| Header Name  | Source Field                  | Required For                                           |
| ------------ | ----------------------------- | ------------------------------------------------------ |
| `Mcp-Method` | `method`                      | All requests                                           |
| `Mcp-Name`   | `params.name` or `params.uri` | `tools/call`, `resources/read`, `prompts/get` requests |
```

And the Client Behavior section: "Append the `Mcp-Method` header and, **if applicable**,
`Mcp-Name` header to the request."

**Answer: NO.** The draft spec does NOT require `Mcp-Name` to be present on requests that have
no logical name. It is required only for the three name-bearing methods. This **contradicts**
the rule pmcp implements today — recorded as **DRIFT-1** in Section D.

### A.8 Deltas against 113-RESEARCH.md

Nothing in the research wire contract was contradicted by the schema itself. Two corrections:

1. **`InputRequest` union order.** RESEARCH quotes `ElicitRequest | CreateMessageRequest |
   ListRootsRequest`; the draft at this commit declares `CreateMessageRequest | ListRootsRequest
   | ElicitRequest`. Semantically identical (an untagged union); recorded for exactness only.
2. **`InputRequest`/`InputResponse` are marked `/** @internal */`** in the draft. They are
   type aliases for the union, not exported wire nouns. No impact on the Rust modelling.

## Verdict

PENDING — no `schema/2026-07-28` directory exists in the upstream spec repository as of
2026-07-25 (three days before the 2026-07-28 publication date). The record above was produced
against `schema/draft/schema.ts` @ `71e306956a4959c9655e5036be215d41986596e6` (2026-07-16),
which is the strongest source available today but is **not** the final schema.

All thirteen mandated tokens were FOUND in the draft and every value matches the research wire
contract, including the three error-code constants at exactly `-32020` / `-32021` / `-32022`
under exactly the identifiers `HEADER_MISMATCH`, `MISSING_REQUIRED_CLIENT_CAPABILITY`,
`UNSUPPORTED_PROTOCOL_VERSION`. **That corroboration does not upgrade the verdict.** The
verdict is a statement about the SOURCE, not about agreement: REQUIREMENTS.md lists
"Hard-coding new `-3202x`/`-32602` error codes before the final schema" as explicitly OUT OF
SCOPE, and VERS-06 says the values come from the final `schema.json` ONLY.

**Consequence for Task 3:** under this PENDING verdict, Task 3 may NOT land the three
constants in `src/types/protocol/error_codes.rs` unless a `## Recorded Exception` section
exists below, written by the developer at the Task 2 checkpoint. The `Cargo.toml` half of
Task 3 (the `ring` + `zeroize` promotion) is unaffected by this verdict — it is gated only on
the package-legitimacy half of the Task 2 checkpoint.

**The exception WAS granted.** See `## Recorded Exception` immediately below.

### Verdict re-verification — plan 12 Task 3 (2026-07-26)

The `## Recorded Exception` below makes plan 12 Task 3's re-verification **binding**. It was
executed. Step 1 of the recorded procedure:

```
$ gh api repos/modelcontextprotocol/modelcontextprotocol/contents/schema --jq '.[].name'
2024-11-05
2025-03-26
2025-06-18
2025-11-25
draft
```

**There is still NO `schema/2026-07-28` directory.** Today is 2026-07-26; the final spec
publishes 2026-07-28, two days from now. Steps 2-3 of the procedure (grep the published
`schema/2026-07-28/schema.ts` for the three identifiers and assert their values and payload
shapes) are therefore **not executable**, and step 4 cannot upgrade this verdict.

| Field | Value |
|-------|-------|
| Re-verified by | Plan 113-12, Task 3 |
| Re-verified on | 2026-07-26 |
| Result | **VERDICT UNCHANGED — still `PENDING`** |
| Consequence | The gate on flipping requirements **FAILED**. HTTP-01..05 and CLNT-01..02 were NOT marked complete. They carry `[~]` (implemented, pending final schema) in `.planning/REQUIREMENTS.md`, and Phase 113 is reported as **blocked on publication**, not complete. |

The three landed constants (`-32020` / `-32021` / `-32022`) therefore remain **pre-final
values held under a developer exception**. The re-verification obligation is **NOT discharged**
— it rolls forward. Re-run this checkpoint on or after 2026-07-28; a mismatch against the
published schema is still a **phase-reopening event**, not an advisory.

---

## Recorded Exception

This section is the deliberate spending of the REQUIREMENTS.md "Out of Scope" rule against
hard-coding `-3202x` error codes before the final schema, and of the VERS-06
values-from-final-schema-only rule. It exists so that the decision is traceable to a named
person, a date, and a specific source — not buried in a commit message.

| Field | Value |
|-------|-------|
| **Granted by** | Guy Ernest (user) |
| **Granted via** | `/gsd-execute-phase 113` — plan 113-01 Task 2 `gate="blocking-human"` checkpoint |
| **Decision** | `approved; verdict: exception` (dependency half: `approved` for BOTH `ring` and `zeroize`) |
| **UTC date** | 2026-07-24 (as stated by the developer at the checkpoint; the executing machine's UTC clock read 2026-07-25T03:0xZ at execution — both recorded rather than reconciled, so the audit trail is exact) |
| **Rule being excepted** | REQUIREMENTS.md → Out of Scope → "Hard-coding new `-3202x`/`-32602` error codes before the final schema"; and VERS-06 ("v2 values are filled ONLY from the final 2026-07-28 schema.json") |
| **Verdict at time of grant** | `PENDING` (no `schema/2026-07-28` directory upstream) |

### Values landed under this exception

Landed in `src/types/protocol/error_codes.rs`:

| Rust constant | Value | Source identifier | HTTP status |
|---------------|-------|-------------------|-------------|
| `HEADER_MISMATCH` | `-32020` | `export const HEADER_MISMATCH = -32020;` | 400 |
| `MISSING_REQUIRED_CLIENT_CAPABILITY` | `-32021` | `export const MISSING_REQUIRED_CLIENT_CAPABILITY = -32021;` | 400 |
| `UNSUPPORTED_PROTOCOL_VERSION` | `-32022` | `export const UNSUPPORTED_PROTOCOL_VERSION = -32022;` | 400 |

**Source of these values:** `schema/draft/schema.ts` @ commit
`71e306956a4959c9655e5036be215d41986596e6`, dated 2026-07-16, in the
`modelcontextprotocol/modelcontextprotocol` repository — lines 434 / 442 / 450. This is the
**draft/RC** schema, NOT the published `schema/2026-07-28`. Full token-by-token evidence is in
Section A above (A.3, A.4).

Each constant's Rust doc comment cites this record and its verdict as the provenance of its
numeric value, and five locking tests pin the values, their pairwise distinctness from each
other and from every pre-existing constant, and their containment in the spec-reserved
`-32020..=-32099` sub-range.

### Re-verification obligation (binding)

**Plan 12 Task 3 MUST re-verify these three values against the published `schema/2026-07-28`
before flipping HTTP-01 or HTTP-02 — or any other requirement — to complete.**

A mismatch between any value landed here and the published schema is a **phase-reopening
event, not a warning**. It does not get recorded as an advisory, deferred to a follow-up, or
absorbed as a known-issue: the affected requirement stays incomplete and the phase reopens to
correct the wire constant, because a pre-final value baked into a released SDK is a
wire-visible break for every downstream client (threat T-113-43).

Re-verification procedure for plan 12 Task 3:

1. `gh api repos/modelcontextprotocol/modelcontextprotocol/contents/schema --jq '.[].name'`
   and confirm `2026-07-28` now exists.
2. Grep `schema/2026-07-28/schema.ts` for `HEADER_MISMATCH`,
   `MISSING_REQUIRED_CLIENT_CAPABILITY`, `UNSUPPORTED_PROTOCOL_VERSION`.
3. Assert each identifier still maps to `-32020` / `-32021` / `-32022` respectively, and that
   the HTTP-400 mappings and the `requiredCapabilities`-is-an-object /
   `supported`-is-a-string-array payload shapes are unchanged.
4. Record the outcome by upgrading this file's `## Verdict` to `PUBLISHED-CONFIRMED` or
   `PUBLISHED-DRIFT`. Only then may requirements be flipped.

---

## Conformance Suite Pin (Section B)

### B.1 Pinned commit

| Field | Value |
|-------|-------|
| Repository | `github.com/modelcontextprotocol/conformance` |
| Branch | `main` |
| **Pinned sha** | `a865118206d4d8cc8dbc5f5201607839281d0c3b` |
| Commit date | 2026-07-23T06:04:40Z |
| Commit subject | `fix request metadata HTTP method handling (#409)` |

Command:

```
gh api repos/modelcontextprotocol/conformance/commits/main --jq '.sha'
```

This pin supersedes any commit referenced in `113-RESEARCH.md`. **Plan 11 MUST build its
scenario-to-test manifest from this section, not from the 113-RESEARCH.md table** (see B.4 for
why that matters concretely).

### B.2 Enumerated `sep-2322` check ids (authoritative)

All ids below were extracted from `src/scenarios/server/input-required-result.ts` (1644 lines)
at the pinned sha. **23 unique check ids across 14 scenario classes.**

| # | Check id | Scenario class `name` |
|---|----------|------------------------|
| 1 | `sep-2322-elicitation-incomplete` | `input-required-result-basic-elicitation` |
| 2 | `sep-2322-elicitation-complete` | `input-required-result-basic-elicitation` |
| 3 | `sep-2322-sampling-incomplete` | `input-required-result-basic-sampling` |
| 4 | `sep-2322-sampling-complete` | `input-required-result-basic-sampling` |
| 5 | `sep-2322-list-roots-incomplete` | `input-required-result-basic-list-roots` |
| 6 | `sep-2322-list-roots-complete` | `input-required-result-basic-list-roots` |
| 7 | `sep-2322-request-state-incomplete` | `input-required-result-request-state` |
| 8 | `sep-2322-request-state-complete` | `input-required-result-request-state` |
| 9 | `sep-2322-multiple-inputs-incomplete` | `input-required-result-multiple-input-requests` |
| 10 | `sep-2322-multiple-inputs-complete` | `input-required-result-multiple-input-requests` |
| 11 | `sep-2322-multi-round-r1` | `input-required-result-multi-round` |
| 12 | `sep-2322-multi-round-r2` | `input-required-result-multi-round` |
| 13 | `sep-2322-multi-round-r3` | `input-required-result-multi-round` |
| 14 | `sep-2322-missing-response-rerequests` | `input-required-result-missing-input-response` |
| 15 | `sep-2322-non-tool-incomplete` | `input-required-result-non-tool-request` |
| 16 | `sep-2322-non-tool-complete` | `input-required-result-non-tool-request` |
| 17 | `sep-2322-result-type-included` | `input-required-result-result-type` |
| 18 | `sep-2322-not-on-unsupported-requests` | `input-required-result-unsupported-methods` |
| 19 | `sep-2322-reject-tampered-state` | `input-required-result-tampered-state` |
| 20 | `sep-2322-respect-client-capabilities` | **`input-required-result-capability-check`** |
| 21 | `sep-2322-ignore-unexpected-params` | `input-required-result-ignore-extra-params` |
| 22 | `sep-2322-validate-input-responses` | `input-required-result-validate-input` |
| 23 | `sep-2322-error-on-protocol-error` | `input-required-result-validate-input` |

### B.3 `input-required-result-capability-check`

The task asked for "every scenario id … or equals `input-required-result-capability-check`".
Resolution: that string is a scenario **class name**, not a check id.

```typescript
// src/scenarios/server/input-required-result.ts:1392-1398
// ─── A13: Respect Client Capabilities ────────────────────────────────────────
export class InputRequiredResultCapabilityCheckScenario implements ClientScenario {
  name = 'input-required-result-capability-check';
  readonly source = { introducedIn: DRAFT_PROTOCOL_VERSION } as const;
  specVersions: SpecVersion[] = [DRAFT_PROTOCOL_VERSION];
```

Its emitted check id is `sep-2322-respect-client-capabilities` (row 20 above). Plan 11 must key
on the **check id**, since that is what `negative-mrtr.test.ts` asserts against
(`c.id === 'sep-2322-…'`).

### B.4 Drift vs. 113-RESEARCH.md — why the pin is load-bearing

The research table is **incomplete and partly mis-keyed**. Concretely, four check ids exist at
the pin that the research table does not list at all:

- `sep-2322-respect-client-capabilities`
- `sep-2322-ignore-unexpected-params`
- `sep-2322-validate-input-responses`
- `sep-2322-error-on-protocol-error`

and the research table lists `input-required-result-capability-check` as if it were a check id
when it is a class name. This is direct evidence for the plan's instruction that **plan 11 must
NOT derive its inventory from `113-RESEARCH.md`.** Four additional server obligations follow
from the newly-surfaced ids and are planning inputs for plans 06/09/11:

| Check id | Obligation it grades |
|----------|----------------------|
| `sep-2322-ignore-unexpected-params` | server must tolerate unexpected/extra params rather than erroring |
| `sep-2322-validate-input-responses` | server must validate the `inputResponses` map it receives |
| `sep-2322-error-on-protocol-error` | a genuine protocol error must surface as a JSON-RPC error (not a re-prompt) |
| `sep-2322-respect-client-capabilities` | `inputRequests` only for capabilities declared in `clientCapabilities` |

### B.5 Other server scenario files touching `sep-2322`

`src/scenarios/server/negative-mrtr.test.ts` references three existing ids
(`sep-2322-result-type-included`, `sep-2322-not-on-unsupported-requests`,
`sep-2322-reject-tampered-state`) as a meta-test over the suite; it defines **no new ids**.
No `sep-2322`-prefixed check id exists under `src/scenarios/server/tasks/`. Client-side MRTR
scenarios live in `src/scenarios/client/mrtr-client.ts` and are **out of scope** for this
enumeration (the task scoped it to `src/scenarios/server/`), but plan 13 should be aware they
exist.

---

## Contract-First Environment (Section C)

### C.1 Literal command outputs

```
$ ls -d ../provable-contracts/contracts/pmcp
ls: ../provable-contracts/contracts/pmcp: No such file or directory

$ ls -d ../provable-contracts
ls: ../provable-contracts: No such file or directory

$ command -v pdmt
(no output; exit status 1)

$ command -v pmat
/Users/guy/.cargo/bin/pmat

$ pmat --version
pmat 3.15.0
```

**ENVIRONMENT CONSTRAINT:** the `../provable-contracts` checkout is absent. The exact path
checked is `/Users/guy/Development/mcp/sdk/provable-contracts/contracts/pmcp` (the sibling
directory named by CLAUDE.md's "Contract-First Development" section). Not merely the `pmcp`
contracts subdirectory but the entire `provable-contracts` repository is missing from this
workspace.

The binding in-repo compliance step is therefore the `comply` stage already chained by
`make quality-gate`:

```
Makefile:673  quality-gate:
Makefile:690      @$(MAKE) comply
Makefile:842  comply:
Makefile:845      pmat comply check --path . || echo "note: pmat comply reported project-level
                  advisories (informational; see CLAUDE.md D-07) ..."
Makefile:849      @$(MAKE) --no-print-directory comply-bindings-check
```

`pmat` IS on PATH at the version CLAUDE.md pins for CI (3.15.0), so `pmat comply check --path .`
and `pmat analyze complexity --max-cognitive 25` are both genuinely executable here. Plan 12
Task 2's complexity run is therefore **not skippable**.

### Deviation from CLAUDE.md MANDATORY directives

CLAUDE.md marks both "Use PDMT for all todos" and "ALL code changes via pmat quality-gate
proxy" as **MANDATORY**. **Phase 113 does NOT satisfy them literally.** This subsection records
that as an explicit, documented deviation — not as compliance. A reviewer should be able to see
at a glance that three directives were consciously deviated from with compensating controls,
rather than quietly satisfied.

**Deviation 1 — PDMT todo generation was NOT run.**

- **Directive:** "MANDATORY: Use PDMT (Pragmatic Deterministic MCP Templating) for all todos";
  `pdmt_deterministic_todos --requirement ... --mode strict --coverage-target 80`.
- **Why not:** `command -v pdmt` returns nothing (exit 1). `pdmt` is not installed in this
  workspace. The MCP tool form was likewise unavailable to the executor.
- **Substitute:** the GSD per-task `<acceptance_criteria>` + `<verify><automated>` blocks in
  every `113-*-PLAN.md`. These carry the same structure PDMT emits — a quality gate
  (`<verify>`), measurable success criteria (`<acceptance_criteria>`), a validation command,
  and an explicit `<done>` definition.
- **Residual risk:** LOW-MEDIUM. PDMT's determinism guarantee (identical todo text for
  identical requirement input) is lost, so task decomposition is model-authored rather than
  template-derived, and PDMT's automatic 80%-coverage-target injection is not applied
  uniformly. Coverage remains enforced only where a plan states it.

**Deviation 2 — file writes did NOT go through the PMAT `quality_proxy`.**

- **Directive:** "MANDATORY: Use pmat quality-gate proxy via MCP during development … All code
  changes MUST go through pmat quality-gate proxy before writing" (write/edit/append).
- **Why not:** the proxy requires a long-running `pmat mcp-server --enable-quality-proxy`
  process registered as an MCP server in the session. A plan executor cannot assume, start, or
  verify that process. No `quality_proxy` tool was available.
- **Substitute (three compensating controls, all real):**
  1. `make quality-gate` locally, which chains `comply` → `pmat comply check --path .`
     (Makefile:690/842-849) plus fmt/clippy/build/test/audit.
  2. The PMAT `quality-gate` job in `.github/workflows/ci.yml`, which is **PR-blocking** via the
     aggregate `gate` job and runs `pmat quality-gate --fail-on-violation --checks complexity`
     at the pinned 3.15.0.
  3. Plan 12 Task 2's mandatory `pmat analyze complexity --max-cognitive 25` run. `pmat` IS on
     PATH (C.1), so this control is **not skippable** on the grounds of tool absence.
- **Residual risk:** MEDIUM→LOW. The lost property is *pre-write* rejection (strict mode
  refusing a write that would exceed cog 25 or introduce SATD). Detection moves from write-time
  to gate-time, so a violating edit can exist transiently on disk; it cannot reach `main`,
  because control (2) blocks the PR. Zero-SATD is likewise enforced at gate time, not
  write time.

**Deviation 3 — contract-first ran in-repo only.**

- **Directive:** "Write or update the contract YAML in `../provable-contracts/contracts/<crate>/`
  … Run `pmat comply check`."
- **Why not:** `../provable-contracts` does not exist in this workspace (C.1). The external
  contract YAML for `pmcp` cannot be read or updated from here.
- **Substitute:** `pmat comply check --path .`, i.e. the in-repo compliance surface, run via
  `make quality-gate`'s `comply` stage, plus the repo's own deterministic
  `comply-bindings-check` source-resolution gate (Makefile:819-835).
- **Residual risk:** MEDIUM. Phase 113's wire-level behavior is not being graded against an
  external, versioned contract before implementation; drift between the shipped SDK and the
  canonical `provable-contracts` YAML would go undetected in this phase. Note also that Codex
  blocking finding 8 asks for contract updates to *precede* implementation; with the checkout
  absent, that ordering cannot be honored here and remains deferred to plan 12.

---

## Mcp-Name Header Rule (Section D)

### D.1 The RULE (cross-plan lock)

> **RULE:** `Mcp-Name` MUST be PRESENT on every v2 request; its VALUE is cross-checked against
> the request's logical name only for the name-bearing methods (`tools/call`, `prompts/get` →
> `params.name`; `resources/read` → `params.uri`); for every other v2 method the value is the
> EMPTY STRING and is not cross-checked.

This is what `require_three_headers` / `cross_check_name` in
`src/server/streamable_http_server.rs` already implement, and what Phase-112 D-05 (strict
reject when `Mcp-Method`/`Mcp-Name` are missing) locks. **It is NOT overridable by Phase 113.**

Verified in source at the current HEAD of `fix/mcp-publisher-oidc-audience`:

```rust
// src/server/streamable_http_server.rs:446-456
fn require_three_headers(headers: &HeaderMap)
    -> std::result::Result<(String, String), &'static str> {
    let version_present = headers.get(MCP_PROTOCOL_VERSION).is_some();
    let method = bounded_header_str(headers, MCP_METHOD);
    let name = bounded_header_str(headers, MCP_NAME);
    match (version_present, method, name) {
        (true, Some(m), Some(n)) => Ok((m, n)),
        _ => Err("v2 requests must carry Mcp-Method, Mcp-Name and MCP-Protocol-Version headers"),
    }
}

// src/server/streamable_http_server.rs:492-504
fn cross_check_name(mcp_name: &str, method: &str, body_name: Option<&str>)
    -> std::result::Result<(), &'static str> {
    if !is_name_bearing_method(method) { return Ok(()); }   // presence-only
    match body_name {
        Some(bn) if bn == mcp_name => Ok(()),
        _ => Err("Mcp-Name header does not match the request's logical name (params.name)"),
    }
}
```

`logical_name_key` (line 477) is the single source of truth for the name-bearing set:
`tools/call | prompts/get → "name"`, `resources/read → "uri"`, everything else → not
name-bearing.

### D.2 DRIFT-1 — the spec does NOT require `Mcp-Name` on every request

**Severity: HIGH. Owner: plans 02 / 04 / 05.**

Section A.7 establishes that the draft transport spec requires `Mcp-Name` only for
`tools/call`, `resources/read`, `prompts/get` ("Required For" column), and instructs clients to
append it only "if applicable". pmcp requires it on **every** v2 request.

This is a **stricter-than-spec** rule, and strictness in the reject direction is an interop
break: a fully conformant v2 client sending `tools/list` with only `Mcp-Method` +
`MCP-Protocol-Version` is rejected by pmcp with `-32020` / HTTP 400.

The conformance suite is measurably one such client. From
`src/scenarios/server/http-standard-headers.ts` at the pinned sha, every `tools/list` probe is
sent with `Mcp-Method` alone and **no** `Mcp-Name`:

```typescript
// lines 365-366, 380, 485-486, 500-501, 515-516
{ jsonrpc: '2.0', id: 0, method: 'tools/list' }, { 'Mcp-Method': 'prompts/list' }
{ jsonrpc: '2.0', id: 0, method: 'tools/list' }, { 'mcp-method': 'tools/list' }
{ jsonrpc: '2.0', id: 0, method: 'tools/list' }, { 'MCP-METHOD': 'TOOLS/LIST' }
```

**Recorded consequence:** the conformance-suite header scenarios cannot pass against pmcp's
current strict-presence rule.

#### ADJUDICATED — decision, not an open question

Adjudicated by Guy Ernest at the plan 113-01 Task 2 blocking-human checkpoint, together with
the Recorded Exception above. **Phase-112 D-05 stays LOCKED.** This is binding on downstream
plans; they inherit it and must NOT re-litigate it:

| Plan | Inherited obligation |
|------|----------------------|
| **Plan 04** | **Keeps the always-required rule** exactly as locked by D-05. Do NOT relax `require_three_headers` to make `Mcp-Name` conditional on `is_name_bearing_method`. pmcp continues to require `Mcp-Name` on every v2 request. |
| **Plan 11** | Marks the affected conformance header scenarios **KNOWN-FAILING against this drift record**. They are NOT a plan-11 defect, and plan 11 must NOT "fix" them by silently loosening the rule. The manifest must cite DRIFT-1 as the cause. |
| **Plans 02 / 05** | Treat "pmcp requires `Mcp-Name` always" as a known, deliberate deviation from the draft spec — settled behavior for this phase, but flagged for re-verification against the published 2026-07-28 transport spec. |

Rationale for holding the stricter rule: relaxing it is a security-relevant loosening of a
fail-closed header gate that Phase 112 landed deliberately, and the draft transport spec may
still move before 2026-07-28. Holding strict and recording the failure is reversible;
loosening and discovering the spec kept the requirement is not.

#### Inventory note inherited by plan 11

Independently of DRIFT-1: **plan 11 builds its scenario list from Section B of this file** —
the 23 `sep-2322` check ids across 14 scenario classes enumerated at conformance pin
`a865118206d4d8cc8dbc5f5201607839281d0c3b` — and **NOT** from the `113-RESEARCH.md` table,
which omits four ids (`sep-2322-respect-client-capabilities`, `-ignore-unexpected-params`,
`-validate-input-responses`, `-error-on-protocol-error`) and misreports
`input-required-result-capability-check` — a scenario **class name** — as if it were a check
id. See B.3 and B.4.

### D.3 DRIFT-2 — OWS trimming on `Mcp-Name` (OPEN VERIFICATION ITEM)

**Severity: MEDIUM. Owner: plan 04.**

The conformance suite asserts a MUST that pmcp has no explicit handling for:

```typescript
// src/scenarios/server/http-standard-headers.ts:419-428
'Server MUST accept leading/trailing whitespace in Mcp-Name value
 (RFC 9110 §5.5: field parsing MUST exclude OWS before evaluating)',
...
{ 'Mcp-Name': `  ${toolName}  ` }
```

pmcp's `bounded_header_str` performs **no trimming** — it length-bounds and returns the raw
value:

```rust
// src/server/streamable_http_server.rs:417-423
fn bounded_header_str(headers: &HeaderMap, name: &str) -> Option<String> {
    let raw = headers.get(name)?;
    if raw.as_bytes().len() > MAX_V2_HEADER_VALUE_LEN { return None; }
    raw.to_str().ok().map(str::to_string)
}
```

If `"  search  "` reaches `cross_check_name` un-trimmed it will not equal `params.name ==
"search"` and the request is rejected `-32020`, failing the check. **Whether it does reach
un-trimmed depends entirely on whether hyper strips OWS before pmcp reads the `HeaderMap`,
which was NOT verified here** (verifying it requires standing up a live server — that is plan
04's surface, not this record's). Recorded as an OPEN VERIFICATION ITEM, deliberately not
asserted as a defect.

### D.4 Related v2 status-mapping gap (context for plan 04)

The spec requires `404 Not Found` alongside `-32601` for unknown methods on the v2 path;
Phase 112's verification documents pmcp's current behavior as `-32601@200`. That is a separate
known item (RESEARCH Pitfall 5) already owned by plan 04's status mapper, restated here only so
the `-32020`/`-32021`/`-32022` → 400 mappings recorded in A.4 and the `-32601` → 404 mapping are
read from one place.

---

## Summary for the Task 2 checkpoint

| Record | Result |
|--------|--------|
| **Schema verdict** | **PENDING** — no `schema/2026-07-28`; draft used @ `71e3069` (2026-07-16); 13/13 tokens FOUND, 0 MISSING; values corroborate the research contract exactly |
| **Task 3 error-code half** | **UNBLOCKED by `## Recorded Exception`** — granted by Guy Ernest, 2026-07-24; all three constants landed; plan 12 Task 3 re-verification is binding and a mismatch reopens the phase |
| **Task 3 Cargo.toml half** | Unaffected by the verdict; gated only on package legitimacy — `approved` for both crates |
| **Conformance pin** | `a865118206d4d8cc8dbc5f5201607839281d0c3b` (2026-07-23), 23 `sep-2322` check ids across 14 scenario classes |
| **Contract-first** | `../provable-contracts` ABSENT; `pdmt` ABSENT; `pmat` 3.15.0 PRESENT; three MANDATORY-directive deviations recorded with compensating controls |
| **`Mcp-Name` rule** | Locked as presence-always / value-checked-only-for-name-bearing; **2 DRIFT items** recorded against it (DRIFT-1 HIGH, DRIFT-2 MEDIUM/open) |

---

*Record produced 2026-07-25 by Phase 113 Plan 01 Task 1. Re-run this checkpoint on or after
2026-07-28 to upgrade the verdict.*
