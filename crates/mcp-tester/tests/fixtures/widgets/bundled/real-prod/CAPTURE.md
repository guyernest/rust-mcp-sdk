# Real cost-coach prod widget capture — Phase 78 cycle 2

## Capture date
2026-05-02 (operator timezone: PT)

## Capture path used
**Path A** — local cost-coach checkout (`~/projects/mcp/cost-coach`)

## Source endpoint or path
`/Users/guy/projects/mcp/cost-coach/widget/dist/` (local checkout, last `npm run build` output)

## Cost-coach commit (Path A only)
- HEAD: `29f46efd929df486e0b01984aa333027486ff66b`
- Subject: `fix(tools,demo): align date defaults with documented tool descriptions`

## Per-fixture provenance

| File | SHA-256 | Bytes | Source |
|---|---|---|---|
| cost-summary.html | `e6368274313d0a941dd3b9f548217f9bd28aa150609339e7dd705aebf2e43e55` | 488838 | verbatim from cost-coach `widget/dist/cost-summary.html` @ 29f46efd |
| cost-over-time.html | `d585a69564daf97e3293fd553aab28a355bdcece85a717df4ed9a24d29c339aa` | 507371 | verbatim from cost-coach `widget/dist/cost-over-time.html` @ 29f46efd |
| savings-summary.html | `0f75e778afb20621981f4ce607739505078a72b41294738f43b99c45bbf95b10` | 486846 | verbatim from cost-coach `widget/dist/savings-summary.html` @ 29f46efd |
| tag-coverage.html | `3e8b6879ec0ed46eda2530be47cdb7a6481df93b43c508fd41f2f4536005e0c8` | 358822 | verbatim from cost-coach `widget/dist/tag-coverage.html` @ 29f46efd |
| connect-account.html | `4d182fcf3fd901977eedecbe7d19318cc8489a824c4d6ade7ed9428236b57077` | 346887 | verbatim from cost-coach `widget/dist/connect-account.html` @ 29f46efd |
| service-sankey.html | `7000b072e9d412e177b123c791ca645bd406eeb169cce2b8ea297ab2ccb713c3` | 374804 | verbatim from cost-coach `widget/dist/service-sankey.html` @ 29f46efd |

## Per-fixture grep evidence

For each fixture, recording cycle-1 pattern hit/miss + broader probes that Plan 78-10 will use to derive the cycle-2 generalized regexes.

### cost-summary.html

```
G1.a @modelcontextprotocol/ext-apps:    0   (cycle-1 import literal — MISS, expected: Vite singlefile inlines the import)
G1.b [ext-apps] log prefix:             1   (cycle-1 — HIT)
G1.c ui/initialize:                     1   (cycle-1 — HIT)
G1.d ui/notifications/tool-result:      2   (cycle-1 — HIT)
G2 cycle-1 regex (id<=5 + name+version): 0  (cycle-1 G2 — MISS, the universal miss this cycle 2 fixes)
G2 broader probe `new <id>(`:           new _o(, new $o(, new $v(, new ae(, new af(, new Array(, new Bh(, new bo(, new bu(, new c(  (10+ unique mangled identifiers — id length varies)
.onteardown=:                           1   (handler — HIT)
.connect(:                              1   (connect — HIT)
```

### cost-over-time.html

```
G1.a @modelcontextprotocol/ext-apps:    0
G1.b [ext-apps] log prefix:             1
G1.c ui/initialize:                     1
G1.d ui/notifications/tool-result:      2
G2 cycle-1 regex:                       0
G2 broader probe:                       new _n(, new _o(, new $o(, new $x(, new Ah(, new Array(, new B(, new Ba(, new Bh(, new bn(  (10+ unique mangled identifiers)
.onteardown=:                           1
.connect(:                              1
```

### savings-summary.html

```
G1.a @modelcontextprotocol/ext-apps:    0
G1.b [ext-apps] log prefix:             1
G1.c ui/initialize:                     1
G1.d ui/notifications/tool-result:      2
G2 cycle-1 regex:                       0
G2 broader probe:                       new _f(, new $f(, new $o(, new Aa(, new an(, new ao(, new Array(, new bf(, new bx(, new c(
.onteardown=:                           1
.connect(:                              1
```

### tag-coverage.html

```
G1.a @modelcontextprotocol/ext-apps:    0
G1.b [ext-apps] log prefix:             1
G1.c ui/initialize:                     1
G1.d ui/notifications/tool-result:      2
G2 cycle-1 regex:                       0
G2 broader probe:                       new _i(, new $i(, new $r(, new Ac(, new Array(, new at(, new Bc(, new br(, new c(, new Cc(
.onteardown=:                           1
.connect(:                              1
```

### connect-account.html

```
G1.a @modelcontextprotocol/ext-apps:    0
G1.b [ext-apps] log prefix:             1
G1.c ui/initialize:                     1
G1.d ui/notifications/tool-result:      2
G2 cycle-1 regex:                       0
G2 broader probe:                       new _i(, new $r(, new Aa(, new Ac(, new ar(, new Array(, new at(, new bh(, new c(, new Cc(
.onteardown=:                           1
.connect(:                              1
```

### service-sankey.html

```
G1.a @modelcontextprotocol/ext-apps:    0
G1.b [ext-apps] log prefix:             1
G1.c ui/initialize:                     1
G1.d ui/notifications/tool-result:      2
G2 cycle-1 regex:                       0
G2 broader probe:                       new $t(, new ai(, new Ar(, new Array(, new Au(, new be(, new Br(, new Bu(, new c(, new ci(
.onteardown=:                           1
.connect(:                              1
```

## Validator results against captured fixtures (matches uat-evidence prod re-run exactly)

After capture, the 6 fixtures were run through the existing post-Plan-06 validator. Per-fixture results:

| Fixture | Tool | P | W | Failed rows |
|---|---|---|---|---|
| cost-summary.html | get_spend_summary | 7 | 0 | **1** (App constructor) |
| cost-over-time.html | get_spend_over_time | 0 | 1 | **8** (full cascade: SDK + constructor + 4 handlers + connect + chatgpt-only-channels) |
| savings-summary.html | find_savings_opportunities | 0 | 1 | **7** (full cascade) |
| tag-coverage.html | assess_tag_strategy | 7 | 0 | **1** (App constructor) |
| connect-account.html | connect_aws_account | 7 | 0 | **1** (App constructor) |
| service-sankey.html | get_service_breakdown | 7 | 0 | **1** (App constructor) |

Multiplied across the 8 prod tool→widget mappings (savings-summary serves 3 tools): 1+8+7+7+7+1+1+1 = **33 Failed rows** — IDENTICAL to the 2026-05-02 prod re-run reported total. The local fixtures reproduce the prod cascade exactly.

## Root cause discovered: `strip_js_comments` is not string-literal aware

Step-by-step probe of cost-over-time through the validator's pipeline:

```
Raw script body:        505,381 bytes,  2 [ext-apps] hits
After HTML comment strip: 505,381 bytes, 2 [ext-apps] hits
After block comment strip: 484,064 bytes, 0 [ext-apps] hits  ← BUG
After line comment strip:  383,745 bytes, 0 [ext-apps] hits
```

The block-comment-stripping regex `/\*.*?\*/` (non-greedy with dotall) destroys 21 KB of SDK code in cost-over-time. Why: the bundle contains a JS string literal — a CSP `frame-src` directive value — that begins with `"/*.example.com..."`. The regex sees that `/*` as a block-comment opener and matches the next `*/` it finds, which is the `*/` at the end of an `@kurkle/color v0.3.4` license-header banner thousands of bytes later. Everything between — including the entire MCP Apps SDK runtime with `[ext-apps]`, `ui/initialize`, `ui/notifications/tool-result`, all 4 handler member-assignments, AND `app.connect()` — gets stripped before the regex match runs.

This explains the **exact** uat-evidence per-widget breakdown:
- cost-over-time / savings-summary contain the `"/*.example.com..."` CSP string → SDK stripped → G1 + handlers + connect all fail → cascade
- cost-summary / tag-coverage / connect-account / service-sankey don't have that specific CSP string positioning → SDK preserved → only G2 (constructor regex shape) fails

The validator's docstring already noted the line-comment limitation ("a `//` inside a JS string literal will be stripped along with the rest of the line") but didn't anticipate the more dangerous block-comment failure mode where `/*` inside a string consumes thousands of bytes of code.

## What cycle 2 actually needs to fix

| Bug | Symptom | Cycle-2 fix |
|---|---|---|
| `strip_js_comments` is not string-literal aware (block comments) | 4 of 6 widgets show full cascade because SDK code is stripped before regex match | Make block-comment stripping respect `'…'`, `"…"`, and `` `…` `` string literal contexts. (Line comments have the same vulnerability — fix both.) |
| G2 cycle-1 regex `name\s*:\s*"[^"]+"\s*,\s*version\s*:\s*"[^"]+"` requires literal string values | All 8 widgets fail G2 because real prod uses `name:"cost-coach-"+t,version:"1.0.0"` (string concatenation, not literal) | Widen G2 regex to accept any expression after `name:` and `version:` — e.g. `\{[^}]{0,200}\bname\s*:[^,}]{0,100}\bversion\s*:` |

The "add new G1 signals" approach in the original cycle-2 plan body is the wrong fix — the existing 4 G1 signals already match prod once the comment stripper is fixed. Cycle 2 scope is now: **(a) fix `strip_js_comments` string-literal handling, (b) widen G2 regex to accept non-literal name/version values**.

Plan 78-09's RED tests bind correctly: all 6 real-prod tests fail today (matching 33-failure prod cascade) and will turn GREEN after Plan 78-10 lands both fixes.

## Why each fixture is in the regression set

- **cost-summary.html** — only G2 fails per uat-evidence (and locally); tells us the constructor shape is the universal miss
- **cost-over-time.html** — both G1 and G2 fail per uat-evidence; locally only G2 fails (see fidelity discrepancy above)
- **savings-summary.html** — same as cost-over-time, plus serves 3 tools so prod regression is amplified 3×
- **tag-coverage.html / connect-account.html / service-sankey.html** — only G2 fails per uat-evidence (and locally); together with cost-summary they prove G2 is universally wrong

## Drift-detection re-verify recipe

To confirm fixtures still match this capture after time passes:
```sh
cd /Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/mcp-tester/tests/fixtures/widgets/bundled/real-prod
shasum -a 256 *.html
```

Compare against the SHA-256 column above. If any drift, re-capture and update CAPTURE.md.

## Sanity check at capture time

```sh
$ cd /Users/guy/Development/mcp/sdk/rust-mcp-sdk/crates/mcp-tester/tests/fixtures/widgets/bundled/real-prod
$ for f in *.html; do test -s "$f" && grep -q '<script' "$f" && echo "OK: $f"; done
OK: connect-account.html
OK: cost-over-time.html
OK: cost-summary.html
OK: savings-summary.html
OK: service-sankey.html
OK: tag-coverage.html
```

All 6 fixtures present, non-empty, contain `<script>` — passed Plan 78-09 Task 1 acceptance line 251-260.
