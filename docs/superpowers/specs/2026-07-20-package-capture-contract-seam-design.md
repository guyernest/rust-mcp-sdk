# Package-Capture Contract Seam — Design

**Date:** 2026-07-20
**Status:** Approved (green-lit with three gating corrections, incorporated)
**Scope:** How `cargo pmcp package capture` binds to pmcp.run's remote capture API,
so the CLI and the platform cannot silently drift.

---

## 1. Problem

`cargo pmcp package capture` is a **pure thin client** of pmcp.run's remote,
already-deployed capture job API. The platform does all substantive work
server-side (reads the component graph from its own DynamoDB models, extracts
config slots, packs the canonical `pmcp-package` OCI layout, pushes it to ECR).
The CLI only **submits a team reference and polls** for a terminal status.

The verb already exists and is hardened (branch `feat/package-remote-capture-show`,
`cargo-pmcp/src/commands/package/capture.rs` + `.../pmcp_run/graphql.rs`). But its
GraphQL binding is **hand-written raw query strings + serde structs over reqwest**,
with **no shared schema and no drift gate**. That is precisely the setup that let
the CLI and platform diverge in Phase 110 (a `capture` verb POSTing to an endpoint
that 404'd everywhere). Nothing structurally prevents it from happening again.

**Goal:** give the two capture operations a **versioned, platform-owned GraphQL
contract**, checked into the SDK repo and **enforced by a test**, so a platform
schema change the CLI hasn't tracked fails the SDK build.

## 2. The two operations (ground truth, verified against the live schema)

Asymmetry between the two ops is real and the contract must capture it **exactly**:

```graphql
# submit — returns captureId
submitPackageCapture(
  rootComponentType: String!   # v1: "team" only
  rootComponentId:   String!   # AgentTeam id (a slug like day-trip-planner-team, NOT a UUID)
  version:           String!   # semver, e.g. 1.0.0
  bump:              String    # major|minor|patch; consulted only on errorCode=BUMP_REQUIRED
): CaptureInfo                 # { captureId, status }

# status — keyed on id, returns id
getPackageCaptureStatus(id: ID!): CaptureStatus
# CaptureStatus { id, status, message, errorCode, divergentComponents[], manifestDigest, updatedAt }
```

- `submitPackageCapture` returns **`captureId`**; `getPackageCaptureStatus` is
  argued and keyed on **`id`** and its return identity field is **`id`**. The CLI
  already threads `CaptureInfo.captureId` → the `id` variable correctly.
- **`status` is a plain `String`**, not a GraphQL enum. Known values observed:
  `queued`, `walking`, `extracting`, `publishing`, `completed`, `failed`,
  `not_found`. The poller treats any *unrecognized* value as in-progress (warns
  once), so additive platform statuses never break an older CLI.

## 3. The contract artifact

A single file: **`contracts/pmcp-run/capture-v1.graphql`** — the **SDL subset** for
exactly the two operations above and their input/output types.

- **Generated, never hand-written.** It is produced by introspecting the live
  source data API and extracting the capture subset (the same mechanism as the
  online gate, §5b). Rationale: careful human transcription already drifted once
  in this very design (a hand-authored field list dropped `id`/`updatedAt` and
  mis-typed `status` as an enum). The artifact must reflect reality, not memory.
- **`status` is typed `String`** in the SDL to match the schema, with the known
  values recorded as an SDL doc-comment — NOT as a GraphQL `enum`. Typing it as an
  enum would make the online schema-vs-schema diff report permanent false drift
  (`enum` in the contract vs `String` in the live schema).
- **Provenance header** (as SDL comments): source endpoint, introspection date,
  contract version. So a reader knows what it was cut from and when.
- **GraphQL SDL, not `contracts/*.yaml`.** The transport is GraphQL, so SDL lets the
  online gate be a direct, mechanical schema-vs-schema diff of introspected output
  against the vendored file. A YAML re-description would be a lossy translation that
  becomes its own drift source (the CLI would be tested against a remembered copy).
  The `*.yaml` convention remains correct for non-GraphQL transports.
- **Ownership:** the platform owns the schema and therefore the contract's
  *contents*; the SDK owns the artifact's *presence and the test that enforces it*.

## 4. CLI binding (Approach B — vendored SDL + contract test)

Keep `submit_package_capture` / `get_package_capture_status` as hand-written ops in
`graphql.rs`. **No codegen, no new runtime dependency.** Add a `#[cfg(test)]`
contract module that, using a GraphQL-parser **dev-dependency**:

1. Parses each operation's query string and validates every selected field,
   argument, and type against `capture-v1.graphql`.
2. Asserts the response structs (`CaptureInfo`, `CaptureStatus`) field-for-field
   against the SDL return types (the field the CLI deserializes must exist in the
   contract).

If a query or struct references something not in the contract, the test fails.
(A full `graphql-client`/`cynic` codegen migration — compile-time binding — is a
reasonable *future* step for all of `graphql.rs`, but is out of scope here: it
would force a codegen migration through a 52 KB hand-rolled client for two ops.)

## 5. Drift gate (CI) — two layers

**a. Offline (blocking, default).** The §4 contract test runs in normal CI with no
network. It catches CLI-vs-contract mismatch on every PR. This is the layer that
forces a platform contract-update PR (§6) to also update the CLI in lockstep.

**b. Online (gated / scheduled, non-blocking on normal PRs).** A separate job
introspects the **live source data API**, extracts the capture subset, and diffs it
against the vendored `capture-v1.graphql`. It flags *platform-ahead-of-contract*
drift — the schema changed but the vendored contract wasn't updated yet. Two
constraints:

- **Auth: M2M client-credentials** (`PMCP_CLIENT_ID` / `PMCP_CLIENT_SECRET`), NOT
  the interactive PKCE user token — the scheduled job must run with no human.
- **Endpoint: the exact source data API behind the target's `api_url`**, NOT the
  merged/front-door API. The two can diverge, and the CLI queries the source API,
  so the diff must be against what the CLI actually talks to.

It is opt-in/scheduled (needs the dev endpoint + M2M secret) so ordinary PRs never
depend on network or secrets; the offline layer is the hard gate.

## 6. Ownership & change process

When the platform changes the capture schema, **it opens a PR to the SDK repo**
updating `capture-v1.graphql` (or bumps to `capture-v2` on a breaking change).
Because the offline contract test (§5a) is blocking, that PR **must** also update
the CLI's ops/structs to match — the two move together in one reviewable change.

This is the concrete resolution of the two-team boundary: pmcp.run does not need to
add CLI verbs (they can't), and the SDK does not implement capture logic (it can't).
The platform PRs the **contract**; the SDK's gate forces the thin client to follow.

## 7. Compatibility policy

- The contract is **versioned** (`capture-v1.graphql`).
- **Additive** platform changes (e.g., a new intermediate status) need **no CLI
  release** — the poller already tolerates unknown statuses.
- **Breaking** changes go to `capture-v2` + a coordinated CLI update.
- So the drift gate fires only on changes that actually affect the client.

## 8. Delivery invariant (steps live in a separate runbook)

The capture verb ships as a **coordinated SDK release** together with
`show` / `import` / `approve` (they are one branch), gated by a green contract test
against a `capture-v1.graphql` generated from the live schema. The verb currently
lives on `feat/package-remote-capture-show`, unmerged and behind `origin/main`, so
landing it requires a rebase-and-release.

The concrete rebase / end-to-end-test / publish steps are **execution logistics that
go stale on completion** and are intentionally kept out of this doc. See:
`docs/runbooks/package-capture-release.md` (to be written).

## 9. Explicitly out of scope (tracked separately)

- **`--team-id` slug-vs-name UX** (was §7 of the draft): the argument accepts a slug
  id (e.g. `day-trip-planner-team`), which is **not** a UUID, so a "reject non-UUID"
  guard would reject valid ids. The separate ticket reworks this: reject only
  *obvious display names* (input containing spaces or mixed-case) with a clear
  message, and surface a clean server-side "AgentTeam not found" otherwise —
  client-side code cannot reliably tell a valid slug-id from a display name. That
  ticket also fixes `capture.rs`'s `long_help`, which currently (wrongly) says the
  argument is a UUID. Orthogonal to the contract seam.
- **`status` → server-side GraphQL enum**: an optional, clean platform schema change
  that a future `capture-v2` contract could then assert truthfully. Not required for
  v1; v1 must match today's `String`.
- **Full `graphql.rs` codegen migration**: a possible future direction, not this work.

## 10. Success criteria

- `contracts/pmcp-run/capture-v1.graphql` exists, is generated from the live source
  data API, types `status` as `String`, and includes `id` + `updatedAt` and the
  `captureId`/`id` asymmetry.
- The offline contract test fails the build if the CLI's capture ops/structs diverge
  from the contract.
- The online gate (M2M-authed, against the source data API) reports platform-side
  drift without blocking ordinary PRs.
- A platform schema change and its CLI update land as one contract-update PR.
