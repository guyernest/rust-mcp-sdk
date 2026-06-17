# Architecture Research: Excel-as-Configuration → MCP-server compiler (v2.3 extraction)

**Domain:** Compile-not-interpret workbook→MCP-server toolchain extracted into the PMCP SDK
**Researched:** 2026-06-09
**Overall confidence:** HIGH (direct read of the lighthouse crates + the SDK integration targets; mirrors a proven v0.5.0 reference impl)

## Executive Summary

The v2.3 milestone extracts a working, penny-reconciled Excel-workbook compiler from the `towelrads-quote-pricing` lighthouse into the SDK as a new "governed Excel" CodeLanguage, slotting alongside the v2.2 SQL and OpenAPI toolkits. The lighthouse already enforces the load-bearing architectural invariant — **the Excel reader (`umya`) never enters the served-binary dependency tree** — via a two-crate split (`workbook-runtime` reader-free leaf, `workbook-compiler` umya-owning offline pipeline) that a `cargo-tree`-provable purity gate asserts. The SDK extraction mirrors this exactly into `pmcp-workbook-runtime` + `pmcp-workbook-compiler`.

The served-tool layer in the lighthouse (`quote-pricing-server/src/workbook/`) is **already ~95% workbook-agnostic** — its schema projection (`schema.rs`), input validation (`input.rs`), and tier enforcement all read from the embedded `Manifest`, not from per-workbook Rust. The single hardcoded seam is `build_reference_manifest` in `workbook-compiler/src/lib.rs`, which inlines the lighthouse's `heat_source` input as a Rust literal. The §5 generalization is therefore **narrower than it looks**: the served projection is reusable as-is; what must change is that the *compiler* must synthesize `manifest.json` purely from the workbook (it already has `manifest::synthesize` — the candidate synthesizer), and `build_reference_manifest` must be deleted in favor of a generic bundle-emit driver.

Shape A is a thin `pmcp-workbook-server` binary that mirrors `pmcp-sql-server` exactly (lib `run`/`serve` + thin `main.rs` shim), differing only in that the "backend" is a `BundleSource` (where the compiled bundle is read from) rather than a SQL connector. Shape B is `cargo pmcp new --kind workbook-server`, a new arm in the existing `new.rs` `--kind` switch with a `templates::workbook_server` module mirroring `templates::sql_server`. The CLI gains `cargo pmcp compile-workbook`/`lint-workbook`/`emit-bundle` as new command modules under `cargo-pmcp/src/commands/`, each a thin shell over `pmcp-workbook-compiler` (which owns umya — so the **compiler is a dependency of cargo-pmcp**, never of the runtime/server). A project-level `pmcp.toml` maps workbook files → bundle IDs so the single-workbook lighthouse assumptions (`ufh-quote`, hardcoded paths) generalize.

The recommended build order ports `pmcp-workbook-runtime` first (RFC §7 — smallest cut, zero reader deps, already serde/schemars-clean), then the served-tool toolkit module against it, then the compiler + CLI with the §5 generalization fixes (manifest-driven emit, CR-01/CR-02/WR-01, umya provenance), then the Shape-A binary and Shape-B scaffold last. Porting the runtime first proves the purity boundary before any umya code lands.

## Standard Architecture

### System Overview — the two dependency spines

The architecture is two non-overlapping dependency cones that meet only at the runtime leaf and the on-disk bundle contract:

```
┌──────────────────────── OFFLINE (build-time, umya allowed) ──────────────────────────┐
│                                                                                       │
│   cargo-pmcp  (compile-workbook / lint-workbook / emit-bundle commands)               │
│        │  depends                                                                     │
│        ▼                                                                              │
│   pmcp-workbook-compiler   ── owns umya, quick-xml, zip (Excel reader + provenance)   │
│        │  ingest → lint → manifest synth → formula parse → DAG compile →              │
│        │  penny-reconcile → artifact emit → promote-gate                              │
│        ▼                                                                              │
│   pmcp-workbook-runtime  ◄─── (re-exports IR/Manifest types so compiler compiles)     │
│                                                                                       │
└───────────────────────────────────────┬──────────────────────────────────────────-──┘
                                         │  emits
                                         ▼
                          ┌───────────────────────────────┐
                          │   compiled bundle (on disk)    │  ← THE CONTRACT
                          │   manifest.json / executable   │     (compiler ↔ server)
                          │   .ir.json / cell_map.json /   │
                          │   layout.json / BUNDLE.lock /  │
                          │   evidence/                    │
                          └───────────────┬───────────────┘
                                          │  loaded by BundleSource
                                          ▼
┌──────────────── SERVED (runtime, NO umya — purity-gated) ────────────────────────────┐
│                                                                                       │
│   pmcp-workbook-server  (Shape A binary)   OR   cargo pmcp new --kind workbook-server │
│        │  depends                                    (Shape B scaffold)               │
│        ▼                                                                              │
│   pmcp-server-toolkit :: workbook module  (NEW — parallels sql/http modules)          │
│        │  calculate / explain / get_manifest / diff_version / render_workbook         │
│        │  schema projection + input validation, ALL manifest-driven                   │
│        ▼                                                                              │
│   pmcp-workbook-runtime  ── owned IR + deterministic executor + writer-only render    │
│        │  depends                                                                     │
│        ▼                                                                              │
│   pmcp  (core SDK: ServerBuilder, ToolInfo, streamable-http)                          │
│                                                                                       │
└───────────────────────────────────────────────────────────────────────────────────-─┘
```

The purity invariant is the whole point of the split: a `cargo tree -p pmcp-workbook-server -i umya` (and `-i swc_ecma_parser`, `-i quick-xml`, `-i zip`) must return empty. This is a `just`/CI gate, mirroring the lighthouse's `just purity-check`.

### Component Responsibilities

| Component | Responsibility | Status | Mirrors / Source |
|-----------|----------------|--------|------------------|
| `pmcp-workbook-runtime` | Owned IR (`Expr`/`Cell`/`CellValue`/`Dag`), deterministic topo executor (`run`), pure-Rust scalar leaf eval (`eval_scalar`, replaces the SWC/JS kernel), manifest projection model (`Manifest`/`CellRole`/`Role`/`Dtype`/`InputTier`/`allowed_values`), bundle artifact model + integrity hash, writer-only render `LayoutDescriptor`, version-changelog model | NEW crate (lift) | `crates/workbook-runtime/src/lib.rs` (reader-free leaf) |
| `pmcp-workbook-compiler` | Offline pipeline: ingest (umya→owned cell map), dialect lint, manifest **synthesis** (`synthesize` — colour+Guide+headers → candidate roles, BA-ratified), formula parse (Pratt over whitelist), DAG reconstruction + Kahn topo-sort, sheet-IR + Excel-semantics, penny reconciliation, provenance/freshness gate (quick-xml/zip part reader), artifact emission (the bundle), promote-time gate (numeric corpus + change-class router), compile-workbook driver | NEW crate (lift) | `crates/workbook-compiler/src/lib.rs` (umya isolated) |
| `pmcp-server-toolkit :: workbook` (feature `workbook`) | The generic served-tool module: load a bundle via `BundleSource`, recompute integrity (fail-closed at boot), register `calculate`/`explain`/`get_manifest`/`diff_version`/`render_workbook` via `ServerBuilder`, project input/output schema from the manifest, tier-enforce overrides | NEW module in existing crate | lift `quote-pricing-server/src/workbook/` (schema/input/handler/diff_version/render_resource), already manifest-driven |
| `BundleSource` trait | Abstract "where the compiled bundle bytes come from": `LocalDirSource` (read `bundles/<id>@<ver>/`) + `EmbeddedSource` (`include_bytes!`/`include_str!` baked at build for Lambda); S3/registry left as a documented seam | NEW trait | lighthouse hardcodes `include_str!`; this generalizes it |
| `pmcp-workbook-server` (Shape A binary) | Thin `run`/`serve` lib + `main.rs` shim: parse CLI (`--bundle-dir`/`--bundle-id`/`--http`), construct a `BundleSource`, build the `pmcp::Server` from the toolkit `workbook` module, serve over streamable HTTP | NEW crate | mirrors `pmcp-sql-server/src/{lib,main,cli,assemble}.rs` exactly |
| `cargo pmcp compile-workbook` / `lint-workbook` / `emit-bundle` | Thin command shells over `pmcp-workbook-compiler`; `compile-workbook` carries the gated `--accept --approver --effective-date` BA approval flow | NEW commands | mirrors `cargo-pmcp/src/commands/*.rs` thin-shell layout |
| `cargo pmcp new --kind workbook-server` | Scaffold a single runnable crate (`Cargo.toml` + `main.rs` + `pmcp.toml` + a sample bundle dir) | NEW `--kind` arm | mirrors `new.rs::execute_sql_server` + `templates::sql_server` |
| `pmcp.toml` (project config) | Map workbook source files → bundle IDs + versions; the home for compile/lint/emit defaults (bundles dir, corpus path) replacing lighthouse justfile literals | NEW config shape | replaces single-workbook `ufh-quote` / justfile assumptions |
| Versioned dialect spec | SDK-owned `workbook-dialect-spec.md` + a `DialectRules` version constant; workbooks declare the dialect version they target | NEW SDK-owned doc | lighthouse `dialect::DialectRules` + `docs/workbook-dialect-spec.md` |

## The crate dependency graph (new vs modified)

```
pmcp (core, EXISTING)
  └── pmcp-workbook-runtime (NEW)  ── reader-free leaf; depends only on pmcp + serde/schemars/sha2
        ├── pmcp-workbook-compiler (NEW)  ── adds umya + quick-xml + zip (OFFLINE ONLY)
        │     └── cargo-pmcp (MODIFIED)   ── depends compiler for compile/lint/emit commands
        └── pmcp-server-toolkit (MODIFIED) ── new `workbook` module + `workbook` feature; depends runtime ONLY
              └── pmcp-workbook-server (NEW) ── Shape A binary; depends toolkit[workbook] + runtime
```

Critical edges to preserve:
- **`pmcp-server-toolkit` depends on `pmcp-workbook-runtime` ONLY** — never on `pmcp-workbook-compiler`. This is the purity boundary expressed as a Cargo edge. The `workbook` module must be feature-gated (`workbook = ["dep:pmcp-workbook-runtime"]`) so the no-default-features toolkit build (and the SQL/OpenAPI consumers) never pull it.
- **`cargo-pmcp` is the only consumer of `pmcp-workbook-compiler`** — umya lives entirely in the CLI/build path. `cargo-pmcp` is a dev tool, never a deployed server, so the purity boundary holds.
- **`pmcp-workbook-compiler` re-exports types FROM `pmcp-workbook-runtime`** (the lighthouse does this — `workbook-compiler/src/lib.rs` lines 155–221 re-export `Expr`/`Dag`/`Manifest`/`ChangeClass`/`VersionChangelog` from `workbook_runtime`). This keeps the compiler's historical call sites compiling while the runtime owns the shared types the server deserializes.

### Workspace publish order (extending CLAUDE.md's v2.2 list)

CLAUDE.md lists items 1–12. The workbook crates slot in by dependency depth: `pmcp-workbook-runtime` is a new leaf (depends only on `pmcp`), so it publishes right after `pmcp` and before the toolkit (whose new `workbook` module depends on it). The compiler depends on the runtime; the Shape-A binary depends on the toolkit + runtime; `cargo-pmcp` (already last) gains a dep on the compiler.

Extended order (new entries marked **NEW**):

1. `pmcp-widget-utils`
2. `pmcp` (core SDK)
2a. **`pmcp-workbook-runtime`** *(NEW — leaf after pmcp, before toolkit; the served binary's only workbook dep)*
3. `pmcp-code-mode`
4. `pmcp-code-mode-derive`
5. `pmcp-server-toolkit` *(MODIFIED — now also depends on `pmcp-workbook-runtime` under the new `workbook` feature; must publish AFTER 2a)*
6. `pmcp-toolkit-postgres`
7. `pmcp-toolkit-mysql`
8. `pmcp-toolkit-athena`
8a. **`pmcp-workbook-compiler`** *(NEW — depends on `pmcp-workbook-runtime`; publish after 2a; no inter-dep with the connector crates or the SQL server)*
9. `pmcp-sql-server`
9a. **`pmcp-workbook-server`** *(NEW — Shape A binary; depends on `pmcp-server-toolkit[workbook]` + `pmcp-workbook-runtime`; must publish AFTER 5 and 2a; sibling to `pmcp-sql-server`, no inter-dep)*
10. `mcp-tester`
11. `mcp-preview`
12. `cargo-pmcp` *(MODIFIED — gains a dependency edge on `pmcp-workbook-compiler`; already last, so it naturally publishes after 8a)*

Rationale: the publish order is a topological sort of the dep graph. `pmcp-workbook-runtime` must precede everything that links it (toolkit, compiler, server). `pmcp-workbook-compiler` must precede `cargo-pmcp`. `pmcp-workbook-server` must follow the toolkit. None of the new crates have inter-dependencies with the SQL/OpenAPI connector cluster, so they interleave as shown.

## The served-tool layer as a toolkit module

### What it must implement (the interfaces)

The lighthouse served layer registers tools via `pmcp::ServerBuilder::tool_arc` directly (it notes it does NOT import the server-toolkit, which "has no native-handler arm and is absent on the current SDK checkout"). In the SDK, this becomes a first-class toolkit module mirroring how `sql`/`http` expose a synthesizer:

1. **A bundle-load + integrity entry point** — `WorkbookBundle::load(source: &dyn BundleSource, id: &str, version: &str) -> Result<WorkbookBundle, BundleLoadError>` that reads the 7 bundle members, parses the `Manifest`/IR/`CellMap`/`LayoutDescriptor`/`VersionChangelog`, recomputes the `BUNDLE.lock` `combined` hash-of-hashes from all members via the shared `workbook_runtime::{update_field, build_bundle_lock}`, and **fails closed at boot** on mismatch. The lighthouse panics; the SDK should return a typed error the Shape-A `run` surfaces as a non-zero exit (matching `pmcp-sql-server`'s `RunError::Serving`).

2. **A builder-extension** mirroring `ServerBuilderExt::try_tools_from_config`. Proposed `ServerBuilderExt::try_workbook_from_bundle(builder, &WorkbookBundle) -> Result<ServerBuilder>` (feature-gated `workbook`). It registers the five tools:
   - `calculate` — manifest-projected typed inputs, enum-gated, structured errors with an `allowed` repair field; recomputes via `run_executor`, returns `structuredContent`.
   - `explain` — per-cell business-language lineage (renders the reconciliation annotation).
   - `get_manifest` — the curated agent-facing manifest projection.
   - `diff_version` — serves the embedded `VersionChangelog`.
   - `render_workbook` — returns the computed `.xlsx` as a provenance-bound `workbook://` resource (writer-only `rust_xlsxwriter` — keeps the binary reader-free).

3. **Schema projection (already manifest-driven, lift as-is).** `schema.rs::output_schema_for_manifest` iterates `manifest.cells`, projects each `Role::Output` cell's `dtype`/`unit`/`meaning` into a strict column-typed JSON Schema, and emits it as the mandatory `outputSchema`. This is the projection interface §4 needs — **it already exists and is generic.** The work is wiring it through the toolkit's `ToolInfo` synthesis (the toolkit synthesizes `ToolInfo` from `[[tools]]`; the workbook module synthesizes it from the manifest instead).

4. **Input validation + tier enforcement (already manifest-driven, lift as-is).** `input.rs::validate_input` reads each `overrides` key, looks it up in the manifest, rejects strict constants (`is_strict_constant`), accepts variable/bounded-variable tiers, maps `inputs` keys through the `cell_map` to executor seed coords, and applies manifest defaults. Enum inputs surface as `{"enum":[...]}` from `allowed_values` with a present-only runtime membership gate.

### Config shape vs the SQL toolkit

The SQL toolkit is config-driven through `ServerConfig` (`[[tools]]` + `[database]`). The workbook module is **bundle-driven, not `[[tools]]`-driven** — the tool surface is projected entirely from `manifest.json`, so the served config is minimal: which bundle(s) to load and from where. This is the cleanest divergence from the SQL/OpenAPI shape and should be explicit: the workbook server's "config" is the `BundleSource` selection (bundle id + version + source kind), not a `[[tools]]` table. A workbook server can still carry the toolkit's auth/secrets/static-resources config; only the tool-synthesis arm differs.

## The §4 manifest-driven generalization (concrete)

The lighthouse's `build_reference_manifest` (workbook-compiler/src/lib.rs lines 524–606) **hand-constructs a `Manifest`** with the `heat_source` enum input as a literal `CellRole`. This is the one piece of per-workbook Rust. The fix:

- **Delete `build_reference_manifest` + `emit_reference_bundle` + `renderer_equivalence_governed`** (all the `ufh-quote`-literal functions). Replace with a generic `compile_workbook(workbook_path, bundles_dir, …)` driver that:
  1. `ingest`s the real `.xlsx` (umya).
  2. Runs `manifest::synthesize` (the candidate synthesizer that already exists — colour + Guide + headers → candidate `CellRole`s with `role`/`dtype`/`unit`/`meaning`/`allowed_values`).
  3. Requires BA `ratify` (the ratification stamp already exists).
  4. Builds the DAG + reconciles against the oracle (cached cell values).
  5. Emits via the existing `emit_bundle` (`build_candidate_model`/`write_candidate_bundle` already take the manifest as a parameter — **only their callers hardcode `ufh-quote`**).
- **The projection interface is the `Manifest` itself.** `Role`/`Dtype`/`unit`/`allowed_values` flow from `synthesize` → `manifest.json` → the served `schema.rs`/`input.rs`. No Rust per workbook anywhere. `role` drives input/output/constant classification; `dtype` drives the JSON Schema primitive; `unit`/`meaning` drive the self-describing labels; `allowed_values` drives the enum gate.

This is why §5 is narrower than it appears: the **served projection is already generic**; only the **compiler's emit driver** hardcodes the lighthouse. The generalization is "route `synthesize`'s output into `emit_bundle` instead of a hand-built manifest."

## BundleSource trait design

```rust
/// Where a compiled workbook bundle's bytes come from. The served path depends
/// ONLY on this trait + the runtime — never on the compiler/umya.
pub trait BundleSource: Send + Sync {
    /// Read a single named member (e.g. "manifest.json", "executable.ir.json",
    /// "evidence/changelog.json") of the bundle `<id>@<version>`.
    fn read_member(&self, id: &str, version: &str, member: &str)
        -> Result<Vec<u8>, BundleSourceError>;
    /// Enumerate available `(id, version)` pairs (multi-bundle servers + diff_version).
    fn list(&self) -> Result<Vec<(String, String)>, BundleSourceError>;
}
```

- **`LocalDirSource { root: PathBuf }`** — reads `root/<id>@<version>/<member>` from disk. The dev/test default; the Shape-A binary's `--bundle-dir` points at it.
- **`EmbeddedSource`** — baked at build via `include_bytes!`/`include_str!` (the lighthouse pattern, `workbook/mod.rs` lines 55–120). For Lambda/deploy where the bundle ships *inside* the binary. The scaffold's generated `main.rs` uses this so the deployed server has no filesystem dependency. Note the contrast with `pmcp-server-toolkit`'s SQL path (`demo_db_path()` + `pmcp::assets::load_string` resolve a writable `/tmp` DB + read-only assets under `/var/task/assets`): the workbook bundle is read-only, so it can be baked straight into the *binary* via `EmbeddedSource` with no `/var/task/assets` round-trip.
- **S3/registry seam** — `S3BundleSource`/`RegistryBundleSource` are documented but deferred; the trait's `read_member`/`list` shape is the seam (an S3 impl is `GetObject` per member, a registry impl is an HTTP fetch + cache). No runtime change needed to add them.

Integrity is computed by the *loader over the source*, not by the source — `WorkbookBundle::load` reads all members through the `BundleSource`, then runs the shared `build_bundle_lock` fold and compares to the embedded `BUNDLE.lock`. So any source (local/embedded/S3) gets the same fail-closed boot check.

## CLI integration (cargo-pmcp)

The existing `cargo-pmcp/src/commands/` modules are thin shells (each `<cmd>.rs` parses clap args and delegates). Add three sibling modules mirroring that layout:

- **`commands/compile_workbook.rs`** — `cargo pmcp compile-workbook <workbook.xlsx> [--bundles-dir DIR] [--bundle-id ID] [--accept --approver NAME --effective-date DATE]`. Delegates to `pmcp_workbook_compiler::compile_workbook` (the build-candidate → gate → write driver). The `--accept` flow records the BA approval into the corpus exactly as the lighthouse `gate::accept::accept` does. **Recommendation (RFC §6 OQ-3 / brief §11 OD-2):** keep `--accept` in the CLI, not in `deploy` — the lighthouse proves it as a reviewable artifact (`bundles/corpus/<id>/cases.json`), and the gate's BA review is its own reviewable step ("distinct compile-workbook step feeding deploy").
- **`commands/lint_workbook.rs`** — `cargo pmcp lint-workbook <workbook.xlsx>`. Delegates to the compiler's `lint`/`LintReport`; exits non-zero on `report.has_errors()`. The fast feedback loop before a full compile.
- **`commands/emit_bundle.rs`** — `cargo pmcp emit-bundle [--bundle-id ID]`. Regenerates a bundle (the generalized `emit_bundle` driver) for an already-ratified workbook; the analog of `just emit-bundle`.

These register in `cargo-pmcp/src/main.rs`'s clap subcommand dispatch. Because they pull `pmcp-workbook-compiler` (umya), they are the **only** umya entry point in the SDK product surface.

### Project-level `pmcp.toml`

The lighthouse hardcodes `ufh-quote`, the bundles dir, and the corpus path in justfile recipes. Generalize into a project-root `pmcp.toml`:

```toml
[workbook]
bundles_dir = "bundles"          # where compiled bundles land / are read
corpus_dir  = "bundles/corpus"   # golden corpus + approval records

[[workbook.workbooks]]
source    = "docs/ufh-quote.xlsx"  # the BA-authored workbook
bundle_id = "ufh-quote"            # → bundles/ufh-quote@<ver>/
dialect   = "1.0"                  # the SDK-owned dialect version it targets
```

`compile-workbook`/`lint-workbook`/`emit-bundle` read `pmcp.toml` to resolve `source → bundle_id`, killing the single-workbook assumption. The Shape-A binary reads the same `[workbook]` table (or CLI flags) to know which bundle id/version to serve.

## Patterns to Follow

### Pattern: lib `run`/`serve` + thin `main.rs` shim (Shape A)
**What:** Put all assembly/serving logic in the binary crate's `lib.rs` (`run`, `run_serving`, `serve`, `build_server`, `load`), keep `main.rs` a ~10-line `#[tokio::main]` shim that parses clap `Args` and calls `run`. **Why:** keeps server construction unit-testable without spawning a process. **Source:** `pmcp-sql-server/src/{lib,main}.rs` — replicate field-for-field, swapping `dispatch(SqlConnector)` for `load(BundleSource)`.

### Pattern: feature-gated toolkit module mirroring `sql`/`http`
**What:** Add `#[cfg(feature = "workbook")] pub mod workbook;` to `pmcp-server-toolkit/src/lib.rs`, with crate-root re-exports of the headline types (`WorkbookBundle`, `BundleSource`, `LocalDirSource`, `EmbeddedSource`, `try_workbook_from_bundle`). **Why:** matches the `#[cfg(feature = "http")] pub mod http;` precedent so no-default-features and SQL-only builds never link the runtime. **Source:** `pmcp-server-toolkit/src/lib.rs` lines 43–44.

### Pattern: re-export shared types from the runtime leaf
**What:** The compiler re-exports `Expr`/`Dag`/`Manifest`/`ChangeClass`/`VersionChangelog` from the runtime crate so its public surface and call sites compile against historical names. **Why:** the served binary deserializes these types, so they MUST live in the reader-free crate; the compiler borrows them upward. **Source:** `workbook-compiler/src/lib.rs` lines 155–221.

### Pattern: cargo-tree purity gate
**What:** A `just purity-check` / CI step asserting `cargo tree -p pmcp-workbook-server -i umya` (and `-i quick-xml -i zip -i swc_ecma_parser`) is empty. **Why:** the entire compile-not-interpret security story rests on the reader never reaching prod; make it mechanically provable, not a convention. **Source:** lighthouse `just purity-check`.

## Anti-Patterns to Avoid

### Anti-Pattern: copying `build_reference_manifest` / hand-built manifests
**What:** Porting the `ufh-quote`-literal `build_reference_manifest`/`emit_reference_bundle` as the bundle-emit path. **Why bad:** it is the single hardcoded seam the whole §5 generalization exists to kill; copying it forks per-workbook Rust into the SDK. **Instead:** route `manifest::synthesize` (already present) → `ratify` → the generic `emit_bundle`.

### Anti-Pattern: letting the toolkit depend on the compiler
**What:** Adding `pmcp-workbook-compiler` to `pmcp-server-toolkit`'s deps "for convenience" (e.g. to re-synthesize a manifest at boot). **Why bad:** drags umya/quick-xml/zip into every served binary, breaking the purity invariant and the security message. **Instead:** the server only ever *reads* a pre-compiled bundle via `BundleSource`; synthesis is build-time only.

### Anti-Pattern: trusting umya-fabricated Excel provenance (§5 caveat)
**What:** Treating a umya-authored/round-tripped workbook as a fresh real-Excel recalc. **Why bad:** umya 3.0.0 hard-codes `<Application>Microsoft Excel</Application>` + `calcId` on every save, so a umya-written workbook passes the Phase-8 freshness gate on **fabricated** identity. **Instead:** any SDK tooling that programmatically mutates a workbook must mark a distinct provenance class (or use a different writer); the freshness gate must not treat umya identity as proof of recalc.

### Anti-Pattern: copying the CR-01/CR-02/WR-01 promote-path debt
**What:** Lifting the promote path as-is. **Why bad:** CR-02 (promotion computes `next_version` but writes back into the same `@1.0.0` dir, overwriting the baseline), CR-01 (demotion-direction change-class changes escape classification and auto-promote), WR-01 (`ratify_tiers` gives enum inputs a `Variable{default:""}` tier, seeding an out-of-enum empty string). **Instead:** fix at extraction — write to the computed `next_version` dir, make the change-class classifier symmetric, skip tiering for enum inputs. These §5 fixes slot into the compiler/promote-gate phase (Phase C).

## Data-flow changes vs the lighthouse

| Flow | Lighthouse | SDK extraction |
|------|-----------|----------------|
| Bundle origin | `include_str!` of in-crate `bundles/ufh-quote@1.0.0/` | `BundleSource` (local-dir dev / embedded deploy / S3 seam) |
| Manifest origin | hand-built `build_reference_manifest` | `manifest::synthesize` → `ratify` → `emit_bundle` (manifest-driven) |
| Bundle identity | hardcoded `ufh-quote` / justfile literals | `pmcp.toml` `[[workbook.workbooks]]` source→bundle_id map |
| Served tool reg | direct `ServerBuilder::tool_arc` in the server crate | toolkit `workbook` module `try_workbook_from_bundle` |
| Compile entry | `cargo run -p workbook-compiler -- compile-workbook …` | `cargo pmcp compile-workbook …` (thin shell over compiler) |
| Integrity boot check | panic on hash mismatch | typed `BundleLoadError` → `RunError` → non-zero exit (matches sql-server) |

## Suggested PHASE BUILD ORDER

Respects the purity boundary (runtime before any umya code) and the dependency cone (runtime → toolkit module → compiler → CLI → Shape A/B). Directly answers RFC §7: **yes, port the runtime first even within a full-extraction milestone** — it is the smallest cut that proves the boundary, has zero reader deps, and unblocks every downstream consumer.

1. **Phase A — Port `pmcp-workbook-runtime`** (RFC §7 first cut). Lift the reader-free leaf: IR types, `run` executor, `eval_scalar`, manifest projection model (`Manifest`/`CellRole`/`Role`/`Dtype`/`InputTier`/`allowed_values`), bundle artifact model + integrity hashing, render `LayoutDescriptor`, changelog model. Establish the `cargo-tree` purity gate. Publish `pmcp-workbook-runtime` (slot 2a). *Dependency: pmcp only. Proves the boundary before any umya lands.*

2. **Phase B — `BundleSource` + served-tool toolkit module.** Add the `workbook` feature + `pub mod workbook` to `pmcp-server-toolkit`. Define `BundleSource` (+ `LocalDirSource`, `EmbeddedSource`). Lift the served layer (`schema.rs`/`input.rs`/`handler.rs`/`diff_version.rs`/`render_resource.rs` — already manifest-driven), wired through `try_workbook_from_bundle` + boot integrity. *Dependency: Phase A. The schema/input projection comes over nearly unchanged because it already reads the manifest.*

3. **Phase C — `pmcp-workbook-compiler` + the §5 generalization fixes.** Lift the offline pipeline (ingest/dialect/manifest-synth/formula/dag/sheet_ir/reconcile/provenance/artifact/gate/change_class). **Do the §5 fixes here, not after:** (a) delete `build_reference_manifest`/`emit_reference_bundle`, replace with a generic `compile_workbook` routing `synthesize`→`ratify`→`emit_bundle` (the manifest-driven kill); (b) CR-02 — write to the computed `next_version` dir; (c) CR-01 — symmetric change-class classifier; (d) WR-01 — skip tiering for enum inputs; (e) umya fabricated-provenance — distinct provenance class. Publish the compiler (slot 8a). *Dependency: Phase A (re-exports runtime types). The fixes belong in the phase that owns the code they fix.*

4. **Phase D — `cargo pmcp compile-workbook` / `lint-workbook` / `emit-bundle` + `pmcp.toml`.** Add the three command modules + the `--accept` BA approval flow + the project-level `pmcp.toml` (workbooks→bundle IDs) resolving the single-workbook assumption. *Dependency: Phase C (the compiler). `pmcp.toml` lands here because the CLI is its first consumer.*

5. **Phase E — Shape A binary `pmcp-workbook-server`.** Mirror `pmcp-sql-server`: lib `run`/`serve` + thin `main.rs`, CLI (`--bundle-dir`/`--bundle-id`/`--http`), construct a `BundleSource`, build the server from the toolkit `workbook` module. Publish (slot 9a). *Dependency: Phase B (toolkit module) + the `BundleSource` impls.*

6. **Phase F — Shape B scaffold `cargo pmcp new --kind workbook-server` + dialect spec.** Add the `--kind workbook-server` arm to `new.rs` + a `templates::workbook_server` (Cargo.toml + `main.rs` using `EmbeddedSource` + a sample `pmcp.toml` + a sample bundle). Publish the SDK-owned versioned `workbook-dialect-spec.md`. *Dependency: Phase E (the scaffold's generated `main.rs` targets the Shape-A wiring).*

**Ordering rationale:** A→B is the runtime-then-server cut (purity proven before umya). C is intentionally after B so the served contract (what the bundle must contain for `calculate`/`explain`/etc.) is locked before the compiler is generalized to emit it — the bundle is the contract, and you freeze the consumer's needs before re-cutting the producer. D/E/F are the DX surfaces over the now-stable runtime+compiler. The §5 fixes concentrate in C (compiler-owned: manifest-driven emit, CR-01/CR-02/WR-01, umya provenance) and D (`pmcp.toml`, single-workbook generalization) — each fix lands in the phase that owns its code.

## Scalability / Evolution Considerations

| Concern | At 1 workbook (lighthouse parity) | At N workbooks (one server) | At a bundle registry |
|---------|-----------------------------------|------------------------------|----------------------|
| Bundle delivery | `EmbeddedSource` (baked in binary) | `LocalDirSource` over a bundles dir; `pmcp.toml` lists all | `RegistryBundleSource` (deferred seam) |
| Tool naming | flat `calculate`/`explain`/… | namespaced per bundle id (`ufh-quote.calculate`) — design registration to prefix | registry resolves id→bundle |
| Integrity | boot fold over embedded members | per-bundle fold on load | signature/registry-attested |
| Versioning | single `@1.0.0` | `diff_version` across versions present in source | registry version negotiation |

Named-range-backed validation lists and the S3/registry bundle store are deferred by design (documented seams), consistent with the milestone scope.

## Sources

- `crates/workbook-runtime/src/lib.rs` (lighthouse) — reader-free leaf, the `pmcp-workbook-runtime` lift target — HIGH
- `crates/workbook-compiler/src/lib.rs` (lighthouse) — umya pipeline + the hardcoded `build_reference_manifest` (lines 524–606) — HIGH
- `crates/quote-pricing-server/src/workbook/{mod,schema,input}.rs` (lighthouse) — the already-manifest-driven served layer — HIGH
- `crates/pmcp-server-toolkit/src/lib.rs` + `Cargo.toml` (SDK) — the `#[cfg(feature="http")] pub mod http` precedent + feature-gating pattern — HIGH
- `crates/pmcp-sql-server/src/{lib,main}.rs` + `Cargo.toml` (SDK) — the Shape-A lib/`run`/`serve` + thin-shim + `RunError` pattern to mirror — HIGH
- `cargo-pmcp/src/commands/new.rs` + `templates/sql_server.rs` (SDK) — the `--kind` switch + scaffold-template pattern — HIGH
- `docs/sdk-issue-excel-workbook-compiler-extraction.md` (RFC §5/§6/§7) — generalization gaps, open questions, runtime-first recommendation — HIGH
- `docs/Excel-as-Configuration-Architecture-Brief.md` — two-surface model, reuse-vs-new, promote-gate philosophy — HIGH
- `CLAUDE.md` Release & Publish Workflow — the v2.2 publish order extended above — HIGH
