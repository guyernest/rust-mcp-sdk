# Project Research Summary

**Project:** PMCP SDK v2.3 — Excel-as-Configuration MCP Servers (governed Excel CodeLanguage)
**Domain:** Compile-not-interpret workbook → MCP-server toolchain; extraction from the TowelRads `quote-pricing` lighthouse into two new SDK crates + a `pmcp-server-toolkit` module
**Researched:** 2026-06-09
**Confidence:** HIGH

## Executive Summary

The v2.3 milestone extracts a working, penny-reconciled Excel-workbook compiler from the `towelrads-quote-pricing` lighthouse (milestone v0.5.0, phases 7–14, all green, ~730 workspace tests) into the SDK as a new "governed Excel" CodeLanguage. The core architectural invariant is already proven in the lighthouse: the Excel reader (`umya-spreadsheet`) lives exclusively in an offline compiler crate and never enters the served-binary dependency tree, enforced by a `cargo tree`-provable purity gate. The served binary evaluates a pre-compiled IR with a pure-Rust scalar evaluator and returns provenance-stamped, typed, structured outputs across five tools (`calculate`, `explain`, `get_manifest`, `diff_version`, `render_workbook`). The extraction slots this alongside the v2.2 SQL and OpenAPI toolkits as a third CodeLanguage under the same `pmcp-server-toolkit` + `cargo pmcp new --kind` + `TypedToolWithOutput` DX pattern the roadmapper already knows.

The stack footprint is minimal: the entire workbook capability adds exactly two non-trivial new third-party crates — `umya-spreadsheet 3.0` (reader, compiler-only) and `rust_xlsxwriter 0.95` (writer, runtime-only) — plus pinned transitive matches `quick-xml 0.37` and `zip 8`. Everything else (serde, schemars, sha2, hex, thiserror, chrono, jsonschema) is already in the SDK workspace at exact version matches. The formula parser, DAG, and Excel-semantics layer are hand-rolled in the lighthouse and must be lifted verbatim; no external formula crate enforces the dialect whitelist-at-parse-time safety primitive, and adopting one would lose the core security property.

The critical risk concentration is narrow but serious: (1) the purity boundary between reader and served tree can silently erode during extraction if the gate is not established on day one, and (2) the lighthouse carries three named promote-path bugs (CR-01 demotion asymmetry, CR-02 version overwrite, WR-01 enum tiering) that must be fixed at extraction time, not copied. The recommended build order is runtime-first (RFC §7: smallest cut proving the boundary before any `umya` code lands), then the served-tool toolkit module, then the compiler with the §5 generalization fixes, then CLI + `pmcp.toml`, then Shape A binary, then Shape B scaffold + dialect spec. The second-workbook test — compile a different workbook and verify the served schema reflects its own inputs — is the single most important generalization gate.

## Key Findings

### Recommended Stack

The lighthouse already runs on every SDK-standard version, so v2.3 is not a dependency upgrade exercise. The two new crates are deliberately role-partitioned by the purity rule: `umya-spreadsheet 3.0` is the sole Excel reader and is confined to the offline compiler crate; `rust_xlsxwriter 0.95` (writer-only, `default-features = false`) is the sole workbook emitter in the runtime and deliberately has no parser surface. The quarantined provenance reader uses `quick-xml 0.37` and `zip 8` pinned to match `umya`'s own transitive lock — these must be re-derived via `cargo tree -p umya-spreadsheet -i quick-xml` at extraction rather than independently resolved. The formula/DAG stack is zero-dependency hand-rolled Rust (~52 KB tokenizer+parser+Pratt, ~200 LOC Kahn DAG) lifted verbatim from the lighthouse; `petgraph` and any formula crate must not be introduced.

The purity gate is a three-layer defense: (1) `cargo tree` negative assertions per served-tree crate with a positive assertion that `rust_xlsxwriter` IS present, (2) `cargo-deny` `[bans]` as a declarative backstop, and (3) the structural crate split itself — `umya` is never a `[dependencies]` entry of `pmcp-workbook-runtime` or the toolkit `workbook` module. The gate must run in CI per feature-combination (not just default features) to block Cargo workspace feature unification from silently leaking a reader in.

**Core technologies:**
- `umya-spreadsheet 3.0` (compiler only): full-fidelity `.xlsx` read — cells, formulas, cached values, colours, named ranges, DV lists, custom sheets. The only mature pure-Rust reader covering the compiler's full surface. Must never enter the runtime tree.
- `rust_xlsxwriter 0.95` (runtime only, `default-features = false`): writer-only `.xlsx` emitter for `render_workbook`. Keeps the served binary reader-free; pulls `zip` as a deflate container (permitted) but has no parser surface.
- `quick-xml 0.37` / `zip 8` (compiler only, transitive-pinned): quarantined raw-bytes provenance reader detecting `umya`'s fabricated `<Application>Microsoft Excel</Application>` + `calcId`. Pin must match `umya`'s own resolved lock.
- Hand-rolled formula/DAG (compiler + runtime): whitelist-gated-at-parse-time Pratt parser + owned `Dag` + Kahn toposort + `sheet_ir` Excel-semantics. The whitelist is the security primitive — no off-the-shelf formula crate can replace it.
- `serde 1` / `serde_json 1` / `schemars 1.0` / `sha2 0.11` / `hex 0.4` / `thiserror 2` / `chrono 0.4` / `jsonschema 0.46`: all already pinned in the SDK workspace at identical versions; zero version changes needed.
- `pmcp-workbook-runtime` as the compiler's re-export source: shared IR types (`Expr`, `Dag`, `Manifest`, `CellMap`, `BundleLock`, `VersionChangelog`) live in the runtime leaf so the served binary deserializes them without linking the reader.

### Expected Features

The generalization is narrower than it looks: the lighthouse's served layer (`schema.rs`, `input.rs`, `handler.rs`) is already ~95% workbook-agnostic. The work is (a) deleting the hardcoded `build_reference_manifest` / `emit_reference_bundle` and routing `manifest::synthesize` → `ratify` → `emit_bundle` instead, (b) fixing CR-01/CR-02/WR-01, and (c) adding the project-level `pmcp.toml` to kill single-workbook assumptions.

**Must have (table stakes) — the credible CodeLanguage floor:**
- `pmcp-workbook-runtime` crate: owned IR types + deterministic topo executor + pure-Rust `scalar_eval` + output-template renderer; reader-free; purity-gated
- `pmcp-workbook-compiler` crate: full offline pipeline (ingest → dialect lint → manifest synth → formula parse → DAG compile → penny-reconcile → artifact emit → promote-time gate); `umya` isolated here
- Compiled bundle contract: `manifest.json` / `executable.ir.json` / `cell_map.json` / `layout.json` / `BUNDLE.lock` / `evidence/` — the compiler↔server interface; boot integrity gate (hash-of-hashes, fail-closed)
- All five served tools fully manifest-driven: `calculate` (tier-enforced, enum-gated, structured error with repair fields), `explain` (per-cell business-language derivation trace), `get_manifest` (curated agent-facing description), `diff_version` (hash-verified recorded changelog), `render_workbook` (provenance-bound `workbook://` resource, writer-only)
- `BundleSource` trait with `LocalDirSource` + `EmbeddedSource` impls
- Generic served-tool layer as a `pmcp-server-toolkit` `workbook` feature module (`try_workbook_from_bundle`)
- Change-class governance model: three classes (HotReload / BlockUntilAccept / NeverAutoPromote) + strictest-policy reducer + `--accept --approver --effective-date` BA approval flow
- Golden-corpus + numeric/semantic schema-diff promote gate
- `cargo pmcp compile-workbook` / `lint-workbook` / `emit-bundle` CLI subcommands
- `cargo pmcp new --kind workbook-server` scaffold
- SDK-owned versioned dialect spec (13-fn whitelist + refuse-set); workbooks declare dialect version
- Project-level `pmcp.toml` mapping workbooks → bundle IDs (kills lighthouse justfile hardcodes)
- Generalization fixes: manifest-driven emit (delete `build_reference_manifest`), CR-01 symmetric change-class, CR-02 versioned bundle dir write, WR-01 enum-input tiering skip
- Closed-enum inputs from inline DV lists (Phase-14 inline-literal path only)

**Should have (differentiators — the compile-not-interpret moat):**
- `explain` tool: ordered per-cell derivation trace with manifest meanings + keystone reconciliation annotation (no SQL/OpenAPI analog)
- `render_workbook` tool: workbook-as-output-template → provenance-verified, regenerate-on-read `workbook://` resource; stateless (Lambda-safe)
- `diff_version` tool: hash-verified recorded version changelog, not a forgeable runtime diff
- Change-class governance: semantic-drift detection (`ir_subdag_hash`) distinguishing numeric drift from redefinition
- Trusted-oracle ingest + staleness gate: `calcPr` + `<Application>` provenance check from raw bytes; distinct provenance class for SDK-authored workbooks
- Umya fabricated-provenance handling: quarantined raw-bytes provenance reader refusing `umya`-round-tripped workbooks with `oracle/non-excel-app`
- Self-describing outputs (name/unit/meaning/provenance in outputSchema)

**Defer (v2+):**
- Named-range-backed validation lists: deferred by design; extension seam documented with precise reason codes
- S3/registry `BundleSource`: documented seam on `BundleSource` trait; local + embedded are the v1 impls
- Capability cells (Rust builtin / remote endpoint / MCP-tool escape hatches): reuses `pmcp-code-mode` policy; v1 does not touch code-mode
- Row-block iteration ("for each room" loops → arbitrary N): hardest parser problem; separate milestone
- Wire deferred error triggers (`stale_oracle` oracle-freshness re-gate, `unapproved_assumption`): currently shape-only in the lighthouse
- Distinct umya-writer provenance class for SDK-mutated workbooks: P2

### Architecture Approach

The architecture is two non-overlapping dependency cones meeting only at the runtime leaf and the on-disk bundle contract. The offline cone — `cargo-pmcp` → `pmcp-workbook-compiler` → `pmcp-workbook-runtime` — owns `umya`, `quick-xml`, and `zip` exclusively in the compiler; the served cone — served binary → `pmcp-server-toolkit[workbook]` → `pmcp-workbook-runtime` → `pmcp` — has zero reader deps. The bundle (compiled artifact set) is the entire compiler↔server interface. The `BundleSource` trait abstracts bundle delivery (local-dir for dev, `include_bytes!` embedded for Lambda), keeping the served binary's integrity gate independent of the source. The Shape A binary (`pmcp-workbook-server`) mirrors `pmcp-sql-server` exactly: lib `run`/`serve` + thin `main.rs` shim, swapping `SqlConnector` for `BundleSource`. Shape B is a `--kind workbook-server` arm in the existing `cargo pmcp new` switch.

**Major components:**
1. `pmcp-workbook-runtime` (NEW, leaf crate) — owned IR types + deterministic executor + `scalar_eval` + `render_xlsx` (writer-only); reader-free; all shared serde types live here so the served binary never links the compiler
2. `pmcp-workbook-compiler` (NEW, offline crate) — full ingest→emit pipeline; owns `umya`; re-exports runtime types; depends on runtime, never depended on by the served tree
3. `pmcp-server-toolkit :: workbook` module (NEW, feature-gated) — generic served-tool layer: `BundleSource`, boot integrity, schema projection, input validation, five tool handlers; mirrors `sql`/`http` module precedent
4. `pmcp-workbook-server` (NEW, Shape A binary) — thin lib+shim mirroring `pmcp-sql-server`; depends on toolkit[workbook] + runtime only
5. `cargo-pmcp` (MODIFIED) — gains `compile-workbook`/`lint-workbook`/`emit-bundle` command modules + `--kind workbook-server` scaffold arm; the only product-surface consumer of `pmcp-workbook-compiler`
6. Compiled bundle (on-disk contract) — seven artifact files + integrity lock; immutable, versioned, append-only directories (CR-02 fix requirement)

**Workspace publish order additions** (slots into existing CLAUDE.md order):
- Slot 2a: `pmcp-workbook-runtime` (after `pmcp`, before toolkit)
- Slot 5 modified: `pmcp-server-toolkit` (now also depends on runtime under `workbook` feature)
- Slot 8a: `pmcp-workbook-compiler` (after runtime, no inter-dep with SQL/OpenAPI connectors)
- Slot 9a: `pmcp-workbook-server` (after toolkit + runtime; sibling to `pmcp-sql-server`)
- Slot 12 modified: `cargo-pmcp` (already last; gains dep on compiler)

### Critical Pitfalls

1. **Purity-boundary erosion (umya leaks into the served tree)** — establish the `cargo tree` purity gate WITH the runtime crate on day one, before any `umya` code lands. Run per feature-combination in CI (not just defaults). Include a positive assertion that `rust_xlsxwriter` IS present. Permit `zip` (writer container) but ban `quick-xml`/`umya`/`calamine`. Warning sign: `Cargo.lock` churn touching `umya`/`quick-xml` lines for a runtime-only change.

2. **umya fabricated-provenance trap** — `umya 3.0.0` hardcodes `<Application>Microsoft Excel</Application>` and `calcId=122211` on every save. Any workbook `umya` has round-tripped passes the Phase-8 freshness gate on fabricated Excel identity. Fix: read provenance from original on-disk bytes via the quarantined raw reader; never pass a re-serialized workbook to the gate. Add a regression test: author a workbook with `umya`, assert the gate REFUSES it with `oracle/non-excel-app`.

3. **Hardcoded `build_reference_manifest` does not survive a second workbook** — the lighthouse inlines `ufh-quote`'s entire schema as literal Rust. Copying it means the served tool schema describes ufh-quote's inputs regardless of the actual workbook compiled. Fix: delete `build_reference_manifest`/`emit_reference_bundle` from every non-test path; route `manifest::synthesize` → `ratify` → `emit_bundle` exclusively. Verification gate: compile two different workbooks and assert each server's `get_manifest` reflects its own inputs with zero shared Rust.

4. **CR-01 / CR-02 governance correctness gaps** — CR-01: `classify_cell_roles` only inspects the current cell's role; demotions (`Input→Constant`, assumption→non-assumption) escape classification, auto-promote with `HotReload`, and bypass the `--accept` gate entirely — a breaking schema change ships to agents with no human approval. CR-02: promotion writes `next_version` in the changelog but keeps `candidate.version = "1.0.0"`, overwriting the prior baseline directory; the audit trail is destroyed on every promotion. Fix both at extraction time; the bundle-store abstraction assumes versioned, immutable, append-only directories (CR-02 must be fixed before generalizing `BundleSource`).

5. **WR-01 enum-input tiering seeds an out-of-enum default** — `ratify_tiers` stamps `Variable{default: Text("")}` on enum inputs; `""` is never a valid enum member; the present-only membership gate does not check seeded defaults. Fix: `ratify_tiers` skips inputs carrying `allowed_values` (leave untiered). Verify against the COMMITTED `manifest.json` (post-emission), not the in-memory builder — the lighthouse's own test misses this by running against the pre-emission path.

## Implications for Roadmap

Research strongly supports the six-phase build order from ARCHITECTURE.md (RFC §7 logic: runtime-first proves the boundary before umya code lands; served consumer locked before compiler generalized; DX surfaces last). The phase structure is a dependency-cone topological sort, not an arbitrary grouping.

### Phase A: Port `pmcp-workbook-runtime` (the purity-proving cut)

**Rationale:** RFC §7 "smallest cut that proves the boundary." Zero reader deps, already serde/schemars-clean in the lighthouse. Every downstream consumer (toolkit module, compiler, served binary) depends on this crate. If the purity gate is not established here, before any `umya` code lands, the reader will silently leak in during Phase C and be expensive to evict.
**Delivers:** `pmcp-workbook-runtime` published at slot 2a; `cargo tree` purity gate in CI + `just purity-check`; the owned IR types, `run` executor, `scalar_eval`, `Manifest`/`CellRole`/`CellMap`/`BundleLock`/`VersionChangelog` models, `LayoutDescriptor`, bundle integrity hashing.
**Addresses:** Table-stakes compiled bundle contract types; the boot integrity gate foundation.
**Avoids:** Pitfall 1 (purity-boundary erosion) — gate is present before any reader code exists.
**Research flag:** Standard patterns; direct lift from lighthouse `crates/workbook-runtime/src/`. No phase-specific research needed.

### Phase B: `BundleSource` trait + served-tool toolkit module

**Rationale:** Lock the served contract (what the bundle must contain for `calculate`/`explain`/etc.) before generalizing the compiler that emits it. The served projection (`schema.rs`, `input.rs`) is already manifest-driven in the lighthouse — this comes over nearly unchanged; the main work is wiring through `try_workbook_from_bundle` and `BundleSource` abstraction.
**Delivers:** `pmcp-server-toolkit` `workbook` feature module; `BundleSource` trait + `LocalDirSource` + `EmbeddedSource` impls; all five tools registered and working against a test bundle loaded via `LocalDirSource`; boot integrity gate (typed `BundleLoadError`).
**Uses:** `pmcp-workbook-runtime` (Phase A output); `rust_xlsxwriter` (render path); `jsonschema 0.46` (already in toolkit).
**Implements:** `pmcp-server-toolkit :: workbook` component; mirrors the `#[cfg(feature = "http")] pub mod http` feature-gate pattern.
**Avoids:** Pitfall 7 (fail-open validation) — wire `WR-05` fail-closed fix here: missing manifest role = error, not `if let Some` skip; `WR-02` numeric-enum rejection.
**Research flag:** Standard patterns; the served layer code is a near-verbatim lift. The `WR-05`/`WR-02` security fixes need careful implementation — flag for a brief implementation research pass on the exact fail-closed contract.

### Phase C: `pmcp-workbook-compiler` + the §5 generalization fixes

**Rationale:** The compiler is the heaviest lift; doing it after Phase B means the bundle contract (what the compiler must emit) is already fixed by a working consumer. The §5 generalization fixes (manifest-driven emit, CR-01/CR-02/WR-01) belong in this phase because they are in compiler-owned code; fixing them after the port is copy-and-fix, not redesign.
**Delivers:** `pmcp-workbook-compiler` published at slot 8a; full offline pipeline (ingest/dialect/manifest-synth/formula/dag/sheet_ir/reconcile/provenance/artifact/gate/change_class); deletion of `build_reference_manifest`/`emit_reference_bundle` replaced by generic `compile_workbook`; CR-01 symmetric change-class classifier; CR-02 versioned bundle directory write; WR-01 enum tiering skip; umya fabricated-provenance distinct class; second-workbook test passing.
**Uses:** `umya-spreadsheet 3.0`; `quick-xml 0.37` / `zip 8` (pinned to umya transitive); hand-rolled formula/DAG lifted verbatim.
**Implements:** `pmcp-workbook-compiler` component; `manifest::synthesize` → `ratify` → `emit_bundle` generic pipeline.
**Avoids:** Pitfall 2 (umya provenance trap); Pitfall 3 (hardcoded manifest); Pitfall 4 (CR-01/CR-02); Pitfall 5 (WR-01); Pitfall 6 (reconcile determinism) — operand-anchored rounding model, `delta.abs()` grep gate, Excel-quirk fixture corpus.
**Research flag:** Needs phase-specific research. The CR-01 symmetric classification fix requires careful design (see `14-REVIEW.md:63-86` for the exact patch). The Excel-quirk fixture corpus (dates, errors, empty-cell coercion) is new work not in the lighthouse. Flag for a research sub-task on the full set of `sheet_ir` semantics edge cases to cover.

### Phase D: `cargo pmcp compile-workbook` / `lint-workbook` / `emit-bundle` + `pmcp.toml`

**Rationale:** CLI is a thin shell over the Phase C compiler; it belongs after Phase C so the command modules are stable thin-shells, not co-evolving with the compiler internals. `pmcp.toml` lands here because the CLI is its first consumer — the project-config generalization kills the last lighthouse-bound assumptions.
**Delivers:** Three new `commands/` modules in `cargo-pmcp`; `--accept --approver --effective-date` BA approval flow; `pmcp.toml` `[[workbook.workbooks]]` source→bundle_id mapping; the single-workbook justfile assumption eliminated.
**Implements:** CLI integration; project-level config; the `--accept` governance artifact flow.
**Avoids:** UX pitfall: single-workbook assumptions mean a second project cannot use the tooling.
**Research flag:** Standard patterns; CLI modules mirror existing `commands/*.rs` thin-shell layout. `pmcp.toml` shape is straightforward from the architecture brief. No phase-specific research needed.

### Phase E: Shape A binary `pmcp-workbook-server`

**Rationale:** The binary is a thin shell over the toolkit module (Phase B) and `BundleSource`. It belongs after Phase D so the `pmcp.toml` + CLI contract is stable when the binary's assembly logic is written.
**Delivers:** `pmcp-workbook-server` crate (lib `run`/`serve` + `main.rs` shim); CLI (`--bundle-dir`/`--bundle-id`/`--http`); `BundleSource` selection from CLI args; `RunError` → non-zero exit matching `pmcp-sql-server`; published at slot 9a.
**Implements:** Shape A binary; mirrors `pmcp-sql-server/src/{lib,main,cli,assemble}.rs` field-for-field.
**Research flag:** Standard patterns; exact mirror of `pmcp-sql-server`. No phase-specific research needed.

### Phase F: Shape B scaffold (`cargo pmcp new --kind workbook-server`) + dialect spec

**Rationale:** The scaffold targets the Shape A wiring; it cannot be written until Phase E's assembly pattern is stable. The SDK-owned dialect spec finalizes what workbooks must declare, closing the governance contract.
**Delivers:** `--kind workbook-server` arm in `new.rs`; `templates::workbook_server` (Cargo.toml + `main.rs` using `EmbeddedSource` + sample `pmcp.toml` + sample bundle dir); SDK-owned versioned `workbook-dialect-spec.md` with `DialectRules` version constant; workbooks declare the dialect version they target.
**Implements:** Shape B scaffold; dialect ownership transfer to SDK.
**Research flag:** Standard patterns for the scaffold (mirrors `execute_sql_server` + `templates::sql_server`). The dialect spec versioning contract (how workbooks declare and compilers validate the dialect version) may need a brief design pass — flag as a light research sub-task.

### Phase Ordering Rationale

- **Runtime before compiler (A before C)** is non-negotiable: the purity gate must exist before any `umya` code lands, and the shared types must live in the reader-free crate before the compiler re-exports them.
- **Served contract before compiler generalization (B before C)** prevents re-work: if the compiler is generalized before the served contract is locked, the bundle schema can drift mid-development. Lock the consumer's expectations first.
- **Compiler before CLI (C before D)** keeps the CLI thin-shells stable: command modules delegate to stable compiler APIs, not co-evolving ones.
- **CLI + config before Shape A (D before E)** ensures `pmcp.toml` is stable when the binary's assembly reads it.
- **Shape A before Shape B (E before F)** ensures the scaffold's generated `main.rs` targets a wiring that is already tested end-to-end.
- **§5 fixes in Phase C** (not deferred): CR-01/CR-02/WR-01 are compiler-owned bugs; the bundle-store abstraction in `BundleSource` assumes immutable versioned dirs — CR-02 must be fixed before that abstraction is built on top.

### Research Flags

Phases likely needing deeper research during planning:

- **Phase C (compiler + §5 fixes):** CR-01 symmetric classification fix requires careful design — the exact patch is sketched in `14-REVIEW.md:63-86` but the SDK generalization needs verification of the symmetry invariant property test. The Excel-quirk fixture corpus (1900 date bug, empty-cell coercion, error propagation, half-rounding boundaries) is entirely new work; a brief research sub-task to enumerate the full `sheet_ir` semantics edge case set is warranted.
- **Phase B (served-tool module):** `WR-05` fail-closed validation contract and `WR-02` numeric-enum rejection need careful implementation design to avoid introducing new fail-open paths. A brief implementation research pass on the exact error-boundary contract before coding.
- **Phase F (dialect spec):** The versioning contract (how a workbook declares a target dialect version and the compiler validates it) needs a light design pass — not complex, but the exact schema shape is not fully specified in the lighthouse.

Phases with standard patterns (research-phase not needed):

- **Phase A (runtime port):** Direct lift from lighthouse `crates/workbook-runtime/src/`; all types are serde-clean; well-documented in the lighthouse.
- **Phase D (CLI + `pmcp.toml`):** Thin-shell command modules mirror the existing `commands/*.rs` pattern; `pmcp.toml` shape is fully specified in the architecture brief.
- **Phase E (Shape A binary):** Mirror of `pmcp-sql-server` field-for-field; no new patterns.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | All versions verified against crates.io 2026-06-09; lighthouse pins are current; SDK workspace already carries every non-workbook dep at exact matching versions. One LOW-MEDIUM gap: whether the SWC/`pmcp-code-mode` JS oracle is still load-bearing for reconcile parity — verify against lighthouse Phase-10 path before Phase C. |
| Features | HIGH | Every feature read directly from lighthouse source (handler, error, input, schema, diff_version, render_resource). The two deferred error triggers (`stale_oracle`, `unapproved_assumption`) are shape-only in the lighthouse — confirmed not load-bearing for v1. |
| Architecture | HIGH | Direct read of lighthouse crates + SDK integration targets; mirrors a proven v0.5.0 reference impl. The `BundleSource` trait design and publish-order extension are new SDK work but straightforward from the patterns. |
| Pitfalls | HIGH | Every pitfall grounded in lighthouse source: CR-01/CR-02/WR-01 from `14-REVIEW.md`, umya provenance from `provenance/gate.rs` inline docs, purity boundary from `justfile:54-92`, hardcoded manifest from `lib.rs:524-606`, reconcile discipline from `reconcile/classifier.rs`. |

**Overall confidence:** HIGH

### Gaps to Address

- **JS oracle necessity (LOW-MEDIUM confidence):** Confirm whether `pmcp-code-mode`/SWC is still load-bearing for offline penny-reconciliation parity or whether pure-Rust `scalar_eval` fully covers it. If not required, drop from the SDK compiler entirely; if required, gate behind a non-default `js-oracle` feature. Verify against the lighthouse Phase-10 reconcile path during Phase C planning.
- **`quick-xml`/`zip` transitive pin re-derivation:** The pins in STACK.md are read from the lighthouse `Cargo.lock`. At extraction time, re-derive via `cargo tree -p umya-spreadsheet -i quick-xml` and `cargo tree -p umya-spreadsheet -i zip` against the actual resolved workspace to avoid forking a second copy.
- **`cargo-deny` `[bans]` per-crate scoping:** `cargo-deny`'s native per-crate ban scoping is coarse; the `cargo tree`-per-crate CI arm is the precise boundary. Verify `cargo-deny` `[bans]` ergonomics during Phase A purity-gate design (it is a declarative backstop, not the primary mechanism).
- **Multi-bundle tool naming:** The `BundleSource` `list()` returning N bundles implies tool naming must be prefixed per bundle ID (`ufh-quote.calculate`). The design of tool registration with namespacing is an open question deferred to Phase E/F but should be noted as a design decision in Phase B.
- **Cell-string metadata sanitization:** Length-cap and sanitization of BA-authored `meaning`/`unit`/`name`/enum strings before they enter tool `description` fields is a Phase B hardening task; the exact bounds are unspecified in the lighthouse and need a decision.

## Sources

### Primary (HIGH confidence)

- `crates/workbook-runtime/src/lib.rs` (lighthouse) — reader-free leaf; the `pmcp-workbook-runtime` lift target
- `crates/workbook-compiler/src/lib.rs` (lighthouse) — umya pipeline; `build_reference_manifest` hardcoded schema (lines 524-606); the §5 generalization source
- `crates/quote-pricing-server/src/workbook/{handler,error,input,schema,diff_version,render_resource,mod}.rs` (lighthouse) — already-manifest-driven served layer
- `towelrads-quote-pricing/.planning/phases/14-*/14-REVIEW.md` — CR-01/CR-02/WR-01..WR-05/IN-01..IN-04 findings (independent goal-backward review)
- `crates/workbook-compiler/src/provenance/gate.rs` — inline Pitfall-2 docs, freshness-gate conjunction, anchored-identity check
- `crates/workbook-compiler/src/change_class/mod.rs:165-234` — `classify_cell_roles` demotion asymmetry
- `crates/workbook-compiler/src/reconcile/classifier.rs:19-21, 281-292` — operand-anchored rounding model, forbidden-`delta.abs()` discipline
- `justfile:54-92` (lighthouse) — proven `purity-check` recipe (reader-vs-writer cargo-tree + value-path-grep gate)
- `docs/sdk-issue-excel-workbook-compiler-extraction.md` (RFC §5/§6/§7) — generalization gaps, open questions, runtime-first recommendation
- `docs/Excel-as-Configuration-Architecture-Brief.md` — two-surface model, reuse-vs-new, promote-gate philosophy
- `docs/workbook-dialect-spec.md` — 13-fn whitelist, refuse-set, enforced-vs-deferred
- `.planning/PROJECT.md` v2.3 Current Milestone section — target features, generalization fixes, scope boundaries
- crates.io API (verified 2026-06-09) — umya-spreadsheet 3.0.0, rust_xlsxwriter 0.95.0, quick-xml 0.40.1, zip 8.6.0, schemars 1.2.1

### Secondary (MEDIUM confidence)

- SDK `crates/pmcp-server-toolkit/src/lib.rs` + `Cargo.toml` — `#[cfg(feature="http")] pub mod http` precedent; feature-gate pattern to mirror
- SDK `crates/pmcp-sql-server/src/{lib,main}.rs` + `Cargo.toml` — Shape-A lib/`run`/`serve` + thin-shim + `RunError` pattern
- SDK `cargo-pmcp/src/commands/new.rs` + `templates/sql_server.rs` — `--kind` switch + scaffold-template pattern
- SDK `CLAUDE.md` Release & Publish Workflow — v2.2 publish order extended in the architecture research

---
*Research completed: 2026-06-09*
*Ready for roadmap: yes*
