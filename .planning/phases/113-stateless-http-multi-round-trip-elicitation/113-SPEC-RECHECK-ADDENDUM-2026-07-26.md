# 113-SPEC-RECHECK Addendum — 2026-07-26 RC investigation

**Status:** informational addendum. Does **not** upgrade `113-SPEC-RECHECK.md`'s `## Verdict`,
which stays `PENDING`. Produced by an independent research pass prompted by the maintainer
pointing at the RC announcement blog post.

**Sources:** `gh` CLI against `modelcontextprotocol/modelcontextprotocol` (main HEAD
`7634684382c3`, 2026-07-23T23:49:30Z) and
`https://blog.modelcontextprotocol.io/posts/2026-07-28-release-candidate/`.

## Finding 1 — `schema/2026-07-28/` does not exist ANYWHERE, including on the RC tag

`gh api repos/modelcontextprotocol/modelcontextprotocol/contents/schema` returns exactly five
directories — `2024-11-05`, `2025-03-26`, `2025-06-18`, `2025-11-25`, `draft` — and returns the
**same five** at each of:

- `?ref=main`
- `?ref=2026-07-28-RC` (the release-candidate tag)
- the release-tracking branch `docs/2026-07-28-release`

`docs/specification/` likewise has no `2026-07-28/`.

**Why this matters.** `113-SPEC-RECHECK.md`'s binding re-verification procedure step 1 is
"confirm `2026-07-28` now exists", steps 2-3 grep `schema/2026-07-28/schema.ts`. As of today
**no commit, PR, or branch in the repository creates that directory.** The procedure is not
merely "not yet executable on 2026-07-26" — there is no in-flight change that would make it
executable on 2026-07-28 either. Re-running the checkpoint on the date may well find the same
five directories.

**Consequence for planning.** Treating "the gate clears on 2026-07-28" as a scheduled certainty
is unsafe. The phase needs a decision for the case where the directory still does not exist on
the date. See Open Question below.

## Finding 2 — the RC tag is OLDER than the draft commit Phase 113 verified against

Tag `2026-07-28-RC` is a **lightweight** tag pointing at commit
`9d700ed62dcf86cb77475c9b81930611a9182f46`, dated **2026-05-29T12:49:07Z**.

Phase 113's baseline is `schema/draft/schema.ts` @
`71e306956a4959c9655e5036be215d41986596e6`, dated **2026-07-16T02:16:04Z**.

The phase's baseline is therefore roughly seven weeks **newer** than the RC tag. The RC tag is
not a more-authoritative source than what the phase already used; it is a less-current one.
Being a lightweight tag, it carries no tagger timestamp, so when it was actually pushed cannot
be determined from the ref.

## Finding 3 — the RC blog post says nothing at all about subscriptions

Term-count over the extracted full text of the RC post (dated May 21 2026, David Soria Parra):
**zero** occurrences of `subscriptions/listen`, `subscriptionId`, subscription acknowledgement,
`resources/subscribe`, `resources/unsubscribe`, or removal of the HTTP GET endpoint.

The post is not a source for any HTTP-04/06/07/08 obligation. Everything Phase 113 implemented
for those requirements traces to the draft schema types plus the conformance `stateless.ts`
scenario — not to the RC announcement.

## Finding 4 — spec basis for the GET-stream removal (supports the new HTTP-06)

`SubscriptionsListenRequest`'s doc comment in the draft schema states it "Replaces the previous
HTTP GET endpoint and ensures consistent [delivery]". Combined with the transport doc's verbatim
"HTTP GET or DELETE to the MCP endpoint: respond with `405 Method Not Allowed`"
(`113-RESEARCH.md:418`), the transport-level removal now split out as **HTTP-06** has a genuine
spec basis and is not a pmcp inference.

## Finding 5 — `subscriptionId` is OPTIONAL on `NotificationMetaObject` ⚠

The `_meta` key `io.modelcontextprotocol/subscriptionId` is **REQUIRED** on
`SubscriptionsListenResultMeta` (the teardown result), but **OPTIONAL** on
`NotificationMetaObject` — absent for notifications not delivered via a subscription.

**This may make the new HTTP-07 overstate the obligation.** HTTP-07 currently reads "every
delivered notification carries `subscriptionId` tagging". The spec requires the tag on
notifications delivered *on a subscription stream*; it does not make the field universally
required on the type. The wording is defensible for the stream path but should be verified
against pmcp's actual emission before HTTP-07 is treated as met.

## Finding 6 — there is no dedicated subscriptions SEP

The changelog attributes subscriptions to **SEP-2575**. `SEP-1803` "Event Subscriptions" and
`SEP-1975` "Conversation Event Subscriptions" exist in the repo but are open/closed, not the
governing SEP. This matches `113-RESEARCH.md:1018`, which already identified `stateless.ts` as
"the SEP-2575 scenario".

## Open Question for the maintainer

`113-SPEC-RECHECK.md` commits to re-running the checkpoint "on or after 2026-07-28" and treats a
mismatch as a phase-reopening event. Given Finding 1, the likely outcome on 2026-07-28 is
**neither confirm nor drift, but "still absent"**. The phase needs a recorded decision for that
third outcome — options include holding `[~]` indefinitely, promoting the draft pin to the
authoritative source with a documented risk acceptance, or gating the milestone's release on the
directory appearing.

This addendum does not make that decision.
