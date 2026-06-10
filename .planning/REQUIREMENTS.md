# Requirements — Milestone v2.3: Excel-as-Configuration MCP Servers

**Goal:** Extract the proven Excel-workbook → MCP-server compiler from the `towelrads-quote-pricing` lighthouse into the PMCP SDK as a new "governed Excel" CodeLanguage — alongside the v2.2 SQL and OpenAPI toolkits — so any project can **compile, not interpret** a governed workbook into a tested, versioned, deterministic MCP server. Generalize the known lighthouse debt (RFC §5) rather than copy it.

**Source:** RFC `sdk-issue-excel-workbook-compiler-extraction.md` + `.planning/research/` (STACK / FEATURES / ARCHITECTURE / PITFALLS / SUMMARY).

**Load-bearing invariant (applies to every requirement):** the Excel reader (`umya`) must NEVER enter the served-binary tree. The served path links only `pmcp-workbook-runtime`; the reader lives only in `pmcp-workbook-compiler`.

---

## v2.3 Requirements

### Workbook Runtime — `pmcp-workbook-runtime` crate (reader-free leaf)

- [ ] **WBRT-01**: Developer can depend on a reader-free crate owning the shared model types (Manifest, CellMap, BundleLock, VersionChangelog, IR `Cell`/`Expr`), serde/schemars-clean, deserialized identically by the offline emitter and the served binary
- [ ] **WBRT-02**: Server can run a compiled workbook's IR through a deterministic evaluator producing typed outputs plus per-cell derivation traces
- [ ] **WBRT-03**: Server can render a computed workbook back to `.xlsx` via a writer-only renderer (`rust_xlsxwriter`) that keeps the served binary reader-free
- [ ] **WBRT-04**: CI/`just` purity gate fails the build if the Excel reader (`umya`/`quick-xml`) appears in the runtime or any served-binary dependency tree (cargo-tree assertion + `cargo-deny [bans]` backstop)

### Workbook Dialect — SDK-owned, versioned

- [ ] **WBDL-01**: SDK owns a versioned dialect spec document (function whitelist + refuse-set) bound to the `WHITELIST` const by a test that fails if doc and code diverge
- [ ] **WBDL-02**: A workbook declares the dialect version it targets, enabling forward-compatible dialect evolution
- [ ] **WBDL-03**: Developer can lint a workbook against the dialect (whitelist-only, deny-by-default) and receive collect-all, located, BA-actionable findings with repair guidance

### Workbook Compiler — `pmcp-workbook-compiler` crate (umya-owning, offline only)

- [ ] **WBCO-01**: Compiler ingests a `.xlsx` (umya, compiler-isolated) and captures cached cell values as a trusted oracle
- [ ] **WBCO-02**: Compiler synthesizes a candidate semantic manifest (inputs/outputs/dtypes/units/meanings/tiers) from colour/Guide/headers with BA ratification — **fully workbook-driven, no per-workbook Rust** (kills the hardcoded `build_reference_manifest`)
- [ ] **WBCO-03**: Compiler parses formulas and reconstructs the dependency DAG with an Excel-semantics layer (`sheet_ir`)
- [ ] **WBCO-04**: Compiler compiles pure cells to executable IR and penny-reconciles computed values against the oracle (operand-anchored rounding, not a naïve abs-delta tolerance)
- [ ] **WBCO-05**: Compiler emits the compiled bundle (manifest.json, executable.ir.json, cell_map.json, layout.json, BUNDLE.lock, evidence/) — the complete compiler↔server contract
- [ ] **WBCO-06**: Compiler synthesizes closed JSON-Schema enums from inline Excel data-validation lists (inline `formula1` quoted literals, ≤10 values); range/named-range sources are rejected with precise reason codes
- [ ] **WBCO-07**: The oracle staleness/freshness gate assigns a **distinct provenance class** to programmatically-authored (umya-stamped, fabricated `<Application>Microsoft Excel</Application>`/`calcId`) workbooks so they cannot pass the freshness gate on fabricated Excel identity

### Workbook Governance — promote-time gate (the differentiating moat)

- [ ] **WBGV-01**: Compiler auto-derives a change class (HotReload / BlockUntilAccept / NeverAutoPromote) from a prior-vs-current manifest+IR diff, with **symmetric coverage of demotion-direction changes** (Input→Constant, source flips) — fixes CR-01
- [ ] **WBGV-02**: A strictest-policy reducer ensures an assumption (yellow-cell) change hard-blocks even when other deltas are hot-reloadable
- [ ] **WBGV-03**: The gate distinguishes numeric drift from semantic redefinition via a stable canonical IR sub-DAG identity hash
- [ ] **WBGV-04**: The golden-corpus gate blocks any over-tolerance named-output delta unless a fingerprint-matching `ApprovalRecord` covers the candidate
- [ ] **WBGV-05**: A BA can record an approval via `--accept --approver <X> --effective-date <D>`, re-baselining the golden corpus and writing a fingerprint-bound `ApprovalRecord`
- [ ] **WBGV-06**: Promotion writes the new bundle to its own `@<next_version>` directory and never overwrites the baseline — fixes CR-02
- [ ] **WBGV-07**: Enum inputs skip Variable-tier assignment so the default path can never seed an out-of-enum empty string — fixes WR-01

### Workbook Served-tool layer — `pmcp-server-toolkit` module (bundle-driven)

- [ ] **WBSV-01**: Agent can call `calculate` with typed, tier-enforced, dtype-checked, enum-gated inputs and receive **all named outputs** (`{value,unit}` each) plus a provenance stamp — no single privileged "headline" output
- [ ] **WBSV-02**: Agent can call `explain` to get an ordered per-cell business-language derivation trace (formula + operand values + manifest meaning), with reconciliation annotations generalized from the lighthouse `coil_band` field
- [ ] **WBSV-03**: Agent can call `get_manifest` to discover input meanings/tiers/defaults/units, output units/meanings, governed data, versions, and changelog
- [ ] **WBSV-04**: Agent can call `diff_version` to read the recorded, hash-verified prev→current changelog (per-output deltas, change class, severity, summary)
- [ ] **WBSV-05**: Agent can call `render_workbook` to receive a provenance-bound `workbook://` resource URI whose `resources/read` statelessly regenerates the computed `.xlsx` (provenance-verified, re-validated, re-run, rendered)
- [ ] **WBSV-06**: Every domain failure returns a structured `isError:true` envelope (in `structuredContent`, never a protocol `Err`) carrying `code`, `reason`, and self-repair fields (`allowed` / `required` / `range`) plus the provenance stamp
- [ ] **WBSV-07**: Input and output schemas are projected entirely from the manifest (`additionalProperties:false`, per-column dtype/unit/meaning; mandatory non-empty `outputSchema`) — parity with the SQL/OpenAPI `TypedToolWithOutput` pattern
- [ ] **WBSV-08**: The server recomputes the `BUNDLE.lock` combined hash at boot and fails closed on any tampered or mismatched artifact before serving
- [ ] **WBSV-09**: A server loads a bundle via a `BundleSource` trait with local-directory and embedded (`include_dir!`) implementations; S3/registry is a documented extension seam, not shipped

### Workbook CLI & Developer Experience

- [ ] **WBCL-01**: Developer can run `cargo pmcp compile-workbook <wb.xlsx>` to ingest→lint→synth→parse→compile→reconcile→**gate**→write a bundle (gate runs before any write)
- [ ] **WBCL-02**: Developer can run `cargo pmcp lint-workbook <wb.xlsx>` to run the dialect linter standalone
- [ ] **WBCL-03**: Developer can run `cargo pmcp emit-bundle` to regenerate a bundle without the gate (dev/reference)
- [ ] **WBCL-04**: A project declares workbooks → bundle IDs in a project-level `pmcp.toml`, replacing the lighthouse's single-workbook justfile/path assumptions
- [ ] **WBCL-05**: Developer can run `cargo pmcp new --kind workbook-server` to scaffold a thin binary over `BundleSource` + the served-tool toolkit module (Shape B)
- [ ] **WBCL-06**: A `pmcp-workbook-server` pure-config binary stands up a live MCP server from a compiled bundle alone, no user Rust (Shape A, mirroring `pmcp-sql-server`)

### Generalization Validation

- [ ] **WBEX-01**: A second, non-lighthouse example workbook compiles and serves end-to-end through the SDK path — the generalization gate proving the manifest is truly synth-driven (no per-workbook Rust, no privileged single output)
- [ ] **WBEX-02**: An Excel-quirk fixture corpus (1900 leap-year, empty-cell coercion, error propagation) verifies reconcile determinism beyond the single golden case

---

## Future Requirements (deferred to v2.x)

- [ ] Wire the two shape-only error triggers end-to-end (`stale_oracle` oracle-freshness re-gate runtime trigger; `unapproved_assumption`)
- [ ] Multi-bundle / N-workbook server with tool-name namespacing per bundle ID
- [ ] Multi-output generalization hardening once many N-output workbooks beyond the lighthouse are validated
- [ ] `cargo pmcp deploy` integration baking embedded bundles into Lambda (EmbeddedSource parity)

## Out of Scope (explicit exclusions)

- **Live workbook interpretation on the hot path** — dissolves the security message; compile-not-interpret is the whole point (reader never enters served binary)
- **Named-range-backed validation lists** — range source unresolved at synth time; deferred by design with a documented extension seam (only inline-literal DV enums ship)
- **S3 / registry `BundleSource`** — adds a network trust + cache-invalidation surface before the local case is proven; `BundleSource` trait leaves the seam
- **Code-mode `validate → token → execute` ceremony for workbooks** — a compiled workbook is curated config trusted by the promote gate + BA curation, not an untrusted runtime token; this explicitly does NOT touch `pmcp-code-mode`
- **Capability cells** (Rust/remote/MCP escape hatches) — non-deterministic, contract-tested not value-tested, needs Cedar/AVP policy wiring; the pure-cell penny-golden path ships first
- **Row-block iteration / "for each room" loops** — arbitrary-N generalization is the hardest parser problem; a later milestone
- **Google Sheets ingest** — alternate front-end into the same dialect; future
- **Widening the function whitelist on demand** — coverage-vs-guarantee law; grow the whitelist deliberately one verified function at a time, route the tail to a developer

---

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| _(filled by roadmap)_ | | |
