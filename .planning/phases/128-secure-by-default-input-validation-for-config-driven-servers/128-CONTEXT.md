# Phase 128: Secure-by-default input validation for config-driven servers - Context

**Gathered:** 2026-09-26
**Status:** Ready for planning

<domain>
## Phase Boundary

Make the input contract a config-driven server already *publishes* binding at `tools/call` time.

The toolkit writes `max_length` / `minimum` / `maximum` / `enum` into `inputSchema` and emits
`"additionalProperties": false` (`crates/pmcp-server-toolkit/src/tools.rs:206`), but no layer checks
any of it: `src/server/mod.rs:2590` and `:2820` pass `req.arguments` straight to `handler.handle`, and
`src/server/core.rs` adds no check. This phase delivers D1–D4 (enforce the declared schema, grow the
`ParamDecl` vocabulary, a position-scoped default string cap, and path-injection closure on BOTH HTTP
surfaces), the E1–E3 escape hatches for rules a schema cannot express, and corrects three false
security claims in `tools.rs` that assert this enforcement already exists against named threat IDs.

**In scope:** D1–D4, E1–E3, SC-1..SC-8 from the ROADMAP, and the ~11-crate coordinated release.

**Out of scope:** the per-request total-time budget (CR open question 6 — deferred to its own phase);
auditing OTHER phases' threat records for the same false-mitigation class (deferred); wiring
`validate_input` into `mod.rs:2590`/`:2820` for hand-written `ToolHandler`s (this phase wires
config-driven tools only, but deliberately leaves the entry point where a later phase can).

</domain>

<decisions>
## Implementation Decisions

### Validator home and draft policy

- **D-01:** **Core owns the input-validation entry point.** Generalize
  `src/server/output_validation.rs` into `server::schema_validation` with a public
  `validate_input(...)` that owns MCP input semantics too — missing `arguments` (`null`) treated as
  `{}`, and the already-emitted `additionalProperties: false` honoured rather than re-added. The
  toolkit calls it. Rejected: a toolkit-local second `jsonschema` path (the CR's drafted rollout),
  because it would let a tool's output be checked under one dialect while its input is checked under
  another, and would duplicate the compiled-validator cache (`output_validation.rs:638`), the draft
  pinning (`:592`) and the value-free error renderer. **This moves part of D1 into `pmcp`, which the
  CR's rollout does not account for — it puts only E3 in core.**
  — **Reversibility:** one-way — `validate_input` becomes published `pmcp` public API; withdrawing it
  later is a breaking core change, and the toolkit's call sites would need a replacement validator.

- **D-02:** **Inputs compile under Draft 2020-12 on both eras.** Reuse `compile_2020_12`
  (`output_validation.rs:592`) for inputs regardless of `Era`, rather than routing through
  `compile_for_era`'s two arms (`:607-611`, where `Era::V1` auto-detects from `$schema` and `Era::V2`
  pins). Rationale: outputs froze V1 at auto-detect because there was shipped behaviour to freeze;
  inputs have none — they were never validated — so there is nothing to preserve, and a
  config- or schemars-declared `$schema` must not be able to change input enforcement semantics. This
  inherits the existing warn at `:585` for free.
  — **Reversibility:** costly — changing the input dialect after release changes which calls a
  deployed server refuses.

- **D-03:** **`src/server/validation.rs` — harvest, then deprecate (not delete).** It is a *third*
  dead validation path: `pub mod validation` at `src/server/mod.rs:206`, 11 public validators, a
  `Validator`/`FieldValidator` builder, **zero callers in `src/`**, and `ARCHITECTURE.md:200` claims
  "Server can validate tool inputs before calling handler" — a fourth false enforcement claim, in the
  codebase map rather than in code. Actions: (a) harvest `validate_safe_path` (`:252`) as the basis for
  D4(a), since it is already the right primitive; (b) mark the module `#[deprecated]` **and**
  `#[doc(hidden)]` so it stops advertising itself on docs.rs; (c) correct `ARCHITECTURE.md:200`; (d)
  book removal against the next major. Outright deletion was the user's first choice but removing a
  `pub mod` from a 2.x line is a semver break, and CLAUDE.md's compat philosophy says core stays
  additive when cheap; a `pmcp` 3.0.0 for this phase alone is disproportionate.
  — **Reversibility:** reversible — deprecation is an attribute; the module still compiles.

- **D-04:** **Split core's `validation` feature.** Add `schema-validation = ["dep:jsonschema"]` and
  redefine `validation = ["schema-validation", "dep:garde"]` (today: `Cargo.toml:327`
  `validation = ["dep:jsonschema", "dep:garde"]`). The toolkit's `input-validation` forwards only
  `pmcp/schema-validation`. Without the split, the CR's default-on `input-validation` would pull
  `garde 0.23` into every curated single-call build for E3 — a feature those servers never use, since
  they write no `TypedTool<T>`. This is the same reasoning that kept SWC/JS out of curated builds
  (SC-1). Additive: existing `validation` users and `full` (`Cargo.toml:280`) see no change.
  — **Reversibility:** costly — `schema-validation` becomes a published feature name others may depend
  on; collapsing it back changes their dependency graph.

### D3 — default string cap

- **D-05:** **Position-scoped enforcement, not uniform.** A string parameter in **path or query
  position** gets a hard default cap; a string in **body position** (SQL `:param` binds, script args,
  request bodies) gets a warning only. Position is derivable from config today — path-position means
  the param appears as `{placeholder}`, query-position means it is listed in `query`. This catches both
  UMLS classes (CR-01 injection and CR-02's uncapped filters) without the breakage review note C
  identified: a uniform 256 refuses free-text search params, SQL filter expressions, base64 values and
  notes fields that work today. Rejected: detect-only-now-enforce-in-0.3.0 (leaves CR-02 open for a
  release), uniform 4096 (stops neither class cleanly), uniform 256 as drafted (review note C's
  objection stands).
  — **Reversibility:** costly — tightening body position later refuses calls that worked; loosening
  path position later is safe but the two-behaviour contract is documented and depended on.

- **D-06:** **Strict cap value is 256** for path/query position. In that position it is genuinely
  generous — a CUI, a version token, a code-list value, a UUID all fit with room to spare. Review note
  C's objection to 256 was about free-text body params, which D-05 now exempts, so 256 keeps its teeth
  exactly where it earned them. It also refuses the CR-01 two-placeholder probe (180+200 = 380 chars)
  on this cap alone.

- **D-07:** **Warn teeth:** `ServerConfig::validate` and `cargo pmcp validate deploy`
  (`cargo-pmcp/src/commands/validate.rs`) both warn on an uncapped body-position string; a
  `[server.validation]` strict flag turns the warning into a hard config error. Matches the CR's
  "warns on an uncapped string, and fails in a strict mode". A running server does not refuse to boot
  over this.

- **D-08:** **D4(c) carries its own always-on placeholder cap, independent of D3.** A path segment has
  no legitimate reason to be long, so the length half of the CR-01 fix must not depend on how D3 is
  configured. Without this, setting `default_max_length = 0` would silently disable the
  "Length via two placeholders" acceptance row along with the free-text policy. Costs one more number
  in the config surface; buys a CR-01 fix that holds regardless of D3.

### D4 — path injection depth and ordering

- **D-09:** **Resolve placeholders in `pmcp-code-mode` BEFORE calling `execute_request`, AND export
  `validate_path_placeholder`.** `HttpExecutor::execute_request(method, path, body)`
  (`crates/pmcp-code-mode/src/executor.rs:2425`) receives the path *template*, and
  `HttpCodeExecutor::execute_request` calls `resolve_path` inside its own impl — which is what makes a
  decorator blind by construction. Moving resolution ahead of dispatch makes **every** `HttpExecutor`
  implementor safe by construction, pmcp.run's built-in `openapi-api` server included. **This answers
  CR open question 5 by construction rather than by asking the platform team to adopt.** The exported
  helper additionally serves implementors with a non-OpenAPI template syntax. Breaking for implementors
  that resolve their own paths, hence `pmcp-code-mode` 0.5.4 → **0.6.0**.
  — **Reversibility:** one-way — the `HttpExecutor` contract change is published and breaking;
  reverting it would break every implementor a second time.

- **D-10:** **The character denylist is an unconditional floor.** `?`, `#`, `/`, `..` and
  percent-encoded forms of these are always refused in a placeholder value; a `pattern` declared in the
  OpenAPI spec narrows further **on top**, never replaces. Review note E led with "pattern supersedes
  denylist when declared", but a loose pattern such as `^.*$` would then silently disable the
  injection check, and specs in the wild are full of those.

- **D-11:** **`/` is liftable only by per-parameter config opt-in, never by the spec's
  `allowReserved`** (CR open question 2). The spec is baked into the package (PKG-03) but is still
  third-party content, and `allowReserved` is widely copy-pasted without intent; an operator's explicit
  `[[tools.parameters]]` opt-in is a stronger signal and is visible to the deploy check that reads the
  config. `..` and percent-encoded traversal are refused unconditionally with **no** escape.

- **D-12:** **E1 (`RequestPolicy`) runs BEFORE outgoing auth is applied** (CR open question 4, as the
  CR proposed). The policy sees tool name, method, resolved path, query pairs and body, and never the
  credential. E1 is the hook teams will write the most custom code in, so keeping secrets out of it by
  construction matters more than letting a policy assert on the credential.

### Rollout

- **D-13:** **Release is ~11 crate versions, not the CR's two or review note D's seven.** Measured:
  `pmcp` 2.20.4 → **2.21.0** (D1 entry point, feature split, D-03's deprecation — a minor, since D-03
  deprecates rather than deletes); `pmcp-code-mode` 0.5.4 → **0.6.0** (D-09, breaking);
  `pmcp-code-mode-derive` (pins `pmcp-code-mode = "0.5.0"` at `Cargo.toml:27`; `^0.5` does not admit
  0.6.0); `pmcp-server-toolkit` 0.1.3 → **0.2.0**; and **seven** toolkit consumers, not six — review
  note D names `pmcp-toolkit-{postgres,mysql,athena}`, `pmcp-sql-server`, `pmcp-workbook-server`,
  `pmcp-openapi-server`, and misses **`pmcp-workbook-compiler`** (`Cargo.toml:107` pins the toolkit and
  it has its own publish step at `release.yml:396`). Six pin `^0.1.0` and `pmcp-openapi-server` pins
  `^0.1.2`; neither admits `0.2.0`. Also to repin: root `Cargo.toml:263` (`pmcp-code-mode = "0.5.3"`)
  and `crates/pmcp-server-toolkit/Cargo.toml:23-24` (`pmcp = "2.9.0"` must move to `"2.21"` to *use*
  the new entry point; `pmcp-code-mode = "0.5.3"` → `"0.6"`).
  **The `pmcp` minor needs no other downstream repin** — every `pmcp` pin in the tree is a caret on 2.x
  (`^2.8.0` … `^2.20.3`), so CLAUDE.md's caret exception applies to that half only, NOT to the
  `pmcp-code-mode` or toolkit halves.

- **D-14:** **One tag, `release.yml`'s existing order.** Bump all ~11 in one commit and let the
  workflow publish in its established dependency order; `scripts/check-release-coverage.sh` (D-10 of
  Phase 124) machine-checks that order inside `make quality-gate`, so a prose-vs-workflow mismatch is a
  build failure rather than a discovery. Known risk accepted: one failing crate mid-order strands the
  rest, and crates.io ownership has stranded this repo before (see
  `project_crates_io_ownership_blocks_ci_publish`).
  — **Reversibility:** reversible — a second tag can finish a partial publish.

- **D-15:** **`ParamDecl`'s forward-incompatibility is a rollout note, not a design change.**
  `#[serde(deny_unknown_fields)]` at `crates/pmcp-server-toolkit/src/config.rs:920` means a config
  written against 0.2.0 using `pattern` / `min_length` / `format` / `items` **fails to parse** on
  0.1.3. Must appear in the CHANGELOG and the rollout note.

### Answered without discussion (do not re-litigate)

- **CR open question 3 (schema engine version) is ALREADY ANSWERED in-tree.** Core pins
  `jsonschema = "0.49"` (`Cargo.toml:220`) and the toolkit pins the same
  (`crates/pmcp-server-toolkit/Cargo.toml:54`). The CR's "0.57 misses U+3000" figure is from outside
  this tree and must be re-measured on 0.49 before it drives anything.
- **Review note B (warn-vs-refuse asymmetry) is a deliverable, not a fork.** Inputs hard-refuse (D1);
  outputs only warn (`output_validation::warn_on_schema_mismatch:93-108` logs and never fails, and is a
  no-op without the feature). The asymmetry is correct — inputs are untrusted and pre-backend, outputs
  are the server's own bug and refusing would break a caller who cannot fix it — and **must be written
  down in the docs deliverable**, because a reviewer who knows the output precedent will ask.
- **Review note F / SC-7 (error messages) is locked.** Return the DECLARED expectations, never the
  rejected value and never the rejected key — the key is attacker-controlled and may itself be PHI
  (`{"Jane Doe DOB 1970-01-01": 1}`). Shape: `unknown argument(s): 2; allowed: cui, version`.
- **Feature gate is the toolkit's existing `input-validation`** (`Cargo.toml:102`, zero references in
  `src/`), **not** `openapi-code-mode` — that umbrella would pull SWC/JS into curated single-call
  builds (SC-1).

### Claude's Discretion

None — every gray area presented was decided by the user. Items left to the planner/researcher by
nature rather than by deferral: E2's exact attach-point signature, the `[server.validation]` opt-out
key names beyond those the CR already fixes, the startup-log format for enforced rules and opt-outs,
and how `validate_path_placeholder`'s public signature is shaped.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase source of truth

- `.planning/phases/128-secure-by-default-input-validation-for-config-driven-servers/128-CHANGE-REQUEST.md`
  — the reviewed change request, verbatim snapshot of Claude doc `9mvbYNAzFwviiwbaF2BkBb` rev 12.
  Carries D1–D4, E1–E3, the acceptance-test matrix and the six open questions (**all six are now
  resolved — see the mapping below**). Every claim in it was verified against the tree at `3b2d7baf`.
- `.planning/phases/128-secure-by-default-input-validation-for-config-driven-servers/128-REVIEW-NOTES.md`
  — the measured review: what was CONFIRMED, what was folded into the CR, and open design items A–F.
  **A, C, D and E are resolved by the decisions above; B and F are deliverables.** Its Process note is
  deferred (see `<deferred>`).
- `.planning/ROADMAP.md` § Phase 128 — SC-1..SC-8, the requirement set, and the P0 sub-goal on the
  three false security claims.

### The code this phase changes

- `src/server/output_validation.rs` — the module to generalize. Compiled-validator cache at `:638`,
  `compile_2020_12` at `:592`, era arms at `:607-611`, `warn_on_schema_mismatch` at `:93-108`, and the
  `jsonschema` version measurements across 0.46.10 / 0.47.0 / 0.48.0 / 0.48.5 / 0.49.2.
- `src/server/mod.rs:2590` and `:2820` — where `req.arguments` passes to `handler.handle` unchecked.
  Not wired this phase, but the reason D-01 puts the entry point in core.
- `src/server/validation.rs` + `src/server/mod.rs:206` — the third dead path. `validate_safe_path` at
  `:252` is the D4(a) harvest target.
- `crates/pmcp-server-toolkit/src/tools.rs` — `:15-17`, `:556-557`, `:615` are the three false claims
  (T-83-05-02 / T-90-03-01 / T-90-05-03). `:206` emits `additionalProperties: false` (asserted by
  `:751` and `:1036`). `build_param_property` at `:216-243`.
- `crates/pmcp-server-toolkit/src/config.rs:910-949` — `ParamDecl`, its nine fields and its
  `#[serde(deny_unknown_fields)]`.
- `crates/pmcp-server-toolkit/src/http/client.rs:145-163` — `substitute_path`, the non-Code-Mode half
  of CR-01: `path.replace(&placeholder, &value_str)` with no character check. `render_scalar` at
  `:354-372`. Timeout defaults at `:43-51`, retry loop at `:290`.
- `crates/pmcp-code-mode/src/executor.rs:2425` — the public `HttpExecutor` trait whose signature D-09
  changes.
- `crates/pmcp-server-toolkit/src/code_mode.rs` — `HttpCodeExecutor::execute_request` and its internal
  `resolve_path` / `scalar_str`.
- `crates/pmcp-server-toolkit/tests/script_tool.rs:185` — the test carrying T-90-05-03; asserts
  argument BINDING, not validation.

### Release and quality machinery

- `CLAUDE.md` § Release & Publish Workflow — the publish-order ledger (items 1–17), the Version Bump
  Rules and the caret exception that D-13 leans on.
- `.github/workflows/release.yml` — the AUTHORITY on publish order. `:396` is
  `pmcp-workbook-compiler`'s step; `:291` is `pmcp-server-toolkit`'s.
- `scripts/check-release-coverage.sh` — the gate that machine-checks the order inside
  `make quality-gate`.
- `CLAUDE.md` § ALWAYS Requirements — fuzz, property, unit and `cargo run --example` coverage are
  mandatory for SC-8.

### Project context

- `.planning/codebase/ARCHITECTURE.md:197-201` — the "Validation" subsection carrying the fourth false
  claim that D-03 corrects.
- `.claude/skills/spike-findings-rust-mcp-sdk/SKILL.md` — auto-loaded implementation blueprint,
  including the schema-server toolkit lift this phase hardens.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- **`src/server/output_validation.rs` (2155 lines)** — the single most valuable asset here. It already
  solves compiled-validator caching (`Mutex<HashMap<(Era, String), Result<Arc<Validator>, Arc<str>>>>`
  at `:638`), draft pinning with a warn on a conflicting `$schema` (`:585-592`), value-free error
  rendering, and carries measurements across five `jsonschema` versions. D-01 generalizes it rather
  than growing a parallel validator. Note `compile_for_era` is deliberately split across three
  functions to stay under CI's cognitive-complexity cap — preserve that split.
- **`src/server/validation.rs:252` `validate_safe_path`** — already the right primitive for D4(a)'s
  traversal and reserved-character check. Harvest it (D-03) rather than writing a new one.
- **`crates/pmcp-server-toolkit/src/http/client.rs:354-372` `render_scalar`** — already rejects
  `Object`/`Array` in a path position (WR-03) and already "names the KEY only — never the value"
  (Pitfall 5), which is the error-message convention SC-7 generalizes. D4 extends this function's
  neighbourhood rather than replacing it.
- **`crates/pmcp-server-toolkit/Cargo.toml:102` `input-validation`** — a declared-but-unimplemented
  feature with zero references in `src/`. Implementing D1 behind it retires the dead flag. It survived
  because `make quality-gate`'s `unused-deps` target is a hardcoded no-op (the machete call is
  commented out) — see `project_local_gate_blind_spots` #14.

### Established Patterns

- **Era-aware compilation** (`compile_for_era`, `output_validation.rs:607-611`) — the V1/V2 arm pattern
  exists and D-02 deliberately does NOT use it for inputs. State that explicitly in the
  implementation, or a maintainer will "fix" the asymmetry.
- **Warn-then-enforce feature ramp** — `warn_on_schema_mismatch` is the in-tree precedent for
  observe-without-refusing, and D-05/D-07 reuse its shape for body-position strings.
- **Position-derived behaviour from config** — `operation.path_parameters()` already drives
  `substitute_path`, so path-vs-query-vs-body position is available without new config surface.
- **Feature severance discipline** — SC-1's refusal to gate on `openapi-code-mode`, and D-04's split of
  core's `validation`, are the same rule: a curated single-call build must not pay for an engine or a
  validator it never uses.

### Integration Points

- **Core → toolkit:** the new `server::schema_validation::validate_input` is the seam. The toolkit's
  `pmcp` pin (`Cargo.toml:23`) must move from `"2.9.0"` to `"2.21"` to reach it.
- **`pmcp-code-mode` → its implementors:** D-09 moves placeholder resolution ahead of
  `execute_request`, changing a public trait contract. This is the seam that closes the class for
  pmcp.run's `openapi-api` without a platform change.
- **`cargo pmcp validate deploy`** (`cargo-pmcp/src/commands/validate.rs`) — where D-07's uncapped
  -string warning surfaces to a reviewer before a pmcp.run deploy.
- **Acceptance harness:** the CR's 13-row matrix runs against a wiremock upstream and asserts **zero
  upstream requests** on refusal. The UMLS team can share its probe crate (real config + real spec) as
  a starting point.

</code_context>

<specifics>
## Specific Ideas

- **The acceptance matrix is the spec.** All 13 rows in `128-CHANGE-REQUEST.md` § Acceptance tests must
  pass, each asserting zero upstream requests on refusal and a refusal message containing neither the
  value nor the path. Five are CR-01 probes: three via Code Mode (from the UMLS review) and two on the
  curated single-call path that needs no JS engine.
- **Two acceptance rows are easy to conflate — keep both.** "Missing `arguments` on a zero-parameter
  tool" must be ACCEPTED as `{}`; "missing `arguments` on a tool with required params" must be REFUSED
  by D1, not passed to the backend as `{}`.
- **The startup log is the regression-tracing mechanism.** The CR's risk paragraph leans on it: a
  server whose loose `enum`/`pattern` was never enforced will start refusing calls, and the per-tool
  list of enforced rules plus once-per-opt-out logging is how that gets traced. Do not treat it as
  optional polish.
- **Evidence is real, not hypothetical.** The UMLS MCP server (Triarii) hand-wrote a `ValidatingTool`
  and an `OutboundGuard`; D1 is that `ValidatingTool` moved upstream, and with E1+E2 that server drops
  both wrappers and keeps only a short PHI policy. The platform's own Code Mode safety held throughout
  (a POST was refused, the API key never left the server) — the gap is in parameter shapes, length
  caps and free text in raw query strings.

</specifics>

<deferred>
## Deferred Ideas

- **Per-request total-time budget** (CR open question 6) — its own phase. Scope it against the
  **measured 127 s**, not the review's ~47 s estimate: `timeout_seconds: 30`, `retries: 3`,
  `retry_backoff_ms: 1000` (`http/client.rs:43-51`) with `for attempt in 0..=max_retries` (`:290`) is
  four attempts plus 1+2+4 s of backoff — 4.4× an API Gateway's 29 s limit, and a Code Mode script may
  make 50 such calls.
- **Sweep other phases' threat records for the same false-mitigation class** — the Process note in
  `128-REVIEW-NOTES.md`: `/gsd-secure-phase` passed Phases 83 and 90 with these three mitigations
  recorded as present, so whatever that check does, it did not verify them against the dispatch path.
  Auditing every prior phase's SECURITY.md is its own scope. SC-6 still closes the three instances in
  `tools.rs` and asserts no remaining toolkit comment claims an unimplemented mitigation.
- **Wire `validate_input` into `src/server/mod.rs:2590`/`:2820`** so hand-written `ToolHandler`s get
  the same enforcement. D-01 deliberately puts the entry point in core so a later phase can do this
  without moving code; this phase wires config-driven tools only.
- **Remove `server::validation` at the next major** — D-03 deprecates and hides it now; the removal is
  booked, not done.
- **Re-measure the `jsonschema` `\s`/`\S` U+3000 behaviour on 0.49** — the CR's figure is from 0.57,
  outside this tree, and must not drive anything until re-measured on the pinned version.

</deferred>

---

*Phase: 128-secure-by-default-input-validation-for-config-driven-servers*
*Context gathered: 2026-09-26*
