---
phase: 92-bundlesource-served-tool-toolkit-module
verified: 2026-06-10T23:00:00Z
status: gaps_found
score: 3/5 must-haves verified
overrides_applied: 0
gaps:
  - truth: "An agent can call calculate with typed, tier-enforced, dtype-checked, enum-gated inputs and receive ALL named outputs ({value,unit} each) plus a provenance stamp — no single privileged headline output — and explain / get_manifest / diff_version / render_workbook each return their bundle-driven projections"
    status: failed
    reason: "CR-01 (empirically verified in REVIEW.md and confirmed in this verification): fixture_gen.rs build_ir() inserts all three input cells (1_Inputs!B2, B3, B4) as CellExpr::Literal into the IR, which the executor's literal arm unconditionally overwrites via env.seed_cell at topo-walk time — AFTER validate_input seeds the caller's values. Every calculate/explain/render_workbook call silently computes from the default inputs (gross_income=60000.0) regardless of what the caller sends. calculate({ gross_income: 100000 }) returns taxable_income=48000 (i.e. 60000-12000) not 88000. The test suite is blind to this because every value-bearing test passes the default values (gross_income=60000.0). The five structural/key-presence tools (get_manifest, diff_version) are unaffected. outputSchema, error envelopes, and provenance stamps are correctly shaped — the defect is confined to compute correctness."
    artifacts:
      - path: "crates/pmcp-server-toolkit/tests/support/fixture_gen.rs"
        issue: "build_ir() at lines 132-184 emits input cells (CELL_GROSS_INCOME, CELL_DEDUCTIONS, CELL_FILING_STATUS) as IR literals, violating the executor's documented seed contract that IR literals MUST NOT carry Role::Input cells"
      - path: "crates/pmcp-workbook-runtime/src/sheet_ir/executor.rs"
        issue: "literal arm at lines 118-129 unconditionally calls env.seed_cell(&key, v) which does HashMap::insert — overwrites any previously-seeded caller value"
    missing:
      - "In build_ir(): do NOT emit Role::Input cells as IR literals; input cells must be absent from the IR so validate_input's tier-default seeds survive the topo walk"
      - "Add a value-asserting regression test using a non-default input, e.g. calculate({ gross_income: 100000 }) must return taxable_income=88000 (not 48000)"
      - "Regenerate the committed golden at tests/fixtures/tax-calc@1.1.0/ after the build_ir() fix"
      - "Defense-in-depth (recommended): make the executor literal arm seed-preserving (if env.get(&key).is_none() { env = env.seed_cell(...) }) or reject a bundle whose cell_map.inputs coordinates appear in the IR"

  - truth: "Every domain failure returns a structured isError:true envelope in structuredContent (never a protocol Err) carrying code, reason, and self-repair fields plus the provenance stamp; validation is fail-closed (WR-05, WR-02)"
    status: failed
    reason: "Multiple fail-open gaps in the validation path undermine the fail-closed claim. WR-02: the override accept arm at input.rs:148-151 does not filter by role — a Role::Output or Role::Formula key is ACCEPTED, seeded, and echoed in accepted_overrides (currently masked by the executor overwriting it, but becomes a live output-forging vector if the CR-01 executor fix makes seeds win). WR-04: project_outputs at handler.rs:78-80 uses `let Some(value) = ... else { continue }` — a declared output absent from the run result silently vanishes from the success payload with no error signal (fails open on cell_map/IR skew, contradicts WBSV-07). WR-07: verify_stamp_binding at bundle_loader.rs:218 uses layout.source_workbook_hash.as_deref().unwrap_or(\"\") — a bundle with absent layout anchor AND lock.workbook_hash=\"\" passes the stamp gate (fails open on the security gate while the comment claims it fails closed). The isError envelope shape itself, error codes, and provenance stamps on rejections are correctly implemented."
    artifacts:
      - path: "crates/pmcp-server-toolkit/src/workbook/input.rs"
        issue: "override accept arm at lines 148-151 (Some(r) branch) accepts Role::Output and Role::Formula cells — contradicts the module's own variable_tier_keys() filter which excludes those roles"
      - path: "crates/pmcp-server-toolkit/src/workbook/handler.rs"
        issue: "project_outputs at lines 77-88 silently continues on a missing output instead of returning an error — fails open on cell_map/IR skew"
      - path: "crates/pmcp-workbook-runtime/src/bundle_loader.rs"
        issue: "verify_stamp_binding at line 218 uses unwrap_or(\"\") — absent source_workbook_hash + empty lock.workbook_hash passes the gate that claims to fail closed"
    missing:
      - "In input.rs override accept arm: add role filter rejecting Role::Output and Role::Formula with an unsupported_option error (mirrors the variable_tier_keys filter already applied to the allowed-list)"
      - "In handler.rs project_outputs: replace continue with a fail-closed error for any declared output absent from the run result"
      - "In bundle_loader.rs verify_stamp_binding: reject an absent layout.source_workbook_hash explicitly (return StampMismatch with member_value: \"<absent>\") rather than defaulting to empty string"
deferred: []
human_verification:
  - test: "Run calculate with a non-default gross_income after the CR-01 fix is applied"
    expected: "calculate({ inputs: { gross_income: 100000 } }) must return taxable_income.value=88000.0 (100000-12000), not 48000.0 (the literal default 60000-12000)"
    why_human: "The test suite is currently blind to this because all value-bearing tests use the IR-literal defaults; needs a live cargo test run after the fixture_gen.rs fix"
---

# Phase 92: BundleSource Served-Tool Toolkit Module — Verification Report

**Phase Goal:** The compiled-bundle contract is frozen from the consumer side: a generic, fully manifest-driven `workbook` feature module in `pmcp-server-toolkit` registers all five tools against a test bundle loaded through a `BundleSource` trait, fails closed on any integrity or validation gap, and emits the same `outputSchema` → `structuredContent` discipline as the SQL/OpenAPI toolkits — with zero per-workbook Rust.
**Verified:** 2026-06-10T23:00:00Z
**Status:** gaps_found
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Agent can call `calculate` with typed inputs and receive ALL named outputs plus provenance; all five tools return bundle-driven projections | FAILED | CR-01: fixture_gen.rs build_ir() emits input cells as IR literals; executor literal arm overwrites caller-seeded values unconditionally; calculate({gross_income:100000}) returns taxable_income=48000 (default), not 88000. Every value-bearing test uses the default values and asserts only key presence — the test suite cannot detect the defect. |
| 2 | Input/output schemas projected entirely from manifest (additionalProperties:false, dtype/unit/meaning); mandatory non-empty outputSchema; parity with SQL/OpenAPI TypedToolWithOutput pattern; no per-workbook handler code | VERIFIED | schema.rs builds input_schema_for_manifest() and output_schema_for_manifest() dynamically from bundle manifest + cell_map. additionalProperties:false present. outputSchema advertised on all 5 handlers via .with_output_schema(). Tests confirm non-empty outputSchema (handler.rs:676-687). Zero per-workbook handler code. |
| 3 | Every domain failure returns structured isError:true envelope in structuredContent (never protocol Err) carrying code, reason, self-repair fields plus provenance stamp; validation is fail-closed | FAILED | isError envelope shape is correct and provenance stamps are present. However: WR-02 override accept arm accepts Role::Output/Formula cells (fails open on override path); WR-04 project_outputs silently drops outputs missing from run result (fails open on cell_map/IR skew, contradicts advertised outputSchema); WR-07 stamp binding passes vacuously when both layout anchor and lock hash are empty strings. |
| 4 | Server recomputes BUNDLE.lock combined hash-of-hashes at boot and fails closed on any tampered or mismatched artifact before serving | VERIFIED | bundle_loader.rs load() enforces frozen member allow-set, recomputes evidence hash and combined hash via build_bundle_lock, fails with IntegrityMismatch on any mismatch. Tests: byte_flip_returns_integrity_mismatch, unexpected_extra_member_fails_closed, tamper_fails_boot_through_the_builder. WR-01 TOCTOU (double-read of cell_map/layout/changelog) is a correctness risk but does not prevent detection of most tampering; noted as warning. WR-07 (empty-anchor gap in verify_stamp_binding) is a partial fail-open. |
| 5 | Server loads bundle via BundleSource trait with local-directory and embedded implementations; S3/registry is documented extension seam | VERIFIED | BundleSource trait in bundle_source.rs exposes only raw-byte access. LocalDirSource and EmbeddedSource (feature-gated workbook-embedded) implemented. S3/registry documented as extension seam in BundleSourceError non_exhaustive enum comments and bundle_source.rs module docs. WorkbookBuilderExt::with_workbook_bundle/try_with_workbook_bundle wired in workbook/mod.rs. |

**Score: 3/5 truths verified**

---

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/pmcp-workbook-runtime/src/bundle_source.rs` | BundleSource trait + LocalDirSource + EmbeddedSource | VERIFIED | All present, substantive, wired |
| `crates/pmcp-workbook-runtime/src/bundle_loader.rs` | BundleLoader::load + fail-closed integrity gate | VERIFIED | load() function with 5-step gate. WR-01 TOCTOU and WR-07 stamp-binding gap noted |
| `crates/pmcp-server-toolkit/src/workbook/handler.rs` | Five tool handlers | VERIFIED (structurally) | All 5 handlers exist and are substantive. Compute correctness broken by CR-01. WR-04 fail-open in project_outputs. |
| `crates/pmcp-server-toolkit/src/workbook/input.rs` | validate_input with fail-closed tier enforcement | VERIFIED (partially) | Input validation gate correct for dtype/enum/missing-role. WR-02 override path accepts Role::Output/Formula. |
| `crates/pmcp-server-toolkit/src/workbook/schema.rs` | Schema projection from manifest | VERIFIED | input_schema_for_manifest + output_schema_for_manifest project from manifest dynamically. |
| `crates/pmcp-server-toolkit/src/workbook/mod.rs` | WorkbookBuilderExt + re-exports | VERIFIED | with_workbook_bundle / try_with_workbook_bundle implemented. Full boot surface re-exported per D-11. |
| `crates/pmcp-server-toolkit/tests/support/fixture_gen.rs` | Correct synthetic golden bundle generator | STUB (defective) | Generates a self-consistent bundle but violates executor seed contract by emitting Role::Input cells as IR literals. This is the root cause of CR-01. |
| `crates/pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0/` | Committed golden bundle | DEFECTIVE | Generated by the defective fixture_gen.rs — must be regenerated after fix. |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| handler.rs CalculateHandler | bundle_loader.rs load() | Arc<WorkbookBundle> shared at boot | WIRED | try_with_workbook_bundle calls load_bundle(source) and shares Arc to all handlers |
| handler.rs run_bundle | executor.rs run | run_executor (re-exported from runtime lib.rs) | WIRED | Uses pmcp_workbook_runtime::run_executor alias for sheet_ir::executor::run |
| workbook/mod.rs WorkbookBuilderExt | all 5 tool handlers | tool_arc registration | WIRED | All 5 handlers registered via tool_arc in try_with_workbook_bundle |
| fixture_gen.rs build_ir | executor.rs run | IR seeds contract | BROKEN | Input cells emitted as IR literals, executor literal arm overwrites caller seeds |
| input.rs validate_input → handler.rs run_bundle | executor | seeds BTreeMap | PARTIALLY BROKEN | Seeds correctly set by validate_input but overwritten by executor literal arm traversal |

---

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|--------------|--------|-------------------|--------|
| handler.rs CalculateHandler | validated.seeds | validate_input (manifest + cell_map) | Seeds correctly set from caller input | HOLLOW — seeds set correctly but executor literal arm overwrites input cell keys before downstream formulas can consume caller values |
| handler.rs project_outputs | run.computed | run_executor result | Correct computation IF seeds survive executor | HOLLOW — computation is correct only for default-valued inputs; non-default inputs silently compute defaults |

---

### Behavioral Spot-Checks

Step 7b: SKIPPED for the root cause (cannot run the server without external tool invocation), but the CR-01 defect is analytically confirmed by reading three source files:

1. `fixture_gen.rs:132-141` — `build_ir()` inserts `literal_num(CELL_GROSS_INCOME, 60_000.0)` into the IR map
2. `executor.rs:118-128` — literal arm: `env = env.seed_cell(&key, v)` — unconditional `HashMap::insert`
3. `eval_bridge.rs:57-62` — `seed_cell` does `self.values.insert(key.into(), j)` — unconditional overwrite

The executor topo-walk visits `1_Inputs!B2` (gross_income, a literal in IR), calls `seed_cell` with 60000.0, replacing any caller-seeded value. The formula `CELL_TAXABLE_INCOME = CELL_GROSS_INCOME - CELL_DEDUCTIONS` then reads env[CELL_GROSS_INCOME] = 60000.0 regardless of what the caller sent. The test suite's `handler.rs:610` passes `gross_income: 60000.0` (the default) and only asserts key presence — no test asserts that `calculate({gross_income: 100000})` returns `taxable_income=88000`.

---

### Probe Execution

No probes declared in PLAN.md files. SKIPPED.

---

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| WBSV-01 | Plans 02-05 | calculate with typed inputs → all named outputs | BLOCKED | CR-01: caller inputs silently ignored for the committed golden bundle; compute returns default-derived values regardless of input |
| WBSV-02 | Plans 03-04 | explain → ordered per-cell derivation trace | BLOCKED | explain re-runs the same broken execute path; explain steps are based on the same input-clobbered computation |
| WBSV-03 | Plans 03-05 | get_manifest → curated manifest projection | SATISFIED | curated_manifest() in handler.rs correctly projects inputs/outputs/governed_data/changelog from bundle; no computation path |
| WBSV-04 | Plans 03-05 | diff_version → hash-verified changelog | SATISFIED | serve_changelog() projects recorded bundle.changelog; no computation path; stamp attached |
| WBSV-05 | Plans 04-05 | render_workbook → provenance-bound workbook:// URI | PARTIALLY BLOCKED | URI encoding and decoding work; provenance correctly bound; BUT the re-run on resources/read would also suffer CR-01 for non-default inputs |
| WBSV-06 | Plans 02-05 | isError:true envelope on domain failures | PARTIALLY BLOCKED | Envelope shape, codes, and provenance stamp are correct. WR-02 override path accepts non-input roles; WR-04 project_outputs fails open silently |
| WBSV-07 | Plans 02-05 | schemas projected from manifest, additionalProperties:false, non-empty outputSchema | SATISFIED | schema.rs generates correct schemas dynamically from manifest. All 5 handlers advertise non-empty outputSchema. |
| WBSV-08 | Plans 01, 05 | Boot-time BUNDLE.lock hash recompute, fail-closed on tamper | VERIFIED (with WR-01/WR-07 caveats) | load() recomputes combined hash and fails with IntegrityMismatch. TOCTOU (WR-01) is a theoretical race; WR-07 empty-anchor gap is a fail-open in the stamp gate |
| WBSV-09 | Plan 01 | BundleSource trait with local-dir and embedded impls; S3 documented seam | SATISFIED | BundleSource trait, LocalDirSource, EmbeddedSource all implemented and wired |

---

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `fixture_gen.rs` | 132-141 | Input cells emitted as IR literals — violates executor seed contract | BLOCKER | Root cause of CR-01: all non-default caller inputs silently ignored for calculate/explain/render_workbook |
| `handler.rs` | 78-80 | `let Some(value) = ... else { continue }` — fails open on declared output absent from run result | BLOCKER | Silent success response with missing output fields; contradicts WBSV-07 advertised outputSchema |
| `input.rs` | 148-151 | Override accept arm without role filter — accepts Role::Output and Role::Formula | BLOCKER | Fails open on override path; will become live output-forging vector after CR-01 executor fix |
| `bundle_loader.rs` | 218 | `unwrap_or("")` on layout.source_workbook_hash before stamp comparison | BLOCKER | Empty anchor + empty lock.workbook_hash passes security gate that claims to fail closed |
| `bundle_loader.rs` | 286, 316-320 | Double-fetch: evidence hash computed over first read, parsers use second read | WARNING | TOCTOU window allows a racing writer to swap a member between hash-check and parse |
| `render_uri.rs` | 143-153 | `encode` has no size guard; `decode` enforces MAX_ENCODED_URI_LEN | WARNING | Can mint dead-pointer URIs that the server's own read side always rejects |
| `handler.rs` | 58-60 | DAG cycle mapped to `invalid_input` code | WARNING | A cycle is an infrastructure defect, not a caller-repairable bad argument |

---

### Human Verification Required

#### 1. Compute Correctness After CR-01 Fix

**Test:** After applying the fixture_gen.rs fix (remove input cells from IR), regenerate the committed golden and run: `cargo test -p pmcp-server-toolkit --features workbook -- --test-threads=1`
**Expected:** The new regression test `calculate({gross_income: 100000})` → `taxable_income=88000` passes. All existing value-bearing tests still pass (they use default inputs, which should still resolve via validate_input's tier-default seeding at step 1).
**Why human:** Requires code edit, golden regeneration, and running the test suite — cannot verify statically.

---

## Gaps Summary

**Two root-cause blockers prevent the phase goal from being achieved:**

### Blocker 1 — CR-01: Caller inputs silently ignored (blocks WBSV-01, WBSV-02, WBSV-05)

The fixture generator (`fixture_gen.rs:build_ir()`) places the three input cells as IR literals, causing the executor's topo walk to unconditionally overwrite caller-seeded values. A correctly-structured bundle (where input cells are absent from the IR) would work correctly — the executor's documented contract explicitly requires input cells to be absent from the IR. The fix requires:
1. Remove the three input literal entries from `build_ir()`
2. Regenerate the committed golden at `tests/fixtures/tax-calc@1.1.0/`
3. Add a regression test asserting a non-default input produces a non-default output

This is a pure test-fixture defect — the executor, handler, input validator, and schema generator are all correctly designed. The architecture is sound; the committed golden bundle violates the architecture's own contract.

### Blocker 2 — Validation fail-open gaps (blocks WBSV-06 full claim)

Three fail-open paths exist in the validation and integrity layer:
- **WR-02**: Override path accepts Role::Output/Formula cells
- **WR-04**: `project_outputs` silently drops declared outputs missing from run result
- **WR-07**: `verify_stamp_binding` passes vacuously when both anchor and lock hash are empty strings

Each requires a 1-3 line fix. WR-04 is particularly important because it contradicts the phase's core WBSV-07 claim that the advertised `outputSchema` enumerates every named output.

The SQL/OpenAPI parity claim (manifest-driven schemas, `TypedToolWithOutput` pattern, no per-workbook Rust) is structurally achieved and well-implemented. The phase goal's "fails closed on any integrity or validation gap" claim is what these three gaps undermine.

---

_Verified: 2026-06-10T23:00:00Z_
_Verifier: Claude (gsd-verifier)_
