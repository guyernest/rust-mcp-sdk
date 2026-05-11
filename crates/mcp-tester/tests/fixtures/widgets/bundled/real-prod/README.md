# Real cost-coach prod widget fixtures (cycle 2)

Captured from cost-coach prod (`https://cost-coach.us-west.pmcp.run/mcp` via
local checkout `~/projects/mcp/cost-coach/widget/dist/` at commit `29f46efd`)
on the date recorded in `CAPTURE.md`. Drives
`crates/mcp-tester/tests/app_validator_widgets_real_prod.rs`. These fixtures
are the cycle-2 RED-phase regression set — they MUST FAIL today against
the post-Plan-78-06 validator (cycle-1 G2 regex misses all 6) and MUST
PASS after Plan 78-10 lands the generalized G2 constructor regex.

## Cycle 2 scope: comment-stripper bug fix + G2 regex widening

Per `CAPTURE.md` "Root cause discovered" section: the local fixtures
reproduce the prod re-run's 33 Failed rows EXACTLY (1+8+7+7+7+1+1+1
across the 8 tool→widget mappings). The cycle-1 plans diagnosed the
wrong root cause — the bug isn't "G1 signals don't match prod"; it's
that the validator's `strip_js_comments` regex is not string-literal
aware and destroys ~21 KB of SDK code in cost-over-time and
savings-summary before the G1 regexes ever run. A JS string literal
`"/*.example.com..."` (a CSP frame-src directive value) opens a phantom
block comment that the non-greedy `/\*.*?\*/` regex closes at a real
`*/` license-header banner thousands of bytes later. Everything between
— including `[ext-apps]`, `ui/initialize`, all four handler
member-assignments, and `app.connect()` — gets stripped.

**Cycle 2 deliverables:**
1. Make `strip_js_comments` string-literal aware (block + line comments).
2. Widen G2 constructor regex to accept non-literal `name`/`version`
   values — real prod uses `name:"cost-coach-"+t,version:"1.0.0"` (string
   concatenation), not the cycle-1-assumed `name:"literal"` shape.

The original cycle-2 plan's "add new G1 signals" direction is now
deprecated — the existing 4 G1 signals already match prod once the
comment stripper is fixed.

## Why a separate `real-prod/` subdirectory

The parent directory (`crates/mcp-tester/tests/fixtures/widgets/bundled/`)
contains the cycle-1 synthetic fixtures (`cost_summary_minified.html`,
`cost_over_time_minified.html`, `synthetic_cascade_repro.html`). Those
were modeled from the feedback report and document what the team thought
minified Vite output looked like — they pass the post-Plan-78-06 validator
but did NOT generalize to real prod (33 false positives in the 2026-05-02
re-run).

The `real-prod/` subdirectory contains the actual prod bytes. Both sets
are kept — cycle-1 documents the cycle-1 model; cycle-2 documents the
cycle-2 reality. Each integration test file consumes only the subset it
targets:
  - `app_validator_widgets_bundled.rs` consumes `bundled/*.html`
  - `app_validator_widgets_real_prod.rs` consumes `bundled/real-prod/*.html`

Plan 10 must keep BOTH integration tests green — generalizing G2 without
regressing the cycle-1 synthetic shapes.

## Files (post-Plan-78-10 expected emission)

| Fixture | claude-desktop | standard | chatgpt | What it tests |
|---------|----------------|----------|---------|----------------|
| `cost-summary.html` | PASS (zero Failed) | PASS (zero Warning) | (zero rows) | G2 fix: actual prod constructor shape (G1 already passes locally) |
| `cost-over-time.html` | PASS (zero Failed) | PASS (zero Warning) | (zero rows) | G2 fix: actual prod constructor shape |
| `savings-summary.html` | PASS (zero Failed) | PASS (zero Warning) | (zero rows) | G2 fix; serves 3 prod tools (find_savings_opportunities/find_quick_wins/track_realized_savings) — regression cascade test |
| `tag-coverage.html` | PASS (zero Failed) | PASS (zero Warning) | (zero rows) | G2 fix |
| `connect-account.html` | PASS (zero Failed) | PASS (zero Warning) | (zero rows) | G2 fix |
| `service-sankey.html` | PASS (zero Failed) | PASS (zero Warning) | (zero rows) | G2 fix |

## Provenance

See `CAPTURE.md` in this directory for capture date, source endpoint or
path, SHA-256 + byte size of each fixture, and per-fixture grep evidence.

## Drift detection

Re-run the SHA-256 capture from `CAPTURE.md`. If a fixture has drifted
(cost-coach updated its widget build), re-capture and update `CAPTURE.md`
in the SAME PR — keep the regression set anchored to known prod bytes.
The validator's job is to track real prod, so a fixture drift is a
feature update signal, not noise.
