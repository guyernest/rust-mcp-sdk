---
phase: 100
reviewers: [codex]
reviewed_at: 2026-06-23
plans_reviewed: [100-01-PLAN.md, 100-02-PLAN.md, 100-03-PLAN.md, 100-04-PLAN.md, 100-05-PLAN.md]
---

# Cross-AI Plan Review — Phase 100

## Codex Review

## Summary

The five-plan sequence is generally strong: it decomposes the phase along the real dependency chain, preserves the reader-free/stateless constraints, and explicitly targets the three BA-trust capabilities with tests at XML, URI, handler, and example levels. The biggest risks are around fixture integrity/provenance in Plan 01, temporary doc/test drift before the actual `verify_accuracy` handler exists, and some underspecified runtime details in `reconcile_reference`, especially how reference inputs are seeded inside the runtime crate without depending on toolkit-only helpers.

## Strengths

- Clear sequential ordering: fixture/test primitives first, writer behavior second, URI/handler plumbing third, reconciliation/tool surface fourth, phase gates last.
- Good recognition of the `serde(deny_unknown_fields)` trap: parsing and stripping `mode` before `validate_input` is essential.
- XML-level tests for `<f>` / `<v>` are the right verification layer for WBVER-01 and WBVER-02.
- Backward compatibility is explicitly handled with `#[serde(default)]` for old `workbook://` payloads.
- Strong panic-freedom discipline: unknown modes and unknown tool filters are specified as `Err`, not panics or silent empty reports.
- The plans preserve the major fences: no runtime reader, stateless pointer-then-regenerate, additive mode/tool surface.
- Plan 05 correctly treats `make quality-gate`, `make purity-check`, `make doc-check`, and PMAT as phase gates, not optional cleanup.

## Concerns

- **HIGH: Plan 01 fixture edit may create provenance ambiguity.** It changes generated bundle artifacts while keeping `workbook_hash` unchanged and no source workbook update. That may be acceptable for a synthetic fixture, but it weakens the meaning of the lock if the lock claims artifacts derive from an unchanged workbook.
- **HIGH: `reconcile_reference` seeding is underspecified.** The plan says to mirror `seed_tier_defaults`, but that helper lives in toolkit input handling. If runtime does not already have an equivalent manifest-default seeding API, reimplementing it in `reconcile.rs` could duplicate logic and drift.
- **MEDIUM: Plan 01 updates "six tools" docs and reserved-name tests before the sixth handler exists.** This is okay only if intermediate wave commits are not treated as releasable. Otherwise, docs and behavior are temporarily inconsistent.
- **MEDIUM: H3 binding test placeholder weakens the binding.** Using string literal `"verify_accuracy"` before `VerifyAccuracyHandler::NAME` exists keeps compilation working, but it can mask the exact drift the test is supposed to catch until Plan 04.
- **MEDIUM: `RenderMode` serde default only solves absent fields, not malformed enum strings.** For forged URI payloads with `"mode":"bogus"`, serde will reject decode. That may be fine, but the plan text says "absent/garbage defaults to Filled" in one threat table. That is not true unless custom deserialization is implemented.
- **MEDIUM: InputsOnly "every formula cell has no `<v>`" tests can be brittle.** Sheet XML has shared strings, inline strings, and formula representation details. Tests should target known cells or parse worksheet XML structurally enough to avoid false positives.
- **MEDIUM: `verify_accuracy` filtering after full reconciliation may complicate `cells_checked` semantics.** If the report is filtered after rollup, the plan must recompute `all_within_tol` and `cells_checked`, not just drop tools.
- **LOW: Plan 02's helper design is a little vague for number vs string literal paths.** A single helper must either accept typed writer closures or there will be awkward branching that could increase complexity.
- **LOW: Plan 04 says compare `Text`/`Bool` equality with `abs_delta = 0.0 when equal`, but does not specify non-equal `abs_delta`.** Use a deterministic sentinel or `null` if the wire type allows it; otherwise clients may misread the value.
- **LOW: Plan 05 says prior five tools unchanged, but tool list count changes by design.** The regression assertion should be "existing tool names and existing render wire behavior unchanged," not "prior five tools + shape unchanged" if tests snapshot exact list length.

## Suggestions

- In Plan 01, explicitly decide whether the fixture is synthetic. If yes, document that `workbook_hash` is fixture provenance, not a source-workbook derivation. Better: regenerate through the compiler if any path exists.
- Move `RESERVED_TOOL_NAMES` and doc count updates into Plan 04 unless Plan 01 truly needs them earlier. If kept in Plan 01, mark the intermediate state as non-releasable.
- For `RenderMode`, make the URI decode behavior precise: absent mode defaults to `Filled`; malformed mode returns a decode error. Do not claim garbage defaults to `Filled` unless custom serde is added.
- Add a small runtime helper for manifest reference defaults, or expose an existing one from the runtime layer, so `reconcile_reference` does not duplicate toolkit validation logic.
- In Plan 04, define comparison semantics exactly:
  - numeric: finite absolute delta within `tol`
  - text/bool: equality, `within_tol = true/false`
  - empty/error/missing: fail closed with deterministic representation
- When filtering `verify_accuracy`, either pass the filter into reconciliation or recompute rollups after filtering.
- Keep XML tests scoped to specific expected cells/formulas where possible, not global `<f>`/`<v>` counts.
- Add one explicit test that `mode` does not leak into `calculate`/`explain` schemas or handlers.
- Add one compatibility test using a literal pre-phase payload without `mode`, not only a freshly serialized struct missing the field.

## Risk Assessment

**Overall risk: MEDIUM.**

The implementation path is sound and likely to achieve WBVER-01 through WBVER-04, but it touches integrity-locked fixture data, cross-crate reserved-name contracts, public-ish URI payloads, and new runtime reconciliation semantics. The main risks are not architectural; they are precision risks: provenance/hash correctness, serde edge behavior, rollup correctness after filtering, and avoiding temporary drift between docs/constants/tests and actual registered tools. If those are tightened, the plan is credible and well-scoped.

---

## Consensus Summary

Single reviewer (Codex). Overall verdict: **MEDIUM risk, sound and well-scoped** — the architecture and sequencing are endorsed; the flagged risks are *precision* risks in task specification, not design flaws. None are blockers, but several are cheap to tighten before execution.

### Agreed Strengths
- Sequential wave decomposition matches the real dependency chain.
- The `deny_unknown_fields` strip-before-`validate_input` handling is correctly anticipated.
- Reader-free / stateless / additive fences are preserved; panic-freedom (`Err` not panic) is disciplined.
- XML-level `<f>`/`<v>` tests are the right verification layer; phase gates (quality/purity/doc/PMAT) are mandatory not optional.

### Agreed Concerns (priority order for `--reviews` replan)
1. **HIGH — `reconcile_reference` seeding source.** Confirm a runtime-layer manifest-default seeding API exists (research located `seed_tier_defaults` at `input.rs:123` in the runtime crate — verify it is reachable from `reconcile.rs` without a toolkit dep, else add a small runtime helper). Prevents logic duplication/drift.
2. **HIGH — Plan 01 fixture provenance.** Decide explicitly whether the tax fixture is synthetic; if so, document that `workbook_hash` is fixture provenance (not source-workbook derivation), or regenerate through the compiler if a generator path exists. Avoids weakening the `BUNDLE.lock` contract's meaning.
3. **MEDIUM — `RenderMode` malformed-string semantics.** `#[serde(default)]` only covers *absent* `mode`; a malformed `"mode":"bogus"` in a forged URI is a *decode error*, not a silent `Filled`. Correct any threat-table text that claims "garbage defaults to Filled" (or add custom deserialization if that behavior is actually desired). Aligns with the locked "unknown mode → Err" decision.
4. **MEDIUM — verify_accuracy filter vs rollup.** When the optional tool-name filter is applied, pass it into reconciliation OR recompute `all_within_tol` / `cells_checked` after filtering — don't just drop tools from a pre-computed rollup.
5. **MEDIUM — intermediate doc/constant drift.** Plan 01 updating "six tools" docs + `RESERVED_TOOL_NAMES` + H3 binding before the Plan 04 handler exists is acceptable only if intermediate wave commits are treated as non-releasable; note that, or move those edits into Plan 04.
6. **MEDIUM — brittle InputsOnly XML assertion.** Scope the no-`<v>` assertion to specific known formula cells rather than a global `<f>`/`<v>` count (shared/inline strings can cause false positives).
7. **LOW — Plan 04 non-equal `abs_delta` for Text/Bool**, Plan 02 helper typed-writer design, and Plan 05 regression wording ("existing tool *names* + render wire behavior unchanged," not exact list length).

### Divergent Views
None — single reviewer.

### Note on items already covered
- The `seed_tier_defaults` location was identified in 100-RESEARCH.md as being in the **runtime** crate (`input.rs:123`), which partially answers Concern #1 — the replan should confirm reachability from `reconcile.rs` rather than re-derive.
- D-03/D-04 (unknown filter → Err; empty oracle vacuous) are locked; Concern #4 is about *rollup recomputation under filtering*, a refinement of D-03, not a conflict.
