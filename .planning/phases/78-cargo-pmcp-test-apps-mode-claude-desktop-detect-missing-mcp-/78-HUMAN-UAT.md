---
status: partial
phase: 78-cargo-pmcp-test-apps-mode-claude-desktop-detect-missing-mcp-
source: [78-VERIFICATION.md]
started: 2026-05-02T00:00:00Z
updated: 2026-05-02T00:00:00Z
---

## Current Test

[awaiting human testing]

## Tests

### 1. AC-78-1 at CLI binary boundary — broken widget fails claude-desktop mode
expected: Run `cargo pmcp test apps --mode claude-desktop` against a real MCP server serving a broken widget (no `@modelcontextprotocol/ext-apps` import, no `new App({})`, no protocol handlers, no `app.connect()`). Process exits non-zero AND stdout/stderr names at least one missing handler (e.g. `onteardown`).
result: [pending]

### 2. AC-78-2 at CLI binary boundary — corrected widget passes claude-desktop mode
expected: Run `cargo pmcp test apps --mode claude-desktop` against the same server but serving the corrected widget (per `crates/mcp-tester/tests/fixtures/widgets/corrected_minimal.html`). Process exits zero.
result: [pending]

### 3. AC-78-3 at CLI binary boundary — Standard mode is permissive
expected: Run `cargo pmcp test apps` (no `--mode` flag) against BOTH the broken and corrected widgets. Both invocations exit zero — no regression for the permissive default.
result: [pending]

### 4. AC-78-4 at CLI binary boundary — chatgpt mode is unchanged
expected: Run `cargo pmcp test apps --mode chatgpt` against both fixtures. Both exit zero AND stderr/stdout MUST NOT contain any of the four protocol handler names — chatgpt mode is a no-op for widget validation.
result: [pending]

### 5. AC-78-5 — UX review of READMEs and `--help` text
expected: Visual review of `cargo pmcp test apps --help` and both READMEs (`cargo-pmcp/README.md`, `crates/mcp-tester/README.md`). Reader can answer "what does --mode claude-desktop do, when should I use it, and what does it check?" from the docs alone.
result: [pending]

## Summary

total: 5
passed: 0
issues: 0
pending: 5
skipped: 0
blocked: 0

## Gaps

(none yet — populated when human testing reveals issues)
