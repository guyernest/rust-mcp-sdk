# Spike: the Excel 1900-leap-year quirk (Plan 96-03 Task 2 / RESEARCH Open Question 1)

**Status:** RESOLVED — disposition (A), DAG-expressible.
**Date:** 2026-06-15
**Requirement:** WBEX-02 (Excel-quirk fixture corpus verifies reconcile determinism beyond the single golden).

## The quirk

Excel incorrectly treats the year 1900 as a leap year (a deliberate 1-2-3-era
compatibility bug). Serial `60` therefore denotes a phantom `1900-02-29`, and as a
consequence **every date serial on or after `1900-03-01` is one greater** than the
astronomically-correct count of days from the `1900-01-01` epoch. This is the
"1900 leap-year" quirk named in 96-CONTEXT.md D-09.

It READS like a date-function test (`DATE(1900,2,29)`), but it is actually a
**serial-number arithmetic** quirk. The constrained dialect WHITELIST
(`crates/pmcp-workbook-dialect/src/lib.rs:35`) has NO `DATE`/`DATEVALUE`, and the
runtime has no date-serial code. Re-implementing date logic would be scope creep
into a non-whitelisted area AND would break the WBDL-01 doc↔const binding (the
`WHITELIST` const is bound to `docs/workbook-dialect-spec.md` by a drift-guard
test). The spike therefore tested whether the quirk is expressible WITHOUT any
date primitive — as plain `f64` serial arithmetic over the whitelisted ops.

## What the spike did

Authored a tiny probe fixture through the Plan 96-03 Task 1 fixture author (genuine
Excel identity via `rust_xlsxwriter`), and compiled + reconciled it through the
`#[cfg(test)]` trusted-fixture override — the SAME penny-reconcile path the golden
gate uses.

Probe (`crates/pmcp-workbook-compiler/tests/fixtures/leap1900-probe.xlsx`), authored
by `fixture_author::leap1900_probe_spec` (env-gated `regenerate_fixtures` generator;
metadata in `leap1900-probe.gen.json`):

| Cell | Role | Content | Meaning |
|------|------|---------|---------|
| `A1` | input (blue font) | `61` | a raw day-count from the `1900-01-01` epoch (1900-01-01 = serial 1 ⇒ 61 = 1900-03-01) |
| `B1` | formula, cached `<v>` = `62` | `=IF(A1>59, A1+1, A1)` | adds the phantom-leap offset for any serial past `1900-02-28` — exactly Excel's serial |
| `out_excel_serial` | named output → `B1` | — | the Excel serial the reconcile oracle grades |

The boundary `A1>59` is the quirk itself: serial `59` = `1900-02-28` (no shift), and
everything strictly after the phantom `1900-02-29` (serial `60`) is shifted `+1`.
The cached `<v>` (`62`) is the Excel-correct serial for `1900-03-01`; the executor
recomputes `IF(61>59, 61+1, 61) = 62` and the penny-reconcile matches it.

Self-test: `fixture_author::tests::leap1900_probe_compiles_and_reconciles`
(also asserts the committed probe classifies `ProvenanceClass::ExcelTrusted`).

## Disposition

**(A) DAG-expressible — a real reconcile fixture.**

The 1900-leap-year quirk IS expressible as pure serial-number arithmetic over bare
`f64` using ONLY whitelisted ops (`IF` + the `>` comparison + `+`). No date function
was added; the WHITELIST is unchanged. The probe fixture compiles and reconciles
through the real penny-reconcile path, so Plan 05 can encode the 1900-leap quirk as a
genuine reconcile fixture (the preferred outcome) rather than a `scalar_eval`-only
numeric assertion or a documented limitation.

The committed `leap1900-probe.xlsx` is RETAINED (disposition A keeps the probe — it is
the proof-of-concept reconcile fixture Plan 05 generalizes from). It is paired with its
`leap1900-probe.provenance-override.json` trusted-fixture marker and
`leap1900-probe.gen.json` generation metadata.

### What was explicitly NOT done (the deferred boundary)

- NO `DATE`/`DATEVALUE` (or any other function) added to the WHITELIST — the dialect
  crate is byte-unchanged. Adding one would break the WBDL-01 doc↔const binding test
  and violate the deferred-functions boundary (96-CONTEXT.md "Explicitly NOT in this
  phase").
- NO date-serial code added to the runtime. The serial offset is encoded entirely in
  the workbook's own formula (the workbook IS the specification), evaluated by the
  existing executor.

## WBEX-02 Traceability

WBEX-02 requires: *"Excel-quirk fixture corpus verifies reconcile determinism beyond
the single golden."* This section maps the 1900-leap quirk to how WBEX-02 stays
satisfied under disposition (A), for consumption by Plan 96-05.

| Named quirk (D-09) | How WBEX-02 is satisfied under disposition (A) | Mechanism |
|--------------------|------------------------------------------------|-----------|
| 1900 leap-year | A dedicated reconcile fixture whose cached `<v>` encodes the Excel-serial `+1` offset for serials past `1900-02-28`; the executor recomputes the offset via `IF(serial>59, serial+1, serial)` and the **real penny-reconcile path** (`reconcile::reconcile` / `within_tol`, TOL = 0.01) grades the recomputation against the cached oracle. A clean compile = a deterministic reconcile beyond the single tax-calc golden. | `leap1900-probe.xlsx` reconcile fixture (proven by `leap1900_probe_compiles_and_reconciles`); Plan 05 generalizes it into the corpus alongside the other three named quirks. |

**Why this still satisfies "reconcile determinism beyond the single golden":** the
quirk is exercised through the identical compile→execute→reconcile pipeline the golden
gate uses (not a side-channel assertion), with a quirk-specific oracle distinct from
tax-calc. Because the disposition is (A), there is NO `scalar_eval`-only fallback and
NO known-limitation note standing in for the reconcile fixture — the reconcile fixture
itself is the witness. Plan 05 SHOULD additionally encode a fast `scalar_eval` unit
assertion of the same serial offset (the D-08 two-layer pattern), but the reconcile
fixture is the load-bearing WBEX-02 artifact for this quirk.

## Notes for Plan 96-05

- Reuse `fixture_author::leap1900_probe_spec` (or extend it) — do NOT re-author by
  hand; the env-gated `regenerate_fixtures` generator is the only sanctioned writer
  into `tests/fixtures/`.
- Pair the 1900-leap reconcile fixture with the D-08 `scalar_eval` unit test for the
  same `IF(serial>59, serial+1, serial)` offset.
- The boundary value to assert is `A1>59` (serial 59 = 1900-02-28, no shift; serial 60
  is the phantom 1900-02-29). A regression that drops the `>59` guard or shifts the
  boundary would change the reconciled oracle and fail the gate.
