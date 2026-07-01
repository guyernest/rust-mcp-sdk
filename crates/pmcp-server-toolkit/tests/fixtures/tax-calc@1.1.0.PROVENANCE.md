# `tax-calc@1.1.0` — SYNTHETIC TEST FIXTURE provenance

This file is a SIBLING of the `tax-calc@1.1.0/` bundle directory (it lives
OUTSIDE the bundle root so the loader's frozen 7-member allow-set is not
violated — `LocalDirSource::list_artifacts` would otherwise fail-close on an
extra member).

## What `workbook_hash` and `BUNDLE.lock` mean for THIS fixture

`tax-calc@1.1.0` is a **synthetic test fixture**, not a bundle compiled from a
real Excel-derived workbook. The committed `tests/fixtures/tax-calc.xlsx` it
mirrors is itself a `rust_xlsxwriter`-authored blob (a pure writer, NOT Excel)
— see `crates/pmcp-workbook-compiler/tests/fixtures/tax-calc.provenance-override.json`.

As of the Phase 100 Plan 01 (D-07) edit — which added one **text** formula
output (`bracket_label` at `3_Outputs!B6`,
`IF(taxable_income>=40000,"bracket_2","bracket_1")` ⇒ `"bracket_2"` at the tier
defaults) and one **boolean** formula output (`is_taxable` at `3_Outputs!B7`,
`taxable_income>0` ⇒ `true`) to the `Calculate_Tax` tool:

- **`workbook_hash` denotes FIXTURE PROVENANCE** — a stable fixture identity. It
  is deliberately UNCHANGED by this edit because no new source `.xlsx` was
  folded in this round. It does **NOT** assert "these four artifacts were
  compiled from the workbook at this hash"; the four data artifacts changed
  without a corresponding source-workbook change.

- **`BUNDLE.lock` attests ARTIFACT-SET INTEGRITY** — that the four edited
  artifacts (`manifest.json`, `executable.ir.json`, `cell_map.json`,
  `layout.json`, the last two folded via `fold_evidence_hash`) are internally
  consistent. It was **re-folded** by driving the runtime's own
  `fold_evidence_hash` + `build_bundle_lock` helpers (NOT by hand-editing hex),
  so the loader's boot-time integrity recompute byte-reproduces it and the
  golden boots without a hash-mismatch panic.

## Why the lock was hand-folded (not compiler-regenerated)

The compiler regeneration path (`pmcp-workbook-compiler/src/reemit_golden.rs` +
`fixture_author.rs`) was NOT used because:

1. The `compile_workbook_with_fixture_override` entry point is `#[cfg(test)]`-only
   and lives in a **different crate** (`pmcp-workbook-compiler`), and
2. `reemit_golden` reproduces the golden by **structural equivalence**, not
   byte-identity, so regenerating through it would perturb the committed golden
   bundle beyond the two added cells.

Hand-folding via the runtime hashing helpers edits exactly the four data
artifacts plus the re-folded lock — nothing else — which is the minimal,
auditable change D-07 calls for.
