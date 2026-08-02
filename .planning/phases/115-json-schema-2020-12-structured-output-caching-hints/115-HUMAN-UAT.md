---
status: partial
phase: 115-json-schema-2020-12-structured-output-caching-hints
source: [115-VERIFICATION.md]
started: 2026-08-02T05:12:44Z
updated: 2026-08-02T05:12:44Z
---

## Current Test

[awaiting human testing]

## Tests

### 1. Decide the disposition of `115-REVIEW.md` CR-01 (`SUBSCHEMA_MAP_KEYWORDS` omits `dependencies`)
expected: Either (a) a further closure plan adds `dependencies` to `SUBSCHEMA_MAP_KEYWORDS` in both walkers plus the two restated mirrors, or (b) the finding is formally booked to `deferred-items.md` with a stated rationale, following the same convention already used for `D-115-AC` (WR-03) and `D-115-AD` (WR-04/05, IN-01/02/03) — i.e. explicit and owned-or-unowned, not silently absorbed.
result: [pending]

### 2. Correct or accept `contracts/mcp-protocol-sdk-v1.yaml`'s `output_schema_draft_pin` `formula:` equation head (lines 248-252)
expected: The equation head still states an unscoped total ("NO string-valued `$schema` anywhere in `s` … root or any depth" / "EVERY such `$schema`") five lines above the correctly-scoped `walk:` clause it introduces (lines 253-261) — round 3's own review WR-04, independently confirmed present by direct read. Either a small doc-only fix (scope the equation head to match the walk clause and the already-corrected invariants), or an explicit `deferred-items.md` entry accepting the inconsistency as documentation debt.
result: [pending]

## Summary

total: 2
passed: 0
issues: 0
pending: 2
skipped: 0
blocked: 0

## Gaps
