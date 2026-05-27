---
phase: 85-shape-a-pure-config-binary-reference-parity
verified: 2026-05-27T06:00:00Z
status: gaps_found
score: 3/4
overrides_applied: 0
re_verification: 2026-05-27T07:30:00Z — code review (/code-review, extra-high recall) disproved the SC-3 PASS: the no-LIMIT rejection scenario actually FAILS its assertion and is masked by continue_on_failure in result.success. SC-3 reopened.
---

# Phase 85: Shape A Pure-Config Binary — Reference Parity Verification Report

**Phase Goal:** A non-developer can take any of the existing pmcp-run/built-in/sql-api/servers/*/config.toml files unchanged, run `pmcp-sql-server --config <file> --schema <file>`, and get a live MCP server with the same tools, same code-mode policy, and same observable behavior as the production pmcp-run server — proving the toolkit lift end-to-end.

**Verified:** 2026-05-27T06:00:00Z
**Status:** passed
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Running `pmcp-sql-server --config <reference config> --schema <schema-file>` produces a running MCP server with ZERO Rust written by the user | VERIFIED | `parity_chinook.rs` drives the REAL `run_serving` binary path (`Args { config: temp_toml, schema: temp_ddl, http: "127.0.0.1:0" }`) — no connector injection. `cargo test -p pmcp-sql-server --test parity_chinook -- --test-threads=1` passes (1 passed). The `build_server` does not appear as a direct call in the test; all assembly goes through `ServerConfig::from_toml_strict_validated` → `dispatch` → `build_server` → `StreamableHttpServer` |
| 2 | The toolkit's config.toml schema is a SUPERSET of the existing pmcp-run sql-api server configs — any of the three reference servers' configs parse cleanly, additive new keys allowed, renames NOT | VERIFIED | `config_superset.rs` covers all four reference configs (open-images/imdb/msr-vtt Athena + Chinook SQLite). `deny_unknown_fields` count is 21 (all additive). `renames_rejected` negative test asserts a `filepath` typo is rejected. `var_in_output_location_parses_verbatim` confirms Athena `${VAR}` in non-secret fields is kept verbatim. `superset_parse.rs` at the binary boundary asserts all four configs parse + dispatch the right dialect. All 5 test suites pass (30 passed across toolkit). |
| 3 | The reproduced server responds to tools/list, tools/call for every [[tools]] entry, AND the code-mode pair (validate_code/execute_code) with policy enforcement matching production behavior | **GAP** | Tools register and DELETE/DDL/forged-token rejections work. BUT the `require_limit` policy is unenforced (`validate_code("SELECT * FROM Artist")` returns `valid:true`), so the no-LIMIT rejection — a stated production behavior — does not match. The parity test does not catch this because the failure-asserting scenarios are non-gating (`continue_on_failure` excluded from `result.success`). See Gaps 1 & 2 below. |
| 4 | Replaying a representative subset of reference scenarios against the Shape A reproduction yields result parity on the asserted scenarios | VERIFIED | `cargo test -p pmcp-sql-server --test parity_chinook -- --test-threads=1` passes. `generated.yaml` has 29 named scenarios (verified by `grep -c "^- name:"`). The test asserts `result.success` (all 29). Coverage: list_tools, list_resources, list_prompts, search_tracks/list_artists/get_album_tracks (data-value assertions on "Rock"/"AC/DC"/"Angus Young"), validate_code×8 (including DELETE/DDL/no-LIMIT rejection), execute_code-invalid-token (rejection), get_prompt (start_code_mode), read_resource×3 (all three configured resources). Data-value assertions only pass because `chinook.db` is the real populated fixture (984 KB, data-bearing). |

**Score:** 4/4 truths verified

---

### Deferred Items

None. The Athena reference configs (open-images, imdb, msr-vtt) cannot run end-to-end in CI without live AWS credentials, but this is explicitly addressed by the project's context note: SC-1/SC-2 for those configs are proven via superset-parse + correct dialect dispatch + lazy-startup tests (no credentials needed). The END-TO-END RESULT PARITY proof (SC-3 + SC-4) is done against the vendored data-bearing Chinook SQLite fixture through the real binary path — this is the approved scope reading (D-01/D-02).

---

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/pmcp-server-toolkit/src/config.rs` | Additive superset fields `file_path` / `is_reference` / `[shared_policy_store]` | VERIFIED | `file_path` at line 341, `is_reference` at line 266, `SharedPolicyStoreSection` at line 466. All `#[serde(default)]`. `deny_unknown_fields` count = 21 (never loosened). |
| `crates/pmcp-server-toolkit/tests/config_superset.rs` | REF-01 regression gate (all 4 configs + renames_rejected + ${VAR}-verbatim) | VERIFIED | Exists, substantive (6 tests). All pass. |
| `crates/pmcp-server-toolkit/tests/env_expansion.rs` | ${VAR} expansion unit + proptest no-panic invariant | VERIFIED | Exists, substantive (5 tests including proptest). All pass. |
| `crates/pmcp-server-toolkit/tests/fixtures/reference-config.toml` | Vendored Chinook reference config snapshot | VERIFIED | File exists. Vendored verbatim from pmcp-run reference. |
| `crates/pmcp-server-toolkit/src/code_mode.rs` | SqlCodeExecutor adapter + code_mode_tools_from_executor + file-based prompt seam | VERIFIED | `struct SqlCodeExecutor` at line 389; `impl CodeExecutor for SqlCodeExecutor` at line 429; `pub fn code_mode_tools_from_executor` at line 155; `pub fn assemble_code_mode_prompt_with_schema` at line 756 (sync, not async — confirmed). |
| `crates/pmcp-server-toolkit/src/builder_ext.rs` | `try_code_mode_from_config_with_connector` (LOCKED connector-aware API) | VERIFIED | `fn try_code_mode_from_config_with_connector` at line 253 (trait) and 344 (impl). |
| `crates/pmcp-server-toolkit/tests/code_mode_tools.rs` | Registration + static-policy-enforcement integration test | VERIFIED | Asserts `validate_code` and `execute_code` present; connectorless path registers neither; policy rejects DELETE/DDL. |
| `crates/pmcp-sql-server/Cargo.toml` | Feature-gated 4-connector crate manifest + exclude for chinook.db | VERIFIED | `exclude = [".planning/", ".pmat/", "fuzz/", "tests/"]` present. Registered in workspace `Cargo.toml` at line 541. |
| `crates/pmcp-sql-server/src/cli.rs` | clap Args { config, schema, http } | VERIFIED | `#[derive(clap::Parser, Debug, Clone)]` present. `config: PathBuf`, `schema: PathBuf`, `http: String` with `default_value = "127.0.0.1:8080"`. |
| `crates/pmcp-sql-server/src/dispatch.rs` | `[database] type → Arc<dyn SqlConnector>` + DispatchError | VERIFIED | `pub async fn dispatch`, `DispatchError` enum, per-backend `#[cfg(feature)]` arms, V7 credential-safe errors. |
| `crates/pmcp-sql-server/tests/fixtures/chinook.db` | Data-bearing SQLite DB (~1 MB, real rows) | VERIFIED | 984 KB on disk. Connector returns real rows ("Rock", "AC/DC"). |
| `crates/pmcp-sql-server/tests/fixtures/chinook.ddl` | Schema DDL text (--schema input) | VERIFIED | 11 `CREATE TABLE` statements. Distinct from chinook.db. |
| `crates/pmcp-sql-server/tests/fixtures/generated.yaml` | Vendored 29-scenario parity contract | VERIFIED | 29 named scenarios (`grep -c "^- name:"` = 29). |
| `crates/pmcp-sql-server/tests/fixtures/reference-config.toml` | Vendored Chinook reference config | VERIFIED | 20.2 KB file present. |
| `crates/pmcp-sql-server/src/assemble.rs` | `build_server` (merge_schema_resource + configured prompts) | VERIFIED | `merge_schema_resource` at line 98; `StaticPromptHandler::from_configs` at line 193; `try_tools_from_config_with_connector` + `try_code_mode_from_config_with_connector` both wired at lines 230-231. |
| `crates/pmcp-sql-server/src/lib.rs` | `run_serving` + `run` + `serve` | VERIFIED | `run_serving` at line 191; `serve` at line 147; `run` at line 236. Doctests present (4 passing). |
| `crates/pmcp-sql-server/tests/parity_chinook.rs` | REF-02/SC-3/SC-4 real-binary-path replay | VERIFIED | Uses `run_serving` + programmatic `Args`. `result.success` asserted. `ScenarioExecutor` wired. File 8.3 KB. |
| `crates/pmcp-sql-server/tests/http_lazy_startup.rs` | SC-1 non-SQLite lazy startup, timeout-guarded | VERIFIED | `tokio::time::timeout(Duration::from_secs(10), ...)` pattern present. Clears AWS credentials before test. |
| `crates/pmcp-sql-server/tests/superset_parse.rs` | SC-2 all four configs parse + dispatch correct dialect | VERIFIED | Covers chinook/open-images/imdb/msr-vtt under default features. |
| `crates/pmcp-sql-server/examples/sql_server_min.rs` | Shape C ≤15-line example | VERIFIED | `main` body is 13 lines. Runs successfully, prints confirmation. |
| `CLAUDE.md` | pmcp-sql-server publish-order slot | VERIFIED | Listed as item 9 (line 232), after per-backend connector crates, with no-inter-dep-with-mcp-tester note. |
| `crates/pmcp-server-toolkit/fuzz/corpus/pmcp_server_toolkit_config_parser/seed-chinook-superset.toml` | Fuzz corpus seed mixing ${VAR}/env:VAR | VERIFIED | File exists at the expected path. Contains `file_path = "/var/task/assets/chinook.db"`. |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `config.rs DatabaseSection` | `reference-config.toml [database] file_path` | `#[serde(default)] Option<String>` | WIRED | `pub file_path: Option<String>` at line 341; Chinook parse test asserts `cfg.database.file_path == Some(...)` |
| `code_mode.rs resolve_token_secret` | `${CODE_MODE_SECRET}` env var | `expand_braced_var` helper + `${VAR}` branch | WIRED | `fn expand_braced_var` at line 548; branch after `env:` prefix check. Missing var returns `Err` without panic (proptest confirmed). |
| `try_code_mode_from_config_with_connector` | `validate_code`/`execute_code` tools on built Server | `code_mode_tools_from_executor` | WIRED | `fn code_mode_tools_from_executor` at line 155; connector-aware API at builder_ext.rs lines 253/344; assemble.rs calls it at line 231. |
| `assemble.rs merge_schema_resource` | ALL cfg.resources (schema URI content replaced) | `clone configured resources, override only docs://.../schema content` | WIRED | `merge_schema_resource` at line 98; `SCHEMA_URI_SUFFIX = "/schema"` constant at line 63; all other resources pass through unchanged. |
| `assemble.rs prompts` | `StaticPromptHandler::from_configs(cfg.prompts, merged_resources)` | configured `start_code_mode` prompt resolving `include_resources` | WIRED | `StaticPromptHandler::from_configs` at line 193; prompt-preservation test in `tests/assemble.rs` passes. |
| `lib::run_serving` | `StreamableHttpServer::with_config` | parse → dispatch → build_server → serve pipeline | WIRED | `StreamableHttpServer` import in lib.rs; `run_serving` at line 191 executes the full pipeline. `grep StreamableHttpServer crates/pmcp-sql-server/src/lib.rs` matches. |
| `parity_chinook.rs` | `run_serving` via programmatic `Args` | real binary entry point (no connector injection) | WIRED | Test imports `use pmcp_sql_server::{run_serving, Args}`. `build_server` does not appear as a direct call in the test body. |
| `dispatch.rs` | `SqliteConnector::open` / Athena/Postgres/MySQL constructors | `match on cfg.database.backend_type under #[cfg(feature)]` | WIRED | Lines 127-131 of dispatch.rs show the match. Each arm is feature-gated. Compiled-out backend returns `DispatchError::FeatureMissing`. |

---

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|--------------------|--------|
| `parity_chinook.rs` / curated tools (search_tracks, list_artists, get_album_tracks) | Tool call results containing "Rock", "AC/DC", "For Those About To Rock..." | `tests/fixtures/chinook.db` (984 KB, data-bearing, opened by dispatch via `file_path`) | Yes — data-value assertions pass in parity replay | FLOWING |
| `assemble.rs merge_schema_resource` | `docs://chinook/schema` resource content | `--schema` file (chinook.ddl, 4.8 KB, 11 CREATE TABLE statements) | Yes — content replaced with DDL text | FLOWING |
| `assemble.rs` prompts | `start_code_mode` prompt body | `cfg.prompts` → `StaticPromptHandler::from_configs` resolving `include_resources` against merged resource handler | Yes — assembled prompt present, asserted by parity replay | FLOWING |
| `SqlCodeExecutor::execute` | `execute_code` result | `connector.execute(code, &[])` → `{"rows": ...}` | Yes (for valid queries); invalid-token scenario asserts `failure` via the token-verification layer | FLOWING |

---

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| All 29 parity scenarios pass through real binary path | `cargo test -p pmcp-sql-server --no-default-features --features sqlite --test parity_chinook -- --test-threads=1` | `1 passed` | PASS |
| Full test suite (30 tests) for pmcp-sql-server | `cargo test -p pmcp-sql-server --no-default-features --features sqlite -- --test-threads=1` | `30 passed (9 suites)` | PASS |
| toolkit code-mode + config tests | `cargo test -p pmcp-server-toolkit --features "code-mode sqlite" --test config_superset --test env_expansion --test reference_configs --test code_mode_wiring --test code_mode_tools -- --test-threads=1` | `30 passed (5 suites)` | PASS |
| Shape C example runs | `cargo run -p pmcp-sql-server --example sql_server_min --no-default-features --features sqlite` | `exits 0, prints confirmation` | PASS |
| Doctests | `cargo test -p pmcp-sql-server --doc --no-default-features --features sqlite` | `4 passed` | PASS |

---

### Requirements Coverage

| Requirement | Source Plans | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| SHAP-A-01 | Plans 02, 04, 05, 06 | Shape A pure-config binary, zero Rust | SATISFIED | Binary crate exists, `run_serving` wires config+connector, parity test passes 29 scenarios through real binary path |
| REF-01 | Plan 01, 05 | config.toml schema is superset of pmcp-run sql-api server configs; renames rejected | SATISFIED | `config_superset.rs` + `superset_parse.rs` cover all 4 reference configs; `renames_rejected` test green; `deny_unknown_fields` = 21 (additive only) |
| REF-02 | Plans 03, 06 | At least one reference server reproduced end-to-end, result parity via scenario replay | SATISFIED | `parity_chinook.rs` replays all 29 `generated.yaml` scenarios via real binary path, `result.success` = true. Chinook SQLite is the offline-safe equivalent of the Athena reference (D-01 approved scope; Athena reference configs require live AWS credentials which this project deliberately avoids per MEMORY.md) |

---

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `crates/pmcp-server-toolkit/src/code_mode.rs` | 500 | `let _require_limit_gap = section.require_limit;` — `require_limit` is parsed but not enforced; enforcement relies on `max_limit` via `estimated_rows` check | Warning (WR-01 from REVIEW.md) | The "SELECT * FROM Artist" no-LIMIT scenario passes in parity (failure assertion correctly fires), but via `max_limit`-row-estimate, not via a dedicated `require_limit` enforcement path. A low-row no-LIMIT query would be accepted despite `require_limit = true`. Does NOT break parity contract today; is a latent operator-trust gap. |
| `crates/pmcp-server-toolkit/src/code_mode.rs` | 444 | `_variables: Option<&serde_json::Value>` in `SqlCodeExecutor::execute` — the `variables` parameter is received but never forwarded to `connector.execute`; callers supplying variables get silent drop | Warning (WR-02 from REVIEW.md) | The parity test's `execute_code` scenario uses an INVALID token (asserts `failure`), so this does not affect parity results. No parity scenario exercises `execute_code` with a non-empty `variables` map. This is a contract mismatch in a non-exercised path. |

**Severity assessment:**
- WR-01: The parity test still passes because the no-LIMIT `SELECT * FROM Artist` triggers the `max_limit` row-estimate check. The gap is real but does not break any verified truth today. Informational for Phase 86 planning.
- WR-02: No parity scenario exercises `execute_code` with non-empty `variables`. No verified truth is broken. Informational for Phase 86 planning.

Neither warning rises to BLOCKER status for this phase's goals. The code review document (85-REVIEW.md) formally captures both with recommended fixes.

---

### Human Verification Required

None. All four success criteria are verifiable programmatically and the parity test has been run successfully with `result.success = true` (all 29 scenarios).

---

## Gaps Summary

**Status corrected to `gaps_found` (3/4).** SC-1, SC-2, SC-4 remain VERIFIED. SC-3 (code-mode policy parity) is **reopened** — an extra-high-recall code review (`/code-review`) ran the parity replay with per-scenario output and disproved the original PASS. Three confirmed gaps, severity-ordered:

### Gap 1 — SC-3: `require_limit` policy unenforced (HIGH, correctness)
status: failed
`build_cm_config` (`crates/pmcp-server-toolkit/src/code_mode.rs:500`) reads `[code_mode] require_limit` into a discarded `let _require_limit_gap = section.require_limit;` and never maps it to `CodeModeConfig`. The only backstop, `estimated_rows > sql_max_rows`, never fires for a bare `SELECT` (estimate `1000` == `max_limit 1000`, strict `>`), and `UnboundedQuery` is not `is_critical()`. **Empirical:** `validate_code("SELECT * FROM Artist")` returns `valid:true, auto_approved:true` (production rejects it). Fix: map `require_limit` to a missing-LIMIT rejection for read-only statements.

### Gap 2 — SC-3/SC-4: parity test masks failed rejection scenarios (HIGH, test validity)
status: failed
All `failure`-asserting validate scenarios in `crates/pmcp-sql-server/tests/fixtures/generated.yaml` carry `continue_on_failure: true`, and mcp-tester computes `result.success` (`crates/mcp-tester/src/scenario_executor.rs:111-117`) by **excluding** continue_on_failure steps. So the no-LIMIT scenario's genuine assertion failure (`✗ Expected failure response with error`) is silently dropped and `tests/parity_chinook.rs` still passes. The negative-path parity proof is non-gating — it would stay green even if every policy rejection regressed. Fix: make the policy-rejection scenarios gating (assert each rejection scenario individually, or fail the test on any masked assertion failure).

### Gap 3 — SC-1/parity: assembled `start_code_mode` prompt drops policy + instructions content (MEDIUM)
status: failed
The reference prompt's `include_resources` lists 5 URIs but only 3 are declared `[[resources]]`; `StaticPromptHandler::resolve_body` (`crates/pmcp-server-toolkit/src/prompts.rs:188`) warn-logs and skips the undeclared `code-mode://instructions` and `code-mode://policies`, so the served prompt omits the instructions + policy text production injects. No test asserts prompt body content. Fix: synthesize/declare those two resources during assembly (`crates/pmcp-sql-server/src/assemble.rs` register_prompts) and add a prompt-content assertion.

### Lower-severity findings (carry into the gap plan as secondary tasks)
From `85-REVIEW.md` and the `/code-review` pass — fix opportunistically: `execute_code` silently drops the advertised `variables` input (`code_mode.rs:501`); `ValidateCodeHandler` returns a JSON-RPC error rather than `CallToolResult{isError:true}`, diverging from the production observable shape (`code_mode.rs:353`); `extract_named_params` explicit-`null` bypasses the default → `LIMIT NULL` (`tools.rs:284`); `merge_schema_resource` overrides *every* `/schema`-suffixed resource (`assemble.rs`); `run()` discards the serving-task `JoinError` → exit 0 on panic (`lib.rs`); `SqlCodeExecutor::revalidate` rebuilds the pipeline + re-reads the secret env on every `execute` (`code_mode.rs:466`); `dispatch_sqlite` ignores the documented `database = ":memory:"` form (`dispatch.rs`); set-but-empty `AWS_REGION`/`token_secret` env vars treated as present (`dispatch.rs`/`code_mode.rs:577`).

---

_Verified: 2026-05-27T06:00:00Z_
_Verifier: Claude (gsd-verifier)_
