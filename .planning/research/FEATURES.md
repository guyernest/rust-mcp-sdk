# Feature Research

**Domain:** Governed-Excel → MCP-server compiler (a new "workbook" CodeLanguage for the PMCP SDK, parallel to SQL/OpenAPI), extracted from the `towelrads-quote-pricing` lighthouse (its phases 7–14, all green).
**Researched:** 2026-06-09
**Confidence:** HIGH — every feature below was read directly from the lighthouse source (served-tool layer `crates/quote-pricing-server/src/workbook/{handler,error,input,schema,diff_version,render_resource,mod}.rs`, `crates/workbook-compiler/src/lib.rs`, the architecture brief, dialect spec, and the extraction RFC). MEDIUM only where a behavior is "shape-only" in the lighthouse (flagged inline).

> NOTE: this file replaces an unrelated stale FEATURES.md (rmcp comparison, 2026-04-10) that predated the v2.3 milestone.

---

## Orientation: what "generalize from the lighthouse" means

The lighthouse is a *single* workbook (`ufh-quote@1.0.0`) embedded into one server crate at compile time. Almost every served-tool behavior is **already bundle-driven** (schemas projected from `manifest.json`, I/O mapped through `cell_map.json`), so the generalization is mostly *removing the last hardcoded literals* and *adding a project config layer*, not rebuilding. The known per-workbook debt is concentrated and named in the RFC §5:

- `build_reference_manifest` / `emit_reference_bundle` in `workbook-compiler/src/lib.rs` inline `"ufh-quote"`, `"1.0.0"`, the `heat_source` enum, and the supply-total cell as literals. **These must be driven from the compiled workbook, not Rust.**
- `cell_map.supply_total_cell` is a single named "headline" output. The general case is *N named outputs* (the `outputs` array already exists alongside it — `project_outputs` in `handler.rs` already iterates it). Generalizing = stop treating one output as privileged.
- Bundle IDs, corpus paths, and justfile recipes are lighthouse-bound → need a project-level `pmcp.toml` mapping workbooks → bundle IDs.
- Promote-path bugs CR-01 / CR-02 / WR-01 (RFC §5) must be fixed, not copied.

The runtime↔served boundary is already clean: the served binary links **only** `workbook-runtime` (no `umya` reader, no SWC, no `quick-xml`/`zip`) and a `just purity-check` cargo-tree gate enforces it. The compiled **bundle** is the entire compiler↔server interface.

---

## Feature Landscape

### Table Stakes (must ship to be a credible CodeLanguage parallel to SQL/OpenAPI)

A "CodeLanguage" in this SDK means: a config/source artifact compiles to a curated, typed, self-describing MCP tool surface served by a thin bundle-driven binary, scaffolded by `cargo pmcp new`, with the same outputSchema→structuredContent discipline the SQL/OpenAPI toolkits already use. Everything here is the floor for parity.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| **`calculate` tool** (typed inputs → typed outputs + provenance) | The core "run the workflow" verb; a workbook server is useless without it | MEDIUM | `handler.rs`: validate → seed via `cell_map` → re-run embedded IR (`workbook_runtime::run_executor`) → read headline output → `project_outputs` (units-bearing `{value,unit}` map) → stamp. Generalize: emit `outputs` for all named outputs, drop the single privileged `supply_total`. |
| **`get_manifest` tool** (curated agent-facing description) | Agents must discover *meaning* (inputs/tier/default/unit, outputs/unit/meaning, governed data, versions, changelog) not just JSON shape | LOW | `handler.rs::curated_manifest` — already a clean projection of the embedded `Manifest`; strips lint-only internals (`colour_evidence`, `source`). Workbook-agnostic as-is. |
| **Manifest-driven input schema** (`additionalProperties:false`, per-column dtype/unit/meaning) | Discovery + strict validation; the "self-describing input" half of the safety story | MEDIUM | `schema.rs::input_schema_for_manifest` already builds it per-`Role::Input` from the manifest. The hardcoded `build_reference_manifest` is the debt to kill (RFC §5). |
| **Manifest-driven output schema** (MTS-02, mandatory non-empty `outputSchema`) | Parity with SQL/OpenAPI's `TypedToolWithOutput`; the "rediscover meaning not shape" mitigation | MEDIUM | `schema.rs::output_schema_for_manifest` → `{value,unit}` per output column + provenance envelope. Depends on SDK `ToolInfo::with_output_schema`. |
| **Structured-error envelope with repair fields** (`isError:true` in `structuredContent`) | Agents must self-repair bad calls; a protocol `Err` is unreadable to an LLM | MEDIUM | `error.rs`: `WorkbookToolError` → `to_iserror_result`. Six codes (below). Always carries the provenance stamp on the failure path. **Generic already.** |
| **The `allowed` repair field on enum/strict violations** | "Allowed-values live in the error" — the agent reads `allowed`/`required`/`range` and retries | LOW | `error.rs`: `strict_constant_override` and `unsupported_option` carry `allowed`; `missing_field` carries `required`; the schema envelope (`result_envelope_schema`) declares all repair fields so a strict client type-checks an error result. Generic. |
| **Tier enforcement on inputs** (strict constant rejected, variable-tier accepted) | A BA-governed constant must not be silently overridable per-request | MEDIUM | `input.rs::validate_input` + `workbook_runtime::is_strict_constant`. Fix WR-01 (RFC §5): enum inputs must not get a `Variable{default:""}` tier seeding an out-of-enum empty string. |
| **dtype type-check before seeding** (WR-01) | A non-number for a numeric cell coerces NaN→0 → plausible-but-wrong answer; must reject loudly | LOW | `input.rs::check_value_dtype` runs before the evaluator is seeded. Generic. |
| **Provenance stamp on every result** (`workflow@version + workbook_hash`) | Every number traceable to the exact spec that produced it; the "no untrusted execution" message made visible | LOW | `ProvStamp.to_json()` attached to success AND error results. Read from `BUNDLE.lock`. Generic. |
| **The compiled bundle contract** (manifest/IR/cell_map/layout/lock/evidence) | The compiler↔server interface; without it there is no compile-not-interpret | HIGH | See "Bundle Contract" section. Already serde-clean and shared between emitter and server (`workbook_runtime` owns the types). |
| **Bundle integrity gate at boot** (recompute `BUNDLE.lock` combined hash, fail-closed) | A tampered embedded artifact must be caught before serving | MEDIUM | `mod.rs` `load_bundle` recomputes the hash-of-hashes from all members; panics at startup on mismatch. Generic. |
| **`cargo pmcp compile-workbook`** (gated compile driver) | The proven core CLI verb; the BA's compile+gate+promote cycle | MEDIUM | `workbook-compiler/src/commands/compile_workbook.rs`: build-candidate → gate → write (gate runs BEFORE the bundle is written). Lift the `cargo run -p workbook-compiler -- compile-workbook …` recipe into a first-class subcommand. |
| **`cargo pmcp new --kind workbook-server`** (scaffold) | Parity with `cargo pmcp new --kind sql-server`; the on-ramp DX | MEDIUM | Thin binary over `BundleSource` + the generic served-tool toolkit module. Depends on the existing scaffold machinery. |
| **Generic served-tool layer as a `pmcp-server-toolkit` module** | So the scaffold is a thin shell, not copied Rust per workbook | MEDIUM | RFC OQ-2 recommends this. The `workbook/` module is *already* workbook-agnostic except the embedded-path literals and the single-output assumption. |
| **`BundleSource` (local-dir + embedded)** | Servers must load a bundle from somewhere; lighthouse only has compile-time embed | MEDIUM | New trait. Local-dir + `include_dir!`/`include_str!` embedded impls. S3/registry deferred (anti-feature). |
| **SDK-owned versioned dialect spec + linter** | The dialect is the moat; workbooks declare a dialect version they target | MEDIUM | `docs/workbook-dialect-spec.md` is the contract, bound to the `WHITELIST` const by `doc_whitelist_table_matches_const`. Move ownership to SDK; add a declared dialect version. The 13-fn whitelist (8 core + 5 widened) + the refuse-set is the enforced contract. |

### Differentiators (the compile-not-interpret governance story — this is the moat)

These are why "governed Excel" is *more* than "SQL/OpenAPI for spreadsheets." Each aligns with the Core Value: *the workbook is simultaneously specification (formula DAG), test oracle (cached values = assertions), and output template.*

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| **`explain` tool** (per-cell business-language derivation trace) | An auditable "show your working" — formula + operand values + manifest meaning per step; safety-by-transparency in a quoting context | MEDIUM | `handler.rs::render_steps`: ordered traces from `RunResult.traces`, each cell rendered with its manifest meaning. **No SQL/OpenAPI analog.** |
| **The "workbook corrected the code" keystone** (zero-delta logic-divergence annotation surfaced first in `explain`) | The lighthouse narrative made a feature: when the workbook rule (`CEILING(req*1.05,50)`) and a legacy algorithm agree, that reconciliation is the headline step | MEDIUM | `handler.rs` reads `bundle.coil_band` from embedded evidence (classifier is NOT re-run — it lives compiler-side). Generalize from a lighthouse-named field to a generic `reconciliation_annotations[]`. |
| **`render_workbook` tool** (workbook-as-output-template → downloadable `.xlsx`) | "Download the fully-computed Excel" — the workbook's third role; the writer is reader-free (`rust_xlsxwriter`) keeping the served binary `umya`-free | HIGH | `handler.rs::RenderWorkbookHandler` returns a provenance-bound `workbook://` *pointer* (not inline bytes); `render_resource.rs` regenerates-on-read. **Unique to this CodeLanguage.** |
| **Provenance-verified, regenerate-on-read `workbook://` resource** | Stateless (Lambda-safe), anti-spoofing (URI provenance must match the embedded bundle before any render), re-validated (URI payload is untrusted) | HIGH | `render_resource.rs::regenerate`: decode → verify provenance → re-validate → re-run → render → base64-in-`text` (SDK `Content::Resource` has no `blob` field). `list()` returns `[]` (resolvable but unlisted). |
| **`diff_version` tool** (recorded, hash-verified prev→current changelog) | Schema *semantic* drift is worse than a crash in quoting; agents read recorded per-output deltas (change class + drift/redefinition severity + human summary) | MEDIUM | `diff_version.rs` serves `bundle.changelog` (folded into the combined hash at emit, re-verified at boot) — NOT a forgeable runtime diff. |
| **Closed enums from Excel data-validation lists** (Phase 14) | A DV dropdown (`heat_pump,boiler`) becomes a JSON-Schema `{"enum":[...]}` + a runtime membership gate; the agent sees the closed domain at discovery | MEDIUM | `schema.rs` emits `enum` from `role.allowed_values`; input stays OPTIONAL (no forced `required`). Out-of-set → `unsupported_option` carrying `allowed`. Deferred: named-range-backed lists (anti-feature). |
| **Change-class governance model** (HotReload / BlockUntilAccept / NeverAutoPromote) | The trust boundary's *semantic axis*: a margin update hot-reloads; an input-schema change blocks until a BA `--accept`; an assumption change can never auto-promote | HIGH | `change_class::{classify, policy, effective_policy}` with a strictest-policy reducer (assumptions hard-block). Fix CR-01 (RFC §5): demotion-direction changes currently escape classification. |
| **Golden-corpus + numeric/semantic schema-diff promote gate** | "Promotion bar is not tests-green but every output delta is zero or BA-approved" — approval testing, fingerprint-bound | HIGH | `gate::{corpus,gate,accept}`: blocks any over-tolerance named-output delta unless a fingerprint-matching `ApprovalRecord` covers *this* candidate; `--accept` re-baselines. `schema_diff::diff_outputs` distinguishes numeric drift from semantic redefinition via `ir_subdag_hash`. |
| **The `--accept --approver --effective-date` approval flow** | The BA's golden-gate review becomes its own reviewable, recorded artifact (an `ApprovalRecord` in `cases.json`) | MEDIUM | `gate::accept::accept`. RFC OQ-3: CLI vs higher governance surface — recommend CLI for v1 (proven). |
| **Self-describing outputs** (name/unit/meaning/provenance in the outputSchema) | The "rediscover meaning, not shape" mitigation against silent LLM rewiring of a changed field | LOW | Emitted by `output_schema_for_manifest`; the differentiator is units+meaning riding in the schema, sourced from workbook headers/colour/notes. |
| **Trusted-oracle ingest + staleness gate** (cached values candidate until provenance proves a real-Excel recalc) | The test oracle is only trustworthy if the cached cell values came from a real recalc, not a fabricated one | HIGH | `provenance` module reads `calcPr` + app-identity from raw `.xlsx` bytes. **Account for the umya fabricated-provenance caveat (RFC §5):** umya hardcodes `<Application>Microsoft Excel</Application>` on save → a umya-authored workbook passes the freshness gate on fabricated identity. SDK tooling that mutates workbooks needs a distinct provenance class. |

### Anti-Features (explicitly out / deferred by design)

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| **Live workbook interpretation on the hot path** | "Just run the spreadsheet" feels simpler | Dissolves the entire security message; running untrusted Excel in prod is exactly what this avoids | **Compile, never interpret.** The bundle is the only thing served; the reader never enters the served binary (`just purity-check` enforces). |
| **Named-range-backed validation lists** (Phase 14) | BAs naturally point DV lists at a named range | The range source is unresolved at synth time; resolving it safely is its own parser problem | **Deferred by design.** Phase 14 resolves only inline `formula1` quoted literals (≤10 values); the disqualifier predicate rejects range/named-range sources with precise reason codes. Extension seam documented. |
| **S3 / registry bundle store** | "Servers should pull the latest bundle from a registry" | Adds a network trust surface + cache-invalidation complexity before the local case is proven | **`BundleSource` with local-dir + embedded only for v1;** S3/registry as a documented seam (RFC OQ-4). |
| **Code-mode `validate → token → execute` ceremony for workbooks** | "Apply the same untrusted-code gate everywhere" | A compiled workbook is *curated* config, trusted by promote-time gate + BA curation, not a runtime token | **Plain directly-callable MCP tool.** The HMAC-token machinery is for the untrusted long-tail (`pmcp-code-mode`), which this explicitly does NOT touch. |
| **Copying `build_reference_manifest` / per-workbook Rust** | "It works for the lighthouse, just parameterize it" | Hardcoded `heat_source`/`ufh-quote`/supply-total literals don't generalize | **Fully manifest-driven schema** — no per-workbook Rust; the bundle carries everything. |
| **A single privileged "headline" output** (`supply_total_cell`) | The lighthouse has one obvious answer | Most workbooks have N named outputs of equal standing | **Emit all named `outputs`** (the array + `project_outputs` already exist); generalize away the privileged single cell. |
| **Widening the function whitelist on demand** | "Our workbook uses OFFSET / INDIRECT / a macro" | Coverage-vs-guarantee law: more Excel = weaker compile guarantee; volatile/array/macro semantics break determinism | **Deny-by-default, located lint error** with a "express this using the supported set" repair; grow the 13-fn whitelist deliberately, one verified function at a time. Route the tail to a developer. |
| **Capability cells (Rust/remote/MCP escape hatches)** | "Excel can't do forecasting; call SageMaker" | Real but non-deterministic; contract-tested not value-tested; needs Cedar/AVP policy wiring | **Out of v1 scope** (brief §7/§12 "later"). The pure-cell penny-golden path ships first. |
| **Row-block iteration / "for each room" loops** | The lighthouse has fixed rows 15–21 / 37–43 | The hardest parser problem (arbitrary-N generalization); no code-mode analog | **Out of v1 scope** for the extraction; generalization is a later milestone (brief §11 risk). |

---

## The served tool surface (catalogue)

All five tools are native `pmcp::ToolHandler` impls registered via `tool_arc` + `ToolInfo::with_ui` (so the returned `Value` lands in `structuredContent`). Every result carries the provenance stamp; every domain failure returns the `isError:true` envelope (never `Err(pmcp::Error)`). All are driven **purely by the compiled bundle** — no per-workbook handler code.

| Tool | Input | Output (success) | Error shape | Bundle-driven by |
|------|-------|------------------|-------------|------------------|
| **`calculate`** | `{inputs:{<key>:<v>}, overrides:{<k>:<v>}}`, `deny_unknown_fields`; tier-enforced; dtype-checked; enum-gated | `{<headline>, outputs:{<key>:{value,unit}}, accepted_overrides[], provenance}` | `isError`+`code`+`reason`+`field`+`allowed`/`range`/`required`+`provenance` | `manifest` (schema/tiers/enums), `cell_map` (key→coord, named outputs), `ir`+`dag` (executor) |
| **`explain`** | same as `calculate` | `{steps[] (ordered derivation: formula+operands+meaning, coil-band keystone first), final_value, final_cell, provenance}` | same envelope | `ir`/`dag` traces + `manifest` meanings + embedded `coil_band` evidence |
| **`get_manifest`** | `{}` (empty) | `{workflow, version, workbook_hash, inputs[](tier/default/unit), outputs[](unit/meaning), governed_data[], changelog[], provenance}` | same envelope (no live trigger today) | `manifest` + `BUNDLE.lock` stamp |
| **`diff_version`** | `{}` (empty) | `{from_version, to_version, deltas[](region, change_class, old/new meta, severity), summary, provenance}` | same envelope (boot-time parse failure → fail-closed) | embedded `evidence/changelog.json` (hash-verified at boot) |
| **`render_workbook`** | same as `calculate` (same strict validate path) | `{resource_uri (workbook://…), accepted_overrides[], instructions, provenance}` — a *pointer*, not bytes | same envelope | `manifest`+`cell_map` (validate), `layout` (render on resource read) |

**Bytes path:** `render_workbook` hands back a `workbook://` URI; `resources/read` of it (`render_resource.rs`) regenerates the `.xlsx` statelessly (provenance-verified, re-validated, re-run, rendered, base64-in-`text`). The URI is resolvable but `list()` returns `[]` (unenumerated).

**The six MTS-05 error codes:** `invalid_input`, `missing_field`, `unsupported_option`, `strict_constant_override` (all runtime-triggered, with repair fields), plus `stale_oracle` and `unapproved_assumption` (**shape-only in the lighthouse** — emittable in the correct shape but no runtime trigger wired; the oracle-freshness re-gate is deferred). The codes are entirely generic; wiring the two deferred triggers is an SDK opportunity (v1.x).

---

## The compiled bundle contract (compiler↔server interface)

```
bundles/<workflow>@<version>/
  manifest.json        # semantic manifest — the interface
  executable.ir.json   # compiled formula DAG — the interface
  cell_map.json        # I/O map (key→seed_coord, named outputs) — the interface
  layout.json          # output-template layout — the interface (render only)
  BUNDLE.lock          # per-artifact + combined hash-of-hashes + workbook_hash anchor
  evidence/
    changelog.json            # recorded prev→current VersionChangelog (served by diff_version)
    golden_corpus.json        # golden cases + coil-band reconciliation annotation
    recalc_metadata.json      # oracle provenance projection
    unsupported_features.json # dialect/lint findings
    renderer_equivalence.json # named-region read-back vs oracle
bundles/corpus/<workflow>/cases.json  # golden corpus + fingerprint-bound ApprovalRecords
```

| File | Contents | Compiler↔server interface? | Read by |
|------|----------|---------------------------|---------|
| `manifest.json` | inputs/outputs, dtypes, units, meanings, tiers, `allowed_values` enums, governed data, capability calls | **YES** (load-bearing) | input/output schema, validation, `get_manifest` |
| `executable.ir.json` | compiled formula DAG (typed `HashMap<String,Cell>`) | **YES** | `run_executor` for `calculate`/`explain`/`render` |
| `cell_map.json` | input-key → seed_coord, named output cells, headline cell | **YES** | input seeding, output projection |
| `layout.json` | full captured workbook-layout descriptor (cells, formats, merges, fills) | **YES** (render only) | `render_xlsx` on `resources/read` |
| `BUNDLE.lock` | per-artifact SHA-256 + combined hash-of-hashes + `workbook_hash` anchor | **YES** (integrity) | boot integrity gate, provenance stamp |
| `evidence/changelog.json` | recorded `VersionChangelog` (deltas + change class + severity + summary) | **YES** (served) | `diff_version` |
| `evidence/golden_corpus.json` | golden cases + coil-band annotation | partial (annotation served) | `explain` keystone |
| `evidence/{recalc_metadata,unsupported_features,renderer_equivalence}.json` | provenance/lint/equivalence evidence | NO (folded into hash only) | boot hash fold |
| `corpus/<wf>/cases.json` | golden corpus + fingerprint-bound `ApprovalRecord`s | NO (compiler/gate only) | promote gate, `--accept` |

The **types are shared**, not mirrored: `workbook_runtime` owns `Manifest`, `CellMap`, `BundleLock`, `VersionChangelog`, the IR `Cell`/`Expr`, and the executor. The offline emitter and the served binary deserialize the *same* definitions — the boundary that keeps the reader out of the served tree. **CR-02 fix required (RFC §5):** promotion computes `next_version` but writes back into the same `@1.0.0` directory, overwriting the baseline — fix before generalizing the bundle store.

---

## The CLI surface

| Subcommand | Behavior | Category | Complexity | Notes |
|------------|----------|----------|------------|-------|
| `cargo pmcp compile-workbook <wb.xlsx>` | ingest → lint → manifest synth → parse → DAG compile → penny-reconcile → build candidate → **gate** → write bundle | Table stakes | MEDIUM | Gate runs BEFORE write (build-candidate→gate→write split). Blocks on change-class. |
| `… --accept --approver <X> --effective-date <D>` | re-baseline the golden corpus, record a fingerprint-bound `ApprovalRecord` | Differentiator | MEDIUM | The BA approval flow; recorded in `cases.json`. RFC OQ-3: keep in CLI for v1. |
| `cargo pmcp lint-workbook <wb.xlsx>` | run the dialect linter only; collect-all, located, BA-actionable findings | Table stakes | LOW | Whitelist-only, deny-by-default; the refuse-set (macros/external-links/hidden/array/CF-on-role/merged-role). |
| `cargo pmcp emit-bundle` | regenerate a bundle without the gate (dev/reference) | Differentiator (dev DX) | LOW | The `emit_reference_bundle` path; useful for scaffolding/regenerating. |

The lighthouse runs these as `cargo run -p workbook-compiler -- …` + justfile recipes; the extraction promotes them to first-class `cargo pmcp` subcommands and replaces single-workbook justfile assumptions with a project-level `pmcp.toml` mapping workbooks → bundle IDs.

---

## The change-class governance model (what's table stakes for a governed-config CodeLanguage)

The trust boundary has two axes. The **numeric axis** (golden corpus) catches value drift; the **semantic axis** (change-class + schema-diff) catches redefinition. Having a promote gate at all is table stakes; the *specific three-class policy* is the differentiator.

| Change class | Policy | Meaning |
|--------------|--------|---------|
| **HotReload** | auto-promote | governed-data value change (a margin/price) — re-baseline silently within tolerance |
| **BlockUntilAccept** | block until `--accept` | input-schema / structural change — needs a recorded BA approval |
| **NeverAutoPromote** | hard-block | assumption (yellow-cell) change — can never auto-promote; strictest-policy reducer wins |

- **Auto-derivation:** `change_class::classify` derives the class from a prior-vs-current manifest+IR diff.
- **Strictest-policy reducer:** `effective_policy` — an assumption change hard-blocks even if other deltas are HotReload-able.
- **Numeric/semantic split:** `schema_diff::diff_outputs` distinguishes drift from redefinition using a stable canonical IR sub-DAG identity hash (`ir_subdag_hash`).
- **Fix CR-01 (RFC §5):** demotion-direction changes (Input→Constant, source flips) currently escape classification and auto-promote — the classifier needs symmetric coverage before generalizing.

**Table-stakes minimum** for the extraction: the golden-corpus numeric gate + the three-class policy + the `--accept` re-baseline. The schema-diff redefinition detector is a differentiator but high-value (silent semantic drift is the named worst case in a quoting context).

---

## Feature Dependencies

```
The compiled bundle contract (manifest/IR/cell_map/layout/lock/evidence)
    └──requires──> pmcp-workbook-compiler pipeline (ingest→lint→synth→parse→DAG→reconcile→emit)
                       └──requires──> SDK-owned versioned dialect spec + linter
    └──requires──> pmcp-workbook-runtime (owned types + executor + renderer, reader-free)
                       └──enforced-by──> just/CI purity gate (umya never in served tree)

Served-tool layer (calculate/explain/get_manifest/diff_version/render_workbook)
    └──requires──> the compiled bundle contract
    └──requires──> BundleSource (local-dir + embedded)
    └──requires──> SDK TypedToolWithOutput / outputSchema → structuredContent   [EXISTING SDK]
    └──requires──> SDK resource serving for workbook://                          [EXISTING SDK]

render_workbook tool ──requires──> workbook:// resource handler + render_xlsx (writer-only)
                                        └──requires──> layout.json in the bundle

diff_version tool ──requires──> evidence/changelog.json (folded into BUNDLE.lock)
                                    └──requires──> the promote-time change-class router

Closed-enum inputs ──requires──> Phase-14 DV-list → allowed_values synthesis
                                    └──conflicts──> named-range-backed lists (deferred)

cargo pmcp new --kind workbook-server ──requires──> generic served-tool toolkit module
                                                        └──requires──> existing scaffold machinery [EXISTING SDK]

--accept approval flow ──requires──> golden-corpus + change-class gate
```

### Dependency Notes

- **Served layer requires the bundle, which requires the compiler, which requires the dialect.** This forces phase ordering: dialect/runtime first (RFC §7 "smallest cut that proves the boundary"), then compiler, then served layer + CLI.
- **`render_workbook` requires `layout.json`** in the bundle and the reader-free `render_xlsx` writer — the *only* tool needing the layout member.
- **`diff_version` requires the change-class router** to have produced `evidence/changelog.json` at promote time — the served tool reads a recorded artifact, never recomputes.
- **Closed-enum inputs conflict with named-range lists:** Phase 14 ships inline-literal enums only; named-range resolution is a documented deferred seam, not a v1 combination.
- **The runtime↔compiler split is load-bearing:** the served binary must link only `workbook-runtime`. Every shared type (Manifest, IR, CellMap, BundleLock, VersionChangelog) lives in runtime so both sides agree without a reader dependency.

---

## Dependencies on existing SDK features

| Workbook feature | Depends on existing SDK capability | How |
|------------------|-----------------------------------|-----|
| All five tools' typed outputs | **`ToolInfo::with_output_schema` / `with_ui` → `structuredContent`** (the `TypedToolWithOutput`/outputSchema pattern the SQL/OpenAPI toolkits use) | Every handler advertises a non-empty `outputSchema`; the returned `Value` lands in `structuredContent`. The `isError:true` envelope rides the same slot. |
| `render_workbook` bytes | **Resource serving** (`ResourceHandler`, `Content::resource_with_text`, `resources/read`) | `workbook://` scheme dispatched alongside the existing `value-schema://` handler behind one `DispatchingResource` (because `.resources()` REPLACES). SDK `Content::Resource` has no `blob` field → base64-in-`text`. |
| Tool registration | **`ServerBuilder::tool_arc`** (the same seam `try_tools_from_config` uses) | Native handler registration; no code-mode arm. |
| Generic served-tool module | **`pmcp-server-toolkit` module pattern** (parallel to the SQL/OpenAPI toolkit modules) | RFC OQ-2: make the `workbook/` module a toolkit module so the scaffold is a thin shell. |
| `cargo pmcp new --kind workbook-server` | **The existing `cargo pmcp new` scaffold machinery** (`--kind sql-server` etc.) | Add a `workbook-server` kind scaffolding a binary over `BundleSource` + the toolkit module. |
| `compile-workbook`/`lint-workbook`/`emit-bundle` | **The `cargo pmcp` CLI command machinery** (`commands/<cmd>.rs` thin-shell layout) | The lighthouse already mirrors cargo-pmcp's command layout; lift into subcommands. |
| Capability cells (future) | **`pmcp-code-mode` policy layer** (Cedar/AVP, `[[code_mode.operations]]`), expression eval kernel | Brief §8: capability cells reuse code-mode's governed-operation node. v1 does NOT touch code-mode. |
| Deploy | **`cargo pmcp deploy`** (EmbeddedSource pattern; config travels in the ZIP) | The embedded `BundleSource` impl mirrors the existing EmbeddedSource Lambda pattern. |

**Explicitly NOT touched:** `pmcp-code-mode` (the untrusted long-tail path). The workbook CodeLanguage is the curated path; the two coexist (brief §4 two-surface model).

---

## MVP Definition

### Launch With (the extraction v1 — proves the boundary + a credible CodeLanguage)

- [ ] **`pmcp-workbook-runtime`** (owned types + executor + reader-free renderer + purity gate) — RFC §7 smallest cut; everything depends on it.
- [ ] **`pmcp-workbook-compiler`** (ingest→lint→synth→parse→DAG→reconcile→emit) with the bundle contract — no served path without it.
- [ ] **SDK-owned versioned dialect spec + linter** — the moat; workbooks declare a dialect version.
- [ ] **Generic served-tool module** (all five tools, fully manifest-driven — kill `build_reference_manifest`, generalize the single-output assumption) — parity floor.
- [ ] **`BundleSource`** (local-dir + embedded) — servers need to load a bundle.
- [ ] **`cargo pmcp compile-workbook` + `lint-workbook` + `emit-bundle`** with the gated `--accept --approver --effective-date` flow — the proven core CLI.
- [ ] **`cargo pmcp new --kind workbook-server`** scaffold — the on-ramp DX.
- [ ] **Change-class gate + golden-corpus promote gate** (with CR-01/CR-02/WR-01 fixes) — the governance differentiator that justifies "governed Excel."
- [ ] **Closed-enum inputs from inline DV lists** (Phase 14, inline-literal only) — self-repairing typed inputs.
- [ ] **`pmcp.toml`** project config mapping workbooks → bundle IDs — replaces single-workbook justfile assumptions.

### Add After Validation (v1.x)

- [ ] **Wire the two deferred error triggers** (`stale_oracle` oracle-freshness re-gate, `unapproved_assumption`) — currently shape-only.
- [ ] **Distinct umya-writer provenance class** — handle the fabricated-`<Application>Microsoft Excel</Application>` caveat for SDK-mutated workbooks.
- [ ] **Multi-output generalization hardening** — once N-output workbooks beyond the lighthouse are tested.

### Future Consideration (v2+)

- [ ] **Named-range-backed validation lists** — the documented deferred seam.
- [ ] **S3 / registry `BundleSource`** — the deferred bundle-store seam (RFC OQ-4).
- [ ] **Capability cells** (Rust builtin / remote endpoint / MCP-tool escape hatches) — reuses `pmcp-code-mode` policy; contract-tested not value-tested.
- [ ] **Row-block iteration detection** ("for each room" loops → arbitrary N) — the hardest parser, the load-bearing generalization.
- [ ] **Google Sheets ingest** — alternate front-end into the same dialect (brief §12).

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| pmcp-workbook-runtime (types+executor+renderer) | HIGH | MEDIUM | P1 |
| pmcp-workbook-compiler + bundle contract | HIGH | HIGH | P1 |
| SDK dialect spec + linter | HIGH | MEDIUM | P1 |
| Generic served-tool module (5 tools, manifest-driven) | HIGH | MEDIUM | P1 |
| BundleSource (local + embedded) | HIGH | MEDIUM | P1 |
| compile-workbook / lint-workbook / emit-bundle CLI | HIGH | MEDIUM | P1 |
| cargo pmcp new --kind workbook-server | HIGH | MEDIUM | P1 |
| change-class + golden-corpus promote gate (+CR/WR fixes) | HIGH | HIGH | P1 |
| --accept approval flow | HIGH | MEDIUM | P1 |
| closed-enum inputs (inline DV) | MEDIUM | MEDIUM | P1 |
| pmcp.toml workbook→bundle mapping | MEDIUM | LOW | P1 |
| wire deferred error triggers | MEDIUM | MEDIUM | P2 |
| umya fabricated-provenance class | MEDIUM | MEDIUM | P2 |
| named-range validation lists | MEDIUM | HIGH | P3 |
| S3/registry BundleSource | LOW | HIGH | P3 |
| capability cells | HIGH | HIGH | P3 |
| row-block iteration | HIGH | HIGH | P3 |

**Priority key:** P1 = must have for the extraction; P2 = add when possible; P3 = future milestone.

## Competitor Feature Analysis (parallel CodeLanguages in the same SDK)

| Feature | SQL toolkit | OpenAPI toolkit | Workbook (this) |
|---------|-------------|-----------------|-----------------|
| Source artifact | DB schema | OpenAPI contract | governed `.xlsx` (dialect-constrained) |
| Compile vs interpret | query at runtime | proxy at runtime | **compile offline; never interpret** |
| Typed tool surface | from schema | from contract | from compiled manifest |
| outputSchema → structuredContent | yes | yes | yes (same SDK pattern) |
| Self-describing meaning/units | columns | schema | **units+meaning+provenance per output** |
| Promote-time governance gate | n/a | n/a | **golden corpus + change-class (the differentiator)** |
| Output-template render | n/a | n/a | **download computed .xlsx (workbook:// resource)** |
| Versioned changelog tool | n/a | n/a | **diff_version (hash-verified)** |
| Scaffold (`cargo pmcp new --kind`) | sql-server | openapi-server | workbook-server |

The workbook CodeLanguage *matches* SQL/OpenAPI on the typed-surface/outputSchema/scaffold parity floor and *adds* the compile-not-interpret governance story (gate, provenance, render, diff) as its differentiating moat.

## Sources

- `docs/sdk-issue-excel-workbook-compiler-extraction.md` (extraction RFC; §3 bundle layout, §5 generalization gaps, §6 open questions) — HIGH
- `docs/Excel-as-Configuration-Architecture-Brief.md` (§4 two-surface model, §5 dialect, §7 cell taxonomy, §8 reuse-vs-new, §9 trust/versioning, §12 milestone shape) — HIGH
- `docs/workbook-dialect-spec.md` (13-fn whitelist, refuse-set, enforced-vs-deferred) — HIGH
- `crates/quote-pricing-server/src/workbook/{handler,error,input,schema,diff_version,render_resource,mod}.rs` (served-tool surface, error envelope, enum projection, bundle load) — HIGH
- `crates/workbook-compiler/src/lib.rs` (module docs, bundle emit, the hardcoded `build_reference_manifest` debt) — HIGH
- `.planning/PROJECT.md` v2.3 Current Milestone section (target features, generalization fixes) — HIGH

---
*Feature research for: governed-Excel → MCP-server compiler (workbook CodeLanguage extraction)*
*Researched: 2026-06-09*
