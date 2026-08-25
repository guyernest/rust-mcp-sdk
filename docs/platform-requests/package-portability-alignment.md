# Alignment request: package portability (`pull`), audit tooling & the next release

**To:** pmcp.run platform dev team
**From:** SDK / cargo-pmcp side
**Date:** 2026-07-21
**Companion document:** `docs/design/package-portability-and-audit.md` — the
full design note this message asks you to review. This message is the
executive summary + the specific asks.

## Where we are (shared wins)

- **`capture` / `import` / `approve` / `show` are merged** to `main`
  (PR #311, cargo-pmcp 0.19.0) with the contract seam
  (`contracts/pmcp-run/capture-v1.graphql` + offline blocking test). All CI
  green.
- **Capture E2E is proven against dev** — deterministic manifest digest across
  re-runs (`sha256:af0ae208…` both times). Your capture worker's
  content-addressing does exactly what the format promises.
- Your per-component pull fix (Phase-171 handoff, FIX #1) is deployed and
  verified on your side; we'll run the CLI-side `import` E2E as part of the
  next release gate.

## Release-plan decision (please note)

**We are NOT tagging cargo-pmcp 0.19.0 for crates.io yet.** The next
*published* CLI will ship the package verbs **together with `pull`** — one
coherent "the package is a real, portable artifact" release. Consequence:
**Phase A below directly gates the next CLI release**, so we'd like your
read on its cost/timing early.

## What we're proposing (read the design note for the full picture)

The one-paragraph version: a `pmcp-package` should be obtainable, verifiable,
and reviewable **outside the platform** — that's the foundation for security
review, policy compliance testing, and eventually the marketplace / agent
store. The format already has the right properties (content-addressed,
deterministic, secret-free via slots, typed scannable layers, real OCI). The
missing pieces are artifact **egress** (yours), local **audit tooling**
(ours), and later an **attestation convention** (shared) — that last one now has
a concrete first step, a one-operation SDL awaiting your ratification: see
**ask 5**. The dogfood proof is
a `package-auditor` agent team — built with the SDK, auditing packages,
extensible by customers through ordinary team composition, and ultimately
able to audit itself.

Boundaries stay as they've worked so far: you own egress, attestation
storage, and admission control (the commercial surface); we own the format
crate (kept pure), the CLI verbs, and the open audit tooling; contracts
between us are versioned SDL files with your export + our blocking test —
the exact capture-v1 playbook, repeated.

## The asks, in priority order

### 1. Phase A — `getPackageArtifact` (gates our next release)

A GraphQL op on the same API the existing verbs use:

```graphql
getPackageArtifact(reference: String!): GetPackageArtifactReturnType
# → { payloadDigest: String!, downloadUrl: String!, expiresAt: String! }
```

Org-scoped auth (same as `show`), short-lived presigned URL for the packaged
OCI layout, **audit-logged egress**. Contract lands as
`portability-v1.graphql` — you export the SDL from the live schema (never
hand-transcribed; we all remember why), we pin the CLI to it with an offline
blocking test. Two design decisions are yours to make (design note §9, Q1–Q2):

- **Artifact packaging:** one tar per package produced at capture time
  (cheap, immutable, cacheable) vs. assembled on demand?
- **Addressing:** should the op also accept a raw payload digest, not just
  `name@version`? (Reviewers will want digest-addressed fetch.)

**Ask: a rough size/timeline estimate, since this gates the CLI release.**

### 2. Verification item — does IAM survive capture today?

`DeployDescriptor` models `[iam]` / `[[iam.statements]]`, and the format packs
it — but your `slot_extract.rs` synthesizes descriptors with `iam: None`.
Please capture a server that actually declares IAM statements and confirm
whether they land in the packed layer, or whether population is a gap. (Our
determinism E2E proved digest stability, not field completeness.) Design note
§5 / §9 Q6.

### 3. Verification item — AVP read scope

Your capture worker reads Cedar policies from AVP via the `PolicySource` seam
(this is excellent — it means packages carry their real enforcement policies,
digest-bound). Question: does that read cover **every** policy store/scope a
server's code-mode tools can be governed by, or only the primary store?
Design note §9 Q8.

### 4. Joint format extension — declarative `[[resources.*]]`

Custom CDK resources (DynamoDB tables etc.) added outside deploy.toml are
invisible to capture and the digest today. We propose extending the
`DeployDescriptor` **closed set** with declarative resource tables (e.g.
`[[resources.dynamodb]]`) — source of truth stays declarative and
digest-bound, the stack stays derived. This is a joint change: format (us) +
capture population (you). We need your input on which resource kinds matter
first and whether your platform-hosted servers can populate them. Design note
§5 / §9 Q7.

### 5. Attestation contract — ratify one operation (`verifyAttestation`)

**What we did.** We vendored an **SDK-PROPOSED** SDL at
`contracts/pmcp-run/attestation-v1.graphql` naming exactly one operation,
`verifyAttestation`, and wired an offline **blocking** contract test at
`cargo-pmcp/tests/package_attestation_contract.rs` that fails the SDK build if
the CLI's operation drifts from that SDL. The file carries no `Source:` and no
`Exported:` line, and its header says plainly that it is SDK-proposed and
awaiting your ratification — deliberately unlike `capture-v1.graphql`, whose
contents you own.

**What we're asking for.** Ratification — or a counter-proposal — of that one
operation: its name, its arguments, and its return shape. Then, once ratified,
an **export of your own SDL** to replace our proposal, exactly as
`capture-v1.graphql` works today. That export is what upgrades the blocking test
from an internal consistency check into a real cross-boundary drift net. Until
it lands, our test can only prove that our query and our proposed schema agree
with each other; it cannot detect drift from a platform that has not spoken, and
we have written that limitation into the test's own module docs so nobody on our
side mistakes green for agreement.

The proposal as it stands:

```graphql
verifyAttestation(
  attestationPayloadBase64: String!   # the attestation layer's raw bytes, base64 (RFC 4648 §4)
  subjectPayloadDigest: String!       # the sha256: digest we re-derived locally
): VerifyAttestationReturnType
# → { verdict: String!, verifiedIdentity: String!, verifiedAt: String! }
```

`verdict` is `String!`, not an enum — the same discipline as `status` in
`capture-v1.graphql`, so a later schema-versus-schema diff does not show
permanent drift. The verdict vocabulary is yours to define.

**Why only one operation.** The attestation **arrives inside the package** —
that is what carriage means — so the CLI never fetches one, which makes a
`getAttestation` op speculative until `import` semantics settle. Issuance is
entirely yours to design, so an `issueAttestation` op is not ours to propose.
One operation to ratify, not three.

**The boundary, restated so this cannot be misread as scope creep.** The SDK does
**carriage and subject-digest comparison only**. It holds no keys, adds no crypto
dependency (machine-checked by a `cargo-deny` allowlist over `pmcp-package`'s
resolved dependency graph), and **cannot verify a signature offline**. That is
precisely why this one call has to exist on your side: "verified against
pmcp.run's identity" is a signature check, and we have deliberately kept
ourselves unable to perform one.

**Timing.** Our side of this is already merged and gated, and the live leg is
parked behind an `#[ignore]` plus a triple environment gate. Nothing here blocks
the `pull` release (ask 1); we are asking for a design response, not a delivery
date.

**FYI on the format, since your capture writes it too.** The package format now
records a component's **declared range alongside its resolution**, and a pack
carrying an attestation **refuses any unresolved component reference** — an
attested package must be fully pinned, or the attestation would be making a claim
about a moving target. Flagging it so the two sides do not diverge silently.

### 6. FYI / later — attestations & admission (Phase E)

Digest-keyed audit reports (produced by the open auditor tooling) become
attestations attached via OCI referrers; your `import` admission policy can
then require them ("import needs a `core-security` pass"). Nothing to build
now — but §9 Q3/Q5 (report signing, marketplace namespacing) will need your
voice when we get there. `revokeApprovedPackage` already gives the
revocation endpoint this needs.

## What we're building meanwhile (no platform dependency)

- `cargo pmcp package pull` (ready to bind the moment your op + SDL exist).
- Audit report schema v1 + policy-pack format; static checks over the
  already-shipped layers — including offline Cedar validation/scenario
  testing (`cedar-policy` crate) against the policy sets your capture
  already packs, and IAM/rendered-stack linting.
- The `package-auditor` reference team, and a `cli-server` built-in
  (governed, Cedar-gated declarative CLI wrapping) that the auditor's tool
  servers will be configurations of.

## Suggested next step

30–60 min joint review of `docs/design/package-portability-and-audit.md`
(§2 boundaries, §3 Phase A, §9 open questions), then we split tickets: your
Phase-A op + the two verification items; our `pull` verb + contract test
scaffolding, ready for your SDL export.
