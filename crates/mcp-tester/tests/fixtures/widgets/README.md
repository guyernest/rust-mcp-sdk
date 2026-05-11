# Widget HTML fixtures

These fixtures drive `AppValidator::validate_widgets` integration tests
(`crates/mcp-tester/tests/app_validator_widgets.rs`) and the
`validate_widget_pair` example.

## Files

| Fixture | claude-desktop | standard | chatgpt | Purpose |
|---------|----------------|----------|---------|---------|
| `broken_no_sdk.html` | FAIL (multiple Failed rows) | WARN (1 summary row) | (zero rows — REVISION HIGH-1) | Cost Coach reproducer shape: uses `window.openai`; no `@modelcontextprotocol/ext-apps` import; no `new App({...})`; no handlers. |
| `broken_no_handlers.html` | FAIL (4+ Failed rows, one per missing handler) | WARN (1 summary row) | (zero rows) | Has SDK import + `new App({...})` but NO handlers and NO `connect()`. |
| `corrected_minimal.html` | PASS (zero Failed) | PASS (zero Warning) | (zero rows) | Minimal valid widget per `src/server/mcp_apps/GUIDE.md` §"Minimal widget example". |

## Fixture comment hygiene (REVISION HIGH-3)

Fixture comments MUST NOT contain the literals `@modelcontextprotocol/ext-apps`,
`new App(`, `onteardown`, `ontoolinput`, `ontoolcancelled`, `onerror`,
`app.connect`, or `connect()`. Plan 01's `strip_js_comments` is the
load-bearing scanner correctness fix; this fixture-cleanup is belt-and-braces.

If you add a new fixture, run:

```sh
! grep -E '(//|/\*|<!--).*(@modelcontextprotocol/ext-apps|new App\(|onteardown|ontoolinput|ontoolcancelled|onerror)' your-fixture.html
```

The grep must exit 0 (zero matches) for the fixture to be acceptable.

## Deferred: `corrected_minified.html`

Per RESEARCH Open Question 1 RESOLVED: empirical Vite-build verification is
moved to a follow-up phase if scanner false-negatives are observed in the
wild.
