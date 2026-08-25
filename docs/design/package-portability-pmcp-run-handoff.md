# AI-Package Portability — Two-Sided Alignment Contract (SDK ⇄ Platform)

**Audience:** pmcp.run platform team, and any other host implementing pack/unpack
**From:** PMCP Rust SDK team (`paiml/rust-mcp-sdk`)
**Date:** 2026-08-25
**SDK milestone:** v2.6 "AI-Package Portability" (Phases 120–124) — 120 and 121 complete
**Status:** the SDK's format half is done and regression-netted. **Two independent
implementations of one format now exist, and keeping them from drifting is the work.**

---

## TL;DR

This is **not** a one-way ask list. Both sides implement the AI-Package format:

- The **SDK** packs and unpacks locally (`pmcp-package`, `cargo pmcp package`).
- The **platform** packs and unpacks too — capture writes packages, import reads them —
  and owns everything the SDK deliberately does not: artifact egress, ECR placement,
  attestation issuance, admission policy.
- **Other hosts may implement the same format.** That is the stated position of the design
  note (§2): `pull` binds to a documented contract, "pmcp.run is the first implementation,
  but any hosting service adopting the format implements the same op."

Two implementations of one format is the actual risk. A package that packs on one side and
fails to unpack on the other — or worse, unpacks into a *different* tool surface — is the
failure mode this document exists to prevent.

So there are two alignment surfaces, and they need different mechanisms:

| Surface | What can drift | Mechanism |
|---|---|---|
| **Code** — media types, manifest shape, canonical digest, slot vocabulary | Two implementations disagreeing about the same bytes | **Shared golden fixtures** (§3.2) — both sides pack/unpack the same corpus and must agree |
| **Operations** — GraphQL ops the CLI calls | Client and server drifting apart | **Vendored SDL + offline blocking test** (§3.1) — already proven once with `capture-v1.graphql` |
| **Instructions** — docs, CLAUDE.md/AGENTS.md, runbooks | Each side documenting a different mental model | **One shared vocabulary, cross-linked** (§6) |

**Highest-leverage item:** publish the golden-fixture corpus as a cross-implementation
conformance suite (§3.2). It already exists in-repo as a single-implementation regression
net; making it two-sided is a small change with a large payoff.

---

## 1. The alignment problem, stated concretely

The SDK's Phase 121 E2E proves a package round-trips **between two environments through one
implementation**: pack in A → unpack in B → tool-list parity. It does not — and cannot —
prove that a package packed by the *platform* unpacks correctly in the *SDK*, or the reverse.

That gap is where portability actually lives. Four ways it can break:

1. **Media-type drift.** One side writes `application/vnd.pmcp.mcp-server.config.v1+toml`,
   the other expects a different string or version suffix → layer silently unrecognized.
2. **Manifest-shape drift.** A field added on one side changes the canonical digest on that
   side only → `digest::verify` rejects a package that is actually intact.
3. **Slot-vocabulary drift.** One side classifies `AuthMode` as identity-bearing, the other
   as behavior-relevant → the target environment is told to supply the wrong set, and
   `required_slots` stops being trustworthy as "what B must fill".
4. **Baked-vs-slot drift.** One side bakes the OpenAPI spec into identity, the other treats
   it as environment → the same logical server gets two different digests, and
   digest-keyed attestations and `ApprovedPackage` admission both break.

None of these produce a loud error. All of them produce a package that looks fine.

---

## 2. Invariants that MUST agree between implementations

This is the code-level contract. Each row names the SDK's source of truth; the platform's
implementation must match it exactly, and any change must move on both sides together.

| Invariant | SDK source of truth | What breaks if it drifts |
|---|---|---|
| **Vendor media types** — every `application/vnd.pmcp.*` layer string | `crates/pmcp-package/src/oci/media_types.rs` (`MT_SERVER_CONFIG`, `MT_SERVER_OPENAPI_SPEC`, `MT_SERVER_BINARY_REF`, `MT_SERVER_BOOTSTRAP`, `MT_SERVER_DEPLOY_DESCRIPTOR`, …) | A layer is unrecognized → silently dropped, not an error |
| **Canonical digest** — deterministic serialization, then hash | `digest/canonical.rs` — `canonicalize()` then `manifest_digest()` | Identical content yields different digests → attestations and admission fail |
| **Integrity verification semantics** — what `verify` does and does NOT mean | `digest/verify.rs` — integrity ONLY, never a signature check | One side believing a verified digest implies authenticity |
| **Slot classification rule** — behavior-relevant iff it carries a `tested_value` | `slot/classification.rs:24-30` — a single predicate, no variant list | Wrong slot set demanded from the target environment |
| **Slot vocabulary** — the variants and which family each is in | `slot/types.rs` — identity-bearing: `Secret`, `OauthClient`, `ChannelBinding`, `HumanRole`; behavior-relevant: `LlmProvider`, `BudgetOverride`, `Endpoint`, `AuthMode` | Same as above; also breaks the "secrets never travel" guarantee if a secret gains a value field |
| **`required_slots` semantics** — THE enumerator of what a target must supply | `slot/required.rs` | An import UI that asks `detect_deviation` instead will never name the credential (see §2.1) |
| **`detect_deviation` semantics** — drift on one known `(tested, proposed)` pair; short-circuits on identity-bearing | `slot/deviation.rs:46-62` | Mistaken for an enumerator → silent omission of required credentials |
| **`name` vs `config_key`** — `name` is the ENV VAR; `config_key` is the dotted CONFIG PATH | `slot/required.rs`, `ConfigSlot::config_key` | Putting the config path in `name` derives a variable no environment can set (`BACKEND.BASE_URL`) |
| **Baked vs slot** — spec is baked (identity); endpoint, credentials, auth mode are slots | Phase 120, enforced by `tests/digest_stability.rs` | Two digests for one logical server, or an environment value entering identity |
| **Dual binary mode** — embedded bootstrap bytes, OR `BinaryRef { digest, media_type }` | `package/server.rs` | A referenced package treated as missing-layer instead of "resolve this digest" |

### 2.1 The one semantic trap worth calling out by name

`detect_deviation` **cannot name a credential.** It compares one already-known
`(tested, proposed)` pair and short-circuits on identity-bearing slots before examining any
value — and `SlotType::Secret` has no value field to compare, by construction.

This was wrong in the SDK's own roadmap until Phase 121 corrected it (D-04/D-05): the
original success criterion routed a set-equality assertion through `detect_deviation`,
which would have asserted a 2-slot set where the truth is 3 — **silently omitting the
credential, the single most important thing a target environment must supply.**

If the platform's import UI answers "what must this environment provide?", it must call
**`required_slots`**. This is the highest-value thing in this document for import.

---

## 3. Two mechanisms for staying aligned

### 3.1 Operations: vendored SDL + offline blocking test *(proven, reuse as-is)*

Both teams have executed this once successfully for `capture`. Reproducing the ownership
rule verbatim from `contracts/pmcp-run/capture-v1.graphql`'s header:

> **OWNERSHIP: the platform owns the file's CONTENTS.** When the schema changes, the
> platform re-exports and PRs an update here; the SDK's blocking contract test then forces
> `cargo-pmcp`'s queries and response structs to follow **in the same PR**.

Mechanics worth keeping identical for new contracts:

- Export via `aws appsync get-introspection-schema --format SDL`, reduced to the relevant
  operations and their return types.
- **Strip `@aws_cognito_user_pools` / `@aws_iam` field directives** — auth config, not shape.
  A drift diff must normalize them out on both sides.
- **Do not "tidy" `String!` status fields into GraphQL enums.** `capture-v1.graphql`
  documents its runtime values as comments precisely because the live schema is not
  enum-typed.
- Where an argument name differs across a submit/status boundary (capture's
  `captureId` → `id`), record a `BOUNDARY NOTE` comment rather than renaming.
- The SDK's half is offline and credential-free, in the default `cargo test` gate — see
  `cargo-pmcp/tests/package_capture_contract.rs`.

**Net effect:** the platform ships ops on its own cadence. The contract landing in the SDK
repo is what unblocks client work; a live endpoint is only needed to un-`#[ignore]` an E2E leg.

### 3.2 Format: shared golden fixtures *(exists in-repo — proposal is to make it two-sided)*

The corpus already exists as a single-implementation regression net:

```
crates/pmcp-package/tests/golden_fixtures/
  canonical/                        # canonical-form goldens
  config_server_london_tube_v1/     # the Phase 120 config-only package kind
  agent_pto_researcher_v1.json
  server_team_fs_v1.json
  team_small_review_v1.json
  workflow_claims_triage_v1.json
  env_ref_grammar_v1.tsv
```

`tests/digest_stability.rs` pins their canonical digests, so a change to the layer set,
layer order, or a media-type string fails the SDK build instead of silently shipping a
package a previously-published CLI cannot read.

**The proposal:** treat this corpus as the **cross-implementation conformance suite**.
Concretely — each item is small and independently useful:

1. **Both sides pin the same digests.** The platform runs the same fixtures through its
   pack/unpack path and asserts the identical canonical digests. A divergence is then a
   build failure on whichever side moved, not a support ticket six weeks later.
2. **A fixture is added with every format change.** A new media type, a new slot variant, a
   new package kind → a new golden. This is already the SDK's habit (Phase 120 added
   `config_server_london_tube_v1`); the ask is that it becomes a joint habit.
3. **Cross-direction round-trip, once egress exists.** Platform packs → SDK unpacks →
   assert tool-list parity, and the reverse. This is Phase 121's E2E with one side swapped,
   and it is the only test that actually proves portability. It needs `getPackageArtifact`
   (§5.1) to exist first.
4. **Decide where the corpus lives.** In the SDK repo (platform vendors it), in a shared
   repo, or duplicated with a drift check. Open question — see §7.

**Why fixtures and not just a schema:** the failure modes in §1 are about *bytes and
semantics*, not shapes. A JSON Schema cannot catch "these two implementations classify
`AuthMode` differently" or "these produce different digests for identical content". A golden
fixture catches both, mechanically.

---

## 4. What the SDK has delivered

All of this is on the v2.6 milestone branch and needs nothing from the platform.

### Phase 120 — Config-Server Packaging ✅ complete (verification passed)

- **Config-only packages.** `pack_server` no longer requires `bootstrap: &[u8]`. A server
  with no bespoke binary has a complete package identity via `MT_SERVER_CONFIG`
  (`…config.v1+toml`) and `MT_SERVER_OPENAPI_SPEC` layers.
- **Dual-mode binary.** Embedded bootstrap bytes, or `BinaryRef { digest, media_type }`
  resolved in the target environment. Unpacking a *referenced* package where the blob is
  absent **reports the digest to resolve** rather than failing as a missing layer — this is
  the shape the platform's import path will consume.
- **Baked vs slot, machine-checked.** One byte changed in the spec ⇒ new canonical digest ⇒
  `digest::verify` rejects the stale one. Endpoint, credentials, auth mode are slots.
- **New slot vocabulary.** `SlotType::Endpoint` / `SlotType::AuthMode`, plus
  `ConfigSlot.config_key` — with the `name` (env var) vs `config_key` (config path) split
  described in §2.

### Phase 121 — Local Round-Trip E2E ✅ complete (verification passed, UAT 37/37)

`crates/pmcp-openapi-server/tests/roundtrip_e2e.rs` packs the London Tube server in
environment **A** and unpacks it in a distinct **B** (separate OCI layouts, separate temp
dirs, different endpoint/credential/auth-mode values, no shared process state), fully
offline against `wiremock`. It asserts:

1. `required_slots` names **exactly** the slots B must fill — set equality against a
   hardcoded list, so a slot added later that B is never told about turns the test red.
2. `detect_deviation` separately reports B's endpoint drift, and returns `None` for the
   credential in both directions (§2.1).
3. Once filled, B serves a tool list **set-equal** to A's, and `london-tube-scenarios.yaml`
   replays green through `mcp-tester`'s `ScenarioExecutor`.
4. Both SC4 directions: adding a field to `ServerPackage` leaves it green; dropping a tool
   from B's surface or leaving a slot unfilled turns it red. **No assertion on manifest
   field names, layer ordering, or digest values** — machine-checked by a structural guard,
   so the E2E survives the manifest refactors this milestone expects.

Gate: `make test-openapi-server` → `parity_replay` 3, `pmcp_package_pin` 2,
`roundtrip_e2e` 8, 42 tests total, with per-binary counts printed so a suite that silently
stops running fails the gate.

### Phases 122–124 — not started

122 (attestation carriage) and 123 (export/import verbs) are **contract-first and parked on
the platform**; 124 is internal release hygiene. Versions: `pmcp-package` **0.2.0** locally
(crates.io max is 0.1.1), `cargo-pmcp` **0.22.0**.

---

## 5. What the platform side owns

Three capabilities, none of which live in this repo. Each is framed as "the platform's half
of a shared format" rather than a favour to the SDK.

### 5.1 `getPackageArtifact` — authenticated artifact egress

**Why it is first:** it gates the cross-direction round-trip in §3.2 item 3, which is the
only real proof of portability. It also gates the entire audit/marketplace track. And it is
deliberately tiny — it repeats the capture-seam playbook exactly.

```graphql
getPackageArtifact(reference: String!): GetPackageArtifactReturnType
# → { payloadDigest: String!, downloadUrl: String!, expiresAt: String! }
```

- Resolves `name@version` (or a raw digest), authorizes against the caller's org (same
  scoping as `show`), returns a **short-lived presigned URL** for a tar of `index.json` +
  `blobs/`.
- **Audit-logged.** Stated precisely: a presigned URL is a *bearer token*, so the audit row
  records **issuance**, not download. Short expiry (~5 min) plus S3 access logs where the
  trail needs actual-GET evidence. That specification is the platform's — it is their
  compliance surface.
- Vendored SDK-side as `contracts/pmcp-run/portability-v1.graphql`.

**SDK acceptance:** `cargo pmcp package pull <ref> --output ./pkg/` downloads, unpacks, and
**re-verifies every blob digest and the payload digest locally** — transport is never
trusted. `cargo pmcp package inspect ./pkg/` then works today, unchanged.

**Out of scope by design, so it need not be defended against:** mutate-and-reimport. Any
byte change changes the payload digest, which matches no `ApprovedPackage` and fails
import's digest assertion. Local `unpack → modify → pack` stays supported for
experimentation — it just produces a new, unapproved identity.

### 5.2 AI-Package import

Scoping decision #2: **GraphQL mediates import**; the platform owns ECR placement. The SDK
therefore adds **no `oci-client`** — the CLI never speaks to a registry. `oci-spec` types
stay, and the manifest types were chosen so a registry client consumes them with zero
translation, which keeps that door open for the platform.

**Needed:** the import operation's SDL, plus the expected slot-binding shape at import time.
What the SDK produces is `required_slots`' output — typed, each carrying `name` (env var)
and `config_key` (config path), and **never carrying values**.

**A collision the SDK must resolve, which affects platform docs:** `cargo pmcp package`
already ships five verbs — `inspect | capture | show | import | approve` — and **`import` is
already taken** by the remote *workflow-manifest dry-run* import. The AI-Package import verb
collides with a shipped verb. The SDK will resolve it explicitly and pin the complete
post-change verb list in `cargo-pmcp/tests/verb_help.rs`. **If the platform has a naming
preference, say so before Phase 123 planning** — after that the rename cost lands on
platform docs too.

**Already committed to, so it can be relied on:** export/import resolves its environment
through `configure`'s existing resolver and the **existing** `pmcp_run` seam —
`get_api_base_url()`'s `PMCP_API_URL` precedence
(`cargo-pmcp/src/deployment/targets/pmcp_run/auth.rs:113`) and the TTL'd, endpoint-keyed
config cache. **No second API path**: no new base-URL env var, no second token cache.

### 5.3 Attestation issuance on version promotion

Scoping decision #1: **attestation is pmcp.run-issued.** Trust is anchored in the platform,
not a developer-held key. The SDK's job is carriage and verification only:

- **No crypto dependency enters `pmcp-package`** — to be enforced by a dependency tripwire test.
- `digest::verify` remains an **integrity** check, never a signature check.
- The attestation rides as an **opaque** layer under an `application/vnd.pmcp.*` media type;
  the crate never deserializes or interprets its bytes.
- `cargo pmcp package inspect` renders presence, subject digest and issuer when one is
  carried, and reports "unattested" when none is — fixture-driven, no network on any path.

**Needed:** `contracts/pmcp-run/attestation-v1.graphql` (issuance + verification-against-
platform-identity), plus the attestation payload schema. The attestation **format** is a
**shared contract**, versioned like `capture-v1.graphql`. Attestation **storage and
admission control** ("which attestations must exist for import") is the platform's — a
commercial policy surface the SDK does not model. Reference payload shape: design note §4.

**Parked-boundary discipline:** the live leg exists SDK-side as an `#[ignore]`d, env-gated
test naming exactly what the backend must ship (the `PMCP_OPENAPI_LIVE_TEST=1` double-gate).
Promoting Phase 122 from parked to blocking is **removing a gate, not writing a new test**.

---

## 6. Instructions alignment

Code alignment without instruction alignment just relocates the drift. Three concrete asks:

1. **One shared vocabulary.** The terms in §2 — *baked* vs *slot*, *identity-bearing* vs
   *behavior-relevant*, *embedded* vs *referenced* binary, `name` vs `config_key`, *payload
   digest* — should mean the same thing in both codebases and both sets of docs. They are
   defined in `crates/pmcp-package/src/slot/types.rs` (module docs) and this file's §2.
   Where platform docs use a different word for the same concept, the two should be
   reconciled rather than each side keeping its own.

2. **Cross-link the agent instructions.** This repo carries `CLAUDE.md` (and a mirrored
   `AGENTS.md`) with the publish ledger and quality gates. If the platform repo has an
   equivalent, each should point at the other for the shared-format sections, so an agent
   working on either side finds the same rules. Specifically worth mirroring: the §2
   invariant table and the §9 guardrails.

3. **State the ownership rule in both places.** §3.1's "platform owns the SDL contents; the
   SDK's blocking test forces the client to follow in the same PR" currently lives only in a
   comment header in the SDK repo. It governs both sides and should be visible from both.

One caveat, stated plainly: the SDK's publish ledger in `CLAUDE.md` is **hand-maintained
prose**, and it has drifted twice (2026-07-27, 2026-08-21) with the text present both times.
Machine checks (`scripts/check-release-coverage.sh`, `tests/pmcp_package_pin.rs`) cover the
code shape, not the prose. Treat any instruction-level agreement as needing a mechanical
backstop wherever one is affordable — which is the argument for §3.2.

---

## 7. Open questions needing a platform answer

Numbering preserved from design-note §10 so the documents stay cross-referenceable. Items
marked **[proposed]** carry the platform team's own 2026-07-21 recommendation and need
ratification, not invention.

| # | Question | State |
|---|---|---|
| 1 | Egress artifact packaging — tar at capture time, digest-keyed to S3? | **[proposed: yes]** — decide **now**; the backfill window closes as packages accumulate |
| 2 | Digest-addressed fetch in v1? (costs a GSI on payload digest) | **[proposed: yes]** |
| 3 | Report signing: SDK keypair, sigstore keyless, or defer all signing to attestation? | open |
| 5 | Marketplace namespace/identity model (org-scoped vs global) | open — `subject.reference`'s format must not foreclose it |
| 7 | `[[resources.*]]` closed set: which kinds first, symbolic IAM reference syntax, `deploy-descriptor.v2` versioning | open |
| 9 | Release bundling: ship CLI package verbs now with `pull` next minor, or one bundled release? | decision holder is the **SDK**; platform has argued for shipping now |
| 10 | Ratify design-note §7: descriptor is the contract, stack is derived, renderer is a shared open-source crate | open — **the largest architectural item** |
| 11 | Per-wave expressiveness checklist: what must `[[resources.*]]` express before each recreation wave? | open |
| — | **Golden-fixture corpus home** — SDK repo (platform vendors), shared repo, or duplicated with a drift check? | **new, from §3.2** |
| — | Naming for the AI-Package import verb, given `package import` is taken | **new** — needed before Phase 123 planning |

Resolved and kept for the record: **Q6** (IAM population — deterministically not captured;
the synthesized descriptor is systematically lossy) and **Q8** (AVP read scope — single
store, server-filtered), both platform-confirmed.

### On §7 of the design note (Q10)

The proposal is to demote the synthesized CDK/CFN stack from a **contract artifact** to a
**derived artifact**: the `DeployDescriptor` becomes the complete declaration and whoever
deploys renders the stack at deploy time. The security argument is the one that matters:
validating client-synthesized CFN means validating attacker-controlled input in a hostile
format (conditions, intrinsics, `Fn::Sub`) from an open-source, replaceable CLI — whereas
validating the closed-set *descriptor* is a small semantic surface, and the stack generated
from it is trusted by construction.

The SDK has already moved on the mechanism half: **`pmcp-cfn-renderer` exists as an
extracted crate** (`crates/pmcp-cfn-renderer/`, pinned by `cargo-pmcp`), so the "shared
open-source renderer" is not hypothetical. The remaining substantive cost is CDK-codegen →
**direct CFN emission from Rust** (no Node toolchain in a Lambda). Migration is by **fleet
recreation in waves**, not renderer/CDK compatibility — the platform's commitment, and what
keeps the renderer out of a logical-ID compatibility tarpit.

**This must not gate §5.1.** Egress depends on none of it.

---

## 8. Guardrails — what the SDK will not do

Settled decisions, not preferences under review. The platform can build against these:

| The SDK will not | Because |
|---|---|
| Add signing keys or PKI to `pmcp-package` | Attestation is platform-issued; trust is anchored there. To be enforced by a dependency tripwire test |
| Add an ECR / OCI registry client (`oci-client`) to the CLI | GraphQL mediates import and owns ECR placement. `oci-spec` types stay so a registry client could consume the manifests later with zero translation |
| Interpret attestation bytes | Carriage is opaque by design |
| Teach the format crate about policies, registries, or reports | All audit logic lives *above* the format. The small, auditable trust kernel is itself part of the security posture |
| Guarantee byte-for-byte package round-tripping | Tool-list parity is the property that matters; byte identity would break on every manifest revision without indicating a real regression |
| Ship secrets inside a package | Slots declare *requirements*, never values — guaranteed by the type system (`SlotType::Secret` has no value field), not by egress-time filtering |

---

## 9. Document map

### Design

| Path | What it is |
|---|---|
| `docs/design/package-portability-and-audit.md` | **The primary document.** SDK ⇄ platform boundary (§2), `getPackageArtifact` / `pull` (§3), the `package-auditor` reference team and attestation report schema (§4), format coverage gaps (§5), descriptor-as-single-source-of-truth (§7), phasing (§8), security (§9), open questions (§10) |
| `docs/design/package-portability-pmcp-run-handoff.md` | **This file** — the two-sided alignment contract |
| `docs/design/tasks-http-upgrade-pmcp-run.md` | Prior platform-facing handoff — the format this document follows |

### Planning (authoritative status)

| Path | What it is |
|---|---|
| `.planning/REQUIREMENTS.md` | The 7 v2.6 requirements (PKG-01..04, PKGX-01/02, PKGR-01), both scoping decisions, traceability, and the "⚠ PKGX-01/02 cannot fully close inside this repo" note |
| `.planning/ROADMAP.md` § `v2.6 AI-Package Portability` | Milestone goal, scoping decisions, non-goals, decisions taken at the open |
| `.planning/ROADMAP.md` § `Phase Details — Current Milestone` | Per-phase success criteria. **Phases 122 and 123 are the platform-relevant ones** — every criterion is achievable offline with the backend unavailable |
| `.planning/phases/120-config-server-packaging/` | 5 plans + summaries, `120-VERIFICATION.md` (passed) |
| `.planning/phases/121-local-round-trip-e2e/` | 5 plans + summaries, `121-VERIFICATION.md` (passed), `121-UAT.md` (37/37), `deferred-items.md` |

### Code seams

| Path | What it is |
|---|---|
| `crates/pmcp-package/src/oci/media_types.rs` | **All `application/vnd.pmcp.*` layer types** — §2 row 1 |
| `crates/pmcp-package/src/digest/canonical.rs` | `canonicalize()` + `manifest_digest()` — the canonical digest both sides must reproduce |
| `crates/pmcp-package/src/digest/verify.rs` | Integrity verification — stays integrity-only |
| `crates/pmcp-package/src/slot/types.rs` | Slot vocabulary + the identity/behavior split, with module docs defining the shared terms |
| `crates/pmcp-package/src/slot/classification.rs` | The single classification predicate |
| `crates/pmcp-package/src/slot/required.rs` | **`required_slots`** — what a target environment must supply. The function an import UI wants |
| `crates/pmcp-package/src/slot/deviation.rs` | `detect_deviation` — drift only; **not** an enumerator (§2.1) |
| `crates/pmcp-package/tests/golden_fixtures/` | **The proposed conformance corpus** (§3.2) |
| `crates/pmcp-package/tests/digest_stability.rs` | Pins the corpus's canonical digests |
| `contracts/pmcp-run/capture-v1.graphql` | **The contract-seam template** + the ownership rule |
| `cargo-pmcp/tests/package_capture_contract.rs` | **The template test** — offline, credential-free `apollo_compiler` validation |
| `cargo-pmcp/src/deployment/targets/pmcp_run/auth.rs` | `get_api_base_url()` (line 113) + TTL'd token cache — **the one API path** |
| `cargo-pmcp/src/commands/package/mod.rs` | The five shipped verbs, incl. the colliding `import` |
| `crates/pmcp-openapi-server/tests/roundtrip_e2e.rs` | The Phase 121 pack-A → unpack-B → parity E2E |
| `crates/pmcp-cfn-renderer/` | The extracted descriptor → CloudFormation renderer (design-note §7) |

---

## 10. Concrete asks

1. **Confirm the §2 invariant table matches the platform's implementation** — particularly
   the slot classification rule and the `name` vs `config_key` split. A mismatch here is the
   highest-probability silent break.
2. **Decide the golden-fixture question (§3.2, §7)**: adopt the corpus as a shared
   conformance suite, and decide where it lives.
3. **Ratify Q1 and Q2 now** (tar-at-capture, digest-addressed fetch) — the backfill window is
   closing.
4. **Schedule `getPackageArtifact` (§5.1)** and export `portability-v1.graphql`. Smallest
   item, gates the most — including the cross-direction round-trip that actually proves
   portability.
5. **Say whether import and attestation issuance are on the roadmap, and roughly when.** Not
   needed to land the contract-first halves; needed to decide whether Phases 122/123 stay
   parked or get promoted with a live E2E leg this milestone.
6. **Answer the import-verb naming question** before Phase 123 planning.
7. **Take a position on design-note §7 (Q10).**

A joint review is the efficient path if more than two of these are live.
