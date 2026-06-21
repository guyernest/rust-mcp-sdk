---
source: pmcp.run dev-team review of fix/cargo-pmcp-deploy-stack-ts (post-Phase-100)
reviewed: 2026-06-20
verified_against_code: true
status: gaps_found
scope_for_100-08: [H1, H2, H3, M4, M5, M6]
deferred_low: [L-i, L-ii, L-iii, L-iv]
---

# pmcp.run dev-team review — verified findings

The dev team reviewed the shipped Phase 100 workbook surface and asked us to hold
publish until the HIGH findings are fixed. Each finding below was **independently
verified against the current code** on `fix/cargo-pmcp-deploy-stack-ts`. The
purity gate, Lambda entrypoints, F2, and F3 (on the served path) were confirmed
correct and are NOT in scope here.

## HIGH — publish blockers (in scope for 100-08)

### H1 — `explain` is a divergent re-implementation that can lie about the served surface
**Verdict: CONFIRMED (all three sub-claims).** `cargo-pmcp/src/commands/workbook/explain_surface.rs` re-derives the tool surface from scratch instead of projecting through the production code, so the pre-deploy preview can disagree with what the server actually serves — defeating the feature's whole purpose.
- (a) **No F3 prefix strip:** `explain_surface.rs:164,207` use the raw Table/role name; the served path strips `in_`/`out_` via `json_key_for_role`/`strip_governance_prefix` (`manifest_model.rs:175`). → explain previews `in_income`/`out_tax_owed`; server serves `income`/`tax_owed`.
- (b) **Hand-rolled A1 walker:** `extract_a1_refs` (`explain_surface.rs:397-425`) rejects ranges (`SUM(B2:B9)`) and cross-sheet refs (`Sheet2!B5`, rejected at `:419`). Production uses the formula DAG (`upstream_input_leaves`). → per-tool inputs can differ.
- (c) **Wrong classification:** explain calls a table "input" iff it has a `tier` column (`:309`); production classifies output cells by `Role::Formula` (`lib.rs:764`, `promote_harvested_tables`).
**Fix:** make `explain` synth a **non-persisted** manifest and project it through the SAME production functions the server uses (`build_tools` / `json_key_for_role` / `input_schema_for_tool`), so it cannot drift by construction. Delete the bespoke `reachable_addrs`/`extract_a1_refs`/`is_input_table` derivation.
**Acceptance:** a test compiles `template.xlsx` AND runs `workbook explain` on it, asserting the explain output's tool names + per-tool input keys + types are byte-identical to the served tool surface (stripped keys, DAG-derived inputs incl. a cross-sheet/range case).

### H2 — F1 narrowed, not eliminated: a numeric/value-shaped name still ships
**Verdict: CONFIRMED.** `json_key_for_role` keeps the `name → meaning → cell` fallback (`manifest_model.rs:175`); `validate_input_keys` (`lib.rs:975`) rejects missing/empty/duplicate keys but NOT a numeric name. A BA typing `60000` in the `name` column yields a served key `"60000"` — the exact original F1 symptom. (Our earlier "structurally eliminates F1" overreached.)
**Fix:** in the `validate_input_keys` backstop, reject value-shaped served keys (numeric / pure-number-like) with a cell-precise `Severity::Error`.
**Acceptance:** a unit test feeds an input row whose `name` is `60000` and asserts compile fails with a cell-located error; a valid identifier name still compiles.

### H3 — reserved-tool-name collision silently drops the BA's tool
**Verdict: CONFIRMED.** The registration loop (`mod.rs:252-283`) registers per-table tools and the four meta tools (`explain`/`get_manifest`/`diff_version`/`render_workbook`) into the same builder (last-writer-wins); the only collision lint (`lib.rs:910`) checks table-vs-table, not against the reserved set. A table sanitizing to a reserved name is silently overwritten — no error, no lint.
**Fix:** add a blocking compile-time check (alongside the existing collision lint) that rejects any output-table tool name colliding with the reserved meta-tool set, with a cell-precise error.
**Acceptance:** a test with an output table named so it sanitizes to `explain` fails compile with a reserved-name error.

## MEDIUM — should-fix (in scope for 100-08)

### M4 — output-key collisions are unguarded (inputs are)
**Verdict: CONFIRMED (substance).** No `validate_output_keys` exists; tool-*name* collision is guarded but two outputs stripping to the same served key silently last-writer-wins in `outputSchema` + the runtime payload.
**Fix:** add `validate_output_keys` mirroring `validate_input_keys` (duplicate/empty/value-shaped) over each tool's output keys; wire into the stage-1 Error gate.
**Acceptance:** two outputs that strip to the same key fail compile.

### M5 — `get_manifest` reports the raw prefixed name, not the served key
**Verdict: CONFIRMED (substance).** `handler.rs:391,409` emit `role.name` (`in_income`/`out_tax_owed`) while the served tool schema advertises the stripped key — so an agent that reads `get_manifest` then calls the tool with the discovered name is rejected.
**Fix:** surface the stripped served key (the `json_key`) in the `get_manifest` input/output projections; keep the prefixed name only as internal/governance metadata if needed.
**Acceptance:** a test asserts `get_manifest`'s advertised input/output keys equal the served tool schema's keys.

### M6 — the per-tool reconcile is vacuous on the production table path
**Verdict: CONFIRMED.** Outputs are never tiered → `oracle_value` returns `None` (`cell_map.rs:424`) → every tool's oracle is empty → `reconcile_tools` never blocks, despite comments/commits implying it gates the emit. (Not a safety hole — the named-output reconcile still works — but a misleadingly-documented dead net.)
**Fix:** either populate each tool's oracle from the cached workbook cell values (as the named-output `comparison_from_outputs` path does) so per-tool reconcile actually grades the table path, OR remove the per-tool reconcile call + correct the comments. Prefer wiring it (it's the per-tool safety net the multi-tool design promised).
**Acceptance:** if wired — a test perturbs one output's cached value and asserts the per-tool reconcile blocks the emit; if removed — no comment/commit claims it gates.

## LOW — deferred to a tracked follow-up (NOT in 100-08)
- **L-i:** overrides keys are raw (`in_income`) while inputs are stripped (`income`) in the same schema (`schema.rs:401` vs `:320`).
- **L-ii:** the legacy `in_*`/`out_*` compile path still runs alongside the table path (`lib.rs:348-350,651-668`) — substrate finding #2 lives on.
- **L-iii:** stale rustdoc says "five tools / calculate" (`mod.rs:5-6,164-165`).
- **L-iv:** the `tax-calc@1.1.0` byte-stability golden is still hand-built/authoritative (mitigated by the real-compile E2E).
