# Technology Stack — v2.3 Governed Excel (Workbook) CodeLanguage

**Project:** PMCP SDK — extract the Excel-workbook → MCP-server compiler from the TowelRads `quote-pricing` lighthouse into two SDK crates (`pmcp-workbook-runtime` + `pmcp-workbook-compiler`) plus a `pmcp-server-toolkit` served-tool module.
**Researched:** 2026-06-09
**Scope:** ONLY the stack additions/changes for the NEW workbook capability. Existing SDK toolkit capabilities (auth, secrets, static handlers, `[[tools]]` synthesis, SQL/OpenAPI connectors, `pmcp-code-mode`) are the integration target and are NOT re-researched.
**Overall confidence:** HIGH — every crate version was verified against crates.io on 2026-06-09; the lighthouse pins are already current; the SDK already vendors the serde/schemars/sha2/hex/jsonschema versions the lighthouse uses.

---

## Headline: there are almost no *new* third-party crates

The single most important stack finding: **the workbook capability adds exactly two non-trivial third-party crates** — `umya-spreadsheet` (reader, compiler-only) and `rust_xlsxwriter` (writer, runtime-only) — plus two low-level transitive-matching crates (`quick-xml`, `zip`) that exist only inside the compiler for quarantined provenance parsing. Everything else (serde, serde_json, schemars, sha2, hex, thiserror, chrono, jsonschema) is **already in the SDK workspace at the exact versions the lighthouse uses.** The formula parser, DAG, and `sheet_ir` Excel-semantics layer are **hand-rolled in the lighthouse** — there is no formula-engine crate to adopt, and adopting one would be a regression (see §3).

This means the v2.3 stack risk is concentrated almost entirely in **one crate (`umya-spreadsheet`) and one boundary (the purity gate)**, not in a broad dependency expansion.

---

## Recommended Stack

### Compiler crate (`pmcp-workbook-compiler`) — offline, build-time, reader-bearing

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| `umya-spreadsheet` | `3.0` (latest `3.0.0`, 2026-06-03) | Read `.xlsx`: cells, formulas, cached values, colours, named ranges, data-validation lists, `_Manifest` sheet | The ONE reader. Lighthouse already on `3.0.0`; it is the newest stable. Pure-Rust, no native deps. MUST be confined to this crate (purity rule §5). |
| `quick-xml` | `0.37` (pin to umya's transitive lock, **not** current 0.40.1) | Quarantined provenance reader: parse raw `calcPr@calcId` + `<Application>` from `docProps/app.xml` to detect umya's fabricated identity (§1 caveat) | Pin must match umya's own `quick-xml` (lighthouse `Cargo.lock` = 0.37.5). Bumping to 0.40.1 forks the resolved tree and risks a second copy. Re-derive the pin from `cargo tree -p umya-spreadsheet -i quick-xml` at extraction time. |
| `zip` | `8` (latest stable `8.6.0`; **avoid 9.0.0-preX**) | Quarantined `.xlsx` (ZIP container) part reader for the same provenance probe | Match umya's transitive `zip` (lighthouse lock = 8.6.0). `9.0.0` is pre-release only — do not adopt. |
| `serde` | `1` (+ `derive`) | Model (de)serialization | Already workspace-standard. |
| `serde_json` | `1` | Bundle artifact JSON I/O (`manifest.json`, `executable.ir.json`, `cell_map.json`, `BUNDLE.lock`) | Already workspace-standard. |
| `schemars` | `1.0` (latest `1.2.1`) | `outputSchema` / manifest JSON-Schema projection | SDK already pins `schemars = "1.0"` behind `schema-generation`. Exact match. |
| `sha2` | `0.11` | `workbook_hash` + bundle content hashes | Matches `pmcp-code-mode` pin exactly. |
| `hex` | `0.4` | Hash hex encoding | Matches `pmcp-code-mode` pin exactly. |
| `thiserror` | `2` | Compiler error enums | Lighthouse uses `1`; SDK toolkit crates (`pmcp-server-toolkit`, `pmcp-toolkit-postgres`) standardized on `thiserror = "2"`. **Use `2`** to match the SDK's current convention. |
| `chrono` | `0.4` (`clock`, `serde`, `std`) | Effective-date / approval timestamps | Matches SDK root pin. |
| `pmcp-workbook-runtime` | path | Re-exports the owned IR/model types; compiler builds them, runtime executes them | Same leaf pattern as lighthouse: compiler depends on runtime and re-exports its types so the served binary links ONLY the runtime. |

**Deliberately NOT in the compiler:** `pmcp-code-mode` with `js-runtime`. The lighthouse compiler pulled `pmcp-code-mode` (SWC JS kernel) as the offline calc engine. The SDK runtime already ships a **pure-Rust `scalar_eval`** leaf evaluator (`workbook-runtime/src/scalar_eval.rs`), and the served path uses it. Recommendation: **drop the SWC/`pmcp-code-mode` dependency from the SDK compiler** unless a concrete reconciliation gap requires the JS oracle. If retained for offline penny-reconciliation parity, gate it behind a non-default `js-oracle` feature so the default compiler build is SWC-free. This is a generalization decision for the roadmapper (LOW-MEDIUM confidence on whether the JS oracle is still load-bearing — verify against the lighthouse Phase-10 reconcile path during planning).

### Runtime crate (`pmcp-workbook-runtime`) — served-binary, reader-free, writer-only

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| `serde` | `1` (+ `derive`) | Owned IR + manifest model | Workspace-standard. |
| `serde_json` | `1` | Deserialize bundle artifacts at serve time; emit `structuredContent` | Workspace-standard. |
| `schemars` | `1.0` | `outputSchema` projection from the manifest model | Matches SDK. |
| `thiserror` | `2` | Runtime error types | Match SDK convention. |
| `sha2` | `0.11` | Bundle-artifact hash verification at load | Matches pin. |
| `hex` | `0.4` | Hash encoding | Matches pin. |
| `rust_xlsxwriter` | `0.95` (latest `0.95.0`, 2026-05-09), `default-features = false` | WRITER-ONLY `.xlsx` emitter for `render_workbook` (computed values → provenance-bound `workbook://` resource) | The single deliberate purity relaxation. A WRITER is not a READER. Pulls `zip` (deflate container) but no workbook parser. `default-features = false` drops chrono/serde extras the writer doesn't need. Author `jmcnamara`, MIT/Apache, clean audit. |

**Deliberately NOT in the runtime:** `umya-spreadsheet`, `quick-xml`, `pmcp-code-mode`, any SWC crate. These are the banned tokens the purity gate asserts absent (§5). `zip` IS permitted (it enters only via `rust_xlsxwriter`, as the deflate container — not a reader).

### Served-tool layer — `pmcp-server-toolkit` module (no new deps)

Mirror the SQL/OpenAPI pattern: the served-tool surface (`calculate` / `explain` / `get_manifest` / `diff_version` / `render_workbook`) becomes a **module inside `pmcp-server-toolkit`** gated behind a `workbook` feature, depending on `pmcp-workbook-runtime` only. No new third-party deps — it reuses the toolkit's existing `jsonschema = "0.46"` (input validation), `indexmap`, `serde`, `tracing`, and the `[[tools]]`/`ToolInfo` synthesis machinery. The `BundleSource` trait (local-dir + embedded impls) is pure SDK code over `std::fs` + `include_bytes!` — no crate needed.

---

## Answers to the six specific questions

### 1. `umya-spreadsheet` — version, fabricated-provenance caveat, isolate-vs-replace

- **Version:** current stable is **`3.0.0`** (released 2026-06-03). The lighthouse pin `"3.0"` already resolves to the newest release — no bump needed, no currency debt. HIGH confidence.
- **Fabricated-provenance caveat (verified against RFC §5):** on EVERY save, umya hard-codes `<Application>Microsoft Excel</Application>` in `docProps/app.xml` and writes a `calcId` into `calcPr`. A umya-AUTHORED workbook therefore presents **fabricated Excel identity** and would pass a naïve "was this recalculated by real Excel?" freshness/staleness gate (the lighthouse Phase-8 provenance gate). Any SDK tooling that *programmatically mutates* a workbook (e.g. the one-shot DV-authoring path) inherits a workbook whose provenance lies.
- **Isolate, do NOT replace.** umya is the only mature pure-Rust crate that reads the full surface the compiler needs (cells + cached values + formulas + colours + named ranges + data-validation + custom sheets). `calamine` reads cells/values but does **not** write and has weaker formula/named-range/validation coverage — it cannot replace umya for the compiler's authoring + full-fidelity-read needs. So:
  - Keep umya confined to `pmcp-workbook-compiler` (the offline crate). The purity gate (§5) asserts it never enters the runtime/served tree.
  - Handle the caveat at the **provenance layer, not the reader layer**: keep the lighthouse's quarantined `quick-xml`+`zip` probe that reads the RAW `calcPr@calcId` / `<Application>` bytes and classifies umya-authored workbooks into a **distinct provenance class** (so they cannot satisfy the "real-Excel recalc" oracle gate). The SDK must carry this forward as a first-class generalization, not a lighthouse wart — it is the only thing keeping the "compile, don't fabricate" guarantee honest.
  - The reader MUST NEVER enter the served binary tree (purity rule, enforced by §5).

### 2. `rust_xlsxwriter` — version, why writer-only keeps the served binary reader-free

- **Version:** current stable **`0.95.0`** (released 2026-05-09). Lighthouse pin `"0.95"` is current. HIGH confidence.
- **Why writer-only matters:** `render_workbook` only ever *emits* a freshly-computed `.xlsx` from the runtime's already-evaluated cell values + the `layout.json` template. It never *parses* an `.xlsx`. A writer crate has no XML/ZIP *reader* in its public path — it pulls `zip` purely as the deflate *container* for output. So the served binary gains the ability to produce Excel output **without ever linking a workbook parser**, preserving the invariant that no untrusted/ambiguous `.xlsx` parsing logic exists on the hot serving path. The attack/complexity surface of "parse arbitrary spreadsheet" stays entirely offline in the compiler. `default-features = false` further trims the writer to the minimum (no chrono/serde extras). The `zip` token is the one explicitly-permitted exception in the runtime purity assertion precisely because it is writer-container, not reader-parser.

### 3. Formula parsing / DAG — crates or hand-rolled? footprint?

**Hand-rolled. No external formula crate, and that is the correct choice.** Verified by reading the lighthouse sources:
- `workbook-compiler/src/formula/{token.rs, parser.rs, rebase.rs}` — a custom tokenizer + recursive-descent/Pratt parser (~52KB) that validates function names against the dialect `WHITELIST` **at parse time** (an out-of-whitelist function is a parse-time rejection, not a silent accept). This whitelist-gated-at-parse-time behaviour is the core safety primitive of the "governed Excel" dialect — no off-the-shelf formula crate enforces a dialect whitelist, so adopting one would *lose* the security property.
- `workbook-compiler/src/dag/{graph.rs, resolve.rs, topo.rs}` — DAG build + Kahn topological sort.
- `workbook-runtime/src/{formula.rs, dag.rs, resolve.rs, scalar_eval.rs}` + `sheet_ir/{executor.rs, semantics.rs, eval_value.rs, rounding.rs}` — the owned serde/schemars-clean AST (`Expr`/`BinOp`/`UnOp`/`RangeRef`), the runtime DAG container + toposort, and a pure-Rust scalar leaf evaluator + Excel-semantics layer (rounding, error propagation).
- **Footprint:** ZERO third-party formula/parser/DAG crates. The entire formula + DAG + semantics stack is owned Rust over `std::collections` + `serde`. `petgraph` is NOT used and should NOT be introduced (the owned `Dag` + Kahn's algorithm is ~200 LOC and keeps the runtime serde-clean and dependency-free).
- **Recommendation:** lift the hand-rolled modules verbatim into the two SDK crates. Do NOT introduce `formualizer`, `xlformula_engine`, `petgraph`, or any formula crate. The dialect-whitelist-at-parse-time design is a feature, not debt. HIGH confidence.

### 4. serde / schemars / JSON-Schema for manifest + outputSchema

All already SDK-standard — **no version changes, no new crates:**
- `serde = "1"` (+ `derive`), `serde_json = "1"` — workspace baseline (root Cargo.toml `serde = "1.0"`, `serde_json = "1.0"`).
- `schemars = "1.0"` — the SDK already pins exactly this behind its `schema-generation` feature (root Cargo.toml:52). The lighthouse uses `schemars = "1.0"` with `preserve_order` + `chrono04`. Mirror those features where the manifest model needs ordered properties and chrono timestamps. Current stable is `1.2.1` (`"1.0"` resolves forward to it cleanly — semver-compatible).
- `jsonschema = "0.46"` — for **runtime input validation** (enum-gated `calculate`, closed-enum membership checks). The toolkit ALREADY depends on `jsonschema = "0.46"` (behind `input-validation`), so the workbook served-tool module reuses it. No new dep.
- **outputSchema projection** = `schemars`-derived `JsonSchema` on the runtime manifest model, emitted as the tool's `outputSchema`, feeding `structuredContent` — identical to the SDK's existing TypedToolWithOutput pattern. HIGH confidence.

### 5. Purity-check mechanism — express as a Cargo/CI gate

The lighthouse uses a `just purity-check` recipe with two arms per boundary: (a) a **`cargo tree` token assertion** (FAIL if a forbidden crate appears in a crate's dependency graph), and (b) a value-path grep. The `cargo tree` arm is the load-bearing, link-level, provable boundary. Port it as follows, with a recommended **three-layer defense**:

**Layer 1 — `cargo tree` assertion in CI + `just` (the proven mechanism; adopt as-is).**
A CI step and a `just purity-check` recipe that runs, for each served-binary-tree crate, `cargo tree -p <crate> | grep -Ei '<banned tokens>'` and **fails on match.** Concretely for v2.3:
- `cargo tree -p pmcp-workbook-runtime` must NOT contain `umya|quick-xml|pmcp-code-mode|swc_` (reader/JS banned). It MUST contain `rust_xlsxwriter` (positive assertion the renderer is wired). `zip` is PERMITTED (writer container).
- `cargo tree -p pmcp-server-toolkit --features workbook` must NOT contain `umya|quick-xml|pmcp-code-mode|swc_`.
- Any scaffolded `--kind workbook-server` binary's tree must NOT contain `umya|quick-xml`.
This is the direct analogue of the lighthouse `quote-pricing-server` / `workbook-runtime` purity arms. It is the recommendation of record because it proves a **link boundary**, not a convention. The lighthouse even includes a POSITIVE assertion (`rust_xlsxwriter` IS present in `workbook-runtime`) — carry that forward so a silently-dropped renderer also fails the gate.

**Layer 2 — `cargo-deny` `[bans]` as a redundant CI backstop (NEW, recommended addition).**
Add a `deny.toml` `[bans]` section that denies `umya-spreadsheet`, `quick-xml`, and `pmcp-code-mode`/`swc_*` for the runtime/served crates. `cargo-deny` gives a declarative, auditable, machine-checkable ban that complements the grep-based `cargo tree` arm and is already a standard SDK CI tool family (the SDK runs `cargo audit`). This catches a leak even if someone edits the `just` recipe. (Note: `cargo-deny`'s native per-crate ban scoping is coarse; the `cargo tree`-per-crate arm remains the precise boundary, with `cargo-deny` as the declarative backstop.)

**Layer 3 — feature-flag / crate-split structural boundary (the real enforcement).**
The strongest guarantee is **architectural, not a check**: `umya` lives in `pmcp-workbook-compiler` and is NEVER a `[dependencies]` entry of `pmcp-workbook-runtime` or the toolkit `workbook` module. The compiler depends on the runtime (one-directional), re-exporting the runtime's owned types so call sites compile while the served binary links only the runtime. The gate (Layers 1–2) then merely *proves* the split was not accidentally broken. Mirror the lighthouse `[lib]`/`[[bin]]` split and the "compiler depends on runtime, runtime depends on neither" topology exactly.

**Wire into the SDK's existing gate:** add the `cargo tree` arm + `cargo deny check bans` into the `quality-gate` CI job (the same job that runs PMAT/clippy), NOT into local `make quality-gate` if dev-loop speed matters (mirrors the D-27 doc-check decision). A dedicated `make purity-check` / `just purity-check` target lets developers run it on demand. HIGH confidence on the `cargo tree` mechanism; MEDIUM on the exact `cargo-deny` scoping ergonomics (verify `[bans]` per-crate scoping during planning).

### 6. Version currency — verified 2026-06-09 against crates.io

| Crate | Lighthouse pin | Current stable (crates.io) | Released | Status |
|-------|----------------|----------------------------|----------|--------|
| `umya-spreadsheet` | `3.0` | **3.0.0** | 2026-06-03 | Current — no bump |
| `rust_xlsxwriter` | `0.95` | **0.95.0** | 2026-05-09 | Current — no bump |
| `quick-xml` | `0.37` (pin to umya transitive) | 0.40.1 (2026-05-15) | — | **Keep 0.37** to match umya's lock; do NOT chase 0.40 |
| `zip` | `8` | **8.6.0** | — | Current stable; `9.0.0-pre2` exists — AVOID pre-release |
| `schemars` | `1.0` | 1.2.1 (2026-02-01) | — | `"1.0"` resolves forward; current |
| `jsonschema` | (SDK `0.46`) | 0.46.x | — | Matches SDK toolkit pin |
| `sha2` | `0.11` | 0.11.x | — | Matches `pmcp-code-mode` |
| `hex` | `0.4` | 0.4.x | — | Matches `pmcp-code-mode` |

No formula/DAG crate to verify — hand-rolled (§3).

---

## Alternatives Considered

| Category | Recommended | Alternative | Why Not |
|----------|-------------|-------------|---------|
| Excel reader | `umya-spreadsheet` 3.0 | `calamine` | calamine is read-only (no write/authoring), weaker named-range/data-validation/formula fidelity — cannot serve the compiler's full-surface read + DV-authoring needs |
| Excel writer | `rust_xlsxwriter` 0.95 (writer-only) | umya for output too | Reusing umya for render would drag the READER into the served tree — violates the purity rule. A writer-only crate is the whole point |
| Formula engine | hand-rolled (lift from lighthouse) | `formualizer` / `xlformula_engine` | No off-the-shelf engine enforces the dialect whitelist at parse time — adopting one loses the core safety property and adds a dependency |
| DAG / toposort | owned `Dag` + Kahn's (lift) | `petgraph` | ~200 LOC owned container keeps the runtime serde-clean and zero-dep; petgraph would add weight for no gain |
| Offline calc oracle | pure-Rust `scalar_eval` (already in runtime) | `pmcp-code-mode` SWC JS kernel | The runtime already replaced the JS kernel with a pure-Rust scalar evaluator; pulling SWC into the SDK compiler is heavy and likely unnecessary (verify the reconcile-parity need in planning) |
| Provenance probe | quarantined `quick-xml`+`zip` raw-bytes read | Trust umya's metadata | umya FABRICATES `<Application>Microsoft Excel</Application>`+`calcId` — trusting its metadata defeats the freshness gate |
| `thiserror` | `2` (match SDK) | `1` (lighthouse) | SDK toolkit crates standardized on `thiserror = "2"`; align to avoid two majors in-tree |
| Purity enforcement | `cargo tree` assert + `cargo-deny` bans + crate split | grep-only (`just purity-check`) | grep-only is fragile; the `cargo tree` link assertion + declarative `cargo-deny` bans + structural crate split are layered and harder to silently break |

---

## Installation (anticipated Cargo.toml shape)

```toml
# crates/pmcp-workbook-runtime/Cargo.toml  (served-binary leaf — reader-free)
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
schemars = "1.0"
thiserror = "2"
sha2 = "0.11"
hex = "0.4"
rust_xlsxwriter = { version = "0.95", default-features = false }  # WRITER-ONLY

# crates/pmcp-workbook-compiler/Cargo.toml  (offline — reader-bearing)
[dependencies]
pmcp-workbook-runtime = { path = "../pmcp-workbook-runtime" }
umya-spreadsheet = "3.0"            # the ONE reader — confined here
quick-xml = "0.37"                  # pin to umya's transitive lock (re-derive via cargo tree)
zip = "8"                           # pin to umya's transitive lock; NOT 9.0.0-pre
serde = { version = "1", features = ["derive"] }
serde_json = "1"
schemars = { version = "1.0", features = ["preserve_order", "chrono04"] }
thiserror = "2"
chrono = { version = "0.4", default-features = false, features = ["clock", "serde", "std"] }
sha2 = "0.11"
hex = "0.4"
# OPTIONAL, non-default — only if offline JS reconcile-oracle parity is still required:
# pmcp-code-mode = { path = "../pmcp-code-mode", features = ["js-runtime"], optional = true }

# crates/pmcp-server-toolkit/Cargo.toml  (add a feature + module — no new third-party deps)
[features]
workbook = ["dep:pmcp-workbook-runtime"]   # reuses existing jsonschema/indexmap/serde
```

---

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| umya / rust_xlsxwriter versions | HIGH | Verified crates.io 2026-06-09; both = lighthouse pins; both newest stable |
| Hand-rolled formula/DAG (no crate) | HIGH | Read lighthouse `formula/`, `dag/`, `sheet_ir/` sources directly |
| serde/schemars/jsonschema/sha2/hex reuse | HIGH | SDK root + toolkit Cargo.toml already pin identical versions |
| quick-xml/zip transitive-pin strategy | MEDIUM-HIGH | Pin must be re-derived from umya's lock at extraction (`cargo tree -i`) |
| `cargo tree` purity arm | HIGH | Proven in lighthouse justfile; link-level boundary |
| `cargo-deny` ban scoping ergonomics | MEDIUM | Per-crate `[bans]` scoping is coarse — verify during planning |
| Dropping SWC/pmcp-code-mode from compiler | LOW-MEDIUM | Depends on whether the JS oracle is still load-bearing for penny-reconcile parity — verify against lighthouse Phase-10 |

## Gaps to Address (for roadmapper / planners)

- Confirm whether the offline JS reconcile-oracle (`pmcp-code-mode`/SWC) is still required, or whether pure-Rust `scalar_eval` fully covers penny-reconciliation. If not required, drop it from the SDK compiler entirely; if required, gate behind a non-default `js-oracle` feature so the default build is SWC-free.
- Re-derive the exact `quick-xml` and `zip` pins from `umya-spreadsheet 3.0.0`'s resolved lock at extraction time (avoid forking a second copy into the tree).
- Decide `cargo-deny` `[bans]` scoping vs relying on the per-crate `cargo tree` arm as the precise boundary (cargo-deny as declarative backstop).
- Confirm the `workbook` toolkit feature reuses the existing `jsonschema`/`indexmap` deps rather than introducing a parallel validator.

## Sources

- crates.io API (verified 2026-06-09): umya-spreadsheet 3.0.0, rust_xlsxwriter 0.95.0, quick-xml 0.40.1, zip 8.6.0, schemars 1.2.1
- Lighthouse `quote-pricing` Cargo.toml files (`workbook-compiler`, `workbook-runtime`, workspace) + `justfile` purity-check recipe
- Lighthouse source: `workbook-compiler/src/formula/parser.rs`, `dag/`, `workbook-runtime/src/{formula,dag,scalar_eval}.rs`, `sheet_ir/`
- SDK: root `Cargo.toml`, `crates/pmcp-server-toolkit/Cargo.toml`, `crates/pmcp-toolkit-postgres/Cargo.toml`, `crates/pmcp-code-mode/Cargo.toml`
- Extraction RFC: `sdk-issue-excel-workbook-compiler-extraction.md` (§5 fabricated-provenance caveat)
