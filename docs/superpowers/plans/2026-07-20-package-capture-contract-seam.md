# Package-Capture Contract Seam Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bind `cargo pmcp package capture`'s two GraphQL operations to a versioned, platform-owned SDL contract enforced by a blocking test, so the CLI and pmcp.run cannot silently drift.

**Architecture:** Approach B — vendored SDL + contract test, no runtime codegen. A `capture-v1.graphql` SDL subset is **generated from the live source data API** and checked in. An offline blocking test validates the CLI's actual runtime query strings and response structs against that SDL. A scheduled, M2M-authed online job re-introspects the live source API and diffs it against the vendored SDL to flag platform-ahead drift. One shared introspect+extract tool powers both generation and the online diff.

**Tech Stack:** Rust, `reqwest` (existing hand-rolled GraphQL), `apollo-compiler` (new dev-dependency, operation-vs-schema validation), the existing `auth.rs` client_credentials (M2M) flow, GitHub Actions.

## Global Constraints

- **Binding approach B only** — no `graphql-client`/`cynic` runtime codegen; the two ops stay hand-written in `graphql.rs`.
- **`status` is typed `String`** in the SDL (NOT a GraphQL `enum`); known values live as an SDL doc-comment. A `status` enum is explicitly deferred to a future `capture-v2` (out of scope).
- **The SDL is generated from the live schema, never hand-authored.**
- **The contract's return types must preserve the asymmetry exactly:** `submitPackageCapture` → `captureId` + `createdAt`; `getPackageCaptureStatus` → `id` + `updatedAt`.
- **Online job auth = M2M** (`PMCP_CLIENT_ID`/`PMCP_CLIENT_SECRET`), never the PKCE user token; it must introspect the **source data API behind `api_url`** (capture ops live in `amplifyData`), never the merged API.
- **Offline contract test is blocking (normal CI); online drift job is non-blocking/scheduled.**
- All work is on branch `feat/package-remote-capture-show` in worktree `~/Development/mcp/sdk/rust-mcp-sdk-package-capture`. Paths below are relative to that worktree root.
- Commit after every task. Match existing repo style (`cargo fmt`, clippy clean).

---

### Task 1: Extract the two capture query strings into shared constants

Makes the runtime queries reachable by the contract test, so the test validates the *actual* queries (not a copy). Pure refactor — behavior-preserving.

**Files:**
- Modify: `cargo-pmcp/src/deployment/targets/pmcp_run/graphql.rs` (the `submit_package_capture` and `get_package_capture_status` fns, ~lines 1310–1400)

**Interfaces:**
- Produces: `pub(crate) const SUBMIT_PACKAGE_CAPTURE_QUERY: &str` and `pub(crate) const GET_PACKAGE_CAPTURE_STATUS_QUERY: &str` (the exact GraphQL operation documents the CLI sends).

- [ ] **Step 1: Add the two constants** immediately above `submit_package_capture`, moving the existing raw strings verbatim:

```rust
/// The exact `submitPackageCapture` operation the CLI sends. Shared with the
/// offline contract test (`tests/package_capture_contract.rs`) so the test
/// validates the real runtime query against the vendored SDL.
pub(crate) const SUBMIT_PACKAGE_CAPTURE_QUERY: &str = r#"
        mutation SubmitPackageCapture(
            $rootComponentType: String!,
            $rootComponentId: String!,
            $version: String!,
            $bump: String
        ) {
            submitPackageCapture(
                rootComponentType: $rootComponentType,
                rootComponentId: $rootComponentId,
                version: $version,
                bump: $bump
            ) {
                captureId
                status
                createdAt
            }
        }
    "#;

/// The exact `getPackageCaptureStatus` operation the CLI sends. Shared with the
/// offline contract test.
pub(crate) const GET_PACKAGE_CAPTURE_STATUS_QUERY: &str = r#"
        query GetPackageCaptureStatus($id: ID!) {
            getPackageCaptureStatus(id: $id) {
                id
                status
                message
                errorCode
                divergentComponents
                manifestDigest
                updatedAt
            }
        }
    "#;
```

- [ ] **Step 2: Replace the inline `let query = r#"..."#;` in both fns** with `let query = SUBMIT_PACKAGE_CAPTURE_QUERY;` and `let query = GET_PACKAGE_CAPTURE_STATUS_QUERY;` respectively. Delete the now-duplicated inline strings.

- [ ] **Step 3: Build and run the existing capture unit/integration coverage to prove no behavior change**

Run: `cargo build -p cargo-pmcp && cargo test -p cargo-pmcp capture`
Expected: PASS (same as before — pure extraction).

- [ ] **Step 4: Commit**

```bash
git add cargo-pmcp/src/deployment/targets/pmcp_run/graphql.rs
git commit -m "refactor(capture): hoist the two capture GraphQL ops to shared consts"
```

---

### Task 2: Build the shared introspect + extract-subset tool

One reusable command that authenticates (M2M), introspects the **source data API**, and emits the capture-subset SDL. Used by Task 2b (generate/vendor) and Task 4 (online diff). Kept as a `cargo-pmcp` subcommand-free internal binary so CI can invoke it without network deps leaking into the library.

**Files:**
- Create: `cargo-pmcp/src/bin/capture_contract.rs`
- Reuse: `cargo-pmcp/src/deployment/targets/pmcp_run/auth.rs` (`get_credentials`), the `execute_graphql`/endpoint plumbing in `graphql.rs`

**Interfaces:**
- Produces a CLI: `capture_contract emit` (prints the extracted capture-subset SDL to stdout) and `capture_contract check <path>` (introspects live, extracts subset, diffs against the SDL at `<path>`, exits non-zero on drift).
- Consumes: `PMCP_CLIENT_ID`/`PMCP_CLIENT_SECRET` (M2M) via `auth::get_credentials`; `PMCP_SOURCE_GRAPHQL_URL` for the source data API endpoint (the amplifyData API behind `api_url`, distinct from the merged `DEFAULT_GRAPHQL_URL`).

- [ ] **Step 1: Write the introspection request** — a standard GraphQL `__schema` introspection query POSTed with the M2M bearer token to `PMCP_SOURCE_GRAPHQL_URL`. Add the well-known introspection query as a `const INTROSPECTION_QUERY: &str` (the standard full `IntrospectionQuery` document).

- [ ] **Step 2: Write the subset extractor.** From the introspected schema JSON, select ONLY: the `Mutation.submitPackageCapture` and `Query.getPackageCaptureStatus` fields, their argument types, and the transitive types they return (`CaptureInfo`-equivalent and `CaptureStatus`-equivalent, whatever the schema names them). Render them as SDL. Type `status` fields as `String` (they already are in the schema) and DO NOT invent an enum.

- [ ] **Step 3: The right-endpoint assertion (source vs merged).** After introspection, assert the extracted schema actually contains `submitPackageCapture` AND `getPackageCaptureStatus`. If either is absent, exit with: `error: introspected schema has no capture ops — wrong endpoint? expected the source data API (amplifyData) behind api_url, got {url}`. (This is the one-line guard against introspecting the merged API, whose shape can lag.)

- [ ] **Step 4: `emit` prints SDL; `check <path>` diffs.** `check` normalizes both the freshly-extracted SDL and the file at `<path>` (parse → canonical print via `apollo-compiler`, so field ordering/whitespace don't cause false diffs) and exits `1` with a unified diff if they differ, `0` if identical.

- [ ] **Step 5: Manual smoke (requires dev M2M creds; skip in offline CI)**

Run: `PMCP_CLIENT_ID=… PMCP_CLIENT_SECRET=… PMCP_SOURCE_GRAPHQL_URL=<source-api> cargo run -p cargo-pmcp --bin capture_contract -- emit`
Expected: prints an SDL block containing `submitPackageCapture`, `getPackageCaptureStatus`, `status: String`, `captureId`, `createdAt`, `id`, `updatedAt`.

- [ ] **Step 6: Commit**

```bash
git add cargo-pmcp/src/bin/capture_contract.rs cargo-pmcp/Cargo.toml
git commit -m "feat(capture): shared introspect+extract tool for the capture contract"
```

---

### Task 2b: Generate and vendor `capture-v1.graphql`

**Files:**
- Create: `contracts/pmcp-run/capture-v1.graphql`

- [ ] **Step 1: Generate from live** using the Task 2 tool against dev, and write the output to the contract file with a provenance header prepended:

```bash
mkdir -p contracts/pmcp-run
{
  echo "# pmcp.run package-capture contract — v1"
  echo "# GENERATED from the live source data API by cargo-pmcp/src/bin/capture_contract.rs."
  echo "# Do NOT hand-edit. Regenerate: capture_contract emit > contracts/pmcp-run/capture-v1.graphql"
  echo "# Source endpoint: <source data API url>   Introspected: 2026-07-20"
  echo "# NOTE: 'status' is String (not an enum). Known values: queued, walking,"
  echo "#       extracting, publishing, completed, failed, not_found."
  echo ""
  PMCP_CLIENT_ID=… PMCP_CLIENT_SECRET=… PMCP_SOURCE_GRAPHQL_URL=<source-api> \
    cargo run -q -p cargo-pmcp --bin capture_contract -- emit
} > contracts/pmcp-run/capture-v1.graphql
```

- [ ] **Step 2: Eyeball the invariants** — confirm the file types `status: String`, includes `createdAt` on the submit return type and `id`+`updatedAt` on the status return type, and preserves `captureId` vs `id`.

- [ ] **Step 3: Commit**

```bash
git add contracts/pmcp-run/capture-v1.graphql
git commit -m "feat(capture): vendor capture-v1 SDL contract (generated from live dev schema)"
```

---

### Task 3: Offline blocking contract test (operation- AND struct-vs-schema validation)

**Files:**
- Create: `cargo-pmcp/tests/package_capture_contract.rs`
- Modify: `cargo-pmcp/Cargo.toml` (`[dev-dependencies]`: add `apollo-compiler`)

**Interfaces:**
- Consumes: `SUBMIT_PACKAGE_CAPTURE_QUERY`, `GET_PACKAGE_CAPTURE_STATUS_QUERY` (Task 1); `contracts/pmcp-run/capture-v1.graphql` (Task 2b).

> **Crate choice (explicit, per review):** use **`apollo-compiler`** — it parses SDL into a validated `Schema` and validates an executable operation against it (field existence, argument types, selection-set shape). Plain `graphql-parser` only *parses* and will NOT validate op-vs-schema; do not use it here. Fallback only if `apollo-compiler` proves unworkable: `async-graphql-parser` + a hand-walked AST check.

- [ ] **Step 1: Add the dev-dependency**

```toml
# cargo-pmcp/Cargo.toml, under [dev-dependencies]
apollo-compiler = "1"
```

- [ ] **Step 2: Write the failing test** — validates BOTH runtime ops against the vendored SDL, plus a struct-field cross-check:

```rust
// cargo-pmcp/tests/package_capture_contract.rs
use apollo_compiler::{ExecutableDocument, Schema};

const SDL_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../contracts/pmcp-run/capture-v1.graphql"
);

fn schema() -> Schema {
    let sdl = std::fs::read_to_string(SDL_PATH).expect("read capture-v1.graphql");
    Schema::parse_and_validate(sdl, "capture-v1.graphql")
        .expect("vendored SDL is itself valid")
        .into_inner()
}

/// Both runtime queries must validate against the vendored contract.
#[test]
fn capture_ops_validate_against_contract() {
    let schema = schema();
    for (name, op) in [
        ("submit", cargo_pmcp::pmcp_run_graphql::SUBMIT_PACKAGE_CAPTURE_QUERY),
        ("status", cargo_pmcp::pmcp_run_graphql::GET_PACKAGE_CAPTURE_STATUS_QUERY),
    ] {
        ExecutableDocument::parse_and_validate(&schema, op, format!("{name}.graphql"))
            .unwrap_or_else(|e| panic!("`{name}` op does not match capture-v1.graphql: {e}"));
    }
}

/// `status` must be a plain String in the contract — never a GraphQL enum
/// (an enum would make the online schema-vs-schema diff show permanent drift).
#[test]
fn status_field_is_string_not_enum() {
    let sdl = std::fs::read_to_string(SDL_PATH).unwrap();
    assert!(sdl.contains("status: String"), "status must be typed String in capture-v1.graphql");
    assert!(!sdl.contains("enum CaptureStatusValue"), "status must not be an enum in v1");
}

/// The response structs' GraphQL field names must exactly equal each op's
/// selection set (struct <-> query <-> schema all agree).
#[test]
fn response_structs_match_selection_sets() {
    // CaptureInfo (submit) selects: captureId, status, createdAt
    for f in ["captureId", "status", "createdAt"] {
        assert!(cargo_pmcp::pmcp_run_graphql::SUBMIT_PACKAGE_CAPTURE_QUERY.contains(f),
            "CaptureInfo field `{f}` missing from submit selection set");
    }
    // CaptureStatus (status) selects: id, status, message, errorCode,
    // divergentComponents, manifestDigest, updatedAt
    for f in ["id", "status", "message", "errorCode", "divergentComponents",
              "manifestDigest", "updatedAt"] {
        assert!(cargo_pmcp::pmcp_run_graphql::GET_PACKAGE_CAPTURE_STATUS_QUERY.contains(f),
            "CaptureStatus field `{f}` missing from status selection set");
    }
}
```

- [ ] **Step 3: Expose the consts to the test.** The test references `cargo_pmcp::pmcp_run_graphql::…`. In `cargo-pmcp/src/lib.rs`, add a narrow test-facing re-export so integration tests can reach the two consts without making the whole GraphQL module public:

```rust
// cargo-pmcp/src/lib.rs
/// Test-facing re-exports for the offline capture contract test. Not a public API.
#[doc(hidden)]
pub mod pmcp_run_graphql {
    pub use crate::deployment::targets::pmcp_run::graphql::{
        GET_PACKAGE_CAPTURE_STATUS_QUERY, SUBMIT_PACKAGE_CAPTURE_QUERY,
    };
}
```

(Adjust the two consts' visibility to `pub` if the re-export requires it, keeping the `#[doc(hidden)]` module as the only exposure.)

- [ ] **Step 4: Run the test to verify it fails first (before the SDL is correct)** — temporarily point `SDL_PATH` at a truncated copy, confirm `capture_ops_validate_against_contract` FAILS, then restore. This proves the test actually validates rather than vacuously passing.

Run: `cargo test -p cargo-pmcp --test package_capture_contract`
Expected (with truncated SDL): FAIL naming the missing field. (with real SDL): PASS.

- [ ] **Step 5: Run the full test green**

Run: `cargo test -p cargo-pmcp --test package_capture_contract`
Expected: 3 tests PASS.

- [ ] **Step 6: Commit**

```bash
git add cargo-pmcp/tests/package_capture_contract.rs cargo-pmcp/Cargo.toml cargo-pmcp/src/lib.rs
git commit -m "test(capture): offline blocking contract test (ops + structs vs capture-v1 SDL)"
```

> The offline test runs in the normal `cargo test` gate (`ci.yml` already runs the workspace tests) — no CI change needed for the blocking layer.

---

### Task 4: Online drift check — PLATFORM-OWNED (no SDK-side workflow)

**Status: no code in this repo.** Revised from the original "scheduled M2M CI job"
after the source `amplifyData` API (apiId `q3gd4hrbabeytc2o2zazld6igy`) was confirmed
IAM-auth'd from the platform backend and **not client-reachable**: no SDK-obtainable
token (M2M client-credentials or PKCE user token) can introspect it, so a headless
SDK CI job structurally cannot perform the diff. The online drift check therefore
lives on the platform side (see revised spec §5b), and the SDK repo ships **no**
`.github/workflows/capture-contract-drift.yml`.

**What the platform owns (their counterpart ticket, filed by pmcp.run):**
- Periodic (e.g. weekly) `aws appsync get-introspection-schema` against
  apiId `q3gd4hrbabeytc2o2zazld6igy` → reduce to the capture subset → diff against
  the vendored `contracts/pmcp-run/capture-v1.graphql`.
- On drift: open a PR to `paiml/rust-mcp-sdk` updating `capture-v1.graphql`. That PR
  trips the offline blocking test (Task 3), forcing the CLI's ops/structs to follow
  in lockstep (spec §6).

**What stays in this repo:** the `capture_contract` dev tool (Task 2) remains a
useful *manual* introspect/diff aid for anyone with source-API access, but nothing
in SDK CI depends on it. The offline blocking test (Task 3) is the SDK's hard gate.

No implementation step, no commit. This task is satisfied by the spec §5b revision
(recording platform ownership) and the request already sent to the platform in
`docs/platform-requests/capture-sdl-export-request.md` (its "ongoing drift check"
section asks them to own exactly this). The runbook (Task 5) references the
platform-owned model rather than an SDK workflow.

---

### Task 5: Release runbook

**Files:**
- Create: `docs/runbooks/package-capture-release.md`

- [ ] **Step 1: Write the runbook** covering, in order: (a) rebase `feat/package-remote-capture-show` onto current `origin/main` (110 commits behind — expect conflict resolution; use the absolute git binary to avoid the `rtk` proxy corrupting diffs, per prior session note); (b) run the not-yet-run **E2E test against dev** — `cargo pmcp login` (or M2M env), then `cargo pmcp package capture <team-slug-id> --version <x.y.z>` end-to-end to a `completed` status + a real manifest digest; (c) the coordinated `cargo-pmcp` release shipping **capture/show/import/approve together** (bump, `make quality-gate`, tag/publish per the repo's Release workflow, including the release.yml crate-ordering already fixed in PR #303). State the invariant that the offline contract test must be green against a live-generated `capture-v1.graphql` before release.

- [ ] **Step 2: Commit**

```bash
git add docs/runbooks/package-capture-release.md
git commit -m "docs(capture): package-capture release runbook (rebase + E2E + coordinated publish)"
```

---

### Task 6: File the two spun-off tickets

Not code — GitHub issues so the DX and future-enum work aren't lost. (If `gh` isn't authenticated for this repo, write them as `docs/tickets/*.md` stubs instead and note to file them.)

- [ ] **Step 1: Slug-vs-name `--team-id` UX + `long_help` fix**

```bash
gh issue create --repo paiml/rust-mcp-sdk \
  --title "package capture: slug-friendly --team-id guard + fix wrong 'UUID' long_help" \
  --body "capture.rs takes an AgentTeam id that is a SLUG (e.g. day-trip-planner-team), not a UUID; its long_help wrongly says UUID. A 'reject non-UUID' guard would reject valid slug ids. Do: (1) fix long_help to say 'AgentTeam id (slug), not the display name'; (2) reject only OBVIOUS display names (input contains a space or is mixed-case) with a clear message; (3) otherwise pass through and surface a clean server-side 'AgentTeam not found'. Client-side code cannot reliably tell a valid slug-id from a display name, so do NOT gate on UUID format. Orthogonal to the contract seam."
```

- [ ] **Step 2: Optional `status` → server-side enum (future capture-v2)**

```bash
gh issue create --repo paiml/rust-mcp-sdk \
  --title "capture-v2 (optional): promote GraphQL 'status' from String to a real enum" \
  --body "capture-v1.graphql types status as String to match the live schema (an enum would false-positive the online drift diff). If the platform promotes status to a server-side GraphQL enum, a future capture-v2 contract can assert the enum truthfully. Optional cleanup, zero dependency on the v1 critical path."
```

- [ ] **Step 3: Commit any doc stubs** (only if `gh` was unavailable and stubs were written)

```bash
git add docs/tickets/ 2>/dev/null && git commit -m "docs(capture): stub the two spun-off tickets" || true
```

---

## Self-Review

**Spec coverage:**
- Spec §3 (generated SDL, String-not-enum, provenance) → Tasks 2, 2b + `status_field_is_string_not_enum` test. ✅
- Spec §4 (Approach B, no codegen, offline test of ops + structs) → Tasks 1, 3. ✅
- Spec §5a (offline blocking) → Task 3 (runs in normal `cargo test`). ✅
- Spec §5b (online drift check) → **platform-owned** (Task 4 revised: no SDK-side workflow; the source API isn't client-reachable). The `capture_contract` dev tool (Task 2) remains a manual aid. ✅
- Spec §6 (platform PRs the contract; offline gate forces lockstep) → the blocking Task 3 test enforces it; documented in Task 5 invariant. ✅
- Spec §7 (compat/versioning) → contract filename `capture-v1`; no code needed. ✅
- Spec §8 (delivery invariant, steps in runbook) → Task 5. ✅
- Spec §9 (out-of-scope tickets: slug UX, enum promotion) → Task 6. ✅

**Placeholder scan:** the only intentionally-parameterized values are live secrets/URLs (`<source-api>`, `PMCP_CLIENT_ID=…`) which are environment inputs, not plan gaps. No TODO/TBD logic.

**Type consistency:** `SUBMIT_PACKAGE_CAPTURE_QUERY` / `GET_PACKAGE_CAPTURE_STATUS_QUERY` named identically in Tasks 1, 3. `capture_contract emit`/`check` used consistently in Tasks 2, 2b, 4. Struct field lists in Task 3 match the verified `CaptureInfo`/`CaptureStatus` definitions.
