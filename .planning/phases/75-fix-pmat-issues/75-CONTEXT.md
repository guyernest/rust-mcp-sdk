# Phase 75: Fix PMAT issues - Context

**Gathered:** 2026-04-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Restore the auto-generated `Quality Gate: passing` badge on the README by
remediating the PMAT findings surfaced after PR #246 fixed the `pmat-cli` →
`pmat` install bug and the `quality-badges.yml` workflow began running real
analysis instead of falling back to sentinel values.

**In scope:**
- Reduce `pmat quality-gate --fail-on-violation` to exit 0 (the badge signal).
- Remediate the 94 cognitive-complexity-over-25 functions (the gating dimension).
- Best-effort improvements to SATD (33), duplicate (439), entropy (4), and
  README sections (2 detected) within the same waves, where they fall out of
  the complexity work or are cheap to address.
- Tune `.pmat/project.toml` (or equivalent) to exclude `examples/`, `tests/`,
  and `benches/` from duplicate detection where appropriate.
- Add a CI check that runs `pmat quality-gate --fail-on-violation` on PRs and
  blocks merge on regression.

**Out of scope (deferred to other phases or backlog):**
- Driving SATD, duplicate, entropy, or sections to absolute zero — those are
  best-effort here, not gating.
- Rewriting hotspot files for reasons other than complexity (architecture
  cleanup, public-API changes, performance work).
- Net-new features in `pmcp-code-mode/`, `pentest/`, `deployment/`, etc. —
  this phase only touches them to fix complexity violations.

**Baseline (worktrees excluded, from commit 28db59a, 2026-04-22):**

| Dimension  | Count | Notes |
|------------|-------|-------|
| complexity | 94    | cognitive complexity > 25 |
| duplicate  | 439   | includes examples/tests/benches noise |
| satd       | 33    | TODO/FIXME/HACK/XXX comments |
| entropy    | 4     | |
| sections   | 2     | only README Installation + Usage detected |

**Hotspots (from commit message):** `crates/pmcp-code-mode/`,
`cargo-pmcp/src/pentest/`, `cargo-pmcp/src/deployment/`,
`src/server/streamable_http_server.rs`, `pmcp-macros/`.

</domain>

<decisions>
## Implementation Decisions

### Definition of Done
- **D-01:** "Phase 75 done" = `pmat quality-gate --fail-on-violation` exits 0
  and the auto-generated README badge flips back to `Quality Gate: passing`.
  Complexity is the gating dimension; SATD, duplicate, entropy, and sections
  are best-effort improvements within the same waves but do NOT block phase
  closure.

### Complexity Remediation Strategy
- **D-02:** Default approach is pragmatic per-function refactor — extract
  helper methods, introduce intermediate types (state structs, command enums),
  decompose deeply-nested matches. For functions that are irreducibly complex
  (parsers, AST walkers, protocol-message dispatch, state machines), use
  `#[allow(clippy::cognitive_complexity)]` with a one-line `// Why:` comment
  immediately above the attribute justifying why decomposition would harm
  clarity. Every `#[allow]` must have a `Why:` justification — no bare allows.

- **D-03:** Hard ceiling on `#[allow(clippy::cognitive_complexity)]` use:
  even with the allow, a function should aim for ≤35 cognitive complexity
  and MUST NOT exceed 50. Functions above 50 require refactor regardless of
  how good the justification is. Signals that the escape hatch is for
  "legitimate but bounded" complexity, not "gave up".

### SATD + Duplicate Policy
- **D-04:** SATD triage is per-comment, three-way:
  - **Trivial / obsolete** (the comment refers to code that no longer exists,
    or the concern was addressed elsewhere) → delete the comment (and any dead
    code attached).
  - **Real follow-up work needed** → file a GitHub issue in `paiml/rust-mcp-sdk`,
    replace the SATD marker with a normal comment that references the issue
    number (e.g. `// See #NNN — defer to streaming response phase`).
  - **Cheap to fix now (<30 min)** → just fix it in this phase.
  Do not blanket-convert all 33 to issues without triage. Do not blanket-fix
  all 33 in this phase.

- **D-05:** Duplicate triage is by file location:
  - Duplicates inside `src/`, `crates/*/src/`, `cargo-pmcp/src/` → real
    refactor (extract helpers, share via internal modules, use macros where
    appropriate).
  - Duplicates inside `examples/`, `tests/`, `benches/`, `fuzz/` → tune the
    PMAT config (`.pmat/project.toml` or `.pmatignore` — verify the actual
    config file PMAT 3.11.x reads) to exclude these paths from duplicate
    detection. Test fixtures and example boilerplate are intentionally
    duplicated for reader clarity and should not be flagged.
  - Goal is "duplicate count drops meaningfully", not "duplicate count = 0".

### Wave Structure
- **D-06:** Split work into waves by hotspot directory rather than by PMAT
  dimension. This lets each wave ship independently and probably flips the
  badge mid-phase as complexity violations clear:
  - **Wave 1:** `src/server/streamable_http_server.rs` + `pmcp-macros/`
    (cross-cutting impact; touching them affects downstream crates).
  - **Wave 2:** `cargo-pmcp/src/pentest/` + `cargo-pmcp/src/deployment/`
    (newer code, mostly self-contained inside cargo-pmcp).
  - **Wave 3:** `crates/pmcp-code-mode/` (largest hotspot, most contained;
    last so earlier waves don't perturb it).
  - SATD triage, duplicate triage, and PMAT config tuning happen *within* the
    relevant wave when the file is being touched anyway, OR in a parallel
    "housekeeping" plan if they don't naturally fall out.
  Planner is free to add a final "Wave 4: enforcement" plan for the CI gate
  (D-07) so it lands after the badge is already green.

### Going-Forward Enforcement
- **D-07:** After the badge flips green, add a CI check that runs
  `pmat quality-gate --fail-on-violation` on every PR and blocks merge if it
  fails. Likely lives in `.github/workflows/quality-badges.yml` (or a new
  workflow specifically for the gate so badges and gating are decoupled). Do
  NOT add to pre-commit hook in this phase — pre-commit already runs
  `make quality-gate` (fmt, clippy, build, doctests) and adding PMAT there
  risks slowing the dev loop too much. CI block alone is enough to prevent
  regression.

### Claude's Discretion
- Which specific functions to refactor first within each wave (planner +
  executor pick based on dependency order and file co-location).
- Whether to introduce shared types/traits to reduce complexity vs duplicate
  helpers per call site — judgment call per case.
- How to structure GitHub issues filed for SATDs (one issue per SATD vs grouped
  by area) — planner can decide based on triage outcome.
- Exact PMAT config file location and syntax for path exclusion (verify
  against `pmat --help` output and the actual `.pmat/project.toml` format used
  by 3.11.1).
- Whether the CI gate workflow should run on PRs only or also nightly — pick
  whichever is least disruptive while preventing regression.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### PMAT and Quality Gates
- `.github/workflows/quality-badges.yml` — Defines the badge generation logic.
  The Quality Gate badge is set by `pmat quality-gate --fail-on-violation`
  exit code. Complexity, TDG, and Tech Debt badges have their own thresholds.
  Any CI gating change for D-07 modifies (or adds a sibling to) this file.
- `.pmat/project.toml` — Existing PMAT project config (currently records
  version 3.11.1 and last_compliance_check). Path exclusions for D-05 likely
  go here or in a sibling `.pmatignore` — agents should verify against
  `pmat --help` for the version actually installed.
- Commit `28db59aa` (`docs(75): add phase — Fix PMAT quality-gate debt`) —
  Records the baseline counts and hotspot directories. Authoritative source
  for what "fixing PMAT" means in this phase.

### Project Standards
- `CLAUDE.md` (project root) — Toyota Way zero-tolerance policy, the
  "complexity ≤25 per function" target that PMAT enforces, and the SATD
  policy (D-04 reconciles with this). Also defines `make quality-gate` as the
  pre-commit gate.
- `Makefile` — Defines `make quality-gate`, `make lint`, etc. Any new CI gate
  added in D-07 should be a thin wrapper around an existing make target where
  possible, not a parallel command path.

### Prior Phase Patterns
- `.planning/phases/74-add-cargo-pmcp-auth-subcommand-with-multi-server-oauth-token/74-PLAN.md`
  files (74-01, 74-02, 74-03) — Reference for plan structure, wave numbering,
  and acceptance-criteria format. Phase 74 used 3 waves with parallel files,
  which matches the wave structure decided in D-06.
- `.planning/phases/73-typed-client-helpers-list-all-pagination-parity-client-01/73-PLAN.md`
  files — Same reference for plan size and complexity.

### Code Hotspots (read first when planning each wave)
- `src/server/streamable_http_server.rs` — Wave 1 target. Big file, likely
  many >25 complexity functions in protocol dispatch.
- `pmcp-macros/` — Wave 1 target. Macro expansion code is naturally complex;
  expect some `#[allow]` with justification (D-02).
- `cargo-pmcp/src/pentest/` — Wave 2 target. Newer code; check for SATDs
  added during rapid development (D-04).
- `cargo-pmcp/src/deployment/` — Wave 2 target.
- `crates/pmcp-code-mode/` — Wave 3 target. Largest hotspot; AST/parser code
  likely irreducible — heavy `#[allow]` use expected, all bounded by D-03.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `make quality-gate` — Already runs fmt, clippy (pedantic + nursery), build,
  test, and audit. Phase 75 should leverage this as the local check before
  PMAT-specific work; only the CI gate (D-07) is new infrastructure.
- `.github/workflows/quality-badges.yml` — Existing workflow already installs
  PMAT and runs the gate command for badge generation. The CI gate (D-07)
  can either extend this workflow with a fail-on-violation job or add a
  sibling workflow that runs on PR events specifically.

### Established Patterns
- **Plan files are large (50–90KB) and detailed** — Phase 73/74 plans set the
  pattern. Phase 75 plans should match: each plan owns one wave with
  per-function task breakdown for the complexity work.
- **Atomic commits per task** — Project standard from CLAUDE.md and prior
  phase artifacts. Each refactored function should be its own commit so
  regressions can be bisected per-function.
- **Cargo workspace structure** — Root crate is `pmcp`; workspace members are
  `pmcp-macros`, `crates/mcp-tester`, `crates/mcp-preview`, `cargo-pmcp`,
  plus newer `crates/pmcp-code-mode`. Touching `pmcp-macros` rebuilds
  downstream — sequencing matters.

### Integration Points
- New CI gate workflow lives in `.github/workflows/`. Existing `quality-badges.yml`
  is the closest analog.
- PMAT config exclusions for D-05 land in `.pmat/project.toml` or a
  `.pmatignore`-style file (planner verifies against installed PMAT version).
- GitHub issues filed for SATDs (D-04) go in `paiml/rust-mcp-sdk`. The
  remaining code comment should reference the issue number, matching how the
  rest of the codebase tracks deferred work.

</code_context>

<specifics>
## Specific Ideas

- The Quality Gate badge is the single most visible signal — it's at the top
  of the README. The user explicitly framed this phase as "restore the
  badge", not "perfect the codebase". Plans should optimize for the badge
  flipping green as early as possible (likely after wave 1 or 2), not for
  reaching theoretical zero across all dimensions.
- Many of the hotspots (`pmcp-code-mode`, `pentest`, `deployment`) are
  newer code. SATDs there are more likely to be active follow-ups (file an
  issue) than obsolete (delete) — bias the D-04 triage accordingly.
- `streamable_http_server.rs` is mature code in `src/`. SATDs there are more
  likely to be obsolete or trivial — bias toward delete or fix-now.
- `pmcp-macros/` complexity violations are likely irreducible (proc-macro
  expansion is naturally branchy). Expect heavy `#[allow]` use bounded by
  D-03's ≤35 / hard cap 50 rule.

</specifics>

<deferred>
## Deferred Ideas

- **Drive SATD/duplicate/entropy/sections to absolute zero** — Out of scope
  for Phase 75 (D-01). If the team wants to chase the perfect PMAT report,
  that's Phase 76+ as separate per-dimension cleanup phases.
- **Add PMAT to pre-commit hook** — D-07 explicitly chose CI-only. If the
  CI gate proves insufficient (regressions still landing because PRs ignore
  red CI), revisit pre-commit integration in a future phase.
- **Raise the cognitive_complexity threshold from 25 to ~35** — Considered as
  an option but rejected. Stays at 25 to keep the CLAUDE.md "complexity ≤25
  per function" promise honest. The `#[allow]` escape hatch (D-02) handles
  the legitimate exceptions.
- **Whole-file rewrites of hotspot files** — Considered as an option but
  rejected. Function-level refactor is lower risk and ships faster. Whole-file
  rewrites can be a future phase if specific files prove unworkable.
- **Sections badge fix (only 2 README sections detected)** — If trivially
  fixed by adding section headers to README during housekeeping, do it.
  Otherwise defer — it's not on the gate path.

</deferred>

---

*Phase: 75-fix-pmat-issues*
*Context gathered: 2026-04-22 via /gsd-discuss-phase*
