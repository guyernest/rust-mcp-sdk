# Phase 114 — Spec Re-Check & D-18 Hold Record

**Produced by:** Plan `114-01`, Task 2
**Run date (UTC):** 2026-07-28
**Purpose:** This is the record of Phase 114's D-18 hold. It states which requirements are held,
what condition releases them, how the release is verified, what the three landing states are, and
— row by row — every wire value this phase writes that must be re-checked when the gate runs.

It deliberately mirrors the section names of
`.planning/phases/113-stateless-http-multi-round-trip-elicitation/113-SPEC-RECHECK.md`
(`## Recorded Exception`, `## Trigger Condition`, `## Procedure`, `## Third Outcome Policy`,
`## Wire-Value Inventory`, `## Verdict`) so the two records are diff-able against each other.

**No source file was modified by the task that produced this record.**

---

## Recorded Exception

Phase 114 implements against a **draft** schema in a repository whose own GitHub description
begins *"Status: Experimental"*. Every wire value it writes is therefore provisional, and every
one of them is booked under this hold.

| Field | Value |
|-------|-------|
| **Hold recorded by** | Plan `114-01` Task 2, per **D-18** (`114-CONTEXT.md:236-246`) |
| **UTC date** | 2026-07-28 |
| **Policy inherited from** | `113-SPEC-RECHECK.md` § Third Outcome Policy — the `hold` decision made by Guy Ernest (maintainer) on 2026-07-27 at the plan 113-28 Task 2 `gate="blocking"` checkpoint |
| **Policy amended by** | **DQ6** (`114-RESEARCH.md` Q6, resolved during planning 2026-07-27; owned by this plan) — see `## Trigger Condition` |
| **Requirements held** | **TASK-01, TASK-02, TASK-03, TASK-04, TASK-05, TASK-06** |
| **Booking** | `[~]` — *implemented; pending final schema* — in `.planning/REQUIREMENTS.md` |
| **Verdict at time of record** | `PENDING` |
| **Vendored evidence** | `schema/vendored/ext-tasks/` @ `2c1425d9a288b9b1f489430fe1e00bb392b47e48` (2026-07-15), digests in `schema/vendored/ext-tasks/PROVENANCE.md` |

**Booking status when this record was written (2026-07-28):** TASK-01…TASK-06 read **`[ ]`** in
`.planning/REQUIREMENTS.md` (lines 84-89; traceability rows `Pending`), and that file has a
0-byte diff from this plan. The `[~]` booking above is the **policy** this hold prescribes; it is
applied by the phase's closing plan, once there is an implementation to book. The distinction
matters only so a reader does not mistake this record for evidence that the booking already
happened — either way, **no checkbox is flipped to `[x]` except on a `PUBLISHED-CONFIRMED`
landing.**

### All six flip together, never individually

TASK-01…TASK-06 are flipped **as a group** on a `PUBLISHED-CONFIRMED` landing, and not otherwise.
No subset may be closed early on the argument that it is "structural" or "schema-independent".

D-18 recorded that this is a **known tradeoff**, chosen deliberately, and it is repeated here so
a reviewer does not rediscover it as a defect: it repeats the failure mode Phase 113 named when
it split HTTP-04 — a phase whose requirements cannot partially close, where each review reopens
all six. The alternative (splitting TASK-01/03/05/06 as schema-independent and holding only the
wire-exact TASK-02/04) was presented during discussion and **not** chosen; uniform consistency
with 113's bookkeeping was preferred.

### What this hold does NOT permit

Carried forward verbatim in substance from `113-SPEC-RECHECK.md` § What this policy does NOT
permit, because the same over-reads are available here:

- It does **not** promote the vendored draft to an authoritative source.
  `schema/vendored/ext-tasks/` @ `2c1425d9…` is the strongest source available and is **not** the
  final schema.
- It does **not** authorise minting a new wire value. Where this phase needed an error code it
  reused an existing one (see the `-32021` and `-32003` rows below); where it needed a status
  string it copied the vendored schema verbatim.
- It does **not** authorise flipping any requirement at a future run on the strength of elapsed
  time, accumulated confidence, or the fact that the vendored bytes have not drifted. Only a
  `PUBLISHED-CONFIRMED` landing may do that.
- It does **not** weaken the phase-reopening consequence of a `PUBLISHED-DRIFT`.

---

## Trigger Condition

> ### THE TRIGGER IS A CONDITION, NOT A DATE.

**This obligation becomes runnable when a versioned (non-`draft`) schema directory exists in
BOTH of the following repositories:**

1. **`modelcontextprotocol/modelcontextprotocol`** — the core spec repo. It governs
   `resultType`, the `extensions` capability map, and the `-3202x` / `-32602` error-code values
   this phase reuses.
2. **`modelcontextprotocol/ext-tasks`** — the tasks extension repo. It governs every tasks wire
   shape: the `Task` fields, the status strings, the four v2 result shapes, and the
   `tasks/update` parameter names.

**BOTH. Not either.** This is the DQ6 amendment and it is the one substantive way this record
departs from Phase 113's.

### Why both — the DQ6 amendment, stated verbatim

Phase 113's hold condition — *"a versioned schema directory exists"* — was written **before**
tasks moved out of the core specification into a separate repository (SEP-2663; the `ext-tasks`
repo was created 2026-04-29). Read literally against 113's wording, a core-only publication
event would satisfy the condition and release the hold. That would be wrong here: a core release
says nothing whatsoever about the tasks wire shapes, which is where five of this phase's six
requirements live.

**Six `[~]` requirements must not flip on a core-only publication event.** Nor on an
extension-only one: TASK-04's `resultType:"task"` discriminator and the `-32602`/`-32021`/
`-32003` codes are graded by the **core** schema, so an ext-tasks-only release leaves those
unverified.

Corroborating assumption, recorded so a re-runner can falsify it rather than inherit it:
`114-RESEARCH.md` **A2** assumes the core repo's `cut-release.yml` `kind=final`
`workflow_dispatch` (per 113-28's finding) governs the **core** schema only, and that the
extension versions independently. Risk assessed **MEDIUM**. If that assumption turns out to be
wrong — if a core release also stamps the extension — the condition above still holds correctly;
it simply becomes satisfiable by one event instead of two.

### The condition is not a date, restated with its two consequences

Inherited from `113-SPEC-RECHECK.md` § TRIGGER, and load-bearing in both directions:

- **The gate is not DISCHARGED merely because a day passed.** `## Verdict` stays `PENDING` while
  either directory is absent, no matter what the calendar says.
- **The gate is not DUE merely because a day passed either.** A re-run finding either directory
  still absent lands in `STILL-ABSENT` (`## Procedure` step 4) and rolls forward. That is a
  recorded outcome, not a failure and not a deferral.

### Measured state at the time of this record

Both listings were measured by this plan on the run date — **2026-07-28, the date the final spec
was due** — not copied from an earlier phase.

```
$ gh api repos/modelcontextprotocol/modelcontextprotocol/contents/schema --jq '.[].name'
2024-11-05
2025-03-26
2025-06-18
2025-11-25
draft
```

```
$ gh api repos/modelcontextprotocol/ext-tasks/contents/schema --jq '.[].name'
draft
```

| Repository | Versioned directories present | `2026-07-28` present? | Condition met? |
|------------|-------------------------------|-----------------------|----------------|
| `modelcontextprotocol/modelcontextprotocol` | `2024-11-05`, `2025-03-26`, `2025-06-18`, `2025-11-25` (plus `draft`) | **NO** | no |
| `modelcontextprotocol/ext-tasks` | *(none — only `draft`)* | **NO** | no |

Both `gh` invocations exited 0 (authenticated, `gh` 2.64.0); neither is recorded as UNAVAILABLE.
**The condition is unmet in both repositories, so the hold remains correctly engaged.**

Note the asymmetry worth carrying forward: `ext-tasks` has **never** published a versioned
directory — it holds `draft` and nothing else, so there is no precedent there for what a release
looks like.

---

## Procedure

The re-verification run. Steps 1–3 gather evidence; step 4 lands exactly one of three outcomes.

### Step 1 — Re-resolve BOTH repositories' schema directory listings

```bash
gh api repos/modelcontextprotocol/modelcontextprotocol/contents/schema --jq '.[].name'
gh api repos/modelcontextprotocol/ext-tasks/contents/schema     --jq '.[].name'
```

Record both listings literally, as § Trigger Condition does. If **either** lacks a versioned
(non-`draft`) directory, step 4 lands in `STILL-ABSENT` and steps 2–3 are not executable.

### Step 2 — Diff the published tasks schema against the vendored copy

The vendored files are the baseline precisely so this diff is text-against-text rather than
memory-against-prose.

```bash
VER=<the published version directory, e.g. 2026-07-28>
BASE=https://raw.githubusercontent.com/modelcontextprotocol/ext-tasks/main/schema/$VER
curl -sSf -o /tmp/pub-schema.ts   "$BASE/schema.ts"
curl -sSf -o /tmp/pub-schema.json "$BASE/schema.json"

diff /tmp/pub-schema.ts   schema/vendored/ext-tasks/schema.ts
diff /tmp/pub-schema.json schema/vendored/ext-tasks/schema.json
```

Also confirm the vendored copy is still the bytes it claims to be, before trusting the diff:

```bash
cargo nextest run --features full -E 'test(/vendored_schema/)'
shasum -a 256 schema/vendored/ext-tasks/schema.ts schema/vendored/ext-tasks/schema.json
# must equal the digests in schema/vendored/ext-tasks/PROVENANCE.md
```

An **empty** diff means the published schema is byte-identical to the pin and every row of
`## Wire-Value Inventory` is confirmed at once. A non-empty diff must still be walked row by row
— a change touching only comments confirms every value, and a change touching one field
invalidates only that field's rows.

### Step 3 — Walk `## Wire-Value Inventory` row by row

Every row, including the ones a reviewer expects to be trivially fine. For each row, assert the
recorded value against the published schema (or, for the two `⚠` rows, against the published
prose), and record CONFIRMED / DRIFT per row. **A row that was not checked is not confirmed.**

Also re-check the two carried forward risks:

- **The `-32003` vs `-32021` upstream disagreement** — its own row below. This is the row most
  likely to move, because the disagreement is between two upstream documents rather than between
  upstream and pmcp.
- **Core PR #2678** (`SEP-2678: Introduce additional error codes to protocol`) — carried from
  `113-SPEC-RECHECK.md` § Two measured facts. It proposes `-32000`/`-32001`/`-32002` in the
  adjacent implementation-defined range and would contradict the draft's *"codes … remain
  reserved and are never reused"* text for `-32002`. Phase 114 relies on that rule via
  `V1_TASK_PENDING` staying v1-only (TASK-03). Re-check #2678's state at every run.

### Step 4 — Land exactly one of THREE outcomes

**THREE landing states are defined and this step cannot end in a fourth.**

| Step-1 / step-2–3 result | Landing state | Action |
|---|---|---|
| Versioned directories exist in **BOTH** repos, and steps 2–3 agree on every inventory row | `PUBLISHED-CONFIRMED` | Upgrade `## Verdict`. **Only then** may TASK-01…TASK-06 be flipped to `[x]` — all six together. |
| Versioned directories exist in **BOTH** repos, and any inventory row disagrees | `PUBLISHED-DRIFT` | Upgrade `## Verdict`. The mismatch is a **phase-reopening event** (see below). **No requirement is flipped.** |
| A versioned directory is **still absent from either repository** | **`STILL-ABSENT`** | Apply `## Third Outcome Policy`. `## Verdict` stays `PENDING`, the obligation is **not discharged** and rolls forward, and no requirement is flipped. |

**A mismatch between any value landed by this phase and the published schema is a
phase-reopening event, not an advisory.** It does not get recorded as a known issue, deferred to
a follow-up, or absorbed. The affected requirement stays incomplete and the phase reopens to
correct the wire value — because a pre-final value baked into a released SDK is a wire-visible
break for every downstream client.

Record the run as a dated sub-section under `### Verdict re-verification` in this file, whatever
it lands, so that *"we checked"* stays distinguishable from *"nobody checked"*.

---

## Third Outcome Policy

This section answers step 4's third branch — what the re-verification does when a versioned
schema directory still does not exist in one or both repositories. It inherits, branch for
branch, the policy `113-SPEC-RECHECK.md` § Third Outcome Policy records under the maintainer's
`hold` decision of 2026-07-27.

| Field | Value |
|-------|-------|
| **Policy inherited from** | `113-SPEC-RECHECK.md` § Third Outcome Policy |
| **Originally decided by** | Guy Ernest (maintainer) |
| **Originally decided via** | `/gsd:execute-phase 113` — plan 113-28 Task 2, `type="checkpoint:decision" gate="blocking"` |
| **Original decision** | **`hold`** — hold the `[~]` requirements indefinitely |
| **Original UTC date** | 2026-07-27 |
| **Conditions stated by the decider** | **none stated** |
| **Review date stated** | **none stated** |
| **Scope narrowing stated** | **none stated** |
| **Applied to Phase 114 by** | plan `114-01` Task 2, 2026-07-28, per D-18's explicit inheritance |
| **Amended for Phase 114 by** | DQ6 only — the *trigger* now names both repositories. The three branches themselves are unchanged. |

The three "none stated" rows are reproduced deliberately. The decider stated no conditions; none
were inferred in Phase 113 and none may be read into this record either.

### The three outcomes

1. **`PUBLISHED-CONFIRMED`** — versioned directories exist in both repositories and every
   inventory row agrees. `## Verdict` is upgraded. TASK-01…TASK-06 flip to `[x]`, together.
2. **`PUBLISHED-DRIFT`** — versioned directories exist in both repositories and at least one
   inventory row disagrees. `## Verdict` is upgraded. **Phase-reopening event.** No requirement
   is flipped; the phase reopens to correct the wire value.
3. **`STILL-ABSENT`** — a versioned directory is absent from at least one repository. This is
   **explicitly legitimate and non-failing.** See the rule below.

### The rule on a `STILL-ABSENT` landing

1. `## Verdict` stays **`PENDING`**. It is not upgraded, not annotated as "effectively
   confirmed", and not given a new state.
2. TASK-01…TASK-06 **stay `[~]`**. No checkbox is flipped.
3. The re-verification obligation is **NOT discharged**. It rolls forward and is re-run whenever
   the trigger condition is next worth checking.
4. The run **is still RECORDED** — a `STILL-ABSENT` result gets a dated sub-section under
   `### Verdict re-verification` exactly as a published landing would, so that *"we checked and
   it was absent"* stays distinguishable from *"nobody checked"*.
5. **Partial publication is still `STILL-ABSENT`.** If exactly one of the two repositories
   publishes, record which one, record the listing, and land here. Do not run steps 2–3 against
   a half-published pair and call it confirmed.

`STILL-ABSENT` exists so that a re-run cannot end in an undefined state, and so that the six
`[~]` requirements stay `[~]` **by recorded decision rather than by default**. That distinction
is the entire reason this branch is written down.

It weakens nothing. A `PUBLISHED-DRIFT` remains a phase-reopening event exactly as before, and
`STILL-ABSENT` is not a licence to treat the vendored draft as published — see
`## Recorded Exception` § What this hold does NOT permit.

---

## Wire-Value Inventory

Every wire value Phase 114 writes, one per row, with the file it lives in once implemented and
the owning plan. **This is the checklist step 3 walks.** A value absent from this table is a
value nobody agreed to re-verify.

Unless a row says otherwise, the source is the vendored
`schema/vendored/ext-tasks/schema.ts` / `schema.json` @ `2c1425d9a288b9b1f489430fe1e00bb392b47e48`
(digests in `schema/vendored/ext-tasks/PROVENANCE.md`).

### Negotiation

| # | Value | Recorded as | Lives in (once implemented) | Owning plan | Source |
|---|-------|-------------|------------------------------|-------------|--------|
| 1 | Extension key | `io.modelcontextprotocol/tasks` | `src/types/capabilities.rs` | 114-03, 114-05 | `schema.ts:3` (`Extension Identifier`), `:374` `TasksExtensionCapability` |
| 2 | Capability value | `{}` — an empty object means support; **no** extension-specific settings are defined | `src/types/capabilities.rs`, `src/server/core.rs` | 114-03, 114-05 | `schema.ts:368-374` — `export type TasksExtensionCapability = Record<string, never>;` |
| 3 | Client declaration site | per-request `_meta.clientCapabilities.extensions`, **not** a handshake | `src/client/mod.rs`, `src/types/capabilities.rs` | 114-03, 114-06 | v2 stateless model; core schema `ClientCapabilities` |

### `Task` object fields

| # | Value | Recorded as | Lives in (once implemented) | Owning plan | Source |
|---|-------|-------------|------------------------------|-------------|--------|
| 4 | `taskId` | **required** `string` | `src/types/tasks.rs`, `src/server/task_dispatch.rs` | 114-11 | `schema.json` `$defs.Task.required[0]`; `schema.ts:50` |
| 5 | `status` | **required**, `TaskStatus` | ditto | 114-11 | `schema.json` `$defs.Task.required[1]`; `schema.ts:55` |
| 6 | `createdAt` | **required**, ISO 8601 `string` | ditto | 114-11 | `schema.json` `$defs.Task.required[2]`; `schema.ts:71` |
| 7 | `lastUpdatedAt` | **required**, ISO 8601 `string` | ditto | 114-11 | `schema.json` `$defs.Task.required[3]`; `schema.ts:76` |
| 8 | `ttlMs` | **required** and **nullable** — `number \| null`, integer milliseconds, `null` = unlimited. Renamed from v1 `ttl`. | ditto | 114-11 | `schema.json` `$defs.Task.required[4]`; `schema.ts:79-84` |
| 9 | `pollIntervalMs` | **optional**, integer milliseconds. Renamed from v1 `pollInterval`. | ditto | 114-11 | `schema.ts:86-91`; absent from every `required` array |
| 10 | `statusMessage` | **optional** `string` | ditto | 114-11 | `schema.ts:57-66` |

Row 8 is the one to read twice: `ttlMs` is **required** (it appears in all five per-variant
`required` arrays) *and* nullable. "Optional because it can be null" is the wrong reading and
would produce a schema-invalid response.

### Status strings

| # | Value | Recorded as | Lives in (once implemented) | Owning plan | Source |
|---|-------|-------------|------------------------------|-------------|--------|
| 11 | `working` | exact lowercase string | `src/types/tasks.rs` | 114-11 | `schema.ts:35` |
| 12 | `input_required` | exact, **snake_case** | ditto | 114-11 | `schema.ts:36` |
| 13 | `completed` | exact lowercase string | ditto | 114-11 | `schema.ts:37` |
| 14 | `failed` | exact lowercase string | ditto | 114-11 | `schema.ts:38` |
| 15 | `cancelled` | exact, **double-l** British spelling | ditto | 114-11 | `schema.ts:39` |

Rows 12 and 15 are the two a re-verifier must read character by character: `input_required` (not
`inputRequired`) and `cancelled` (not `canceled`).

### The four v2 result shapes

| # | Value | Recorded as | Lives in (once implemented) | Owning plan | Source |
|---|-------|-------------|------------------------------|-------------|--------|
| 16 | `CreateTaskResult` | **FLAT** — `Result & Task`, with `resultType: "task"` | `src/server/task_dispatch.rs`, `src/server/core.rs` | 114-11, 114-12 | `schema.ts:226-233` |
| 17 | `resultType` discriminator value | the exact string `"task"` | `src/server/core.rs` (`inject_v2_result_envelope`) | 114-11, 114-12 | `schema.ts:228-229` — *"The resultType field MUST be set to `\"task\"`"* |
| 18 | `GetTaskResult` | **FLAT** — `Result & DetailedTask`, `resultType: "complete"` | `src/server/task_dispatch.rs` | 114-11 | `schema.ts:252-259` |
| 19 | `UpdateTaskResult` | **empty acknowledgement** — `Result` only, `resultType: "complete"` | `src/server/task_dispatch.rs` | 114-11, 114-14 | `schema.ts:282-288` |
| 20 | `CancelTaskResult` | **empty acknowledgement** — `Result` only, `resultType: "complete"`; cancellation is cooperative and eventually consistent | `src/server/task_dispatch.rs` | 114-11 | `schema.ts:305-312` |

Rows 16/18 are the v1→v2 reshape: v1 nested the task under a `task` key; v2 **flattens** it into
the result. Rows 19/20 are the ones most easily got wrong by analogy — they carry no task body
at all.

### Per-variant required fields (the `DetailedTask` union)

| # | Value | Recorded as | Lives in (once implemented) | Owning plan | Source |
|---|-------|-------------|------------------------------|-------------|--------|
| 21 | `result` | **required** on `CompletedTask` only | `src/types/tasks.rs`, `src/server/task_dispatch.rs` | 114-11 | `schema.json` `$defs.CompletedTask.required` = `[taskId,status,createdAt,lastUpdatedAt,ttlMs,result]` |
| 22 | `error` | **required** on `FailedTask` only | ditto | 114-11 | `schema.json` `$defs.FailedTask.required` = `[…,error]` |
| 23 | `inputRequests` | **required** on `InputRequiredTask` only, **top-level** on the `tasks/get` result | `src/server/core.rs` (reserved-field registry), `src/server/task_dispatch.rs` | 114-10, 114-11 | `schema.json` `$defs.InputRequiredTask.required` = `[…,inputRequests]`; `schema.ts:157-165` |
| 24 | `WorkingTask` / `CancelledTask` | carry **no** extra required field beyond the five `Task` fields | `src/types/tasks.rs` | 114-11 | `schema.json` `$defs.WorkingTask.required`, `$defs.CancelledTask.required` |

Row 23 is the highest-severity row in this table. Phase 113's `own_reserved_result_fields`
(`src/server/core.rs`) **silently deletes** a top-level `inputRequests` key from any v2 result
whose disposition is not `InputRequired`. Left unfixed, a v2 `tasks/get` on an `input_required`
task would emit a schema-invalid response, and an integration test asserting only "the request
succeeded" would pass. Owned by 114-10 (DQ2).

### `tasks/update`

| # | Value | Recorded as | Lives in (once implemented) | Owning plan | Source |
|---|-------|-------------|------------------------------|-------------|--------|
| 25 | Method name | `tasks/update` | `src/types/protocol/mod.rs`, `src/shared/protocol_helpers.rs`, `src/server/task_dispatch.rs` | 114-13 | `schema.ts:266-267` |
| 26 | **Param name** | **`inputResponses`** — *not* `inputs` | `src/server/task_dispatch.rs`, `src/types/mrtr.rs` | 114-13, 114-14 | `schema.ts:274-278`; `schema.json` `$defs.UpdateTaskRequest` |
| 27 | `params.taskId` | **required** `string` | ditto | 114-13, 114-14 | `schema.ts:268-272` |
| 28 | `inputResponses` key rule | each key **MUST** correspond to a currently-outstanding `inputRequest` key | `src/server/task_dispatch.rs` | 114-14 | `schema.ts:140-149`, `:274-278` |

Row 26 is a **correction to the v2.5 research pack**, which said the parameter was `inputs`.
`114-RESEARCH.md` measured `inputResponses` against the schema. If a re-verifier finds `inputs`
in the published schema, that is DRIFT against this record and reopens the phase — it is not a
reason to "restore" the older name.

### Error codes

| # | Value | Recorded as | Lives in (once implemented) | Owning plan | Source |
|---|-------|-------------|------------------------------|-------------|--------|
| 29 | `-32602` | task-not-found / invalid `taskId` on **v2**. **MUST** for `tasks/get`, SHOULD elsewhere. pmcp emits `-32603` today; v2 maps `TaskStoreError::NotFound` → `-32602`, every other error stays `-32603`. | `src/server/task_dispatch.rs` | 114-11 | ext-tasks `specification/draft/tasks.md` § Error Handling |
| 30 | `-32021` | **non-declaring-client** refusal — the client did not declare `io.modelcontextprotocol/tasks` on a `tasks/*` request. `error.data.requiredCapabilities` is an **OBJECT** (`{"extensions":{"io.modelcontextprotocol/tasks":{}}}`), never an array. | `src/server/task_dispatch.rs`, `src/server/core.rs` | 114-09 | core `schema/draft/schema.ts` `MISSING_REQUIRED_CLIENT_CAPABILITY = -32021`; already in `src/types/protocol/error_codes.rs:213` |
| 31 | `-32003` | **auth** refusal — unauthenticated caller on a server that has an auth provider (D-08, the 113-23 shape, at HTTP 200) | `src/server/task_dispatch.rs`, `src/server/core.rs` | 114-09 | pmcp `AUTHENTICATION_REQUIRED`; 113-23's `subscriptions/listen` precedent |
| 32 | `-32601` | wrong-era method — `tasks/list` and `tasks/result` on v2 | `src/server/task_dispatch.rs` | 114-08 | 112 D-10; `tasks/list` and `tasks/result` are absent from the v2 extension schema |
| 33 | `-32002` (`V1_TASK_PENDING`) | **FROZEN, v1-only.** Not re-litigated by this phase. | `src/server/task_dispatch.rs` (era-gated by 113-29) | 114-08 | ROADMAP `-32002` RESOLVED note; `error_codes.rs` |

Row 29 carries an **anti-oracle constraint** that survives into the re-check: task-not-found,
owner-mismatch and pending must remain **indistinguishable**. The `-32602` message must not vary
between them. Making the code more specific must not make the message an existence oracle.

### ⚠ Known upstream disagreement — `-32003` vs `-32021`

**This is its own row because two upstream documents contradict each other, and the direction
must be re-checked at the gate.**

| Field | Value |
|-------|-------|
| **The disagreement** | `ext-tasks` **prose** (`specification/draft/tasks.md`) uses **`-32003`** for missing-required-client-capability, and makes it a **MUST** for a non-declaring client issuing `tasks/get` / `tasks/update` / `tasks/cancel`. The **core draft schema** declares `MISSING_REQUIRED_CLIENT_CAPABILITY = -32021`. |
| **Why it likely exists** | The three `-3202x` codes were **renumbered after a locked release candidate** (113-SPEC-RECHECK § Finding 8). The ext-tasks prose appears **stale** — written before the renumbering. |
| **How Phase 114 resolved it** | **DQ3** (`114-RESEARCH.md` Q3, resolved during planning; owned by `114-09`). **Both codes, two meanings:** `-32003` keeps the **auth** refusal (D-08 verbatim, the 113-23 shape); the existing `-32021 MISSING_REQUIRED_CLIENT_CAPABILITY` carries the **non-declaring-client** refusal, with an OBJECT-shaped `requiredCapabilities`. |
| **Why not just follow the prose** | On a tasks method a bare `-32003` is ambiguous between *"you did not declare the extension"* and *"you are not authenticated"* — the exact undiscoverability D-08 chose `-32003` to avoid. Splitting them keeps both refusals diagnosable. |
| **What was NOT done** | **No new wire value was minted.** Both codes already exist in `src/types/protocol/error_codes.rs`. The schema hold is respected. |
| **THE OBLIGATION** | **Re-check the DIRECTION at the gate.** If the published ext-tasks prose still says `-32003` where the published core schema says `-32021`, record the disagreement as persisting and keep DQ3's split. If the published prose has been updated to `-32021`, record that pmcp already agrees. If either published document says something else entirely, that is **DRIFT** and a phase-reopening event. |

### Transport / routing

| # | Value | Recorded as | Lives in (once implemented) | Owning plan | Source |
|---|-------|-------------|------------------------------|-------------|--------|
| 34 | `Mcp-Name` header on `tasks/*` | Client **MUST** set `Mcp-Name` to `params.taskId` for `tasks/get`, `tasks/update`, `tasks/cancel` over Streamable HTTP, so intermediaries route to the instance holding the task state | `src/client/mod.rs`, `src/types/mrtr.rs`, `src/shared/streamable_http.rs` | 114-06 | ext-tasks `specification/draft/tasks.md` § Streamable HTTP: Routing Headers |
| 35 | `Mcp-Name` server-side enforcement | **deliberately OFF** this phase. `cross_check_name` returns `Ok(())` for a non-name-bearing method, so a conformant client's header is accepted, not rejected. | `src/server/streamable_http_server.rs` (unchanged) | 114-06 (DQ4) | measured against the current tree |
| 36 | `notifications/tasks` | Servers **MAY** push; pmcp declines this phase. Clients subscribe via `subscriptions/listen` with `taskIds`. | *(not implemented)* | — | `schema.ts:314-364`; `114-RESEARCH.md` A7 |

Row 34's implementation route is DQ4's finding and must not be re-derived by a re-verifier:
`logical_name_key` and `mrtr_eligible` **both** derive from `MRTR_METHODS`, so adding a row there
would make `tasks/update` MRTR-eligible and `splice_mrtr_params` would delete its entire payload.
A **separate** name-key table is used instead.

Row 36 rests on `114-RESEARCH.md` **A7** (MEDIUM risk): the `MAY` is explicit in the spec text,
but a conformance suite sometimes grades optional features when advertised. pmcp does not
advertise `taskIds` in an acknowledgement, so exposure should be nil. Re-check that the published
extension has not upgraded the `MAY`.

### Removed-on-v2 surface

| # | Value | Recorded as | Lives in (once implemented) | Owning plan | Source |
|---|-------|-------------|------------------------------|-------------|--------|
| 37 | `tasks/list` | **REMOVED on v2.** Its removal is named by the spec as a *security* improvement — without it a server cannot inadvertently leak the existence of one caller's tasks to another. | `src/server/task_dispatch.rs` (era gate → `-32601`) | 114-08 | SEP-2663; the method is absent from the vendored schema |
| 38 | `tasks/result` | **REMOVED on v2.** `tasks/get` inlines `result` / `error` on the terminal `DetailedTask` variant — one round trip. | `src/server/task_dispatch.rs` (era gate → `-32601`) | 114-08 | SEP-2663; absent from the vendored schema |
| 39 | Client `task` field on `tools/call` | **DOES NOT EXIST on v2.** Creation is **server-directed**; the server is *"the sole decider"*. The v2 create trigger is the client's per-request extension declaration. v1 keeps its `task` field. | `src/server/task_dispatch.rs`, `src/server/core.rs` | 114-12 (DQ1) | SEP-2663; no `task` request field anywhere in the vendored schema |

Row 39 is DQ1, **explicitly approved by the user pre-execution on 2026-07-27**, superseding
CONTEXT.md's deferral for the create-trigger question. Read literally, that deferral would have
made v2 task creation unreachable and TASK-04 undemonstrable end to end.

Rows 37/38 are the reason TASK-03 and TASK-05 are *"the same improvement viewed from two
angles"*: removing enumeration and binding the owner are one security posture.

### ⚠ Carried obligation — the Phase-114 contract-first waiver

**This is its own row, and it is deliberately NOT numbered, because it is not a wire value.** It
is an obligation created by an **owner decision** and carried to *this* gate because the same
condition releases it. Row numbering stops at 39; nothing below is an inventory value. A
re-runner walking `## Procedure` step 3 must still read this row — a carried obligation that was
not checked is not discharged, exactly as a wire value that was not checked is not confirmed.

| Field | Value |
|-------|-------|
| **What was waived** | `CLAUDE.md` § *Contract-First Development* step 1 — authoring a contract YAML covering Phase 114's surface (TASK-01…TASK-06) **before** implementation — is waived for Phase 114. |
| **Decided by** | **Guy Ernest (owner)**, 2026-07-28, at the `114-20` Task 2 `type="checkpoint:decision" gate="blocking"` checkpoint. Recorded in `114-CONTRACT-DECISION.md` § 4 as `Chosen: option-b`. **Owner-decided, not executor-authored** — this is the one substantive way it departs from the Phase 113 precedent it otherwise continues. |
| **Ground for the waiver** | **D-18 provisional values, and nothing else.** A contract authored now would pin the 39 values inventoried above — values this gate is expected to move — and would need re-authoring at the same gate. |
| **NOT a ground for the waiver** | *"There is nowhere to write it."* `114-CONTRACT-DECISION.md` §1.5 **measured that premise and found it false**: `contracts/` is in-repo, git-tracked (38 files, 3 YAMLs) and already graded by `pmat comply check --path .` (CB-1200/1202/1205/1305). The absent `../provable-contracts/` holds the `pv` CLI and `proof-status.json`, not the authoring destination. This row records the falsification so the waiver cannot later be cited on a premise that was already withdrawn. |
| **THE CONDITION** | **WHEN** a versioned (non-`draft`) schema directory exists in **BOTH** `modelcontextprotocol/modelcontextprotocol` **AND** `modelcontextprotocol/ext-tasks` — the same both-repositories condition `## Trigger Condition` states under DQ6 — **THEN** the contract-first question **re-enters**. It is not scheduled, not dated, and not "revisited later". |
| **What re-entry requires** | Exactly one of two outcomes, recorded: **(a)** author the Phase-114 equations in `contracts/mcp-protocol-sdk-v1.yaml` (or a sibling YAML) with their `contracts/binding.yaml` rows, against the **published** values, and run `pmat comply check --path .`; **or (b)** record a **further explicit owner waiver**, with its own `Chosen:` / `Decided by:` / `Date:`. A plan may not choose (b) on its own authority. |
| **Third outcome — `STILL-ABSENT`** | If a versioned directory is absent from **either** repository, this obligation lands in **`STILL-ABSENT`** exactly as `## Procedure` step 4 and `## Third Outcome Policy` define it: **not discharged**, rolls forward, and **still recorded** in the dated `### Verdict re-verification` sub-section, so *"we checked and it was absent"* stays distinguishable from *"nobody checked"*. **Partial publication is `STILL-ABSENT`**, not a fourth state — do not author a contract against a half-published pair and call the obligation met. |
| **Change detector** | `114-01`'s SHA-256 vendored-schema provenance tripwire over `schema/vendored/ext-tasks/` @ `2c1425d9a288b9b1f489430fe1e00bb392b47e48` (`cargo nextest run --features full -E 'test(/vendored_schema/)'`) — the same detector `## Procedure` step 2 uses. |
| **Residual cost accepted at the time of the waiver** | `contracts/mcp-protocol-sdk-v1.yaml` stays stale — 116 days, **zero** `task` hits, **zero** `extension` hits, metadata describing *"SDK v2.1"* while the crate is at 2.17 — and **CB-1409 already flags this phase's own `114-01` commits** as lacking work contracts. Both were accepted by the owner, not resolved. A re-runner should expect to find them unchanged and should not read that as drift. |
| **Binds** | `114-18` **cites** this waiver rather than declining the contract on its own authority (**T-114-106**). The condition wording above is what keeps the waiver from quietly becoming permanent (**T-114-107**) — the failure mode §2 of `114-CONTRACT-DECISION.md` observed in Phase 113 in the wild. |

**Relationship to the six held requirements:** this obligation is carried *alongside*
TASK-01…TASK-06, not *among* them. A `PUBLISHED-CONFIRMED` landing flips those six together; it
does **not** by itself discharge this row. This row is discharged only by outcome (a) or (b)
above, recorded.

---

## Verdict

**PENDING**

No versioned (non-`draft`) schema directory exists in **either**
`modelcontextprotocol/modelcontextprotocol` **or** `modelcontextprotocol/ext-tasks` as of
2026-07-28 — the date the final specification was due. Both listings were measured on that date
and are recorded verbatim in `## Trigger Condition`.

Every wire value in `## Wire-Value Inventory` was read from
`schema/vendored/ext-tasks/` @ `2c1425d9a288b9b1f489430fe1e00bb392b47e48` — a `draft/`
directory in a repository whose own description begins *"Status: Experimental"*. That is the
strongest source available and it is **not** the final schema.

**Consequence:** TASK-01, TASK-02, TASK-03, TASK-04, TASK-05 and TASK-06 are booked `[~]`
— *implemented; pending final schema* — in `.planning/REQUIREMENTS.md`. They are flipped
together, never individually, and only on a `PUBLISHED-CONFIRMED` landing of `## Procedure`
step 4. The obligation is **not discharged**; it rolls forward.

### Verdict re-verification

*(No re-verification run has been executed yet. Each future run — including a `STILL-ABSENT`
one — appends a dated sub-section here.)*
