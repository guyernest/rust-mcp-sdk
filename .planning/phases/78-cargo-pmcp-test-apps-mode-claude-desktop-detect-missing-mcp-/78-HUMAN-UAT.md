---
status: re-verify
phase: 78-cargo-pmcp-test-apps-mode-claude-desktop-detect-missing-mcp-
source: [78-VERIFICATION.md, 78-05-PLAN.md, 78-06-PLAN.md, 78-07-PLAN.md]
started: 2026-05-02T00:00:00Z
updated: 2026-05-02T12:00:00Z
gap_closure_landed: 2026-05-02
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

result: [pending]

## Summary

total: 6
passed: 0
issues: 0
pending: 6
skipped: 0
blocked: 0

## Gaps

(none yet — populated when human re-verification reveals issues)

## Re-verification context

- **Plan 05** (Wave 1) captured 3 bundled fixtures + a RED-phase integration test cluster encoding the cost-coach false-positive class.
- **Plan 06** (Wave 2) replaced the fragile `import.*ext-apps` and `new App\(` regexes with minification-resistant patterns and eliminated the SDK→handler/connect cascade. The 5 RED-phase tests turned GREEN.
- **Plan 07** (Wave 3) added `--widgets-dir <path>` source-scan mode + 3 CLI-boundary integration tests via `assert_cmd`. The fixture-binary skip-gate that blocked v1 binary verification is no longer the only path.
- **Plan 08** (Wave 4 — this plan) extended the property test corpus + bundled-fixture example + READMEs and rewrote this UAT file.

Pre-fix evidence: 8 cost-coach prod widgets produced 33 false-positive failures.
Post-fix expected: 8 cost-coach prod widgets produce zero false-positive failures.
