---
status: re-verify
phase: 78-cargo-pmcp-test-apps-mode-claude-desktop-detect-missing-mcp-
source: [78-VERIFICATION.md, 78-05-PLAN.md, 78-06-PLAN.md, 78-07-PLAN.md]
started: 2026-05-02T00:00:00Z
updated: 2026-05-02T18:30:00Z
gap_closure_landed: 2026-05-02
gap_closure_validated: false
---

## Current Test

[awaiting human re-verification post Plan 78-05/06/07/08 gap closure]

## Tests

### 1. AC-78-1 at CLI binary boundary — broken widget fails claude-desktop mode
expected: Run `cargo pmcp test apps --mode claude-desktop --widgets-dir <path>` against a directory containing a deliberately-broken widget (no SDK presence signals: no `@modelcontextprotocol/ext-apps`, no `[ext-apps]` log prefix, no `ui/initialize`, no `ui/notifications/tool-result`; no `new <id>({name:"...",version:"..."})`; no protocol handlers; no `app.connect()`). The simplest reproducer is to copy `crates/mcp-tester/tests/fixtures/widgets/broken_no_sdk.html` into a tempdir and point `--widgets-dir` at it. Process exits non-zero AND stdout/stderr names at least one missing handler (e.g. `onteardown`).

Why this is now testable without a fixture binary: Plan 78-07 added `--widgets-dir`, which bypasses the `cli_acceptance.rs` fixture-binary skip-gate that previously blocked binary-boundary verification of AC-78-1.

result: [pending]

### 2. AC-78-2 at CLI binary boundary — corrected widget passes claude-desktop mode
expected: Run the same `--widgets-dir` command against a directory containing `crates/mcp-tester/tests/fixtures/widgets/corrected_minimal.html`. Process exits zero.

result: [pending]

### 3. AC-78-3 at CLI binary boundary — Standard mode is permissive
expected: Run `cargo pmcp test apps --widgets-dir <path>` (no `--mode` flag) against BOTH the broken and corrected widgets. Both invocations exit zero — no regression for the permissive default.

result: [pending]

### 4. AC-78-4 at CLI binary boundary — chatgpt mode is unchanged
expected: Run `cargo pmcp test apps --mode chatgpt --widgets-dir <path>` against both fixtures. Both exit zero AND stderr/stdout MUST NOT contain any of the four protocol handler names — chatgpt mode is a no-op for widget validation.

result: [pending]

### 5. AC-78-5 — UX review of READMEs and `--help` text (now includes `--widgets-dir`)
expected: Visual review of `cargo pmcp test apps --help` and both READMEs (`cargo-pmcp/README.md`, `crates/mcp-tester/README.md`). Reader can answer:
- "What does `--mode claude-desktop` do, when should I use it, and what does it check?"
- "What does `--widgets-dir` do, when should I prefer source scan over bundle scan?"

Both questions answerable from the docs alone.

result: [pending]

### 6. Re-verify against cost-coach prod (the gap-closure proof)
expected: Run `cargo pmcp test apps --mode claude-desktop https://cost-coach.us-west.pmcp.run/mcp`. Expected outcome:
- Process exits zero.
- Output reports zero Failed rows on the 8 production widgets.
- This contrasts with the v1 result captured in 78-VERIFICATION.md "Gaps Summary" (97 tests, 60 passed, 33 failed, 4 warnings — all 33 failures were false positives).

Optionally, ALSO run `cargo pmcp test apps --mode claude-desktop --widgets-dir cost-coach/widget` against a local cost-coach checkout. Expected: same — zero Failed rows.

Source feedback driving this re-verification: `/Users/guy/projects/mcp/cost-coach/drafts/feedback-pmcp-test-apps-v1-false-positives.md` (8.4 KB, dated 2026-05-02). The feedback documents the v1 false-positive class; this re-verification confirms its absence post-fix.

result: **FAILED — false-positive class still present.** 33 Failed rows on cost-coach prod, identical count to v1. See `uat-evidence/2026-05-02-cost-coach-prod-rerun.md`. Diagnosis: G2 (constructor regex) misses all 8 prod widgets; G1 (SDK signals) misses 4 of 8. Synthetic fixtures in Plan 05 didn't match real Vite-singlefile prod output — the gap-closure cycle did not generalize to production bundles. Routes to `/gsd-plan-phase 78 --gaps` for a second iteration.

## Summary

total: 6
passed: 0
issues: 1
pending: 5
skipped: 0
blocked: 0

## Gaps

### G6 — Plan 05/06 fix did not generalize to real cost-coach prod bundles
- **Date:** 2026-05-02
- **Source:** Test 6 operator re-verification against `https://cost-coach.us-west.pmcp.run/mcp`
- **Evidence:** `uat-evidence/2026-05-02-cost-coach-prod-rerun.md` (97 tests, 60 passed, 33 failed, 4 warnings — same 33 failure count as v1)
- **Failure modes:**
  - G2 (constructor regex) misses **all 8** prod widgets — synthetic `new yl({...})` shape in Plan 05 fixtures does not match real Vite-singlefile minified output
  - G1 (SDK-presence signals) misses **4 of 8** prod widgets — `[ext-apps]` log prefix, `ui/initialize`, `ui/notifications/tool-result`, and `@modelcontextprotocol/ext-apps` literal don't appear in those 4 widgets even though they render in Claude Desktop
  - 3 widgets pass G1 + G3 + connect but fail only G2 → constructor pattern is the universal miss
- **Why the cycle missed it:** Plan 05's fixtures were synthetic models of "what minified Vite output should look like," not samples of actual prod bundles. The RED→GREEN cycle proved the validator handles the synthetic shape, but real prod uses different mangled-id and SDK-loading patterns.
- **Recommended next step:** `/gsd-plan-phase 78 --gaps` — second gap-closure iteration. New plan should:
  1. Fetch a real cost-coach prod widget (e.g. `cost-summary.html`) and replace the synthetic fixtures with actual prod-shape samples
  2. Generalize G1 SDK-detection signals to cover whatever pattern the 4 false-negative widgets use
  3. Generalize G2 constructor regex to match the actual mangled-id pattern in prod
  4. Re-run Test 6 end-to-end against cost-coach prod after the second-cycle fixes land
- **Status:** open (Plan 78-08 stays in-progress; phase 78 stays `gaps_found`)

## Re-verification context

- **Plan 05** (Wave 1) captured 3 bundled fixtures + a RED-phase integration test cluster encoding the cost-coach false-positive class.
- **Plan 06** (Wave 2) replaced the fragile `import.*ext-apps` and `new App\(` regexes with minification-resistant patterns and eliminated the SDK→handler/connect cascade. The 5 RED-phase tests turned GREEN.
- **Plan 07** (Wave 3) added `--widgets-dir <path>` source-scan mode + 3 CLI-boundary integration tests via `assert_cmd`. The fixture-binary skip-gate that blocked v1 binary verification is no longer the only path.
- **Plan 08** (Wave 4 — this plan) extended the property test corpus + bundled-fixture example + READMEs and rewrote this UAT file.

Pre-fix evidence: 8 cost-coach prod widgets produced 33 false-positive failures.
Post-fix expected: 8 cost-coach prod widgets produce zero false-positive failures.
