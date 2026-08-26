# Reply — both answers landed, and one of them found a bug in our proposal

**To:** pmcp.run platform dev team
**From:** SDK / cargo-pmcp side (`paiml/rust-mcp-sdk`)
**Date:** 2026-08-26
**Re:** your review of `docs/platform-requests/attestation-carriage-format-changes.md`

Four things changed on our side because of this review. All four are done, not planned.

| What | Where |
|---|---|
| SDL argument renamed `subjectPayloadDigest` → `subjectManifestDigest` | `contracts/pmcp-run/attestation-v1.graphql`, `cargo-pmcp/src/deployment/targets/pmcp_run/graphql_contract.rs` |
| New invariant: **refuse, never normalize** | handoff §2 |
| Verb-naming decision recorded, with our false premise corrected | handoff §5.2 |
| Your two blockers recorded as platform-owned, with the version pin escalated to our top ask | handoff §5.4, §10 |

---

## 1. The SDL bug — you were right, it's fixed

This was the most valuable single item in your reply, and you're right that it was cheap
now and expensive after ratification.

Confirmed before changing anything: our own §3.3 defines the subject as the **manifest**
digest, and the gate that produces the value calls `would_be_unattested_manifest_digest`
returning a `ManifestDigest`. So the SDL didn't merely clash with *your* vocabulary — it
contradicted **our own message two sections earlier**, and the argument's own comment
compounded it by saying "payload digest" in prose. The Rust parameter was already
generically `subject_digest`; only the wire name and its docs were wrong.

Renamed to **`subjectManifestDigest`** across the SDL, the operation string, the request
builder's variable key and both assertions. The offline blocking contract test passes —
3 passed, 1 ignored (the parked live leg) — which also means the operation string and the
schema are back in agreement.

The argument's comment now records *why*, including your Phase 171 bug, so the next person
to read it cannot re-derive the wrong name. Please still treat the whole file as
SDK-proposed; the rename doesn't change its provenance.

## 2. "Refuse, never normalize" is the best thing in your reply

We documented the **rule** and never documented the **remedy**, and your framing is exactly
right: two implementations can agree "no control characters" and still diverge on
reject-vs-normalize, producing different digests for the same logical input. That is a
digest divergence hiding behind an apparent agreement — strictly worse than an open
disagreement, because it looks settled.

It is now an invariant in its own right in handoff §2, credited to this review. Your
planned fix at the slot-construction boundary is the right shape for exactly the reason you
give: rewriting would silently change package identity.

Your Q2 answer is folded into §2.2 in full, including the part where we guessed wrong.
Component names being safe by a minting point, and config slots being the real exposure
because `roleLabel` and `toolDescription` are documented free text with no charset
constraint between the admin UI and the canonicalizer — that is a more useful finding than
the one we went looking for.

## 3. On the verb count — you're right, and it was our own repository

We verified before accepting it. `feat/package-172-cli` carries both commits you name,
dated 2026-07-21. Its `PackageCommand` enum has **eight** variants, not five. And its
`Import` rustdoc reads "Submit a REAL import job … halts honestly at `awaiting_activation`,
D-14", directly contradicting the branch we were reading, whose comment still says
"dry-run is the ONLY mode this phase".

So a `verb_help.rs` pinning five would have encoded a list contradicting your live control
plane and broken the moment that branch merged.

Worth naming the pattern, because it is now twice: **this is the second unmerged line in
our own repository to distort a document we sent you.** The first was Phases 120–122
themselves — §7's caveat that the paths don't resolve on `main`. We caught that one and
flagged it; we did not catch this one, and you found it from outside. We have written
"measure the verb surface across all live branches before pinning it" into §5.2, but the
honest generalization is broader: our repo has enough parallel unmerged work that
single-branch measurement is unsafe for anything we state to you as fact.

**Decision recorded:** `import` stays — a rename landing on `submitImport`/`getImportStatus`,
four data models, the 173.5 admin UI, an ADR and a live D-14 acceptance is not a rename, it
is a migration. The local file round-trip is **`save` / `load`**, which we checked is free
on both branches (as are `push`/`pull`). `install` excluded per Phase 184. Phase 123 plans
against that vocabulary.

## 4. The version pin is now our top ask

`pmcp-package = "0.1"` under caret cannot resolve `0.2` or `0.3`. The consequence is larger
than one stale dependency: **every invariant added since Phase 120 has no writer on your
side to check against**, which means our §10 ask 3 — "confirm the invariant table matches
your implementation" — is currently unanswerable, and has been for two format revisions.

We have promoted it to the single blocking item in handoff §10, ahead of everything else,
because the rest is downstream of it. It is source-breaking in all five ways our §4 lists,
so it is real work and we are not pretending otherwise.

The mitigating fact from our side: `pmcp-package` 0.2 was **never published** — crates.io's
max is still 0.1.1 — so this is one bump from 0.1.0 to 0.3.0, not two.

## 5. Team packages and the attestation gate — agreed, and please don't ask us to soften it

Your `publish.rs` shipping teams with an empty entry point and no members means every team
package you write today is refused an attestation under our fully-pinned rule. You've
called that ours-to-fix and the loud failure the right outcome; we agree, and we want to be
explicit that **we will not add an escape hatch**, so you can plan against that.

An attestation over a team whose entry point is `*` would be a signed claim about whatever
that star resolves to tomorrow. The refusal is the feature. If something needs to pack
before the 170-09 gap closes, pack it **unattested** — fully supported, and it degrades
exactly the property that isn't yet true.

Note the depth-1 boundary still holds in your favour: an attested team whose pinned agent
*itself* holds a range still packs. Requiring attestation transitively remains your
admission policy, not our format.

---

## What's next

Nothing here blocks you. On our side Phase 123 (`save`/`load`, plus `export`/`import`
against your API) plans against the vocabulary above.

Open, in the order we'd care about them:

1. **The `pmcp-package` bump** (§4 above) — gates your ability to check anything in §2.
2. **Ratify or counter-propose `verifyAttestation`** — now with the corrected argument name.
   Still one operation, and the payload schema is still yours to name and version.
3. **The golden-fixture corpus decision.** If you take it, we write the attested-package
   fixtures first — that hole is ours and we said so.
4. **`getPackageArtifact`**, unchanged from July and still the smallest item gating the most.

One process note: we'd rather receive reviews like this one than agreement. Three of the
four changes above exist because you checked our claims at source — the `olpc-cjson`
escape path, the two commits on our own branch, the two digests in your `publish.rs` —
rather than reading and nodding.
