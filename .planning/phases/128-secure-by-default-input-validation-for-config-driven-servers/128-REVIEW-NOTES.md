# Phase 128 — review notes on the change request

Review of `128-CHANGE-REQUEST.md`, performed 2026-09-26 against the tree at commit `3b2d7baf`
(branch `fix/oauth-discovery-optional-fields`). Every claim below was measured, not inferred; each
carries the `file:line` it was measured at so a planner can re-check it.

Findings 1–3 and two factual corrections are already folded INTO the change request. The remaining
findings are recorded here because they change the design but were left out of the CR, which is the
external-facing artifact. **`/gsd-discuss-phase 128` should resolve items A–E before planning.**

## Verified as stated in the CR

| CR claim | Verified at | Verdict |
| --- | --- | --- |
| No layer enforces `inputSchema` at `tools/call` | `src/server/mod.rs:2590`, `:2820` pass `req.arguments` straight to `handler.handle`; `src/server/core.rs` has no input check | CONFIRMED |
| `garde` is never called | zero matches for `garde` in `src/`; `Cargo.toml:327` `validation = ["dep:jsonschema", "dep:garde"]` | CONFIRMED |
| `ParamDecl` lacks `pattern` / `min_length` | `crates/pmcp-server-toolkit/src/config.rs:921-949` — only name, type, description, required, default, max_length, minimum, maximum, enum | CONFIRMED |
| `build_param_property` emits only a subset | `crates/pmcp-server-toolkit/src/tools.rs:216-243` | CONFIRMED |
| A Code Mode `HttpExecutor` wrapper sees only the template | `HttpCodeExecutor::execute_request` calls `Self::resolve_path(path, &body)` at step (1) INSIDE the impl (`code_mode.rs:~975`); `scalar_str` (`:~950`) rejects only non-scalars | CONFIRMED |
| Versions | `pmcp` 2.20.4, `pmcp-server-toolkit` 0.1.3, `pmcp-code-mode` 0.5.4, `pmcp-openapi-server` 0.1.2 | CONFIRMED |

## Folded into the CR

1. **CR-01's class also exists on the curated single-call path.** `HttpClient::substitute_path`
   (`http/client.rs:150-163`) does `path.replace(&placeholder, &value_str)` with no character check and
   no percent-encoding; `render_scalar` (`:354-372`) returns any `String` verbatim. D4 was scoped to
   `code_mode.rs` only, which would have left the default JS-engine-free build exposed.
2. **Three documented mitigations do not exist** (`tools.rs:15-17`, `:556-557`, `:615`, against
   T-83-05-02 / T-90-03-01 / T-90-05-03). The test carrying T-90-05-03
   (`crates/pmcp-server-toolkit/tests/script_tool.rs:185`) asserts argument binding only.
3. **Feature flag.** `input-validation = ["dep:jsonschema"]` exists at
   `crates/pmcp-server-toolkit/Cargo.toml:102` with zero references in `src/`. The CR's proposed
   `openapi-code-mode` gate would drag SWC/JS into curated builds.
   - Why a dead optional dep survived: `make quality-gate`'s `unused-deps` target is a hardcoded
     no-op (the machete call is commented out), so nothing flags it.
4. `additionalProperties: false` IS already emitted (`tools.rs:206`, asserted by `tools.rs:751` and
   `:1036`) — D1 must honour it, not add it.
5. The timeout budget is ~**127 s**, not 47 s: `timeout_seconds: 30`, `retries: 3`,
   `retry_backoff_ms: 1000` (`http/client.rs:43-51`) with `for attempt in 0..=max_retries` (`:290`).

## Open design items for /gsd-discuss-phase (NOT in the CR)

### A. Reuse core's existing validator instead of a second jsonschema path

`src/server/output_validation.rs` (2155 lines) already solves compiled-validator caching
(`Mutex<HashMap<(Era, String), Result<Arc<jsonschema::Validator>, Arc<str>>>>` at `:638`), draft
pinning (`draft202012::new` at `:592`, era arms at `:607-611`) and value-free error rendering, with
measurements recorded across `jsonschema` 0.46.10 / 0.47.0 / 0.48.0 / 0.48.5 / 0.49.2. Core and the
toolkit both pin **0.49**.

Two independent jsonschema paths in one server means a tool's output could be checked under Draft
2020-12 while its input is checked under `$schema` auto-detect. **Recommendation:** generalize that
module (e.g. `server::schema_validation`) with a validate-input entry point and have the toolkit call
it, rather than growing a parallel validator. This moves part of D1 into `pmcp`, which the CR's
rollout does not account for (it puts only E3 in core).

This also **answers CR open question 3**: the pin exists and is measured. The CR's "0.57 misses
U+3000" figure is from outside this tree and must be re-measured on 0.49 before it drives anything.

### B. Warn-vs-refuse asymmetry must be stated

`output_validation::warn_on_schema_mismatch` (`:93-108`) **logs a warning and never fails**, and is a
no-op without the `validation` feature. D1 proposes hard refusal, default on. The asymmetry is
defensible — inputs are untrusted and pre-backend; outputs are the server's own bug and refusing would
break a caller who cannot fix it — but it should be written down, because a reviewer who knows the
output precedent will ask.

### C. D3's default-on 256 cap breaks working servers

The CR says "existing servers that already send valid arguments see no change." That does not follow
from "these args were never checked." A 256-char cap on every uncapped string refuses calls that work
today: free-text search params, SQL filter expressions, base64 values, description/notes fields.

**Recommendation:** ship *detection* on by default (`ServerConfig::validate` + `cargo pmcp validate
deploy` warn on uncapped strings) and the *cap* off in 0.2.0, flipping it on in 0.3.0 after a release
of warnings. If it must ship on, use a much higher default (~4096) that still catches the 5,000-char
PHI-dump class without refusing ordinary free text — 256 is tuned to UMLS code-list params, not to the
population of config servers.

### D. Rollout is seven crates, not two

`0.1.3 → 0.2.0` is semver-INCOMPATIBLE on a 0.x line, so per CLAUDE.md's Version Bump Rules every
crate pinning the toolkit moves as one set. Per the release ledger those are items 6–9b:
`pmcp-toolkit-postgres`, `pmcp-toolkit-mysql`, `pmcp-toolkit-athena`, `pmcp-sql-server`,
`pmcp-workbook-server`, `pmcp-openapi-server`. The CR names only `pmcp-openapi-server`.
`scripts/check-release-coverage.sh` must stay green.

Also: `ParamDecl` carries `#[serde(deny_unknown_fields)]`, so a config written against 0.2.0 using
`pattern` FAILS TO PARSE on 0.1.3 — forward-incompatible, and worth a rollout note.

### E. D4 ordering, and the trait-level alternative

- **Prefer the allowlist.** D4(b) (validate against the spec's declared `pattern`) is the strong
  primitive; D4(a)'s character denylist should be the fallback when no pattern is declared. The CR
  leads with the denylist.
- **Trait-level vs impl-level.** `HttpExecutor` is a PUBLIC trait
  (`crates/pmcp-code-mode/src/executor.rs:2425`) whose `execute_request(method, path, body)` signature
  is what makes a decorator blind. Fixing only the SDK's impls leaves every other implementor exposed —
  including pmcp.run's built-in `openapi-api` server, which is CR open question 5. Two options:
  (i) resolve placeholders in `pmcp-code-mode` before calling `execute_request`, so every implementor
  is safe by construction (breaking for implementors that resolve themselves, so `pmcp-code-mode`
  0.6.0); or (ii) ship a shared `validate_path_placeholder` helper the SDK calls and third parties are
  documented to call (additive, but opt-in, so it does not close the class). The CR implicitly picks
  (ii) by scoping D4 to two files. **Name the choice explicitly in the plan.**

### F. Error-message policy — suppress values, not the shape of the problem

Not echoing values is right and matches the in-tree rule (`scalar_str` "names the KEY only — never the
value", Pitfall 5). But the caller is usually a model, and a refusal it cannot act on becomes a retry
loop. The rejected KEY is attacker-controlled and may itself be PHI (`{"Jane Doe DOB 1970-01-01": 1}`),
so the resolution is to return the DECLARED expectations, never the rejected input: e.g.
"unknown argument(s): 2; allowed: cui, version". The CR's acceptance row was updated to this shape;
the implementation must match.

## Process note

`/gsd-secure-phase` passed Phases 83 and 90 with these three mitigations recorded as present. Whatever
that check does, it did not verify them against the dispatch path. Worth a look while this phase is
open — the same class may exist in other phases' threat records.
