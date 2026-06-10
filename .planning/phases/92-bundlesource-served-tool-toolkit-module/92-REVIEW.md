---
phase: 92-bundlesource-served-tool-toolkit-module
reviewed: 2026-06-10T21:46:26Z
depth: standard
files_reviewed: 33
files_reviewed_list:
  - crates/pmcp-server-toolkit/Cargo.toml
  - crates/pmcp-server-toolkit/examples/workbook_server_http.rs
  - crates/pmcp-server-toolkit/src/error.rs
  - crates/pmcp-server-toolkit/src/lib.rs
  - crates/pmcp-server-toolkit/src/workbook/error.rs
  - crates/pmcp-server-toolkit/src/workbook/handler.rs
  - crates/pmcp-server-toolkit/src/workbook/input.rs
  - crates/pmcp-server-toolkit/src/workbook/mod.rs
  - crates/pmcp-server-toolkit/src/workbook/render_resource.rs
  - crates/pmcp-server-toolkit/src/workbook/render_uri.rs
  - crates/pmcp-server-toolkit/src/workbook/schema.rs
  - crates/pmcp-server-toolkit/tests/fixture_byte_stability.rs
  - crates/pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0/BUNDLE.lock
  - crates/pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0/cell_map.json
  - crates/pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0/evidence/changelog.json
  - crates/pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0/evidence/parser_equivalence.json
  - crates/pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0/executable.ir.json
  - crates/pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0/layout.json
  - crates/pmcp-server-toolkit/tests/fixtures/tax-calc@1.1.0/manifest.json
  - crates/pmcp-server-toolkit/tests/support/fixture_gen.rs
  - crates/pmcp-server-toolkit/tests/support/mod.rs
  - crates/pmcp-server-toolkit/tests/support/tamper.rs
  - crates/pmcp-server-toolkit/tests/workbook_integration.rs
  - crates/pmcp-server-toolkit/tests/workbook_provstamp_contract.rs
  - crates/pmcp-workbook-runtime/Cargo.toml
  - crates/pmcp-workbook-runtime/src/artifact_model.rs
  - crates/pmcp-workbook-runtime/src/bundle_loader.rs
  - crates/pmcp-workbook-runtime/src/bundle_source.rs
  - crates/pmcp-workbook-runtime/src/lib.rs
  - crates/pmcp-workbook-runtime/src/manifest_model.rs
  - crates/pmcp-workbook-runtime/tests/fixtures/embedded_bundle/evidence/changelog.json
  - crates/pmcp-workbook-runtime/tests/fixtures/embedded_bundle/manifest.json
  - docs/workbook-uri-spec.md
findings:
  critical: 1
  warning: 7
  info: 8
  total: 16
status: issues_found
---

# Phase 92: Code Review Report

**Reviewed:** 2026-06-10T21:46:26Z
**Depth:** standard
**Files Reviewed:** 33
**Status:** issues_found

## Summary

Phase 92 adds the governed-Excel workbook served-tool module: the `BundleSource`
trait + fail-closed `BundleLoader` in `pmcp-workbook-runtime`, and the five
served tools (`calculate`/`explain`/`get_manifest`/`diff_version`/
`render_workbook`) + `workbook://` render resource in `pmcp-server-toolkit`,
plus the committed `tax-calc@1.1.0` golden fixture and its generator/tamper
test harness.

The hardening architecture (frozen member allow-set, integrity recompute via
the shared hasher, stamp binding, URI size guard, total decode, provenance
verification, input re-validation on read) is genuinely well-constructed, and
the negative-path test coverage is broad. However, the review found **one
empirically-verified critical correctness defect**: for the committed golden
bundle, caller-supplied inputs and overrides are **silently ignored** — every
`calculate`/`explain`/`render_workbook` call computes from the bundle's baked-in
default inputs. The test suite cannot detect this because every value-bearing
test supplies inputs equal to the defaults. Several fail-open gaps in the
validation and loader paths were also found.

## Critical Issues

### CR-01: Caller inputs and overrides are silently ignored — the golden bundle's IR literals clobber the validated seeds

**File:** `crates/pmcp-server-toolkit/tests/support/fixture_gen.rs:132-184` (root cause), `crates/pmcp-server-toolkit/src/workbook/handler.rs:50-61` (affected path)
**Issue:** The executor contract (`pmcp-workbook-runtime/src/sheet_ir/executor.rs:90-91`) is explicit: the seed env "carries pre-loaded `Role::Input` cells … (cells **ABSENT** from `ir`)". The executor's literal arm (`executor.rs:118-129`) unconditionally calls `env.seed_cell(&key, v)` for every IR cell holding a `CellExpr::Literal`, and `CellEnv::seed_cell` (`eval_bridge.rs:57-62`) does an unconditional `HashMap::insert` — **overwriting** any caller-seeded value for that key. The Phase 92 fixture generator violates the contract: `build_ir()` (`fixture_gen.rs:135-142`) inserts all three input cells (`1_Inputs!B2/B3/B4`) into the IR as literals carrying the defaults. Because `build_dag` adds every IR key as a node, the topo walk visits each input cell **before** its dependents and replaces the caller's validated seed with the baked-in default.

Net effect, **empirically verified** against the committed golden (probe run during this review):

```
calculate { gross_income: 100000 }  →  taxable_income = 48000   (i.e. 60000 − 12000, the defaults)
                                       expected 88000 (100000 − 12000)
```

Every input-bearing served surface is affected: `calculate`, `explain`, `render_workbook`, and the `workbook://` regen-on-read pipeline (the rendered `.xlsx` is byte-identical regardless of the inputs encoded in the URI). Overrides are equally ineffective while still being reported in `accepted_overrides` — the response actively misrepresents what was computed, under a valid provenance stamp. For a governance-focused feature whose whole premise is "the served numbers are the governed computation over YOUR inputs", this is a silent correctness failure.

The test suite is structurally blind to it: `handler.rs:610`, `render_resource.rs:203`, and every other value-bearing test passes `gross_income: 60000.0` / `filing_status: "single"` — exactly the IR-literal defaults — and asserts only key presence, never an input-dependent value.

**Fix:** Two coordinated changes plus a regression test:
1. In `build_ir()`, do NOT emit `Role::Input` cells as IR literals (honoring the executor's documented seed contract). The tier-default seeding in `validate_input` step 1 (`input.rs:103-109`) already guarantees omitted inputs resolve, so the formula refs stay satisfied:
```rust
fn build_ir() -> BTreeMap<String, Cell> {
    let mut ir: BTreeMap<String, Cell> = BTreeMap::new();
    // Input cells are deliberately ABSENT from the IR: they are seeded per call
    // (executor contract — an IR literal would clobber the caller's seed).
    // Governed bracket rate table only:
    for (k, c) in [ ... bracket literals ... ] { ir.insert(k, c); }
    // Outputs (formulas) ...
}
```
   Then regenerate the committed golden via the `regenerate_committed_golden` ignored test.
2. Add a value-asserting regression test using a NON-default input, e.g.:
```rust
let v = handler.compute(json!({ "inputs": { "gross_income": 100000.0 } })).unwrap();
assert_eq!(v["outputs"]["taxable_income"]["value"], json!(88000.0));
```
3. Defense in depth (recommended): either make the executor's literal arm seed-preserving (`if env.get(&key).is_none() { env = env.seed_cell(&key, v); }`) or have `load_bundle`/`run_bundle` reject a bundle whose `cell_map.inputs` seed coordinates appear in the IR — otherwise any future compiler-emitted bundle that repeats this shape reintroduces the bug silently.

## Warnings

### WR-01: Loader verifies one set of bytes but parses a second read (double-fetch / TOCTOU in the integrity gate)

**File:** `crates/pmcp-workbook-runtime/src/bundle_loader.rs:286,316-320`
**Issue:** `load` computes the evidence hash by reading `cell_map.json`, `layout.json`, `evidence/changelog.json`, `evidence/parser_equivalence.json` inside `recompute_evidence_hash` (line 286), then **re-reads** `cell_map.json` / `layout.json` / `evidence/changelog.json` from the source at lines 316-320 to parse them. The bytes that pass the integrity gate are therefore not guaranteed to be the bytes the server actually serves: with a `LocalDirSource`, a writer racing the boot window can swap a member between the hash read and the parse read, defeating the gate's central guarantee. The inconsistency is telling — `ir_bytes`/`manifest_bytes` ARE correctly read once and reused for both hashing and parsing (lines 283-315), so the double-fetch on the other three members looks unintended.
**Fix:** Read every member exactly once up front into a map (or struct of buffers), feed those buffers to both the evidence fold and the parsers:
```rust
let cell_map_bytes = read_member(source, MEMBER_CELL_MAP)?;
let layout_bytes = read_member(source, MEMBER_LAYOUT)?;
let changelog_bytes = read_member(source, MEMBER_CHANGELOG)?;
let parser_equiv_bytes = read_member(source, MEMBER_PARSER_EQUIV)?;
let evidence_hash = fold_evidence(&[(MEMBER_CELL_MAP, &cell_map_bytes), ...]);
// later: parse_member(&cell_map_bytes, MEMBER_CELL_MAP)? — same bytes that were hashed
```

### WR-02: Overrides are accepted on `Role::Output` and `Role::Formula` cells (fails open against the documented variable-tier contract)

**File:** `crates/pmcp-server-toolkit/src/workbook/input.rs:141-160,242-247`
**Issue:** The override gate rejects strict constants (`is_strict_constant`) and unknown keys, but the accept arm `Some(r) => { ... seeds.insert(r.cell.clone(), ...) }` has no role filter: `find_role_by_key` matches ANY manifest cell by `name` or `cell`, so `overrides: { "out_tax_owed": 0 }` or `overrides: { "3_Outputs!B3": 0 }` is ACCEPTED, seeded, and echoed back in `accepted_overrides`. The docs (`input.rs:55-56`, schema description `schema.rs:308-309`) promise "variable-tier parameter overrides" only — and `variable_tier_keys` (line 251-258) itself excludes `Role::Output | Role::Formula` from the allowed-alternatives list, so the accept arm contradicts the module's own allow-list. Today the seeded output value is recomputed-over by the executor's formula arm, masking the impact — but if CR-01 is fixed in the "seeds win over IR" direction (option 3), this becomes a live output-forging vector: a caller could pin a served output to an arbitrary value under a valid provenance stamp.
**Fix:** Mirror the `variable_tier_keys` filter in the accept arm:
```rust
Some(r) if matches!(r.role, Role::Output | Role::Formula) => {
    return Err(WorkbookToolError::unsupported_option(
        key.clone(),
        variable_tier_keys(manifest),
    ));
},
```

### WR-03: The inputs path never checks the resolved manifest role is `Role::Input` — a skewed cell_map can route caller input onto a BA-governed constant

**File:** `crates/pmcp-server-toolkit/src/workbook/input.rs:124-135`
**Issue:** Step 2 resolves the supplied input's `seed_coord` to a manifest role and rejects when NO role exists (WR-05 fail-closed), but accepts any role kind that IS found. A skewed `cell_map.inputs` entry whose `seed_coord` points at a `Role::Constant` cell (the exact manifest/cell_map partial-regeneration skew the module's own WR-05 comment warns about, lines 12-16) lets a caller seed a strict BA-governed constant through `inputs`, bypassing the V4 strict-constant protection that guards only the `overrides` map. The gate already holds the `role` value — the check is one line away.
**Fix:** After resolving `role`, fail closed on non-input roles:
```rust
if !matches!(role.role, Role::Input) {
    return Err(WorkbookToolError::invalid_input(format!(
        "internal: input '{key}' maps to {} whose manifest role is not an input",
        entry.seed_coord
    )));
}
```

### WR-04: `project_outputs` silently drops outputs missing from the run result (fails open on cell_map/IR skew)

**File:** `crates/pmcp-server-toolkit/src/workbook/handler.rs:77-80`
**Issue:** `let Some(value) = run.computed.get(&entry.seed_coord) else { continue; };` — an output declared in the verified `cell_map` whose seed coordinate was never computed (cell_map/IR skew, a partial regeneration) simply vanishes from the success payload. The client receives a success-shaped result with a valid provenance stamp and a subset of the advertised outputs, with no signal anything is wrong. This is exactly the `if let Some(...) skip that fails open` pattern the input module's docs (`input.rs:10-11`) forbid, and it contradicts WBSV-07 (the advertised `outputSchema` enumerates every named output).
**Fix:** Replace the `continue` with a fail-closed error:
```rust
let Some(value) = run.computed.get(&entry.seed_coord) else {
    return Err(WorkbookToolError::invalid_input(format!(
        "internal: declared output '{}' ({}) was not computed by the bundle IR",
        entry.json_key, entry.seed_coord
    )));
};
```

### WR-05: `encode` has no size guard — the server can mint `workbook://` URIs its own read side will always reject

**File:** `crates/pmcp-server-toolkit/src/workbook/render_uri.rs:143-153`
**Issue:** `decode` enforces `MAX_ENCODED_URI_LEN` (64 KiB) but `encode` does not. A `Dtype::Text` input WITHOUT `allowed_values` (a legal manifest shape per `manifest_model.rs:131` — `None` means the input "stays DYNAMIC") accepts arbitrarily long strings through `validate_input`, so `render_workbook` can return a success result carrying a URI longer than the bound — a dead pointer every `resources/read` rejects as `BadUri`. The published spec (`docs/workbook-uri-spec.md` §4) says "a conforming workbook input set must encode within it", but nothing enforces conformance at mint time; the failure surfaces on a later read as a confusing protocol error instead of an actionable domain error at the tool call.
**Fix:** Check the bound in `encode` and return the domain error there:
```rust
let uri = format!("{RENDER_URI_PREFIX}{b64}");
if uri.len() > MAX_ENCODED_URI_LEN {
    return Err(WorkbookToolError::invalid_input(format!(
        "inputs too large to encode: the workbook:// URI would exceed the \
         {MAX_ENCODED_URI_LEN}-byte limit ({} bytes)", uri.len()
    )));
}
Ok(uri)
```

### WR-06: Example `--bundle-dir` parsing fails open — a missing value silently serves the embedded bundle instead

**File:** `crates/pmcp-server-toolkit/examples/workbook_server_http.rs:67-72`
**Issue:** `std::env::args().skip_while(|a| a != "--bundle-dir").nth(1)` yields `None` both when the flag is absent AND when it is supplied without a value (`--bundle-dir` as the last arg). In the second case the operator believes they pointed the server at an out-of-band updated bundle, but the binary silently falls back to the baked-in embedded golden and serves stale governed logic. The example's own doc comment (lines 19-24) sells exactly this operator workflow as the live-update seam, so the silent fallback is an operational hazard, not just a UX nit. The naive scan also mis-fires if the literal string `--bundle-dir` appears as a VALUE of another argument.
**Fix:** Treat a flag-without-value as a hard error:
```rust
let mut args = std::env::args().skip(1);
let bundle_dir = match args.position(|a| a == "--bundle-dir") {
    Some(_) => Some(args.next().ok_or("--bundle-dir requires a path argument")?),
    None => None,
};
```

### WR-07: Stamp binding passes vacuously when both the layout anchor and the lock hash are empty

**File:** `crates/pmcp-workbook-runtime/src/bundle_loader.rs:216-226`
**Issue:** The comment claims "An ABSENT anchor makes the binding impossible — fail closed", but the code is `layout.source_workbook_hash.as_deref().unwrap_or("")` compared against `lock.workbook_hash`: a bundle whose layout omits `source_workbook_hash` (`None`) AND whose lock records `workbook_hash: ""` satisfies `"" == ""` and passes the T-92-02 gate. Nothing in `build_bundle_lock` rejects an empty `workbook_hash`, so this fail-open arm sits inside a security gate while its own comment asserts the opposite.
**Fix:** Reject the absent anchor explicitly before comparing:
```rust
let Some(layout_hash) = layout.source_workbook_hash.as_deref() else {
    return Err(BundleLoadError::StampMismatch {
        field: "workbook_hash",
        lock_value: lock.workbook_hash.clone(),
        member_value: "<absent>".to_string(),
        member: "layout.json (source_workbook_hash)",
    });
};
```
(and optionally reject an empty `lock.workbook_hash` in the same gate).

## Info

### IN-01: Stale module documentation describes the skeleton state

**File:** `crates/pmcp-server-toolkit/src/workbook/mod.rs:9-24`
**Issue:** The "Wiring discipline" section still says the submodule declarations "stay COMMENTED until the plan that creates each file uncomments the matching `pub mod` line" — all six `pub mod` lines (57-62) are live and the handlers exist. The paragraph documents a transitional state that no longer exists.
**Fix:** Rewrite the section in the past tense or delete it.

### IN-02: `explain` steps are lexicographically ordered by cell key, not derivation order

**File:** `crates/pmcp-server-toolkit/src/workbook/handler.rs:244-247,308-311`
**Issue:** `render_steps` sorts trace keys lexicographically; the tool description advertises an "ordered business-language derivation trace". For the golden's `1_/2_/3_` sheet-name convention the two coincide, but for any bundle whose sheet names do not sort in dependency order, a step can reference operand values whose own derivation appears LATER in the list. Topological order (the executor already walks it) is the truthful ordering.
**Fix:** Emit steps in the executor's topo order (e.g. carry an ordered trace list in `RunResult`, or re-toposort `bundle.dag` and filter to traced keys), or soften the description to "deterministically ordered".

### IN-03: A DAG cycle is misclassified as the caller-repairable `invalid_input` code

**File:** `crates/pmcp-server-toolkit/src/workbook/handler.rs:58-60`
**Issue:** `run_bundle` maps an executor cycle failure to `WorkbookToolError::invalid_input("executor failed: ...")`. By the module's own taxonomy (`mod.rs:26-38`) a cyclic bundle is an infrastructure/bundle defect — no input change can repair it — yet the agent receives the "fix your argument" self-repair code.
**Fix:** Either surface it as a protocol-level internal error (the infrastructure class) or add a distinct non-repairable code; at minimum document why `invalid_input` was chosen.

### IN-04: Input schema advertises roleless cell_map entries as valid `number` inputs the runtime then rejects

**File:** `crates/pmcp-server-toolkit/src/workbook/schema.rs:277-278`
**Issue:** `input_schema_for_manifest` falls back to `Dtype::Number` when a cell_map input has no manifest role and still advertises the property — but `validate_input` (WR-05 gate, `input.rs:124-133`) rejects any supplied value for that key as an internal-consistency error. The module doc (`schema.rs:9-10`) promises "a client trusting the schema never sends a key the runtime then rejects"; this is the one path where it does.
**Fix:** Skip roleless entries in the schema projection (`let Some(role) = role_for_seed(...) else { continue; }`) so the advertised surface mirrors the gate.

### IN-05: The published crate's `workbook_server_http` example cannot compile from the crates.io artifact

**File:** `crates/pmcp-server-toolkit/Cargo.toml:16`, `crates/pmcp-server-toolkit/examples/workbook_server_http.rs:50`
**Issue:** `examples/` ships in the published package, but the example's `include_dir!("$CARGO_MANIFEST_DIR/tests/fixtures/tax-calc@1.1.0")` points into `tests/`, which is `exclude`d from the artifact. Anyone building the example from the downloaded crate (`cargo build --examples --features workbook-embedded,http`) gets an opaque macro error. Latent (examples are not built for dependents), but a broken artifact nonetheless.
**Fix:** Either exclude this example from the package, or document in the example header that it builds only from the repository checkout.

### IN-06: Bundle member-name constants duplicated between the loader and the fixture generator

**File:** `crates/pmcp-server-toolkit/tests/support/fixture_gen.rs:57-63`, `crates/pmcp-workbook-runtime/src/bundle_loader.rs:39-63`
**Issue:** The seven member names and the evidence fold set are re-declared as private consts in both crates. The byte-stability test would catch a generator-side drift, but a loader-side rename would desync silently until a fixture regeneration. The plan's own Pitfall 2 ("the generator and loader MUST fold the identical set") argues for one definition.
**Fix:** Export the member-name consts (and `EVIDENCE_FOLD_MEMBERS`) from `pmcp-workbook-runtime` and consume them in the generator.

### IN-07: The integrity gate is self-referential — a wholesale bundle replacement passes every check

**File:** `crates/pmcp-workbook-runtime/src/bundle_loader.rs:1-23`, `crates/pmcp-server-toolkit/src/workbook/mod.rs:166-171`
**Issue:** `BUNDLE.lock` is unsigned and the recompute feeds the lock's own `bundle_id`/`version`/`workbook_hash` back into `build_bundle_lock`, so the gates detect corruption, partial swaps, and desyncs — but an attacker with write access to the bundle directory can regenerate a fully self-consistent lock and "verify" cleanly. The pervasive "tampered bundle aborts the boot" language can lead an operator to assume cryptographic authentication that is not there.
**Fix:** Add one sentence to the loader/builder-ext docs distinguishing tamper-EVIDENCE (hash self-consistency) from tamper-PROOF (signature over the lock — a documented future seam).

### IN-08: `get_manifest`/`diff_version` ignore their arguments despite advertising a strict empty input schema

**File:** `crates/pmcp-server-toolkit/src/workbook/handler.rs:412,496`
**Issue:** Both handlers advertise `empty_input_schema()` (`additionalProperties:false`) but accept any `_args` without validation — a caller sending junk arguments gets a clean success, unlike `calculate`/`explain` where the schema is mirrored by a runtime gate.
**Fix:** Reject non-empty argument objects with an `invalid_input` envelope for consistency, or note the asymmetry in the handler docs.

---

_Reviewed: 2026-06-10T21:46:26Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
