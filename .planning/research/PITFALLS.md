# Pitfalls Research

**Domain:** Excel-as-Configuration → MCP-server compiler — extraction + generalization of the TowelRads `quote-pricing` lighthouse (workbook-compiler/workbook-runtime) into the PMCP SDK (milestone v2.3)
**Researched:** 2026-06-09
**Confidence:** HIGH (grounded in the lighthouse source — `14-REVIEW.md` CR-01/CR-02/WR-01 findings, `provenance/gate.rs` inline Pitfall-2 docs, the `just purity-check` recipe, `change_class/mod.rs`, `reconcile/classifier.rs`, `lib.rs build_reference_manifest`; the RFC §5 known-gap list)

> Scope note: these are pitfalls **specific to extracting and generalizing THIS proven-but-lighthouse-bound system** into a reusable SDK toolkit. They are not generic Rust advice. Every flagged RFC §5 gap is covered with a verifiable fix, the two trap-class pitfalls (purity-boundary, umya provenance) carry concrete gate designs, and each pitfall maps to a v2.3 phase.

---

## Critical Pitfalls

### Pitfall 1: Purity-boundary erosion — the Excel reader (umya) leaks into the served-binary tree

**What goes wrong:**
The whole value proposition is "compile, don't interpret": the served MCP binary evaluates a pre-compiled IR with a pure-Rust scalar evaluator and **never parses Excel at runtime**. During extraction the lighthouse's hard boundary — `umya-spreadsheet` (the reader/parser) lives ONLY in `workbook-compiler`, never in `workbook-runtime` or the served crate — can silently erode. Three concrete leak vectors:

1. **A shared SDK crate pulls umya.** If the SDK puts model types (`Manifest`, `CellRole`, `CellMap`, `executable.ir.json` deserialization) in a crate that *also* re-exports an ingest helper, or if `pmcp-server-toolkit` gains a `compile-workbook` convenience module behind a non-default feature, Cargo feature unification can pull umya into the served binary the moment any sibling crate in the same build enables it.
2. **Transitive dep via the writer.** Phase 12 deliberately links the writer-only `rust_xlsxwriter` into the runtime (the `render_workbook` tool returns a computed `.xlsx`). A writer is NOT a reader, but `rust_xlsxwriter` pulls `zip` — so a naive "ban zip" gate would false-positive, and a naive "allow zip" gate could let umya's own transitive `zip`/`quick-xml` slip through unnoticed.
3. **Workspace feature unification.** In a single `cargo build` of the whole workspace, enabling a `compiler` feature on a dev-dependency unifies features across the graph; the served crate's dependency tree can acquire umya even though its own `Cargo.toml` never names it.

**Why it happens:**
The lighthouse boundary is enforced by a *bespoke `just purity-check` recipe* (grep + `cargo tree`), not by the type system or Cargo itself. Extraction tends to drop the bespoke recipe ("we'll add CI later") or generalize crate layout in a way that re-merges reader and runtime. The reader/writer asymmetry (`zip` permitted for the writer, banned for the reader) is subtle and easy to get wrong.

**How to avoid — concrete gate design (port and harden the lighthouse `just purity-check`):**
The lighthouse recipe is the proven template. Port it verbatim, then generalize the crate names and add positive assertions:

- **Negative cargo-tree assertions** (per served-tree crate `pmcp-workbook-runtime`, `pmcp-server-toolkit` with the workbook module, and the scaffolded server):
  - `cargo tree -p <served-crate> | grep -Ei 'umya|calamine|quick-xml'` must find NOTHING (reader/parser stack banned).
  - **Permit `zip`** for the runtime (writer container) but ban the reader's XML parser (`quick-xml`) — this is the exact reader-vs-writer line the lighthouse draws (`justfile:77-84`).
- **Positive assertion:** `cargo tree -p pmcp-workbook-runtime | grep -qi 'rust_xlsxwriter'` MUST succeed — proves the writer IS wired and the gate is actually testing the right crate (a deleted dependency must not silently make the negative gate vacuously pass).
- **Value-path token grep:** grep each runtime evaluator source file's value path (everything before its `#[cfg(test)]` block) for forbidden tokens (`umya`, `std::fs`, `async`, `tokio`, `.unwrap(`, `.expect(`). Test modules are excluded (they legitimately read fixtures).
- **Compiler enforcement backstop:** keep the crate-level `#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]` on runtime value paths — the recipe is the cheap first line, the deny is the compiler enforcement.
- **Run it in CI on every PR**, not just locally, and run it per-feature-combination (`--no-default-features`, `--features full`) so feature unification cannot hide a leak. This is the gate that keeps "compile-not-interpret" honest.

**Warning signs:**
- `cargo tree -p <served-crate>` output grows after a "harmless refactor" that moved a type into a shared crate.
- A new `[features]` entry on the runtime or toolkit crate that gates a compiler helper.
- CI for the purity gate is green but only runs the default feature set.
- `Cargo.lock` churn touching `umya`/`quick-xml`/`calamine` lines for a runtime-only change.

**Phase to address:**
The very FIRST extraction phase (port `pmcp-workbook-runtime`, per RFC §7 "smallest cut that proves the boundary"). The purity gate must land WITH the runtime crate, before the compiler is ported, so the boundary is defended from day one. Re-verified in the phase that introduces the `pmcp-server-toolkit` workbook module and again in the scaffold phase.

---

### Pitfall 2: The umya fabricated-provenance trap — SDK tooling that mutates workbooks stamps fake Excel identity and falsely passes the freshness gate

**What goes wrong:**
The Phase-8 oracle-staleness gate (`provenance/gate.rs`) is the single most security-load-bearing component: it decides whether a workbook's cached cell values are a *trusted oracle* (real Excel computed them) or *stale/untrusted candidates*. It accepts only if the conjunction holds: `calcMode == "auto"` ∧ `!fullCalcOnLoad` ∧ `calcId != 0` ∧ no missing formula cache ∧ `<Application>` starts with "Microsoft Excel". **`umya-spreadsheet` 3.0.0 hard-codes `<Application>Microsoft Excel</Application>` and a non-zero `calcId` (122211) on every save.** So any workbook that umya has round-tripped — a test fixture, a programmatically-mutated workbook, anything written by the SDK's own tooling — **passes the freshness gate on fabricated Excel identity**, even though no real Excel ever recalculated it. The gate would admit garbage cached values as a trusted oracle.

**Why it happens:**
umya is the natural choice for *both* reading and writing `.xlsx` in Rust, so it is tempting to use it to author test fixtures or to build a "renderer" that mutates a workbook. The fabrication is invisible — the saved file genuinely contains the Excel identity strings, so the gate's check is true. The lighthouse already hit this and documented it inline (`gate.rs:1-19, 82-94`: "NEVER a umya-round-tripped copy, which FABRICATES `calcId=122211` + 'Microsoft Excel'").

**How to avoid — concrete provenance design:**
1. **The gate reads ORIGINAL on-disk bytes via a quarantined raw reader, never a umya copy.** The lighthouse's `read_calc_pr` / `read_app_props` (`provenance/raw_parts.rs`) parse the raw OOXML zip parts directly from `std::fs::read(path)?` of the file the BA supplied — they do NOT round-trip through umya's object model. Port this contract: **the gate's input is `&[u8]` of the original file, and the API makes it impossible to pass a re-serialized workbook.** Document this as the load-bearing invariant.
2. **Distinct provenance class for SDK-authored workbooks.** If the SDK programmatically mutates a workbook (the renderer's `render_workbook`, or any fixture authoring like the lighthouse `author_lighthouse_dv.rs` bin), it must NOT then feed that file to the freshness gate as a trusted oracle. Tag SDK-authored output with a distinct provenance marker (e.g. an `OracleProvenance.authoring_app = "pmcp-workbook-compiler"` class) and make the gate REFUSE it — fabricated Excel identity from umya must be classified as `oracle/non-excel-app`, not accepted. The renderer (Phase 12) deliberately uses the writer-only `rust_xlsxwriter`, NOT umya, partly to avoid this; preserve that choice.
3. **Anchored identity, not substring.** The gate must require `<Application>` to START with "Microsoft Excel" (trimmed), not `.contains` — the lighthouse fixed a `.contains` hole (`gate.rs:248-252`, WR-03 in Phase 8) where "Not Microsoft Excel" / "FauxMicrosoft Excelerator" passed.
4. **Fail-closed defaults.** A raw-reader failure (malformed/oversize/missing OOXML part) must produce an `oracle/*` `Severity::Error` refusal with `stale = true`, never a panic or a silent default that fabricates passing values (`gate.rs:103-119, 317-333`).

**How it interacts with the Phase-8 oracle-staleness gate:**
The gate is *objective-metadata-only* — it proves real-Excel recalc *provenance*, it does NOT prove semantic agreement between a cached `<v>` and its `<f>` (that is Phase 10 penny-reconciliation). The fabrication trap defeats the provenance proof specifically. If umya-authored fixtures pass the gate, the penny-reconciliation in Phase 10 would then "reconcile" against fabricated oracle values — a green pipeline built on a lie. **The two gates are independent layers and the provenance layer must not be bypassable by the SDK's own writer.**

**Warning signs:**
- Any test fixture authored by writing a workbook with umya and then ingesting it as a "real" oracle.
- The renderer or any tooling depending on umya's write path (it should use `rust_xlsxwriter`).
- The gate accepting a workbook whose `authoring_app` was set by SDK code.
- A `calcId` of exactly `122211` (umya's hard-coded value) in any accepted `OracleProvenance`.

**Phase to address:**
The Phase-8-equivalent (trusted-oracle ingest + staleness gate) extraction phase. The renderer extraction phase (Phase-12-equivalent) must re-confirm the writer is `rust_xlsxwriter` (not umya) and that rendered output is never re-ingested as an oracle. Add a regression test: author a workbook with umya, assert the gate REFUSES it with `oracle/non-excel-app` (or the new SDK-author class).

---

### Pitfall 3: Generalizing the manifest — hardcoded `build_reference_manifest` does not survive a second workbook

**What goes wrong:**
The lighthouse's `build_reference_manifest` (`lib.rs:524-606`) **inlines the ufh-quote workbook's entire schema as literal Rust**: the `heat_source` enum input with `allowed_values: ["heat_pump","boiler"]`, the four cost input cells (`7_Quote!C9/C10/...`), the margin constant (`2_Constants!Margin`), the supply-total output (`7_Quote!C11`), plus their dtypes/units/meanings/cells. Worse, the `mk_role` helper *hardcodes `Dtype::Number`* and the enum input is a hand-written `CellRole` exception (`lib.rs:573-585`). If this is copied as-is and a second, different workbook is compiled, the served tool schema would describe ufh-quote's inputs regardless of the actual workbook — the generalization is a no-op. Everything (dtype, role, units, allowed_values, tiers) must be **synthesized from the workbook + ratified manifest**, with zero per-workbook Rust.

**Why it happens:**
The lighthouse was a single-workbook proof; hardcoding the reference manifest was the fastest path to a green golden-reconcile and let the schema-projection layer be developed against a known shape. The hardcoding is load-bearing for the lighthouse's tests (`build_reference_manifest` is called by `renderer_equivalence_layout_and_cell_map` and the emit path, `lib.rs:351, 678`), so it is not obviously "debt" until you try to compile a different workbook.

**What breaks when a second workbook is compiled (the generalization stressors):**
- **dtype inference must generalize** beyond `Dtype::Number` — a Text/enum input, a date, a boolean must all be inferred from the cell's value + DV, not assumed Number (the `mk_role` Number hardcode breaks immediately).
- **role projection** (Input/Constant/Output/Formula) must come entirely from the colour/Guide/header synthesis + BA ratification, not a literal cell list.
- **`allowed_values`** must be synthesized from the workbook's data-validation lists (Phase-14 inline-literal resolution), not a hand-written exception.
- **tiers** (`InputTier`) must be assigned by `ratify_tiers` per inferred dtype/role — and must handle enum inputs correctly (see Pitfall 5 / WR-01).
- **units/meaning** must come from the two-layer metadata model (named ranges + hidden `_Manifest` sheet), not literals.

**How to avoid:**
Delete `build_reference_manifest` from the served/emit path entirely. The served tool schema must be projected from the bundle's `manifest.json` at runtime (`schema.rs` style projection), and the manifest must be produced solely by the synth pipeline (`manifest/synth.rs`) over the ingested workbook. Keep an equivalent ONLY as a *test* fixture that asserts the synthesized manifest for the reference workbook equals the expected shape (anti-drift), never on the production path.

**Warning signs:**
- Any `CellRole` constructed with literal cell addresses (`"7_Quote!C9"`) outside a test.
- `Dtype::Number` appearing as a default/hardcode in a manifest builder.
- The served schema for a freshly-compiled second workbook still mentions ufh-quote inputs.
- A workbook ID (`ufh-quote`, `"7_Quote"`, `"1_Inputs"`) hardcoded in non-test Rust.

**Phase to address:**
The manifest-synth extraction phase (Phase-7/11-equivalent) plus the generic-served-tool phase. Verification: compile TWO different workbooks (the reference + a deliberately-different second fixture, e.g. a simple margin calculator) and assert each server's `get_manifest` / `tools/list` schema reflects ITS OWN inputs with zero shared Rust. This second-workbook test is the single most important generalization gate.

---

### Pitfall 4: Change-class governance correctness gaps (CR-01 demotion asymmetry, CR-02 version overwrite) — silent auto-promote of breaking changes + baseline destruction

**What goes wrong:**
Two Critical findings from the lighthouse `14-REVIEW.md` that MUST be fixed during extraction (RFC §5):

- **CR-01 (demotion asymmetry):** `classify_cell_roles` (`change_class/mod.rs:165-234`) only inspects the *current* cell's role for cells present in both manifests. Promotions are caught (non-input→input, non-assumption→assumption), but **demotions escape classification entirely**:
  - `Input → Constant/Formula`: a flip *away* from `Role::Input` emits no class (the `Role::Constant | Role::Formula => {}` arm, `mod.rs:217`) — an input is silently dropped from the served schema with NO `InputSchema` change. The Phase-14 enum-domain check (`allowed_values` change) is *also* bypassed by this route.
  - Assumption → non-assumption (yellow-assumption `source` edited): `is_assumption(cur)` is false, falls into the `Constant | Formula => {}` arm, removed-keys loop skips it (key still exists) → **zero classes → `effective_policy(&[]) == HotReload` → auto-promotes with no BA review**, defeating the D-09 `NeverAutoPromote` hard rule.
  Result: a breaking schema change (dropping a required input, re-classifying an assumption) **auto-promotes silently**. The numeric gate does not catch it when the computed value is unchanged.

- **CR-02 (version overwrite / baseline destruction):** `gate_and_promote` computes `next_version = bump_patch(prev.version)` for the changelog `to_version` but **never sets `candidate.version`** (it stays the hardcoded `"1.0.0"` from `build_candidate_model`). `write_candidate_bundle` then writes to `{name}@{candidate.version}` = the SAME `@1.0.0/` directory just used as the baseline. Consequences, all visible in the committed lighthouse bundle:
  - `BUNDLE.lock` says `1.0.0` while `evidence/changelog.json` says `1.0.0 → 1.0.1` — `diff_version` reports a transition the bundle's own provenance contradicts.
  - The prior baseline's `manifest.json`/`executable.ir.json` are **overwritten and destroyed** every promotion — the audit baseline the changelog `from_version` references no longer exists (data loss).
  - The on-disk version never advances, so `find_prior_bundle_dir` re-discovers `1.0.0` and re-bumps to `1.0.1` forever; the semver-greatest baseline selection can never engage.
  - Compounding bug WR-04: the approval fingerprint anchors on `prev.version` (a label) not the bundle content hash, and with CR-02 the label never changes — so cross-baseline no-inherit protection is inert.

**Why they're dangerous:**
This is the *governance* heart of the system — the whole point is that a BA edit becomes a *gated* compile/promote cycle, never a live reinterpretation. CR-01 means a breaking change can ship to agents with no human approval. CR-02 means the audit trail (which the `--accept`/approver/effective-date flow exists to produce) is destroyed on write — you cannot prove what the prior version was, and version provenance is internally inconsistent. Both undermine the trust model that justifies exposing BA spreadsheets to agents at all.

**How to verify the fix:**
- **CR-01 fix:** classify from BOTH sides in the present-in-both branch — assumption involvement on either side is an `Assumption` class; a role flip *away from* `Input`/`Output` is an `InputSchema`/`OutputSchema` class (the review gives the exact patch, `14-REVIEW.md:63-86`). **Symmetric classification tests:** assert `assumption→constant`, `input→constant`, `output→formula`, and `enum-drop + role-flip` EACH produce a non-empty change class that routes to `BlockUntilAccept`/`NeverAutoPromote`, never `HotReload`. Property test: for any pair (A,B) of manifests, `classify(A→B)` and `classify(B→A)` produce the same *cardinality* of schema-axis classes (symmetry invariant).
- **CR-02 fix:** set `candidate.version = next_version` and `candidate.changelog.to_version = next_version` before `write_candidate_bundle`, so promotion writes a NEW `{name}@{next_version}/` directory and the prior baseline survives. **Version-store integration tests:** promote twice and assert (a) two distinct on-disk version directories exist, (b) the prior baseline's `manifest.json` is byte-identical before/after the second promote (not overwritten), (c) `BUNDLE.lock` version == `changelog.to_version` (internal consistency), (d) `diff_version` reports a real `N → N+1` transition matching the directories on disk. Also fix WR-04: anchor the approval fingerprint on the prior bundle's `BUNDLE.lock` `combined` content hash, not the version label.

**Warning signs:**
- A promote that succeeds with no BA `--accept` after a role/source edit.
- `BUNDLE.lock` version ≠ `changelog.to_version` in any emitted bundle.
- The bundles directory never grows a second `@x.y.z` folder across promotions.
- A change-class test suite that only tests promotions, not demotions.

**Phase to address:**
The promote-gate / change-class extraction phase (Phase-13-equivalent) and the bundle-store generalization phase. CR-02 specifically must be fixed *before* generalizing the bundle store (RFC §5) — the bundle-store abstraction (`BundleSource` trait) assumes versioned, immutable, non-overwriting directories. These are not "port then fix later"; they must be redesigned in the port.

---

### Pitfall 5: Enum-input tiering (WR-01) — `ratify_tiers` seeds an out-of-enum empty string on the default path

**What goes wrong:**
`ratify_tiers` maps every untiered `Role::Input` to `InputTier::Variable` with a dtype-derived default (`Dtype::Text → CellValue::Text("")`). For an enum input (`allowed_values: Some([...])`), this stamps `tier: {variable, default: Text("")}` — and `validate_input` step 1 seeds the cell to `""` on **every call** where the input is absent. `""` is not a member of the enum, and the membership gate is **present-only** (`input.rs:224-227` checks supplied inputs via `value.as_str()`, not seeded defaults), so the system seeds an *illegal out-of-enum value* into the evaluator on the default path. In the lighthouse this is numerically inert today (the `heat_source` enum cell `1_Inputs!C6` isn't wired into the IR), but the moment an enum input is wired into a computation, every default-path call computes from an illegal value — a plausible-but-wrong result with no error.

**Why it happens:**
`ratify_tiers` was written before enum inputs existed (Phase 14 added them); the tiering logic predates the `allowed_values` axis and was never made enum-aware. The locked test `served_manifest_advertises_optional_heat_source_enum` passes because it runs against the PRE-emission `build_reference_manifest`, not the post-`ratify_tiers` committed manifest — a test/production skew that hides the bug (`14-REVIEW.md:106-110`).

**How to avoid / verify the fix:**
Make `ratify_tiers` **skip tiering for inputs carrying `allowed_values`** (leave them untiered/optional, so an absent enum input never seeds anything), OR default them to the FIRST enum member (a guaranteed-legal value). The review prefers skip. **Verification test:** load the COMMITTED `manifest.json` (post-emission, not the pre-emission builder) and assert: any `Role::Input` with `allowed_values: Some(vs)` has either `tier: None` OR a default that is a member of `vs`. Property test: for any synthesized manifest, no seeded default is ever outside its cell's `allowed_values`.

**Warning signs:**
- Any default value `Text("")` on an input that also has `allowed_values`.
- Enum-membership tests that run against a pre-emission manifest builder instead of the committed bundle.
- A "present-only" gate paired with auto-seeded defaults (the two must agree on the default).

**Phase to address:**
The manifest-synth / tiering extraction phase (Phase-14-equivalent), co-located with the enum-input work. Verify against the committed bundle, not the in-memory builder.

---

### Pitfall 6: Determinism / penny-reconciliation regressions when generalizing the formula DAG + evaluator to arbitrary workbooks

**What goes wrong:**
The lighthouse reconciles the golden quote to ±£0.01 against the oracle, and its reconcile classifier (`reconcile/classifier.rs`) is sophisticated: it **forbids the naive `delta.abs() < X` tolerance heuristic** (an explicit grep gate asserts `delta.abs()` never appears in the file, `classifier.rs:19-21`) and instead anchors acceptance on *operand-anchored rounding boundaries* — a divergence is only acceptable if the deciding cell is a rounding op (`ROUND`/`ROUNDUP`/`CEILING`), its operand sits within `BOUNDARY_EPSILON = 1e-6` of the rounding boundary, AND the divergence is ≤ one rounding step (`classifier.rs:281-292`). Generalizing the formula parser/DAG to *arbitrary* workbooks exposes Excel-semantics edge cases the single golden workbook never hit:

- **Excel float quirks:** Excel uses IEEE-754 f64 but displays 15 significant digits and has its own rounding rules (round-half-away-from-zero, not banker's rounding) — a generalized evaluator must replicate `excel_round`/`excel_ceiling`/`excel_floor` (`workbook_runtime::sheet_ir::rounding`) exactly, not Rust's `f64::round` (which differs at .5 / is half-to-even in some paths).
- **Excel date serial / 1900 leap-year bug:** Excel treats 1900 as a leap year (serial 60 = the non-existent Feb 29 1900). Any date arithmetic in a generalized workbook hits this.
- **Empty-cell coercion:** an empty cell coerces to 0 in arithmetic but to "" in concatenation — getting this wrong yields plausible-but-wrong results.
- **Error propagation:** `#DIV/0!`, `#N/A`, `#VALUE!` propagate through formulas; a generalized evaluator must model Excel error values, not panic or produce NaN→0.
- **Order-of-operations / implicit intersection / type coercion** edge cases the one golden workbook never exercised.

**Why it happens:**
A single golden workbook reconciling to the penny gives false confidence that the formula semantics are general. Excel's quirks are numerous and the lighthouse only had to be correct for ONE formula DAG. The temptation is to use a generic float tolerance to "absorb" divergences — exactly the anti-pattern the lighthouse explicitly forbids, because a tolerance band hides real semantic errors.

**How to avoid:**
- **Keep the operand-anchored rounding model, never a blanket tolerance.** Port the `reconcile/classifier.rs` discipline and its grep gate (`delta.abs()` must not appear). A divergence is either explained by a *specific* rounding-boundary mechanism or it is a reconciliation FAILURE.
- **Port `workbook-runtime::sheet_ir::rounding` exactly** (`excel_round`/`excel_ceiling`/`excel_floor`) and unit-test against Excel-produced values, not Rust stdlib rounding.
- **Whitelist-only function set** (the dialect's `DIA-05` whitelist): a generalized parser must REFUSE functions outside the compilable subset with a precise reason code, not silently mis-evaluate. Generalization MUST NOT expand the whitelist implicitly.
- **Per-workbook penny-reconciliation is mandatory in the pipeline** — every compile reconciles the compiled DAG against the workbook's own cached oracle values; a divergence outside the rounding model blocks the bundle. This makes the oracle the test, per workbook, automatically.
- **Build a fixture corpus of Excel-quirk workbooks** (dates spanning 1900, empty-cell coercion, division errors, half-rounding boundaries) and reconcile each — generalization confidence comes from breadth of fixtures, not the single golden case.

**Warning signs:**
- `delta.abs() < epsilon` or any blanket tolerance appearing in reconcile code.
- Rust `f64::round` / `.round()` used instead of the `excel_round` helper.
- A second workbook reconciles "close enough" rather than exactly within the rounding model.
- NaN, `inf`, or a silent `0` appearing where Excel would show `#DIV/0!`.

**Phase to address:**
The formula-parser / DAG-compile / reconcile extraction phases (Phase-9/10-equivalent). The Excel-quirk fixture corpus is a generalization-specific addition that the lighthouse did not need; it belongs in the reconcile phase and is the primary generalization verification.

---

### Pitfall 7: Governance / security gaps when exposing BA-edited spreadsheets to agents

**What goes wrong:**
The system deliberately exposes business-analyst-authored spreadsheets to agents. Several abuse/bypass vectors must be closed during generalization:

- **Cell-content injection:** a BA (or anyone who edits the workbook) can put arbitrary strings in cells — `meaning`, `unit`, `name`, enum values, error messages. These flow into the served tool schema (`description`, `enum`) and into structured error `allowed` repair fields, which an agent's LLM reads. Untrusted-looking content (prompt-injection payloads in a cell comment or a DV list) could reach the model. The dialect's whitelist-only ingest mitigates formula injection, but *string metadata* is a softer surface.
- **Enum-membership runtime gate bypass (WR-05):** `validate_input` only runs the dtype + enum gate `if let Some(role) = manifest.cells.find(...)` (`input.rs:110-113`). If a `cell_map` input entry's `seed_coord` has no matching manifest `CellRole` (the two are separate embedded files that can skew across a partial regeneration), the supplied value seeds the evaluator with **no dtype check and no enum gate** — fail-open, the exact NaN→plausible-wrong-quote path the gate exists to close. Must fail CLOSED: a cell_map entry with no manifest role is an internal-consistency ERROR.
- **Numeric-enum fail-open (WR-02):** a numeric-dtyped enum (`"1,2,3"` DV on a Number cell) produces `{"type":"number","enum":["1","2","3"]}` — no JSON number satisfies a string enum (schema fail-closed) while the membership gate runs only via `value.as_str()` and silently no-ops for numbers (runtime fail-OPEN). The two halves disagree. Only attach `allowed_values` for Text-dtype inputs, or reject non-string values on any enum input.
- **`--accept` approval flow skippable:** the gated approval (`--accept --approver --effective-date`) is the human-in-the-loop that authorizes a breaking change. If CR-01 lets a breaking change escape classification (auto-`HotReload`), the approval is never even requested — the gate is *skippable by omission*. The approval flow's integrity depends entirely on the change classifier being complete (Pitfall 4).

**Why it happens:**
The threat model shifts when generalizing from a single trusted lighthouse workbook (authored by the project's own BA) to *any project's* workbook (authored by arbitrary BAs, potentially from less-trusted sources). Fail-open validation paths and string-metadata passthrough that were acceptable for one trusted workbook become attack surface at scale.

**How to avoid:**
- **Fail-closed everywhere in the served validation path:** a missing manifest role for a supplied input is an error (WR-05 fix — `.ok_or(...)` not `if let Some`), a non-string value on an enum input is rejected (WR-02 fix), a mismatched cell_map/manifest is rejected.
- **Treat cell-string metadata as untrusted:** length-cap and sanitize `meaning`/`unit`/`name`/enum strings before they enter tool schemas; do not let raw cell text become tool `description` without bounds. Document that BA-authored strings reach the agent.
- **The change classifier must be complete (Pitfall 4 CR-01 fix)** so the `--accept` flow can never be skipped by an unclassified change. Symmetric classification is a *security* property, not just correctness.
- **Bind approvals to content hashes** (WR-04 fix), not version labels, so an approval cannot be inherited across different bundle contents that share a version string.
- **Present-only gate + safe defaults must agree** (Pitfall 5 / WR-01) so no default seeds an out-of-domain value.

**Warning signs:**
- Any `if let Some(role) = ...` validation that silently skips when the role is absent (should be `.ok_or(...)` fail-closed).
- Raw cell strings reaching `tools/list` schema `description` without length/charset bounds.
- An enum gate that only fires for `value.as_str()`.
- An approval record keyed on a version label rather than a content hash.

**Phase to address:**
The generic-served-tool / input-validation extraction phase (Phase-14-equivalent input.rs/schema.rs) for the fail-closed fixes; the change-class phase for the `--accept`-skippability fix (shared with Pitfall 4). String-metadata sanitization is a generalization-specific hardening that should be a named task in the served-tool phase.

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Hardcode the reference manifest in Rust (`build_reference_manifest`) | Fast green golden-reconcile for one workbook | Generalization is a no-op; second workbook served wrong schema | Only as a TEST fixture, never on the emit/served path |
| Use umya to author test fixtures, then ingest them as oracles | One library for read+write | Fabricated Excel provenance silently passes the freshness gate | Never — author fixtures with a non-umya writer or use real Excel-saved `.xlsx` |
| `delta.abs() < epsilon` blanket reconciliation tolerance | Absorbs float noise quickly | Hides real semantic divergence; un-auditable | Never — use the operand-anchored rounding model |
| Port the `just purity-check` recipe "later" | Faster initial extraction | Reader leaks into served binary undetected | Never — the gate lands WITH the runtime crate |
| Classify only promotions, not demotions (CR-01) | Simpler classifier | Breaking changes auto-promote with no approval | Never — symmetric classification is a security property |
| Pin `candidate.version` while bumping changelog (CR-02) | Stable bundle path | Baseline destroyed on every promote; provenance inconsistent | Never — version must advance and prior baseline must survive |
| Anchor approvals on version label, not content hash (WR-04) | One fewer field to thread | Approvals inherit across different contents sharing a version | Never — anchor on `BUNDLE.lock` combined hash |
| `if let Some(role)` skip-when-absent validation (WR-05) | Tolerant of artifact skew | Fail-open: unchecked value seeds the evaluator | Never on a served value path — fail closed |

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| `umya-spreadsheet` (reader) | Letting it into the runtime/served tree; using its writer for fixtures | Quarantine in the compiler only; raw-bytes provenance read; never round-trip a workbook through it before the freshness gate |
| `rust_xlsxwriter` (writer) | Banning `zip` to keep readers out also bans the writer's container | Permit `zip`, ban only the reader/parser stack (`quick-xml`/`umya`/`calamine`); positively assert the writer IS present |
| Cargo feature unification (workspace) | A compiler feature on a sibling crate pulls umya into the served binary via unification | Run the purity gate per feature-combination in CI; keep compiler helpers out of any crate the served binary depends on |
| Bundle store (`BundleSource` trait) | Treating versioned dirs as mutable/overwritable (CR-02) | Versioned, immutable, append-only directories; never overwrite a published `@x.y.z` |
| Embedded vs local-dir bundles | Schema/IR skew across a partial regeneration | Content-hash the bundle (`BUNDLE.lock combined`); fail closed on cell_map/manifest mismatch |

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Re-reading the bundle from disk on every tool call | Latency per `calculate` | Load + validate the bundle once at server start; serve from in-memory IR | High request rate / large bundles |
| Re-projecting the JSON schema per request | CPU on `tools/list` | Project schema once at load, cache it | Frequent schema introspection |
| Large `Err` payloads (rich repair fields) cloned per validation failure | Memory churn on bad input | Acceptable per the lighthouse design (MTS-05 "allowed-values live in the error"); box the large variant if it dominates | Very high invalid-input rate |

## Security Mistakes

| Mistake | Risk | Prevention |
|---------|------|------------|
| Fail-open input validation when manifest role absent (WR-05) | Unchecked value seeds evaluator → plausible-wrong result | Fail closed: missing role = internal-consistency error |
| Numeric-dtype enum (WR-02) | Schema fail-closed, runtime fail-open — disagreement | Attach `allowed_values` only for Text dtype; reject non-string on enum inputs |
| Trusting umya-fabricated Excel provenance | Untrusted cached values admitted as a trusted oracle | Read provenance from original bytes; refuse SDK-authored / umya-round-tripped workbooks |
| Breaking change escaping classification (CR-01) | Auto-promote to agents with no human approval | Symmetric (promotion + demotion) classification |
| Raw BA cell strings reaching tool schema descriptions | Prompt-injection content reaches the agent's model | Length-cap + sanitize cell metadata before it enters schemas |
| Approval inherited across baselines (WR-04) | An approval authorizes content it never reviewed | Bind approval fingerprint to bundle content hash |

## UX Pitfalls

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| Single-workbook assumptions (bundle ID `ufh-quote`, corpus paths, justfile recipes hardcoded) | A second project cannot use the tooling | Project-level `pmcp.toml` mapping workbooks → bundle IDs; generalize CLI args |
| Rejecting named-range-backed DV lists with an opaque error | BA does not know why their enum was ignored | Precise reason codes (the lighthouse already does this for the deferred named-range case); document the extension seam |
| Misleading reason codes (IN-01: `not_inline_literal` for an empty literal) | BA hunts for a nonexistent range source | Distinct reason per case (`empty_literal`, `multiple_dvs`, `non_text_dtype`) |
| `--accept` flow buried | BA does not know a change needs approval | Clear gate output stating the change class and the exact `--accept --approver --effective-date` command to run |

## "Looks Done But Isn't" Checklist

- [ ] **Manifest generalization:** Compile a SECOND, different workbook and verify its served schema reflects ITS inputs — not ufh-quote's. (Often missing: only the reference workbook is ever compiled.)
- [ ] **Purity gate:** Run `cargo tree` per feature-combination, not just defaults — verify umya/quick-xml absent AND `rust_xlsxwriter` present. (Often missing: positive writer assertion; per-feature runs.)
- [ ] **Provenance trap:** Author a workbook with umya, assert the freshness gate REFUSES it. (Often missing: no negative test for fabricated identity.)
- [ ] **Change-class symmetry:** Test demotions (`input→constant`, `assumption→constant`, `output→formula`) each produce a class. (Often missing: only promotions tested.)
- [ ] **Version store:** Promote twice, assert two on-disk version dirs and the prior baseline survives byte-identical. (Often missing: only one promote ever run.)
- [ ] **Enum tiering:** Assert the COMMITTED manifest (not the in-memory builder) seeds no out-of-enum default. (Often missing: tests run against pre-emission builder.)
- [ ] **Reconcile:** Grep that `delta.abs()` / blanket tolerance never appears; reconcile an Excel-quirk fixture corpus (dates, errors, empty cells). (Often missing: only the golden case.)
- [ ] **Fail-closed validation:** No `if let Some(role)` skip on the served value path. (Often missing: skew-tolerant code that fails open.)

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Reader leaked into served binary | MEDIUM | Add the missing `cargo tree` gate; move the offending type/helper out of the shared crate; re-verify per feature |
| umya provenance trap shipped | HIGH | Audit every accepted bundle for `calcId=122211` / SDK authoring_app; re-ingest oracles from original Excel-saved files; quarantine umya to compiler |
| Hardcoded manifest shipped to generalized server | HIGH | Re-derive manifest from synth pipeline; add the second-workbook test; re-emit all bundles |
| CR-02 baseline destroyed | HIGH (data loss) | Reconstruct prior baseline from git/evidence; fix version write; re-emit with correct version dirs |
| CR-01 breaking change auto-promoted | HIGH | Symmetric-classify fix; re-audit promotion history for unapproved schema changes; require re-approval |
| WR-01 out-of-enum default shipped | LOW (if enum unwired) / HIGH (if wired) | Skip tiering for enum inputs; re-emit; assert committed manifest invariant |

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| 1. Purity-boundary erosion (umya leak) | Phase: port `pmcp-workbook-runtime` (FIRST cut) | `cargo tree` per-feature: reader stack absent, `rust_xlsxwriter` present; value-path token grep clean |
| 2. umya fabricated-provenance trap | Phase: trusted-oracle ingest + staleness gate (Phase-8 equiv); re-confirmed in renderer phase (Phase-12 equiv) | umya-authored workbook is REFUSED (`oracle/non-excel-app`); provenance read from original bytes; renderer uses `rust_xlsxwriter` not umya |
| 3. Hardcoded manifest generalization | Phase: manifest-synth + generic-served-tool (Phase-7/11 equiv) | Compile two different workbooks; each server's schema reflects its own inputs; zero per-workbook Rust |
| 4. CR-01 demotion asymmetry + CR-02 version overwrite | Phase: promote-gate / change-class + bundle-store (Phase-13 equiv) | Symmetric classification tests; promote-twice integration test (two dirs, baseline survives, lock==changelog) |
| 5. WR-01 enum-input tiering | Phase: manifest-synth / tiering (Phase-14 equiv) | Committed-manifest invariant: no out-of-enum seeded default |
| 6. Determinism / penny-reconciliation | Phase: formula-parse / DAG-compile / reconcile (Phase-9/10 equiv) | Per-workbook penny-reconcile; Excel-quirk fixture corpus; `delta.abs()` grep gate; `excel_round` parity tests |
| 7. Governance / security (WR-05, WR-02, injection, skippable `--accept`) | Phase: generic-served-tool / input validation (Phase-14 equiv) + change-class phase | Fail-closed validation tests; numeric-enum rejection; cell-string sanitization; classifier completeness gates the approval flow |
| Single-workbook assumptions (`pmcp.toml`) | Phase: CLI + project-config | A second project compiles via `pmcp.toml` workbook→bundle mapping with no lighthouse paths |

## Sources

- `towelrads-quote-pricing/.planning/phases/14-*/14-REVIEW.md` — CR-01, CR-02, WR-01..WR-05, IN-01..IN-04 findings (HIGH — independent goal-backward review of the lighthouse) [primary source for Pitfalls 4, 5, 7]
- `crates/workbook-compiler/src/provenance/gate.rs` — inline "Pitfall 2" docs, the D-01∧D-07 freshness conjunction, anchored-identity check (`gate.rs:248-252`), fail-closed paths (HIGH) [primary source for Pitfall 2]
- `justfile` `purity-check` recipe (`justfile:54-92`) — the proven reader-vs-writer cargo-tree + value-path-grep gate design (HIGH) [primary source for Pitfall 1]
- `crates/workbook-compiler/src/change_class/mod.rs:165-234` — `classify_cell_roles` demotion hole (HIGH) [Pitfall 4 CR-01]
- `crates/workbook-compiler/src/lib.rs:524-606` — `build_reference_manifest` hardcoded schema, `mk_role` Dtype::Number hardcode (HIGH) [Pitfall 3]
- `crates/workbook-compiler/src/reconcile/classifier.rs:19-21, 281-292` — operand-anchored rounding model, the forbidden-`delta.abs()` discipline, `BOUNDARY_EPSILON` (HIGH) [Pitfall 6]
- `crates/quote-pricing-server/src/workbook/input.rs:110-113, 224-227` — present-only enum-membership gate, fail-open `if let Some(role)` (HIGH) [Pitfall 7 WR-05]
- RFC `docs/sdk-issue-excel-workbook-compiler-extraction.md` §5 known generalization gaps (HIGH — the explicit do-not-copy list)
- `docs/workbook-dialect-spec.md` — whitelist-only function set (DIA-05), refuse-set (DIA-02), named-range deferral (HIGH) [Pitfall 6 whitelist, UX named-range]

---
*Pitfalls research for: Excel-as-Configuration MCP-server compiler extraction + generalization (PMCP v2.3)*
*Researched: 2026-06-09*
