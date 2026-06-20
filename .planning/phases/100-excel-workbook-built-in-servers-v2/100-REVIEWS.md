---
phase: 100
reviewers: [codex]
reviewed_at: 2026-06-20T17:53:12Z
plans_reviewed: [100-01-PLAN.md, 100-02-PLAN.md, 100-03-PLAN.md, 100-04-PLAN.md, 100-05-PLAN.md, 100-06-PLAN.md]
---

# Cross-AI Plan Review — Phase 100

> Reviewer: **Codex** (OpenAI `codex exec`, default model). Running inside Claude Code, so the
> `claude` CLI was skipped for independence. Single external reviewer this run (`--codex`).

## Codex Review

## Summary

The phase is well researched and the six-plan wave structure is mostly sound: template first, ingest, model lift, served fan-out, preview/docs, then gates. The biggest risks are not conceptual but execution-order and contract mismatches: Plan 02 claims malformed umya table XML becomes `CompileError` while explicitly saying not to catch panics, Plan 03 may break the workspace before Plan 04 by replacing `CellMap.outputs` before served call sites are updated, and Plan 04 packs several large retirements plus served fan-out into one wave. The plans can achieve Phase 100, but they need a few tighter interfaces and intermediate compatibility shims to avoid long red periods.

## Strengths

- Clear dependency sequencing from reference template → ingest → model → served runtime → CLI/docs → gates.
- Strong attention to the reader-free purity boundary; `umya` remains compiler-only and `rust_xlsxwriter` stays test/generator-side.
- Good insistence on property/fuzz/unit/example coverage, especially around DAG reachability and tool-name/schema boundaries.
- Plan 04 correctly recognizes the atomicity risk in the single-tool to multi-tool fan-out.
- The F2 keep / F1-F3 retire ledger is explicit, which lowers the risk of half-migrating the old named-range model.
- Validation has concrete commands and artifacts rather than vague "test it" checkpoints.

## Concerns

- **HIGH: Plan 02 panic-safety is internally inconsistent.** It says malformed table XML must become a clean `CompileError`, but also says "do NOT catch panics here." If umya really panics on malformed table XML, a fuzz target will find a crash unless ingest wraps the umya read/table extraction boundary with `catch_unwind` or isolates parsing another way.

- **HIGH: Plan 03 may leave the workspace uncompilable until Plan 04.** Replacing `CellMap.outputs` with `tools` while `schema.rs`, `handler.rs`, `mod.rs`, and compiler reconciliation still reference `outputs` will likely break downstream crates before the next wave. Either Plan 03 needs transitional compatibility, or Plan 03 and Plan 04 must be merged.

- **HIGH: Plan 04 is very large and couples fan-out, reconciliation, lints, fixture retirement, and legacy deletion.** It is correctly atomic at the served boundary, but the scope is broad enough that debugging failures will be hard. Fixture deletion in the same plan increases blast radius.

- **MEDIUM: Template provenance expectations look slightly muddled.** The plan includes `template.provenance-override.json` while also requiring `classify(template.xlsx) == ExcelTrusted`. If the template is genuinely trusted, the override should not be necessary for that assertion. If an override is needed, the test is not proving the stated property.

- **MEDIUM: Tool-name canonicalization is underspecified.** The design allows `^[a-zA-Z0-9_-]{1,64}$`, but tests expect `Calculate_Tax` to become `calculate_tax`. Lowercasing should be locked explicitly, including collision handling after sanitization.

- **MEDIUM: Sanitized tool-name collisions are not covered.** `Calculate Tax`, `calculate_tax`, and `calculate-tax` could collide depending on sanitizer rules. The compiler should fail with cell-precise diagnostics for post-sanitize duplicates.

- **MEDIUM: Plan 02 per-row harvest needs stronger coordinate mechanics.** The plan says harvest table rows but does not spell out enough about mapping table header columns to `CellRecord`s by worksheet coordinates, handling blank physical rows, totals rows, hidden rows, or tables on non-default sheets.

- **MEDIUM: The property tests may become too shallow.** Several properties allow "thin harness" testing of projection helpers. That will not catch real workbook/table coordinate bugs. At least one property or integration test should exercise actual generated `.xlsx` ingest.

- **MEDIUM: Per-tool reconcile aggregation is underdefined.** Plan 04 says aggregate per-tool `Comparison`s into the existing report shape, but this is a likely type/API friction point. The report model should be specified before execution.

- **LOW: Two committed copies of `template.xlsx` can drift.** The plan says byte-identical but does not add an explicit byte-equality test.

- **LOW: Docs negative grep may produce false positives or false confidence.** A blanket ban on `named range` could block legitimate "retired model" migration notes, while not proving docs are complete.

## Suggestions

- Add an explicit Plan 02 task to make `ingest::ingest` panic-contained for table parsing, with a test using a known malformed table XML fixture. The fuzz target should verify that boundary, not merely hope it exists.

- Add a transitional `CellMap` shape during Plan 03 if plans remain separate: `inputs`, `tools`, and maybe deprecated/defaulted `outputs` or helper accessors until Plan 04 lands. Alternatively, merge Plans 03 and 04 into one execution wave.

- Split Plan 04 internally into two commits/checkpoints: first served fan-out with old legacy paths still compiling, then reconciliation/lints/retirement. Keep it one wave, but reduce debugging blast radius.

- Define sanitizer semantics precisely: lowercase or preserve case, illegal-run replacement, trimming, all-invalid behavior, and duplicate detection after sanitization.

- Add tests for sanitized name collision and exact lint location for duplicate tool names.

- Make `template.provenance-override.json` either justified or removed from WBV2-01. If it remains, add separate tests for raw provenance classification vs override behavior.

- Add a byte-equality test for the two committed template locations, or avoid duplication by having one canonical artifact and copying at packaging time.

- In Plan 02, require at least one test that ingests the real generated `template.xlsx` and verifies row-level harvested fields: key, dtype, unit, enum, tier, description, and table/caption linkage.

- In Plan 04, define the per-tool reconcile report structure before refactoring `comparison_from_outputs`, including how multiple failures are displayed and how exit status is derived.

- Add a Plan 06 explicit grep/check for retired symbols: `strip_governance_prefix`, `name_named_inputs`, `CalculateHandler`, and old `in_`/`out_` fixture injection sites.

## Risk Assessment

**Overall risk: MEDIUM-HIGH.** The design direction is strong and the plan coverage is unusually thorough, but the implementation crosses several deep contracts at once: persisted artifact shape, compiler ingest, runtime DAG model, served tool registration, schema projection, and CLI/docs. The top risks are temporary uncompilable states between Plans 03 and 04, panic containment around umya table parsing, and Plan 04's broad scope. With the suggested compatibility shim or merged wave, explicit panic boundary, and sanitizer collision tests, this drops to **MEDIUM**.

---

## Consensus Summary

Single external reviewer (Codex) this run, so "consensus" reflects Codex's findings plus how they
intersect the in-house plan-checker pass (which had already PASSED after fixing 2 blockers + 4 warnings).

### Agreed Strengths

- Sound wave sequencing (template → ingest → model → served → CLI/docs → gates).
- The reader-free **purity boundary** discipline (umya compiler-only; rust_xlsxwriter generator/test-side) — flagged by both Codex and the in-house checker as well-handled.
- Explicit F2-keep / F1-F3-retire cleanup ledger lowers half-migration risk.
- Concrete validation commands/artifacts rather than vague checkpoints.

### Agreed Concerns (highest priority — feed into `--reviews` replan)

1. **[HIGH] Plan 03 transitional compile state.** Replacing `CellMap.outputs` → `tools` in wave 3 while served call sites still reference `outputs` until wave 4 risks a multi-wave red workspace. *New finding* — the in-house checker verified per-plan atomicity but did not flag the cross-wave 03→04 compile gap. **Strongest candidate for action:** add a defaulted/deprecated `outputs` accessor (or `#[serde(default)]` shim) in Plan 03, or merge 03+04 execution.
2. **[HIGH] Plan 02 panic-safety wording is self-contradictory.** "malformed XML → clean CompileError" vs "do NOT catch panics here." Lock the umya table-parse boundary containment mechanism (`catch_unwind` at the ingest seam or equivalent) and make the fuzz target assert *that* boundary.
3. **[HIGH] Plan 04 blast radius.** Already atomic-by-design, but Codex recommends internal commit checkpoints (served fan-out first while legacy compiles, then reconcile/lint/retire) — complements the in-house "add sub-checkpoints" warning that was already applied.
4. **[MEDIUM] Tool-name sanitizer is underspecified** (lowercasing rule + **post-sanitize collision** detection with cell-precise diagnostics). The revision added a charset property test but not a *collision* test — genuine gap.
5. **[MEDIUM] Provenance-override vs ExcelTrusted contradiction** in WBV2-01 (`template.provenance-override.json` should be unnecessary if the template is genuinely trusted).
6. **[MEDIUM] Per-tool reconcile report shape** should be specified before refactoring `comparison_from_outputs_for_tool` (multi-failure display + exit-status derivation).
7. **[MEDIUM] Property tests risk being too shallow** — at least one property/integration test should ingest the real generated `template.xlsx`, not just thin projection helpers.

### Divergent Views

- None (single reviewer). Note one tension to weigh during replan: Codex's "merge Plan 03 and Plan 04" suggestion conflicts with the in-house checker's wave-granularity preference and the design's atomic-fan-out-in-one-plan rule. The **compat-shim** alternative (keep plans separate, add a transitional `outputs` accessor) satisfies both and is the recommended resolution.
