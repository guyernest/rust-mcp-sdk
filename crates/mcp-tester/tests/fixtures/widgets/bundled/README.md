# Bundled widget fixtures (Vite singlefile shape)

Captured/synthesized from cost-coach prod evidence (feedback dated 2026-05-02:
`/Users/guy/projects/mcp/cost-coach/drafts/feedback-pmcp-test-apps-v1-false-positives.md`).
Drives `crates/mcp-tester/tests/app_validator_widgets_bundled.rs`. These fixtures
expose the v1 false-positive class — they MUST FAIL with v1 patterns and PASS
after gap-closure plan 06 lands G1+G2+G3.

## Files

| Fixture | claude-desktop (post-fix) | standard (post-fix) | chatgpt | Tests the |
|---------|---------------------------|---------------------|---------|-----------|
| `cost_summary_minified.html` | PASS (zero Failed) | PASS (zero Warning) | (zero rows) | G1 (log prefix + method strings) + G2 (mangled `yl` constructor) |
| `cost_over_time_minified.html` | PASS (zero Failed) | PASS (zero Warning) | (zero rows) | G2 across mangled-id variance (`gl` vs `yl`) |
| `synthetic_cascade_repro.html` | PARTIAL (SDK Failed + constructor Failed; handlers + connect + ontoolresult Passed) | WARN (1 summary listing SDK + constructor missing; handlers, connect, ontoolresult present) | (zero rows) | G3 (handlers, connect, AND ontoolresult detected independently of `has_sdk`) |

## Comment hygiene (REVISION HIGH-3 from Plan 78-01)

Fixture comments MUST NOT contain any of these literals:
`@modelcontextprotocol/ext-apps`, `new App(`, `onteardown`, `ontoolinput`,
`ontoolcancelled`, `onerror`, `ontoolresult`, `app.connect`, `connect()`,
`[ext-apps]`, `ui/initialize`, `ui/notifications/tool-result`.

Validate before adding new fixtures:

```sh
! grep -E '(//|/\*|<!--).*(@modelcontextprotocol/ext-apps|new App\(|onteardown|ontoolinput|ontoolcancelled|onerror|ontoolresult|app\.connect|connect\(\)|\[ext-apps\]|ui/initialize|ui/notifications/tool-result)' <fixture>
```

The grep must exit 0 (zero matches).

## Why synthesized rather than copied verbatim

Cost-coach's full minified bundles are ~50 KB each and contain Vite runtime
glue irrelevant to validator behavior. The synthesized payloads above
preserve the load-bearing signals (mangled constructor, intact `{name,
version}` payload, member-name handler assignments, `[ext-apps]` log
prefix, JSON-RPC method strings) without the noise. If a future false
positive class shows up that requires the actual bytes, capture from
cost-coach prod into a new fixture and add a row to the table above.

## Cycle 2 update — real-prod fixtures live in `real-prod/`

Cycle 1 (Plans 78-05/06/07/08) validated against the synthetic fixtures
above and shipped Plan 78-06's G1+G2+G3 fix. The 2026-05-02 cost-coach
prod re-run reported 33 Failed rows — the synthetic shapes did not
generalize. Cycle 2 (Plans 78-09/10/11) ADDS `real-prod/` with bytes
captured from a local cost-coach checkout (`~/projects/mcp/cost-coach/widget/dist/`
at commit `29f46efd`). See `real-prod/README.md` and `real-prod/CAPTURE.md`
for capture provenance.

The cycle-1 synthetic fixtures in this directory are PRESERVED untouched
— they document what the team thought prod looked like, and the
integration test `app_validator_widgets_bundled.rs` continues to assert
against them. Plan 78-10 must not regress those tests when generalizing
the regexes.
