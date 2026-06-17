# Requirements — Milestone v2.3: Excel-as-Configuration MCP Servers

**Goal:** Extract the proven Excel-workbook → MCP-server compiler from the `towelrads-quote-pricing` lighthouse into the PMCP SDK as a new "governed Excel" CodeLanguage — alongside the v2.2 SQL and OpenAPI toolkits — so any project can **compile, not interpret** a governed workbook into a tested, versioned, deterministic MCP server. Generalize the known lighthouse debt (RFC §5) rather than copy it.

**Source:** RFC `sdk-issue-excel-workbook-compiler-extraction.md` + `.planning/research/` (STACK / FEATURES / ARCHITECTURE / PITFALLS / SUMMARY).

**Load-bearing invariant (applies to every requirement):** the Excel reader (`umya`) must NEVER enter the served-binary tree. The served path links only `pmcp-workbook-runtime`; the reader lives only in `pmcp-workbook-compiler`.

---

## v2.3 Requirements

### Workbook Runtime — `pmcp-workbook-runtime` crate (reader-free leaf)

- [x] **WBRT-01**: Developer can depend on a reader-free crate owning the shared model types (Manifest, CellMap, BundleLock, VersionChangelog, IR `Cell`/`Expr`), serde/schemars-clean, deserialized identically by the offline emitter and the served binary
- [x] **WBRT-02**: Server can run a compiled workbook's IR through a deterministic evaluator producing typed outputs plus per-cell derivation traces
- [x] **WBRT-03**: Server can render a computed workbook back to `.xlsx` via a writer-only renderer (`rust_xlsxwriter`) that keeps the served binary reader-free
- [x] **WBRT-04**: CI/`just` purity gate fails the build if the Excel reader (`umya`/`quick-xml`) appears in the runtime or any served-binary dependency tree (cargo-tree assertion + `cargo-deny [bans]` backstop)

### Workbook Dialect — SDK-owned, versioned

- [x] **WBDL-01**: SDK owns a versioned dialect spec document (function whitelist + refuse-set) bound to the `WHITELIST` const by a test that fails if doc and code diverge
- [x] **WBDL-02**: A workbook declares the dialect version it targets, enabling forward-compatible dialect evolution
- [x] **WBDL-03**: Developer can lint a workbook against the dialect (whitelist-only, deny-by-default) and receive collect-all, located, BA-actionable findings with repair guidance

### Workbook Compiler — `pmcp-workbook-compiler` crate (umya-owning, offline only)

- [x] **WBCO-01**: Compiler ingests a `.xlsx` (umya, compiler-isolated) and captures cached cell values as a trusted oracle
- [x] **WBCO-02**: Compiler synthesizes a candidate semantic manifest (inputs/outputs/dtypes/units/meanings/tiers) from colour/Guide/headers with BA ratification — **fully workbook-driven, no per-workbook Rust** (kills the hardcoded `build_reference_manifest`)
- [x] **WBCO-03**: Compiler parses formulas and reconstructs the dependency DAG with an Excel-semantics layer (`sheet_ir`)
- [x] **WBCO-04**: Compiler compiles pure cells to executable IR and penny-reconciles computed values against the oracle (operand-anchored rounding, not a naïve abs-delta tolerance)
- [x] **WBCO-05**: Compiler emits the compiled bundle (manifest.json, executable.ir.json, cell_map.json, layout.json, BUNDLE.lock, evidence/) — the complete compiler↔server contract
- [x] **WBCO-06**: Compiler synthesizes closed JSON-Schema enums from inline Excel data-validation lists (inline `formula1` quoted literals, ≤10 values); range/named-range sources are rejected with precise reason codes
- [x] **WBCO-07**: The oracle staleness/freshness gate assigns a **distinct provenance class** to programmatically-authored (umya-stamped, fabricated `<Application>Microsoft Excel</Application>`/`calcId`) workbooks so they cannot pass the freshness gate on fabricated Excel identity

### Workbook Governance — promote-time gate (the differentiating moat)

- [x] **WBGV-01**: Compiler auto-derives a change class (HotReload / BlockUntilAccept / NeverAutoPromote) from a prior-vs-current manifest+IR diff, with **symmetric coverage of demotion-direction changes** (Input→Constant, source flips) — fixes CR-01
- [x] **WBGV-02**: A strictest-policy reducer ensures an assumption (yellow-cell) change hard-blocks even when other deltas are hot-reloadable
- [x] **WBGV-03**: The gate distinguishes numeric drift from semantic redefinition via a stable canonical IR sub-DAG identity hash
- [x] **WBGV-04**: The golden-corpus gate blocks any over-tolerance named-output delta unless a fingerprint-matching `ApprovalRecord` covers the candidate
- [x] **WBGV-05**: A BA can record an approval via `--accept --approver <X> --effective-date <D>`, re-baselining the golden corpus and writing a fingerprint-bound `ApprovalRecord`
- [x] **WBGV-06**: Promotion writes the new bundle to its own `@<next_version>` directory and never overwrites the baseline — fixes CR-02
- [x] **WBGV-07**: Enum inputs skip Variable-tier assignment so the default path can never seed an out-of-enum empty string — fixes WR-01

### Workbook Served-tool layer — `pmcp-server-toolkit` module (bundle-driven)

- [x] **WBSV-01**: Agent can call `calculate` with typed, tier-enforced, dtype-checked, enum-gated inputs and receive **all named outputs** (`{value,unit}` each) plus a provenance stamp — no single privileged "headline" output
- [x] **WBSV-02**: Agent can call `explain` to get an ordered per-cell business-language derivation trace (formula + operand values + manifest meaning), with reconciliation annotations generalized from the lighthouse `coil_band` field
- [x] **WBSV-03**: Agent can call `get_manifest` to discover input meanings/tiers/defaults/units, output units/meanings, governed data, versions, and changelog
- [x] **WBSV-04**: Agent can call `diff_version` to read the recorded, hash-verified prev→current changelog (per-output deltas, change class, severity, summary)
- [x] **WBSV-05**: Agent can call `render_workbook` to receive a provenance-bound `workbook://` resource URI whose `resources/read` statelessly regenerates the computed `.xlsx` (provenance-verified, re-validated, re-run, rendered)
- [x] **WBSV-06**: Every domain failure returns a structured `isError:true` envelope (in `structuredContent`, never a protocol `Err`) carrying `code`, `reason`, and self-repair fields (`allowed` / `required` / `range`) plus the provenance stamp
- [x] **WBSV-07**: Input and output schemas are projected entirely from the manifest (`additionalProperties:false`, per-column dtype/unit/meaning; mandatory non-empty `outputSchema`) — parity with the SQL/OpenAPI `TypedToolWithOutput` pattern
- [x] **WBSV-08**: The server recomputes the `BUNDLE.lock` combined hash at boot and fails closed on any tampered or mismatched artifact before serving
- [x] **WBSV-09**: A server loads a bundle via a `BundleSource` trait with local-directory and embedded (`include_dir!`) implementations; S3/registry is a documented extension seam, not shipped

### Workbook CLI & Developer Experience

- [x] **WBCL-01**: Developer can run `cargo pmcp compile-workbook <wb.xlsx>` to ingest→lint→synth→parse→compile→reconcile→**gate**→write a bundle (gate runs before any write)
- [x] **WBCL-02**: Developer can run `cargo pmcp lint-workbook <wb.xlsx>` to run the dialect linter standalone
- [x] **WBCL-03**: Developer can run `cargo pmcp emit-bundle` to regenerate a bundle without the gate (dev/reference)
- [x] **WBCL-04**: A project declares workbooks → bundle IDs in a project-level `pmcp.toml`, replacing the lighthouse's single-workbook justfile/path assumptions
- [x] **WBCL-05**: Developer can run `cargo pmcp new --kind workbook-server` to scaffold a thin binary over `BundleSource` + the served-tool toolkit module (Shape B)
- [x] **WBCL-06**: A `pmcp-workbook-server` pure-config binary stands up a live MCP server from a compiled bundle alone, no user Rust (Shape A, mirroring `pmcp-sql-server`)

### Generalization Validation

- [x] **WBEX-01**: A second, non-lighthouse example workbook compiles and serves end-to-end through the SDK path — the generalization gate proving the manifest is truly synth-driven (no per-workbook Rust, no privileged single output)
- [x] **WBEX-02**: An Excel-quirk fixture corpus (1900 leap-year, empty-cell coercion, error propagation) verifies reconcile determinism beyond the single golden case

---

## Future Requirements (deferred to v2.x)

- [ ] Wire the two shape-only error triggers end-to-end (`stale_oracle` oracle-freshness re-gate runtime trigger; `unapproved_assumption`)
- [ ] Multi-bundle / N-workbook server with tool-name namespacing per bundle ID
- [ ] Multi-output generalization hardening once many N-output workbooks beyond the lighthouse are validated
- [ ] `cargo pmcp deploy` integration baking embedded bundles into Lambda (EmbeddedSource parity)

## v2.4 Requirements - GitHub-Native Deployment Automation

**Goal:** Provide two complementary GitHub deployment paths: portable GitHub Actions scaffolding in the open-source `cargo-pmcp` CLI for all supported targets, and a pmcp.run managed connected-repository path for hosted builds, deploy logs, rollbacks, and previews.

- [ ] **GHDEP-01**: Developer can run `cargo pmcp github init --target-type <target>` to generate an idempotent `.github/workflows/pmcp-deploy.yml` for each cargo-pmcp deployment target (`pmcp-run`, `aws-lambda`, `google-cloud-run`, `azure-container-apps`, `cloudflare-workers`)
- [ ] **GHDEP-02**: The generated workflow reuses `.pmcp/deploy.toml`, named target conventions, and `cargo pmcp deploy`; no second deployment config file is introduced for GitHub automation
- [ ] **GHDEP-03**: pmcp.run GitHub Actions deployments prefer GitHub OIDC (`id-token: write`) and exchange the GitHub OIDC JWT for a short-lived pmcp.run deploy credential instead of requiring long-lived PMCP secrets
- [ ] **GHDEP-04**: The pmcp.run OIDC trust contract is represented explicitly in SDK-side command/help/docs so the service can validate repository, ref, audience, and environment claims before minting deploy credentials
- [ ] **GHDEP-05**: Non-pmcp.run targets get transparent GitHub Actions workflows that keep cloud credentials in the user's GitHub/cloud account and do not route AWS/GCP/Azure/Cloudflare credentials through pmcp.run
- [ ] **GHDEP-06**: Documentation and examples distinguish the open-source GitHub Actions path from the pmcp.run connected-repository path, including trigger behavior, authentication, logs, rollbacks, and preview deployment ownership

## v2.4 Requirements - Deploy `stack.ts` Regeneration Safety (cargo-pmcp)

**Goal:** `cargo pmcp deploy` must stop silently clobbering an operator-curated `deploy/lib/stack.ts`, and curated template metadata (`mcp:serverType`, `mcp:snapshotBaked`) must be reproducible-from-config so it survives any regeneration. Source: debug session `.planning/debug/deploy-overwrites-stack-ts.md`.

- [x] **DSTK-01**: `cargo pmcp deploy` does not overwrite an existing `deploy/lib/stack.ts` on EITHER target (pmcp-run `targets/pmcp_run/deploy.rs`, aws-lambda `commands/deploy/deploy.rs`) without an explicit `--regenerate-stack` (alias `--force`) opt-in; IAM validation still runs and a "preserved existing stack.ts" notice is printed when the write is skipped
- [x] **DSTK-02**: `.pmcp/deploy.toml` gains an optional `[metadata]` block (`server_type`, `snapshot_baked`) on `DeployConfig`, threaded through `render_stack_ts_for_deploy` / `render_stack_ts` and `McpMetadata`, so a custom / pmcp.toml server's `mcp:serverType` is config-overridable instead of hardcoded `'custom'`
- [x] **DSTK-03**: `mcp:snapshotBaked` is representable end-to-end — `McpMetadata` carries `snapshot_baked`, `to_cdk_context` emits `-c 'mcp:snapshotBaked=…'`, and the generated stack template emits the `mcp:snapshotBaked` metadata literal so curated values are reproducible from config
- [x] **DSTK-04**: ALWAYS coverage — exists-guard unit tests on BOTH deploy targets (preserved without flag, overwritten with flag), config-survives-render unit/property tests, golden-file update in `tests/backward_compat_stack_ts.rs` for the new `mcp:snapshotBaked` line, and `--regenerate-stack` documented in `cargo-pmcp/docs/commands/deploy.md`

## v2.4 Requirements - Workbook-Crate Cognitive-Complexity Reduction (PMAT gate debt)

**Goal:** The v2.3 workbook crates carry 21 cognitive-complexity violations that the org-required PMAT gate (`pmat quality-gate --fail-on-violation --checks complexity`) blocks — surfaced on PR #279 because PMAT is CI-only (Phase 75 D-07) and the milestone never ran through a PMAT-gated CI. PMAT ignores `#[allow(clippy::cognitive_complexity)]` (Phase 75 D-10-B), so each function must be refactored to clear the gate. Behavior is preserved via the milestone's existing golden/reconcile/quirk test net. No `.pmatignore` weakening of production crates.

- [x] **CPLX-01**: All flagged `pmcp-workbook-runtime` functions pass the PMAT complexity gate — `render/mod.rs::render_xlsx` (93), `sheet_ir/executor.rs::eval_expr` (58), `bundle_loader.rs::load` (28), `sheet_ir/semantics.rs::f_index` (24) / `f_search` (31) — refactored to the gate threshold with golden/reconcile tests still green
- [x] **CPLX-02**: All flagged `pmcp-workbook-compiler` functions pass the gate — `change_class/mod.rs::classify_cell_roles` (74), `change_class/ir_identity.rs::dependency_order` (24), `ingest/mod.rs::ingest` (57) / `references_external_workbook` (31), `formula/token.rs::tokenize` (52) / `lex_quoted_sheet_ref` (33) / `scan_atom_run` (30), `dialect/linter.rs::extract_function_tokens` (29), `fixture_author.rs::author_xlsx` (29), `dag/resolve.rs::walk` (25), `gate/corpus.rs::derive_case_grid` (34) / `no_seeded_value_outside_allowed` (46), `provenance/gate.rs::gate_inner` (29), `provenance/raw_parts.rs::parse_calc_pr` (44) / `parse_app_props` (39) — refactored, behavior preserved
- [x] **CPLX-03**: The flagged `pmcp-server-toolkit` function `workbook/input.rs::validate_input` (33) passes the gate, with input-validation behavior preserved
- [x] **CPLX-04**: `pmat quality-gate --fail-on-violation --checks complexity` reports ZERO violations workspace-wide; no production crate added to `.pmatignore`; full test suite + `make quality-gate` green (no behavior regressions); PR #279 CI complexity gate green

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
| WBRT-01 | Phase 91 | Complete |
| WBRT-02 | Phase 91 | Complete |
| WBRT-03 | Phase 91 | Complete |
| WBRT-04 | Phase 91 | Complete |
| WBDL-01 | Phase 91 | Complete |
| WBDL-03 | Phase 93 | Complete |
| WBSV-01 | Phase 92 | Complete |
| WBSV-02 | Phase 92 | Complete |
| WBSV-03 | Phase 92 | Complete |
| WBSV-04 | Phase 92 | Complete |
| WBSV-05 | Phase 92 | Complete |
| WBSV-06 | Phase 92 | Complete |
| WBSV-07 | Phase 92 | Complete |
| WBSV-08 | Phase 92 | Complete |
| WBSV-09 | Phase 92 | Complete |
| WBCO-01 | Phase 93 | Complete |
| WBCO-02 | Phase 93 | Complete |
| WBCO-03 | Phase 93 | Complete |
| WBCO-04 | Phase 93 | Complete |
| WBCO-05 | Phase 93 | Complete |
| WBCO-06 | Phase 93 | Complete |
| WBCO-07 | Phase 93 | Complete |
| WBGV-01 | Phase 93 | Complete |
| WBGV-02 | Phase 93 | Complete |
| WBGV-03 | Phase 93 | Complete |
| WBGV-04 | Phase 93 | Complete |
| WBGV-05 | Phase 93 | Complete |
| WBGV-06 | Phase 93 | Complete |
| WBGV-07 | Phase 93 | Complete |
| WBCL-01 | Phase 94 | Complete |
| WBCL-02 | Phase 94 | Complete |
| WBCL-03 | Phase 94 | Complete |
| WBCL-04 | Phase 94 | Complete |
| WBCL-06 | Phase 95 | Complete |
| WBCL-05 | Phase 96 | Complete (96-02) |
| WBDL-02 | Phase 96 | Complete (96-01) |
| WBEX-01 | Phase 96 | Complete |
| WBEX-02 | Phase 96 | Complete (96-05) |
| GHDEP-01 | Phase 97 | Pending |
| GHDEP-02 | Phase 97 | Pending |
| GHDEP-03 | Phase 97 | Pending |
| GHDEP-04 | Phase 97 | Pending |
| GHDEP-05 | Phase 97 | Pending |
| GHDEP-06 | Phase 97 | Pending |
| DSTK-01 | Phase 98 | Complete |
| DSTK-02 | Phase 98 | Complete |
| DSTK-03 | Phase 98 | Complete |
| DSTK-04 | Phase 98 | Complete |
| CPLX-01 | Phase 99 | Complete |
| CPLX-02 | Phase 99 | Complete |
| CPLX-03 | Phase 99 | Complete |
| CPLX-04 | Phase 99 | Complete |
