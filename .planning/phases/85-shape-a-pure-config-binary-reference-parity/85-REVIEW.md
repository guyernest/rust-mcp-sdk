---
phase: 85-shape-a-pure-config-binary-reference-parity
reviewed: 2026-05-27T02:14:37Z
depth: standard
files_reviewed: 10
files_reviewed_list:
  - crates/pmcp-server-toolkit/src/config.rs
  - crates/pmcp-server-toolkit/src/code_mode.rs
  - crates/pmcp-server-toolkit/src/builder_ext.rs
  - crates/pmcp-server-toolkit/src/tools.rs
  - crates/pmcp-server-toolkit/src/lib.rs
  - crates/pmcp-sql-server/src/assemble.rs
  - crates/pmcp-sql-server/src/cli.rs
  - crates/pmcp-sql-server/src/dispatch.rs
  - crates/pmcp-sql-server/src/lib.rs
  - crates/pmcp-sql-server/src/main.rs
findings:
  critical: 0
  warning: 2
  info: 4
  total: 6
status: issues_found
---

# Phase 85: Code Review Report

**Reviewed:** 2026-05-27T02:14:37Z
**Depth:** standard
**Files Reviewed:** 10
**Status:** issues_found

## Summary

Phase 85 adds the Shape A pure-config SQL MCP binary (`pmcp-sql-server`) plus the
toolkit-side foundations it needs: superset config fields (`file_path`,
`is_reference`, `[shared_policy_store]`), a real connector-backed code-mode
registration (`SqlCodeExecutor` + `validate_code`/`execute_code` handlers), a
file-based prompt seam, and backend dispatch with credential-safe errors.

The code is high quality and the security-critical paths hold up well:

- **V7 credential safety is solid.** `DispatchError` never echoes URLs, file
  paths, or credentials; the SQLite open path is mapped to a path-free
  `SqliteOpen` variant; URL backends rely on the connectors' already-redacted
  `ConnectorError` (`sanitize_url` / `strip_aws_credentials`, verified present).
- **Secret resolution never panics or falls back to a weak secret.** Both
  `env:VAR` and `${VAR}` forms read the env var and surface a typed
  `ToolkitError::CodeMode` on a miss; inline literals are default-rejected (R9);
  `expand_braced_var` is correctly scoped to exact `${NAME}` so it cannot widen
  the inline-secret hole.
- **Code-mode policy is enforced and surfaced as MCP errors.** A policy rejection
  (writes/deletes/DDL) returns `Ok(ValidationResult{is_valid:false})` from the
  pipeline, which `ValidateCodeHandler` correctly converts to `isError:true` via
  the `to_json_response().1` flag. `SqlCodeExecutor::execute` re-validates before
  ever reaching the connector (defense-in-depth). The 29-scenario Chinook parity
  test passes through the real `run_serving` binary path (confirmed by running it).
- **Offline-safe dispatch (SC-1) is real** — Postgres/MySQL use lazy pools, the
  Athena arm pins an explicit region to avoid IMDS probing; no `execute()` /
  `schema_text()` round-trip happens at dispatch.

Two warnings concern config fields whose *stated* semantics are not actually
enforced (`require_limit`) and a variable-binding mismatch in `execute_code`.
Neither breaks the parity contract, but both can mislead an operator who trusts
the config field's documented meaning. The remaining items are informational.

## Warnings

### WR-01: `[code_mode] require_limit` is silently unenforced — the no-LIMIT guarantee relies on `max_limit` instead

**File:** `crates/pmcp-server-toolkit/src/code_mode.rs:497-500`
**Issue:**
`build_cm_config` deliberately leaves `require_limit` unmapped
(`let _require_limit_gap = section.require_limit;`) and `pmcp-code-mode` has no
require-limit concept (grep for `require_limit` across the crate returns zero
matches). The `CodeModeSection::require_limit` doc comment states *"Whether
SELECT queries must declare a LIMIT"*, but nothing enforces that.

The reference config sets `require_limit = true` AND `max_limit = 1000`, and the
`'Validate: SELECT without LIMIT should be rejected'` parity scenario
(`SELECT * FROM Artist`, `tests/fixtures/generated.yaml:251-261`) passes — but it
passes via the unrelated `estimated_rows > sql_max_rows` check
(`validation.rs:1219`), NOT via `require_limit`. A bounded-but-LIMIT-less query
such as `SELECT Name FROM Artist WHERE ArtistId = 1` (low `estimated_rows`) would
be **accepted** despite `require_limit = true`. The field is therefore dead
config that an operator could reasonably rely on for a safety guarantee it does
not provide.

This is also a latent parity fragility: the no-LIMIT scenario only passes because
`max_limit (1000)` happens to be below the unbounded-query row estimate. If
`max_limit` were raised or removed, the no-LIMIT scenario would start passing
validation (no `failure`) and the parity test would break — with no `require_limit`
backstop.

**Fix:** Either (a) wire `require_limit` into enforcement — e.g. map it to a
`sql_require_limit` flag in `pmcp-code-mode` and reject read-only statements
without `info.has_limit` in `check_sql_config_authorization`; or (b) if the
toolkit's position is that `max_limit` subsumes it, downgrade the field's doc
comment to explicitly say "advisory only; enforcement is via `max_limit`" and add
a unit test asserting a low-row no-LIMIT SELECT is accepted, so the gap is
intentional and locked rather than incidental. Option (a) matches the field's
documented intent and the threat-model "no-LIMIT under read-only" line.

### WR-02: `execute_code` accepts `variables` but `SqlCodeExecutor::execute` ignores them and binds no params

**File:** `crates/pmcp-server-toolkit/src/code_mode.rs:441-455` (executor) and `:335` (handler call)
**Issue:**
`ExecuteCodeHandler::handle` forwards `input.variables.as_ref()` to the executor,
but `SqlCodeExecutor::execute` names the parameter `_variables` and never reads
it, then calls `self.connector.execute(code, &[])` with an **empty** parameter
slice. So any `variables` an `execute_code` caller supplies are silently dropped.

Two consequences:
1. **Silent contract mismatch.** The `execute_code` tool schema advertises a
   `variables` input (via `CodeModeToolBuilder::build_execute_tool`), so a client
   that passes variables expecting them to bind will get a query executed with
   none — a confusing, hard-to-debug failure (or, worse, a query that runs against
   unsubstituted placeholders).
2. **SQL-shape ambiguity.** Because the full `code` string is executed verbatim,
   any client doing its own string interpolation of `variables` into `code` would
   bypass parameter binding entirely. The token is bound to the code hash, so this
   is not an injection escalation, but it removes the parameterized-binding safety
   the `variables` channel implies.

The doc comment on the struct (lines 376-388) acknowledges the single-method
collapse but does not call out that `variables` is dropped.

**Fix:** Either bind the variables — translate `input.variables` into the
`&[(String, Value)]` slice `SqlConnector::execute` expects (mirroring
`tools::extract_named_params`) so `:name` placeholders resolve — or, if Phase 85
intentionally defers variable binding, reject a non-empty `variables` map with a
clear `ExecutionError` (`"execute_code variables are not yet supported"`) and
document the deferral on the struct, so a caller cannot silently lose data. A
no-op drop of a schema-advertised input is the worst of the three options.

## Info

### IN-01: `SqlCodeExecutor::execute` rebuilds the validation pipeline (and re-reads the env secret) on every call

**File:** `crates/pmcp-server-toolkit/src/code_mode.rs:406-409`
**Issue:** `revalidate` calls `validation_pipeline_from_config(&self.config)` on
every `execute`, which re-runs `build_cm_config`, `resolve_token_secret`
(re-reading the env var), and `ValidationPipeline::from_token_secret`. Functionally
correct, but it re-resolves the HMAC secret from the environment on every
execute_code call. If `CODE_MODE_SECRET` is unset/changed after startup, an
execute that passed token verification (against the startup pipeline) could then
fail re-validation with a confusing "pipeline unavailable" error. (Performance is
out of v1 scope, so this is flagged only for the correctness/consistency angle.)
**Fix:** Construct the `ValidationPipeline` once at executor-build time and store
it in `SqlCodeExecutor` (the executor already owns the full config); reuse it in
`revalidate`. This also guarantees execute-time re-validation uses the same
secret the startup token was signed with.

### IN-02: `ValidateCodeHandler` / `SqlCodeExecutor` use hardcoded placeholder `ValidationContext` strings

**File:** `crates/pmcp-server-toolkit/src/code_mode.rs:264-269` and `:410-415`
**Issue:** Both the validate handler and the executor build a `ValidationContext`
from the literals `"schema-hash"` / `"perms-hash"` (and fixed user/session IDs).
This is documented as the static-policy, no-live-user shape for the pure-config
binary, and the two contexts match each other, so token context-hash binding is
self-consistent. Worth noting only because the strings are deliberately fake — if
a future change makes the validate context and the executor context diverge, the
context-hash binding would break silently. Not a defect today.
**Fix:** Extract the four context strings into a shared `const` (e.g.
`CONFIG_CONTEXT_IDS`) referenced by both sites so they cannot drift apart.

### IN-03: `expand_braced_var` only supports a single bare `${VAR}`, not nested/partial interpolation

**File:** `crates/pmcp-server-toolkit/src/code_mode.rs:548-554`
**Issue:** The helper accepts only an exact `${NAME}` token (strip `${` prefix +
`}` suffix). A `token_secret = "prefix-${VAR}"` or `"${A}${B}"` falls through to
the inline-secret path and is rejected. This is the *correct and intended* scoping
(it preserves the R9 guarantee and avoids accidentally expanding Athena
`output_location` substrings), and it is well-documented — flagged purely so the
deliberate limitation is on record. The research's Open-Q3 note about a "general
env-expansion pass" was intentionally NOT taken, which is the safer choice.
**Fix:** None required. Optionally add a one-line doctest showing
`"prefix-${VAR}"` is rejected, to lock the scoping decision.

### IN-04: `merge_schema_resource` schema-detection matches any URI ending in `/schema`

**File:** `crates/pmcp-sql-server/src/assemble.rs:106` (and `SCHEMA_URI_SUFFIX` at `:68`)
**Issue:** The schema resource is identified by `r.uri.ends_with("/schema")`. If a
config declared two resources whose URIs both end in `/schema` (e.g.
`docs://a/schema` and `docs://b/schema`), BOTH would have their content
overwritten with the `--schema` DDL, and `found_schema` would be set on the first
match so no append happens. The reference config has exactly one, so this is
benign today, but the suffix match is broader than the single-schema assumption
the surrounding logic makes.
**Fix:** Either override only the first match (break after the first `is_schema`)
or document that exactly one `/schema`-suffixed resource is supported. A debug
assertion / warn when more than one schema-suffixed resource is found would catch
a future misconfig.

---

_Reviewed: 2026-05-27T02:14:37Z_
_Reviewer: Claude (gsd-code-reviewer)_
_Depth: standard_
