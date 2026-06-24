---
status: issues_found
phase: 100-workbook-accuracy-verification-surface
depth: standard (focused — see scope note)
files_reviewed: 3
reviewed_by: orchestrator-inline
findings:
  critical: 0
  warning: 1
  info: 2
  total: 3
---

# Phase 100 Code Review — Workbook Accuracy-Verification Surface

## Scope note

The standard-depth `gsd-code-reviewer` agent was spawned twice but both runs hit
an API socket timeout before writing output (a recurring infra issue this session,
not a code problem). Code review is advisory/non-blocking per the workflow. Rather
than risk further long-agent timeouts, the orchestrator performed a **focused
inline review of the highest-risk new trust-bearing logic** — `reconcile.rs` (the
new reference-reconciliation comparator) and the `verify_accuracy` handler — and
relied on the already-green automated gates for the rest:

- `make quality-gate` PASSED (rustfmt, clippy **pedantic + nursery**, build, full
  workspace test, audit) — covers all 14 changed files for style/lint/compile/test.
- `make purity-check` PASSED (reader-free leaf preserved; `zip` test-only).
- `make doc-check` PASSED (zero rustdoc warnings).
- PMAT cognitive-complexity ≤25 PASSED (no violations across 726 files).
- The end-to-end example runs and `verify_accuracy` reconciles all 7 outputs.

The 11 files NOT deep-read inline are covered by those gates; the trust-critical
new module was read in full.

## Findings

### WR-01 (Warning): `compare_output` ignores the threaded `tol`, hardcodes the const `TOL`
**File:** `crates/pmcp-workbook-runtime/src/reconcile.rs:178-191`

`reconcile_reference(.., tol: f64)` accepts a tolerance parameter and stamps it
into the report (`tolerance: tol`, line 335). But the actual numeric grading in
`compare_output` compares against the module **constant** `TOL` (0.01), not the
threaded `tol`:

```rust
(Some(CellValue::Number(s)), Some(CellValue::Number(o))) if s.is_finite() && o.is_finite() => {
    let delta = (s - o).abs();
    (delta, delta <= TOL)   // <-- const TOL, not the `tol` parameter
}
```

`compare_output` doesn't even take `tol` as an argument, so the `tol` parameter of
`reconcile_reference` is **dead for grading**. Today this is **masked**: the only
production caller (`handler.rs:721`) passes `reconcile::TOL`, so reported tolerance
== grading tolerance == 0.01 and behavior is correct. But the public API invites a
custom tolerance, and if one is ever passed the report would claim a tolerance it
did not actually grade at — a silently-wrong attestation in a *trust* feature.

**Recommendation:** thread `tol` into `compare_output(server, oracle, tol)` and use
`delta <= tol`, OR drop the `tol` parameter from `reconcile_reference` and document
that grading is fixed at the const `TOL` (matching the compiler penny-reconcile).
The former is preferred — it makes the reported `tolerance` honest for all callers.

### INFO-01: `reconcile.rs` is exemplary for a trust-bearing comparator
Total/panic-free on the value path (`?`/`get`/`match`, `u32::try_from(..).unwrap_or(u32::MAX)`),
deterministic discrete deltas (never `NaN`), and explicit fail-closed handling of
type-mismatch / Empty / Error / missing-value pairings. Edge cases D-01 (cell
address), D-02 (oracle key with no output entry → `cell: None`, still graded),
and D-04 (empty-oracle tool → vacuous `true`, contributes 0 to `cells_checked`)
are all implemented and unit + property tested. No action.

### INFO-02: `verify_accuracy` handler filter is honestly fail-closed
The optional `tool` filter (handler.rs) recomputes `cells_checked`/`all_within_tol`
over the filtered set and returns `isError` listing available tools on an unknown
filter (D-03), verified live in the example. No action.

## Disposition

WR-01 is a latent (currently-masked) correctness defect, not a blocker — no current
wire behavior is wrong. Recommend addressing it as a small follow-up (either honor
the `tol` parameter or remove it). It does not block phase verification.
