# Phase 128: Secure-by-default input validation for config-driven servers - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-09-26
**Phase:** 128-secure-by-default-input-validation-for-config-driven-servers
**Areas discussed:** Validator home & draft, D3 string cap default, D4 depth & ordering, 0.2.0 blast radius

**Area selection:** all four presented gray areas were selected (multiSelect).

---

## Validator home & draft

### Q1 — Where should D1's schema-enforcement engine live?

| Option | Description | Selected |
|--------|-------------|----------|
| Core owns the entry point | Generalize `output_validation.rs` into `server::schema_validation` with a public `validate_input(tool, schema, args, era)` owning MCP input semantics (missing `arguments` = `{}`, `additionalProperties` honoured). Toolkit calls it. One cache, one draft policy, one renderer. Costs a `pmcp` minor carrying part of D1; leaves a future phase able to wire `mod.rs:2590`/`:2820`. | ✓ |
| Core exposes primitives only | Core makes `cached_validator(era, schema)` + the error renderer public; toolkit composes them and owns all policy. Minimal new core API, but MCP input semantics stay toolkit-only. | |
| Toolkit-local, as drafted | D1 entirely in `pmcp-server-toolkit` behind `input-validation`, as the CR's rollout describes. Zero core API change, but two jsonschema paths, a duplicate cache and a second renderer. | |

**User's choice:** Core owns the entry point
**Notes:** Consequence flagged and accepted — this moves part of D1 into `pmcp`, which the CR's rollout does not account for (it puts only E3 in core). Drives `pmcp` 2.20.4 → 2.21.0.

### Q2 — Which JSON Schema draft does input validation compile under?

| Option | Description | Selected |
|--------|-------------|----------|
| Always 2020-12 | Inputs compile via `compile_2020_12` on both eras. Outputs froze V1 at `$schema` auto-detect because there was shipped behaviour to freeze; inputs have none. Inherits the existing warn at `:585`. | ✓ |
| Follow the era arms | Reuse `compile_for_era` as-is (V1 auto-detect, V2 pinned). Symmetric with outputs, but a declared `$schema` could change input enforcement on V1. | |
| Mechanism in core, policy at caller | Core keeps the era-symmetric signature; toolkit always passes `Era::V2`. Separates mechanism from policy at the cost of one more per-caller decision. | |

**User's choice:** Always 2020-12
**Notes:** Because this deliberately diverges from the output arms, the implementation must say so explicitly or a maintainer will "fix" the asymmetry.

### Q3 — What happens to `src/server/validation.rs`, the third dead validation path?

| Option | Description | Selected |
|--------|-------------|----------|
| Harvest then delete | Lift `validate_safe_path` (`:252`) as D4(a)'s basis, delete the module, fix `ARCHITECTURE.md:200`. Retires the fourth false enforcement claim alongside the other three. Breaking core API change. | ✓ |
| Deprecate, delete next major | `#[deprecated]` with a pointer to the new entry point, correct ARCHITECTURE.md, removal later. Non-breaking, but two plausible validation homes survive. | |
| Fix the claim only | Correct `ARCHITECTURE.md:200`, leave the module. Smallest diff; ships a documented-but-unwired validator knowingly. | |

**User's choice:** Harvest then delete
**Notes:** Refined in the 0.2.0 blast-radius area (Q1 there) once the semver cost was measured — removing a `pub mod` from a 2.x line is a break. Landed as harvest + `#[deprecated]` + `#[doc(hidden)]` in 2.21.0 with removal booked against the next major. The harvest and the ARCHITECTURE.md correction — the parts the user was actually buying — are unchanged.

### Q4 — How should the feature gate be shaped?

| Option | Description | Selected |
|--------|-------------|----------|
| Split the core flag | Add `schema-validation = ["dep:jsonschema"]`, redefine `validation = ["schema-validation", "dep:garde"]`. Toolkit forwards only `pmcp/schema-validation`. Additive; `garde` stays out of curated builds, consistent with SC-1's reasoning on SWC/JS. | ✓ |
| Forward validation wholesale | Toolkit's `input-validation` forwards `pmcp/validation`. One flag, but `garde 0.23` compiles into every default config-server build. | |
| Entry point behind its own core flag | New core `input-validation = ["dep:jsonschema"]`, independent of `validation`. Naming symmetry, but two jsonschema-bearing flags to tell apart and `full` lists both. | |

**User's choice:** Split the core flag
**Notes:** Context given: core `validation = ["dep:jsonschema", "dep:garde"]` (`Cargo.toml:327`) is not in `default` but is in `full`; toolkit `default = ["code-mode"]` and the CR wants `input-validation` forwarded from `default`.

### Area wrap

**Asked:** More questions about the validator home, or move on?
**User's choice:** Next area

---

## D3 string cap default

### Q1 — How does D3's default string cap ship in 0.2.0?

| Option | Description | Selected |
|--------|-------------|----------|
| Position-scoped: strict on path/query, warn on body | Hard cap on the CR-01 injection surface; warn-only where free text is legitimate (SQL filters, script args, bodies). Catches both UMLS classes without the breakage review note C lists. Two behaviours to document. | ✓ |
| Detect-only now, enforce in 0.3.0 | Review note C's recommendation. Zero chance of breaking a working server, but CR-02 stays open for a release and D4(c) needs its own cap meanwhile. | |
| On everywhere at 4096 | Review note C's fallback. Catches the 5,000-char PHI dump, clears ordinary free text. A compromise that stops neither a determined exfiltrator nor a long legitimate value. | |
| On everywhere at 256, as drafted | The CR as written. Strongest default, but 256 is tuned to UMLS code-list params, not the population of config servers. | |

**User's choice:** Position-scoped: strict on path/query, warn on body
**Notes:** Position confirmed derivable from config today — path position = appears as `{placeholder}`, query position = listed in `query`, body position = SQL `:param` binds, script args, request bodies.

### Q2 — Where does the placeholder length limit in D4(c) come from?

| Option | Description | Selected |
|--------|-------------|----------|
| Own cap, independent of D3 | D4(c) carries its own always-on placeholder cap, so the CR-01 two-placeholder acceptance row passes no matter how D3 ships. One more number in the config surface. | ✓ |
| Reuse D3's cap, fall back to a built-in | Use `default_max_length` when set, a built-in limit when D3 is off or detect-only. | |
| Strictly D3's cap | One number governs both. Turning D3 off silently disables the length half of the CR-01 fix and the acceptance row with it. | |

**User's choice:** Own cap, independent of D3
**Notes:** Raised as a dependency the CR does not call out — D4(c) says "apply the default string cap to placeholders", so a detect-only D3 would have left it with no cap to apply.

### Q3 — Strict default cap on path/query-position strings?

| Option | Description | Selected |
|--------|-------------|----------|
| 256 | The CR's number. Generous in path/query position — CUI, version token, code-list value, UUID all fit. Review note C's objection was about body params, which the scoping now exempts. Refuses the 380-char two-placeholder probe on this cap alone. | ✓ |
| 1024 | Four times the headroom for signed tokens, composite keys, base64 in a path segment. The two-placeholder row would then depend on D4(c)'s own cap. | |
| 512 | Middle ground; still catches the 380-char concatenation directly. | |

**User's choice:** 256

### Q4 — Where does the uncapped-body-string warning have teeth?

| Option | Description | Selected |
|--------|-------------|----------|
| Warn in both, fail only in strict mode | `ServerConfig::validate` and `cargo pmcp validate deploy` warn; a `[server.validation]` strict flag makes it a hard config error. Matches the CR's wording. | ✓ |
| Warn at startup, fail the deploy check | `validate deploy` exits non-zero so an uncapped string cannot reach a deploy unacknowledged. Strongest pressure, but breaks anyone running that verb in CI today. | |
| Warn everywhere, no failure mode | Zero disruption. A warning with no escalation path is the condition that let `input-validation` sit dead in `Cargo.toml:102`. | |

**User's choice:** Warn in both, fail only in strict mode

### Area wrap

**Asked:** Anything still open on the D3 string cap before we leave it?
**User's choice:** Settled

---

## D4 depth & ordering

### Q1 — Trait-level or impl-level? (also decides CR open question 5, platform adoption)

| Option | Description | Selected |
|--------|-------------|----------|
| Resolve before dispatch + export the helper | `pmcp-code-mode` resolves and validates placeholders before `execute_request`, so every implementor is safe by construction including pmcp.run's `openapi-api` — CR Q5 answered by construction. Plus export `validate_path_placeholder` for non-OpenAPI template syntaxes. Breaking → 0.6.0. | ✓ |
| Resolve before dispatch only | Same closure, smallest public surface. An implementor with a non-OpenAPI syntax has no sanctioned validator and will hand-roll one. | |
| Shared helper only | Additive, `pmcp-code-mode` stays 0.5.x. Opt-in, so the class is not closed and CR Q5 becomes a request to the platform team. | |

**User's choice:** Resolve before dispatch + export the helper
**Notes:** Grounded on `executor.rs:2425` — `execute_request(method, path, body)` receives the template, and `HttpCodeExecutor` calls `resolve_path` inside its own impl, which is what makes a decorator blind.

### Q2 — Does a declared spec `pattern` replace D4(a)'s denylist or sit on top?

| Option | Description | Selected |
|--------|-------------|----------|
| Denylist is an unconditional floor | `?`, `#`, `/`, `..` and percent-encoded forms always refused; a pattern narrows further on top. One invariant regardless of spec quality. | ✓ |
| Pattern replaces denylist when declared | Review note E as written. Respects spec-author intent, but `^.*$` silently disables the check and specs in the wild are full of those. | |
| Pattern replaces it only if anchored and bounded | Gets review E's intent without trusting spec quality; costs a pattern-quality check to specify and explain. | |

**User's choice:** Denylist is an unconditional floor
**Notes:** Diverges from review note E's stated recommendation, deliberately.

### Q3 — CR open question 2: should `/` in a placeholder ever be permitted?

| Option | Description | Selected |
|--------|-------------|----------|
| Config opt-in per parameter, never from the spec | A deliberate `[[tools.parameters]]` opt-in lifts `/` for one parameter; the spec's `allowReserved` is ignored. The spec is baked (PKG-03) but still third-party, and `allowReserved` is widely copy-pasted without intent. Exceptions stay visible to the deploy check. | ✓ |
| Always refused, no escape | One rule, no exception surface. A spec legitimately putting a path-like value in a segment cannot be served at all. | |
| `allowReserved: true` lifts it | Standards-faithful, no new config surface, but the lift is driven by spec content rather than an operator decision. | |

**User's choice:** Config opt-in per parameter, never from the spec
**Notes:** `..` and percent-encoded traversal remain refused unconditionally with no escape, under every option.

### Q4 — CR open question 4: E1 before or after outgoing auth?

| Option | Description | Selected |
|--------|-------------|----------|
| Before auth, as proposed | Policy sees tool name, method, resolved path, query pairs, body — never the credential. Keeps secrets out of the hook teams write the most custom code in. | ✓ |
| After auth, credentials redacted | Policy runs on the request as it will be sent, auth present but redacted. Redaction becomes a security-critical path. | |
| Before auth, plus an auth-applied observer | Refusal before auth; a read-only post-auth observation point. Two hooks where the CR names one. | |

**User's choice:** Before auth, as proposed

---

## 0.2.0 blast radius

### Q1 — How should "harvest then delete" actually land, given the semver cost?

| Option | Description | Selected |
|--------|-------------|----------|
| Harvest + `#[deprecated]` + `#[doc(hidden)]` in 2.21.0 | Harvest `validate_safe_path` now, stop the module advertising itself on docs.rs, fix `ARCHITECTURE.md:200`, book removal against the next major. Keeps the intent without a semver violation or a 3.0.0 this phase does not need. | ✓ |
| Delete it in 2.21.0 anyway | Minor bump accepting the break — internal-org audience, nothing in-workspace calls it, prior deliberate breaking windows exist. An external caller's build breaks on a minor. | |
| pmcp 3.0.0 | Correct major for a clean removal, but disproportionate — drags every consumer pin for a validation phase with no other 3.0 content. | |

**User's choice:** Harvest + `#[deprecated]` + `#[doc(hidden)]` in 2.21.0
**Notes:** Re-opened deliberately after measuring that removing a `pub mod` from a 2.x line is breaking, which the earlier "harvest then delete" answer had not priced in. CLAUDE.md's compat philosophy ("toolkit/core: stay additive when cheap") applies.

### Q2 — How does the ~11-crate set reach crates.io?

| Option | Description | Selected |
|--------|-------------|----------|
| One tag, release.yml's existing order | Bump all ~11 in one commit, one v-tag, established dependency order; `check-release-coverage.sh` machine-checks it inside `make quality-gate`. One failing crate mid-order strands the rest. | ✓ |
| Core first, toolkit tree follows | Two tags, each half independently green before the next. Toolkit sits on an unreleased core in-tree in between. | |
| One tag, toolkit tree gated on a green core | Single cycle with an explicit failure boundary, but new conditional logic `check-release-coverage.sh` does not model. | |

**User's choice:** One tag, release.yml's existing order
**Notes:** Measured blast radius presented: `pmcp` 2.20.4→2.21.0, `pmcp-code-mode` 0.5.4→0.6.0, `pmcp-code-mode-derive`, `pmcp-server-toolkit` 0.1.3→0.2.0, and **seven** toolkit consumers — one more than review note D's six, because `pmcp-workbook-compiler` (`Cargo.toml:107`, publish step `release.yml:396`) was missed. Risk accepted: crates.io ownership has stranded this repo before.

### Q3 — CR open question 6: is the ~127 s timeout budget in this phase or its own?

| Option | Description | Selected |
|--------|-------------|----------|
| Its own phase | This phase is about what a caller may send; the timeout budget is about what the server spends — a different failure mode with a different acceptance matrix. The CR itself says it "may belong in a separate request". | ✓ |
| Fold in as a capped default | One PR touches `http/client.rs` once. Widens an already ~11-crate phase and changing retry behaviour breaks servers relying on four attempts. | |
| Detection only, in scope | No behaviour change; warn when timeout × retries × backoff exceeds a threshold, mirroring D3's detect-then-enforce shape. | |

**User's choice:** Its own phase
**Notes:** Deferred with the measured figure (127 s), not the review's ~47 s estimate.

---

## Final check

**Asked:** Anything else before writing CONTEXT.md? (offering the `/gsd-secure-phase` audit sweep as a possible in-scope addition)
**User's choice:** Ready for context — the secure-phase audit is captured as a deferred idea.

## Claude's Discretion

None. Every gray area presented was decided by the user; no "you decide" option was taken. Items left to the planner or researcher by nature rather than by deferral: E2's attach-point signature, `[server.validation]` opt-out key names beyond those the CR fixes, the startup-log format for enforced rules and opt-outs, and `validate_path_placeholder`'s public signature.

## Deferred Ideas

- Per-request total-time budget (CR Q6) — own phase, scoped against the measured 127 s.
- Sweep other phases' threat records for the same false-mitigation class (the review notes' Process note).
- Wire `validate_input` into `src/server/mod.rs:2590`/`:2820` for hand-written `ToolHandler`s.
- Remove `server::validation` at the next major (booked by the deprecation, not done).
- Re-measure `jsonschema`'s `\s`/`\S` U+3000 behaviour on the pinned 0.49.
