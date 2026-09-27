---
phase: 128-secure-by-default-input-validation-for-config-driven-servers
plan: 09
subsystem: api
tags: [request-policy, argument-validator, escape-hatch, egress, async-trait, tracing, redirect, ssrf, toolkit-hooks]
status: complete

requires:
  - phase: 128-01
    provides: "`input-validation` feature; `ValidatingToolHandler` at all three handler push sites"
  - phase: 128-02
    provides: "`validate_input`, `render_refusal`, `validate_resolved_path`, `PLACEHOLDER_MAX_LENGTH`"
  - phase: 128-03
    provides: "`[server.validation]`; `ValidatingToolHandler::enforce_schema` separated from the decorator's existence; `tools.rs:200`'s `has_registered_validator = false` seam; `ServerConfig::validation_report()`"
  - phase: 128-05
    provides: "The Code Mode `execute_request` numbered-step structure; the `?`-narrowing"
  - phase: 128-06
    provides: "The curated `substitute_path` / `check_composed_path` surface; `Parameter::placeholder_rules`"
  - phase: 128-08
    provides: "`HttpCodeExecutor::with_schema` / `has_schema`; `narrowed_executor` in `build_server`; `lint_against_spec`'s startup-warn precedent"
provides:
  - "E1: `RequestPolicy` (async, `Send + Sync`) + `OutboundRequest<'a>` + `PolicyRefusal`, hooked before auth on BOTH HTTP surfaces"
  - "E2: `ArgumentValidator` (refuse-only, `&Value`) + `ArgumentRefusal` + `ArgumentValidators`, run strictly AFTER the declared-schema check inside `ValidatingToolHandler`"
  - "`ToolkitHooks` — the parameter-passed registration value with `with_request_policy` / `with_argument_validator`"
  - "`ServerBuilderExt::try_tools_from_config_with` + `try_tools_from_config_with_connector_and_hooks`; the two pre-existing methods became thin wrappers"
  - "Four PUBLIC `*_and_hooks` free synthesizers, so a different crate (`pmcp-openapi-server`) can register hooks"
  - "`render_validation_report` + `emit_validation_report` — ONE formatter, two assembly call sites, reporting what is OFF as well as ON"
  - "`HttpConnector::execute_for_tool` / `has_request_policy` / `governed` — three DEFAULT trait methods"
  - "`HttpConnectorError::PolicyRefused`; `HttpClient::with_request_policy`; `HttpCodeExecutor::with_request_policy` / `with_tool_label` / `tool_label` / `has_request_policy`"
  - "`pmcp-openapi-server`: `build_server` takes a 5th `&ToolkitHooks`; the shared reqwest client is built with `redirect(Policy::none())`"
  - "`pub use async_trait` at the toolkit crate root"
  - "`tests/request_policy.rs` (11 rows, in `REQUIRED_TEST_BINARIES`); `examples/e05_input_validation.rs`"
affects: [128-10, 128-11]

tech-stack:
  added: []
  patterns:
    - "Registration by PARAMETER, never by builder-field accumulation: `ServerBuilderExt` is implemented for core's `pmcp::ServerBuilder`, whose fields are private, and a Rust extension trait cannot add a field to a foreign type"
    - "Additive trait evolution via DEFAULT methods, so an out-of-repo `HttpConnector` impl compiles unchanged"
    - "A public boolean accessor per hook attachment site (`has_request_policy`, mirroring 128-08's `has_schema`), so a registered-but-unreached hook cannot look registered from another crate"
    - "An enforcement log that states what is OFF as well as what is ON, so silence cannot be read as safety"

key-files:
  created:
    - crates/pmcp-server-toolkit/src/policy.rs
    - crates/pmcp-server-toolkit/tests/request_policy.rs
    - crates/pmcp-server-toolkit/examples/e05_input_validation.rs
  modified:
    - crates/pmcp-server-toolkit/src/lib.rs
    - crates/pmcp-server-toolkit/src/tools.rs
    - crates/pmcp-server-toolkit/src/builder_ext.rs
    - crates/pmcp-server-toolkit/src/http/mod.rs
    - crates/pmcp-server-toolkit/src/http/client.rs
    - crates/pmcp-server-toolkit/src/code_mode.rs
    - crates/pmcp-server-toolkit/Cargo.toml
    - crates/pmcp-openapi-server/src/assemble.rs
    - crates/pmcp-openapi-server/src/dispatch.rs
    - crates/pmcp-openapi-server/src/lib.rs
    - crates/pmcp-openapi-server/examples/{contoso_m365_min,london_tube_min,openapi_server_min}.rs
    - Makefile

decisions:
  - "T-128-39a resolved by option (i): the OpenAPI binary's shared reqwest client carries `redirect(reqwest::redirect::Policy::none())`, so a redirect surfaces as a 3xx the caller handles rather than as a hop inside the client that the E1 hook never saw"
  - "The Code Mode step-(4) split WAS structurally possible: the non-auth remaining-body-to-query conversion moved above the hook as step (2a), and only the auth-supplied query additions stay behind it at (3a). `OutboundRequest.query` is populated on BOTH surfaces; the documented-asymmetry fallback was NOT needed"
  - "`OutboundRequest.tool` is populated on both surfaces, which required three DEFAULT methods on `HttpConnector` and a `with_tool_label` builder on `HttpCodeExecutor` — neither was in the plan's artifact list. An always-empty `tool` field would have been the same present-documented-and-blind defect the plan called out for `query`"
  - "`emit_validation_report` takes `(&ServerConfig, &ToolkitHooks)`, not the plan's `(&ServerConfig)`, so the log can state E1/E2 registration state and warn on a validator bound to a tool the config does not declare"
  - "`HttpConnectorError::PolicyRefused` is its own variant rather than reusing `Backend`: a refusal by a security control and a broken backend are different facts"
  - "`ToolkitHooks::argument_validator_for`, not `validator_for` — the short name collided with the root crate's `jsonschema` construction-site tripwire needle"
  - "E2 ships REFUSE-ONLY (`&Value`), so the change request's normalization use is out of scope for this signature; recorded in the trait's rustdoc rather than settled by the shape of a first signature"

metrics:
  duration: "~3h"
  completed: 2026-09-27

actuals:
  tokens: 42841
  tasks: 3
  commits: 9
plan_head_before: 8b76c868b937ee1d6551d90282b24754eabfc527
# `commits: 9` is MEASURED as `git rev-list --count 8b76c868..HEAD` at the tip of this
# plan: 6 code commits (RED/GREEN x2, integration, the tripwire fix) + 3 docs commits
# (the SUMMARY, the metadata commit, and this correction, which was AMENDED into place
# rather than added so the number does not chase itself). Earlier drafts read 6 then 8 —
# each was the count at its own write moment, which is exactly the trap: the figure has
# to be measured at the tip and the write that records it folded into that tip.
---

# Phase 128 Plan 09: E1 + E2 Escape Hatches Summary

A `RequestPolicy` governing what may leave the server, hooked before auth on both HTTP
surfaces so third-party policy code cannot observe a credential, plus a per-tool
`ArgumentValidator` running strictly after the declared-schema check — both registered on a
parameter-passed `ToolkitHooks` value, both reaching the shipped OpenAPI binary, and both
announced in a once-at-startup log that states what is OFF as well as what is ON.

## The final signatures (for plan 11's docs deliverable)

```rust
// E1 — a rule about what may LEAVE the server.
#[non_exhaustive]
#[derive(Debug, Clone, Copy)]
pub struct OutboundRequest<'a> {
    pub tool: &'a str,      // the MCP tool this call came from
    pub method: &'a str,    // upper-cased
    pub path: &'a str,      // FULLY RESOLVED: placeholders substituted, base URL joined, no query
    pub query: &'a [(String, String)],  // EXCLUDING every auth-contributed pair
    pub body: Option<&'a serde_json::Value>,
}
impl<'a> OutboundRequest<'a> {
    pub fn new(tool: &'a str, method: &'a str, path: &'a str,
               query: &'a [(String, String)], body: Option<&'a serde_json::Value>) -> Self;
}

#[non_exhaustive] pub struct PolicyRefusal;      // ::new(impl Into<String>), ::message(), Display, Error
#[async_trait]
pub trait RequestPolicy: Send + Sync {
    async fn check(&self, req: &OutboundRequest<'_>) -> Result<(), PolicyRefusal>;
}

// E2 — a rule about a COMBINATION of values. REFUSE-ONLY.
#[non_exhaustive] pub struct ArgumentRefusal;    // same shape as PolicyRefusal
pub trait ArgumentValidator: Send + Sync {
    fn validate(&self, args: &serde_json::Value) -> Result<(), ArgumentRefusal>;
}

// Registration.
#[derive(Clone, Default)]
pub struct ToolkitHooks;
impl ToolkitHooks {
    pub fn new() -> Self;
    pub fn with_request_policy(self, policy: Arc<dyn RequestPolicy>) -> Self;
    pub fn with_argument_validator(self, tool: impl Into<String>,
                                   validator: Arc<dyn ArgumentValidator>) -> Self;
    pub fn request_policy(&self) -> Option<Arc<dyn RequestPolicy>>;
    pub fn argument_validator_for(&self, tool: &str) -> Option<Arc<dyn ArgumentValidator>>;
    pub fn validator_names(&self) -> Vec<&str>;   // sorted
    pub fn is_empty(&self) -> bool;
}
```

Attachment points, all cheap-clone builders, none of which changed a constructor signature:
`HttpClient::with_request_policy`, `HttpCodeExecutor::with_request_policy`,
`HttpCodeExecutor::with_tool_label`. Observability accessors, public precisely so a
registered-but-unreached hook cannot look registered from another crate:
`HttpConnector::has_request_policy` (a default-`false` trait method that `HttpClient`
overrides), `HttpCodeExecutor::has_request_policy`, `HttpCodeExecutor::tool_label`.

`HttpConnector` gained three DEFAULT methods — `execute_for_tool(tool, operation, args)`
(delegates to `execute`), `has_request_policy() -> false`, and
`governed(policy) -> Option<Arc<dyn HttpConnector>>` returning `None`. Defaults, so an
out-of-repo implementation compiles unchanged.

## The startup enforcement log — exact line formats (plan 11 input)

`render_validation_report(&ServerConfig, &ToolkitHooks) -> Vec<ReportLine>` renders;
`emit_validation_report(&ServerConfig, &ToolkitHooks)` logs, once per server, at
`target: "pmcp_server_toolkit::policy"`. `ReportLine { level: ReportLevel, text: String }`,
`ReportLevel::{Info, Warn}` — `Info` for an enforcement that is ON, `Warn` for one that is OFF
or a registration that cannot take effect. OBSERVED output, verbatim, from
`cargo run --example e05_input_validation`:

```
  [Info] input validation: schema_check=ON default_max_length=256 additional_properties=false strict=false tools=1
  [Info] input validation: tool 'fetch_range' enforces version: Path, required, pattern, maxLength=256 (default); start: Query, required; end: Query, required
  [Info] input validation: no [server.validation] opt-out is active — every rule this config can enforce is enforced.
  [Info] input validation: E1 RequestPolicy registered=true
  [Info] input validation: E2 ArgumentValidator registered for fetch_range
```

The other shapes, each asserted by a test in `tools.rs::enforcement_report`:

| Condition | Level | Text |
|---|---|---|
| schema check off | Warn | `input validation: schema_check=OFF default_max_length=… …` |
| `input-validation` feature off | Warn | `input validation: the \`input-validation\` feature is OFF, so NO tool's arguments are checked against its declared inputSchema. …` |
| a tool declaring no rules | Info | `input validation: tool 'X' enforces (none declared; only the always-on path-placeholder character floor and length cap apply)` |
| each ACTIVE opt-out | Warn | `input validation: [server.validation] opt-out ACTIVE — {lint finding}` |
| no validator registered | Info | `input validation: no E2 ArgumentValidator is registered` |
| validator on an undeclared tool name | Warn | `input validation: an ArgumentValidator is registered for 'X', which this config declares no [[tools]] entry for — it will never run` |
| a policy on a path with no HTTP egress | Warn | `a RequestPolicy is registered but THIS assembly path has no HTTP egress surface to apply it to, so it will never run. …` (`target: pmcp_server_toolkit::builder_ext`) |

Deduplication is a hash of `(server.name, server.version, rendered lines)` in a process-global
set, so a process reaching both assembly paths for one server logs once while a process hosting
two different servers logs for each. Two byte-identical servers in one process log once between
them — stated on the function as an accepted limitation, since nothing in the report would differ.

## Which assembly paths were wired, and which were not

| Path | Hooks | Startup report | Why |
|---|---|---|---|
| `ServerBuilderExt::try_tools_from_config_with` | ✅ E2 | ✅ | The builder path. E1 has no HTTP egress surface here — it WARNS rather than accepting silently. |
| `ServerBuilderExt::try_tools_from_config_with_connector_and_hooks` | ✅ E2 | ✅ | Same, for the SQL-connector variant. |
| `pmcp-openapi-server::build_server` | ✅ E1 + E2 | ✅ | This phase's principal HTTP surface. It calls the free synthesizer directly (`assemble.rs`) and never touches `try_tools_from_config` — T-128-39b / T-128-42a. |
| The four pre-existing free synthesizers | empty default | — | Signatures unchanged; each is now a thin wrapper over its `*_and_hooks` sibling. |
| `run_serving` / `run` (the Shape A binary) | empty default | ✅ via `build_server` | Deliberate: a pure-config deployment has no Rust in which to register a hook. A Rust embedder calls `build_server` directly with its own `ToolkitHooks` — that is the wiring E1/E2 are reachable through, and the three in-repo examples now show the shape. |
| `try_code_mode_from_config*` | not threaded | — | Registers no tools (it is the validation-only connectorless path), so there is nothing for E2 to attach to and no egress for E1. |
| SQL connector traffic | out of reach | — | E1 governs HTTP egress only. A `SqlConnector` request is a statement plus bound parameters, not a method/path/query, so it needs a different seam and a different trait. Stated on `RequestPolicy`'s rustdoc and on `try_tools_from_config_with`'s. |

## Where each hook sits

**E1, curated surface** — `http/client.rs::execute_inner`, after `join_url` (so the policy sees
the URL as it will be sent) and before `self.auth.apply` (the first moment a credential exists in
`headers`/`query`). The query snapshot is SORTED for determinism, since the source is a `HashMap`.
`build_body` was hoisted above the hook so `OutboundRequest.body` is populated.

**E1, Code Mode surface** — `code_mode.rs::execute_request`, between the existing numbered step
(2) `join_url` and step (3) `auth.apply`. **The step-(4) split proved structurally possible**: the
non-auth remaining-body-to-query conversion moved above the hook as a new comment-labelled step
(2a), and the auth-supplied query additions join at (3a) after it. D-12 is preserved exactly —
the credential's query contribution is the part that stays behind the hook. The plan's
documented-asymmetry fallback was NOT needed; `OutboundRequest.query` is populated on both
surfaces.

**E2** — inside `ValidatingToolHandler`, which now calls `check_schema` then
`check_registered_validator`, in that order, from BOTH `handle` and `handle_output`.

**Relative to 128-08's three points in `build_server`:** the policy attachment sits AFTER
`narrowed_executor` (`assemble.rs:355`) and BEFORE both fan-outs — the script fan-out at the free
synthesizer call and the Code Mode fan-out at `code_mode_http_tools_from_executor`. The position
is load-bearing for exactly the reason `with_schema`'s is: `HttpCodeExecutor` is a cheap-clone
value, so a clone taken before the attachment would be permanently ungoverned. The startup report
is emitted at the very top of `build_server`, before any of the three.

## `tools.rs:200`'s binding is gone

128-03 handed over `let has_registered_validator = false;` consumed by
`if !validation.enforce_input_schema && !has_registered_validator`. It is now:

```rust
let registered_validator = hooks.argument_validator_for(&decl.name);
if !validation.enforce_input_schema && registered_validator.is_none() {
```

and `registered_validator` is threaded into `ValidatingToolHandler::wrap`. The
`enforce_schema`-versus-validator separation 128-03 built was used exactly as handed over; no
part of it needed changing.

## Mutation testing

Every enforcement point was mutation-tested: the mutation applied, the suite measured, the file
restored and the restoration confirmed by `shasum` against a pre-mutation copy. Nine mutations,
eight killed immediately and one captured as a deliberate survivor that a later commit killed.

| # | Mutation | Rows turned red |
|---|---|---|
| 1 | curated E1 hook call removed | 4: `http::client::request_policy_seam::{a_refusing_policy_stops_the_request_before_auth_and_before_the_send, the_same_refused_call_twice_is_identical_and_sends_nothing, the_policy_sees_the_resolved_path_and_the_query_pairs, the_policy_is_told_which_tool_the_call_came_from}` |
| 2 | Code Mode E1 hook call removed | 4: `code_mode::request_policy_seam::{a_refusing_policy_stops_the_request_before_auth_and_before_the_send, the_policy_never_sees_the_credential, the_policy_sees_the_remaining_body_query_pairs_on_a_get, the_policy_is_told_which_tool_the_executor_serves}` |
| 3 | policy handed an EMPTY query slice (the pre-split shape) | 1: `code_mode::request_policy_seam::the_policy_sees_the_remaining_body_query_pairs_on_a_get` — the row that fails if the step-(4) split is ever reverted |
| 4 | `redirect(Policy::none())` reverted to `Client::new()` | 1: `dispatch::tests::the_dispatched_client_does_not_follow_a_redirect` |
| 5 | `HttpToolHandler` reverted to `execute()` | **SURVIVOR at the time**: 337 passed, exit 0. See below. |
| 6 | validator consultation removed from `check` | 4: `tools::argument_validator_seam::{a_registered_validator_refuses_a_combination_the_schema_permits, ..._allows_a_valid_combination, ..._still_runs_with_enforce_input_schema_false, handle_output_runs_the_validator_as_well}` |
| 7 | D1/E2 ORDER INVERTED | 1: `tools::argument_validator_seam::a_registered_validator_is_not_invoked_when_the_schema_refuses` (T-128-41) |
| 8 | the schema opt-out drops the decorator (plan 03's first-draft bug) | 1: `tools::argument_validator_seam::a_registered_validator_still_runs_with_enforce_input_schema_false` |
| 9 | `governed_connector` returns the bare connector | 2: `assemble::tests::a_registered_policy_reaches_both_http_surfaces` |

**The survivor and its kill.** Mutation 5 — reverting `HttpToolHandler::handle` from
`execute_for_tool(&self.info.name, …)` back to `execute(…)`, which silently drops the tool
attribution — killed NOTHING in the 337-test unit suite, because no unit test drives a
*synthesized* handler over a policy-carrying connector. It was recorded in the GREEN commit as an
open gap rather than glossed. Task 3's `request_policy_sees_the_tool_name` exists specifically to
close it, and the mutation was re-applied after that row landed: **10 passed, 1 failed**, exactly
`request_policy_sees_the_tool_name`. Restored byte-exact.

## Measured verification

All commands with `RUSTFLAGS=""`. Every filtered command was run, its `running N tests` line read,
and the count checked for SCOPE as well as for being nonzero.

| Command | Before | After |
|---|---|---|
| `--lib policy::` | **0** (module absent — re-measured, not assumed) | **7** |
| `--lib http::client` | 41 | **48** |
| `--lib tools::` | 35 | **45** |
| `--lib builder_ext::` | 4 | **7** — see the correction below |
| `--lib code_mode::` (`openapi-code-mode`) | 33 | **39** |
| `--test request_policy` | did not exist | **11** |
| `--test input_validation_acceptance` | 4 | **4** |
| `make test-server-toolkit` | 399 | **440** |
| `make test-server-toolkit-code-mode` | 436 | **483** |
| `make test-code-mode` | 317 | **317** (unchanged, as expected) |
| `make test-cargo-pmcp` | 1477 | **1477** (unchanged) |
| `make test-openapi-server` | 46 | **49** |
| `cargo test -p pmcp-openapi-server --no-fail-fast` | 46 | **50**, 0 failed |
| `pmcp --lib schema_validation` | 52 | **52** |
| `cargo nextest run --features full --no-fail-fast` | 3358 | **3358 run, 3358 passed, 5 skipped**, exit 0 |
| `cargo doc -p pmcp-server-toolkit` warnings | 37 (recorded baseline) | **36** — did not rise |

`pmat quality-gate --fail-on-violation --checks complexity`: **PASSED, 0 violations** (run three
times across the plan). `make doc-check`: exit 0, zero rustdoc warnings. `make quality-gate`:
exit 0 with the **`ALL TOYOTA WAY QUALITY CHECKS PASSED`** banner present in the captured output
(13 925 lines captured unpiped to a file; the five advisory `✗` sub-check lines — file health, CB-200
TDG, CB-1204, CB-1208, CB-1308 — are pre-existing and non-blocking. Verified not caused here:
`code_mode.rs` and `tools.rs` were ALREADY over 2 000 lines before this plan (2 824 and 2 116), and
`client.rs` went 1 307 → 1 727, crossing no threshold).

`src/server/schema_validation.rs`: **`git diff 8b76c868 HEAD` = 0 lines.** Plan 02 owns it; its 52
tests are the fence, and this plan met its goals with a zero-line diff on it — the fourth
consecutive plan to do so.

### Verify-command corrections (the hazard this phase keeps producing)

1. **`--lib builder_ext::` was nonzero but OUT OF SCOPE.** It selected 4 tests, none of which
   touched `try_tools_from_config_with` or `try_tools_from_config_with_connector_and_hooks`. A
   nonzero count that does not cover the changed code is the worse failure — it reads as coverage.
   Corrected by adding three rows (`try_tools_from_config_with_registers_handlers_and_reaches_the_validator`,
   `try_tools_from_config_is_a_thin_wrapper_over_the_hooks_variant`,
   `a_policy_on_the_sql_path_is_reported_and_not_an_error`), taking it to 7.
2. **`--test input_validation_acceptance` reported exit 1 once — a shell artifact, not a failure.**
   Run inside a zsh `for` loop with an EMPTY filter parameter, the empty word was passed through as
   a test-name filter. Re-measured standalone: `running 4 tests`, 4 passed, exit 0. That standalone
   reading is the one recorded. This is the same class as plan 05's unquoted-`--include` gate.
3. **`gsd_run check tdd-red-evidence` could not verify either RED.** It is a node-TAP parser
   (`parseNodeTestSummary` / `tapFailedTestNames`) and returns
   `INVALID_RED / zero_tests_discovered` on cargo-test output, which is not TAP. `workflow.tdd_mode`
   is absent from `.planning/config.json` and this plan's `type` is `execute`, so the gate is not
   enforced; the REDs are evidenced by the captured cargo output instead (target test failing on a
   planned-behaviour assertion, with passing controls). Recorded in `WINDOWS.md` as an
   `unrun-verify`, because a verifier that cannot read this project's test output is a gap worth
   being visible rather than a thing to work around silently.

Also noted, matching this phase's tooling hazard: `rtk` rewrites `cargo test` output even when
redirected to a file, replacing the `running N tests` / `test result:` pair with its own
`cargo test: N passed` summary. Every count above was taken through `rtk proxy cargo …`, which
preserves the raw lines.

## The runnable example, observed

`cargo run -q -p pmcp-server-toolkit --example e05_input_validation --features input-validation,http`
— exit 0. `grep -c "pmcp_server_toolkit::"` on the file = **1** (the single crate-root import block).
Full output after the startup report quoted earlier:

```
== layer 1: D1, the config-declared schema (a rule about ONE value) ==
  REFUSED by D1: Validation error: /version: must match ^v[0-9]+$

== layer 2: E2, a per-tool ArgumentValidator (a rule about a COMBINATION) ==
  these same values pass the declared schema — two integers, no constraint
  REFUSED by E2: Validation error: `end` must not precede `start`

== layer 3: E1, a RequestPolicy (a rule about what may LEAVE the server) ==
  these values pass BOTH layers above, so the call reaches the outbound hook
      [policy saw] tool="fetch_range" method=GET path=https://backend.invalid/content/v3/records query=[("end", "9"), ("start", "1")] body=None
  policy ALLOWED, then the unreachable backend failed: Internal error: connector error: http request failed: transport error contacting backend

  and with a policy that refuses this endpoint:
      [policy saw] tool="fetch_range" method=GET path=https://backend.invalid/content/v3/records query=[("end", "9"), ("start", "1")] body=None
  REFUSED by E1: Internal error: connector error: outbound request refused by policy: outbound endpoint is not on the allowlist

All three layers refused something only they could express, and no request reached the network.
```

The `[policy saw]` lines are the credential guarantee made visible: the config declares a
`Bearer` token and the policy's view of the request contains no header field at all and no
auth-contributed query pair. The backend URL is deliberately unreachable, so each line above being
a REFUSAL rather than a transport error is itself the proof that the layer ran before dispatch.

## Deviations from Plan

### Auto-fixed issues

**1. [Rule 3 — Blocking] `ToolkitHooks::validator_for` renamed to `argument_validator_for`**
- **Found during:** Task 3, at the root `cargo nextest` sweep.
- **Issue:** `tests/v2_schema_tripwires.rs` scans every workspace source file for the token
  `validator_for(` as the signature of a `jsonschema` validator being CONSTRUCTED, and requires
  each site to declare its dialect policy. My method collided by NAME ONLY. Measured: `3358 run,
  3357 passed, 1 FAILED` — `v2_schema_tripwires_validator_construction_sites_are_accounted_for`,
  naming two UNKNOWN sites (`policy.rs:497` at file scope, `tools.rs:237` in
  `enforce_input_schema`).
- **Fix:** renamed, not allowlisted. An allowlist entry would place a non-dialect site in a
  dialect register, so the next reader would believe a `jsonschema` validator is built there; and
  the needle is a CALL-site token, so every future call would fire the tripwire again — the
  allowlist would grow with the code rather than with the risk. The reason is recorded on the
  method so the short name is not "restored" later.
- **Commit:** `8a62fd7e`. After: tripwire binary 14 passed; full sweep 3358/3358.

**2. [Rule 2 — Missing critical functionality] `emit_validation_report` takes the hooks too**
- The plan specified `emit_validation_report(&ServerConfig)`. It ships as
  `emit_validation_report(&ServerConfig, &ToolkitHooks)`.
- **Why:** the phase prohibition requires that a registered-but-unreached hook cannot look
  registered. Without the hooks in scope the log could not say whether E1/E2 were registered, and
  could not warn that a validator is bound to a tool name the config does not declare. The plan's
  own prohibition outranks its artifact signature.

**3. [Rule 2 — Missing critical functionality] three DEFAULT methods on `HttpConnector`**
- `execute_for_tool`, `has_request_policy`, `governed` — none in the plan's artifact list.
- **Why:** the plan's first must-have says the policy "sees the tool name", and neither surface
  could supply one. `Operation` describes an endpoint, not a tool, and has 41 struct-literal
  construction sites across the workspace plus `Serialize`/`Deserialize`, so adding a field to it
  was out of scope. Default trait methods are additive and let the synthesized handler pass the
  name while an out-of-repo `HttpConnector` compiles unchanged. `governed` exists because
  `build_server` receives the connector already erased to `Arc<dyn HttpConnector>` and would
  otherwise have no route to attach a policy at all; `has_request_policy` exists so the attachment
  is PROVABLE from another crate. An always-empty `tool` field would have been the same
  present-documented-and-blind defect the plan itself called out for `query`.

**4. [Rule 2 — Missing critical functionality] `HttpCodeExecutor::with_tool_label` / `tool_label`**
- The Code Mode counterpart of deviation 3. `HttpExecutor::execute_request` is a
  `pmcp-code-mode` trait method carrying no tool name, so the label is attached where a PER-TOOL
  executor is minted: `ScriptToolHandler::new` (a script tool's own `[[tools]]` name) and
  `code_mode_http_tools_from_executor` (`execute_code`).

**5. [Rule 2 — Missing critical functionality] `HttpConnectorError::PolicyRefused`**
- A dedicated variant rather than reusing `Backend`. A refusal by a security control and a broken
  backend are different facts, and an operator tracing a rule that started refusing calls needs
  to tell them apart. The enum is `#[non_exhaustive]`, so the addition is additive. The RED row's
  expected string was updated to match this refinement, noted in the GREEN commit body.

**6. [Rule 3 — Blocking] `pub use async_trait` at the toolkit crate root**
- `pmcp-openapi-server` has no `async-trait` dependency, so its `RequestPolicy` test impl could
  not write `#[async_trait]`. Re-exporting also removes the version-skew failure mode, where an
  implementor on a different `async-trait` version gets a signature mismatch that reads as a
  lifetime bug.

**7. [Rule 3 — Blocking] seven added `lib.rs` re-exports**
- `HttpClient`, `create_auth_provider`, `AuthConfig`, and the four `*_and_hooks` synthesizers.
  Required by the plan's own binding constraint that a toolkit example's imports are ONE
  crate-root block, and by `tests/request_policy.rs` being an external consumer. This is D-15
  working as intended: the fix for an un-importable type is the re-export, never a qualified path
  in the example.

**8. [Rule 3 — Blocking] `render_validation_report` made public**
- Not in the plan's artifact list. Splitting rendering from emission lets the line FORMATS be
  asserted without installing a `tracing` subscriber, and gives plan 11 one authority for what an
  operator sees. The example prints through it.

No architectural decisions were required, so no Rule 4 checkpoint was raised. None of the three
operator decisions recorded for this phase (`publish-as-specified`, `breaking-newtype`,
`Narrow the '?' rule only`) was invoked or reinterpreted here.

## Inherited narrowing: honoured

The operator's `Narrow the '?' rule only` decision is untouched. `check_composed_path` and
`ResolvedPath::from_checked` were not modified, no test name or comment added here suggests that
authors must migrate query values to body params, and the example's `[[tools]]` `path` carries no
`?` either way. `OutboundRequest.query` carries the pairs the toolkit itself will append — it is a
different mechanism from an author-written query string and does not interact with the narrowing.

## Flagged assumptions — status

- **E1 · unclassified (what "every outbound request" obliges).** Partly closed. The retry half is
  now a STATED contract rather than an open question: `RequestPolicy::check`'s rustdoc names
  `send_with_retries` and says the seam gives one invocation per LOGICAL outbound request, so a
  policy counting for a rate budget under-counts wire attempts. The redirect half is closed by
  construction (deviation-free, resolution (i)). What genuinely remains open is a Code Mode
  script's 50th call within one `tools/call`: each call gets its own invocation, all carrying the
  same tool label, and whether that is the right granularity for a per-request budget belongs to
  the deferred time-budget phase.
- **E2 · unclassified (what a validator may DO besides refuse).** NOT closed, and deliberately so.
  The shipped signature is refuse-only and its rustdoc says in as many words that mutation and the
  change request's normalization use are out of scope for it, and why: a mutating validator has a
  different contract (idempotency, what the published schema then describes, whether the schema
  check should re-run on the rewritten value). Recorded rather than settled by the shape of a first
  signature.

## Handoffs

**To plan 128-11 (version and pin moves — D-14, ONE commit).** This plan adds public API to two
crates and moves no version and no pin, as instructed. The obligations:

- `pmcp-server-toolkit` — **MINOR** bump. New public module `policy`; seven new public types; three
  new DEFAULT methods on the public `HttpConnector` trait; two new `ServerBuilderExt` methods; four
  new public free functions; a new `#[non_exhaustive]` `HttpConnectorError` variant; new builders
  on `HttpClient` and `HttpCodeExecutor`; a new `async_trait` re-export. All additive — no
  pre-existing signature changed.
- `pmcp-openapi-server` — **BREAKING** on a `0.x` line, so also a **MINOR** bump. `build_server`
  grew a fifth parameter (`hooks: &ToolkitHooks`). Every in-repo caller was updated in this plan;
  an out-of-repo caller will not compile. If `pmcp-openapi-server`'s published `0.1.x` has
  consumers, that is a hard break to call out in the release notes rather than a silent widening.
  Its `pmcp-server-toolkit` pin may need to move with the toolkit bump; the crate publishes AFTER
  the toolkit (CLAUDE.md item 5 vs item 9b), so the ordering is already correct.

**To plan 128-11 (docs).** The signature block and the startup-log format table above are written
to be lifted directly. Carry the E1 boundary statement with them: E1 governs the two HTTP egress
surfaces and does NOT intercept SQL connector traffic, and a redirect is refused rather than
followed on the OpenAPI binary's client.

**To plan 128-10.** `ToolkitHooks` is the registration surface; `render_validation_report` is the
observable form of what a server enforces, usable from a test without a subscriber.

## Known stubs

None. No stub, placeholder, `TODO`, `FIXME`, skipped test or `#[ignore]` was added by this plan.
Two deliberate NON-stubs worth naming so a reader does not mistake them for one: `HttpConnector`'s
three default method bodies (a delegation, a `false`, and a `None`) are the additive-evolution
contract for out-of-repo implementors and each says so in its rustdoc; and `ArgumentValidator`
being refuse-only is a recorded scope decision with its reasoning, not an unfinished signature.

## Threat flags

None. No file changed here introduces security-relevant surface outside the plan's
`<threat_model>`. The one surface that could have looked new — `dispatch.rs`'s reqwest client — was
already in the register as T-128-39a and is now mitigated rather than flagged.

All eleven threat rows are addressed: T-128-39 (mitigated; asserted by
`request_policy_never_sees_credential` with a positive control), T-128-40 (accepted; documented on
both refusal types), T-128-41 (mitigated; mutation 7), T-128-42 (mitigated; the report is built
from declarations and a test asserts it echoes no credential token), T-128-43 (mitigated; the
per-tool enforced-rules list ships), T-128-44 (accepted; documented on the trait), T-128-39a
(mitigated by `Policy::none()`; mutation 4), T-128-39b (mitigated; mutation 9), T-128-42a
(mitigated; one formatter, two call sites), T-128-39c (mitigated by the step-(4) split; mutation 3).

## Self-Check: PASSED

All three created artifacts exist on disk (`src/policy.rs`, `tests/request_policy.rs`,
`examples/e05_input_validation.rs`) and all seven commits resolve in `git log --all`:
`4d12d66f` (RED E1), `1117c4b6` (GREEN E1), `c42c669e` (RED E2), `90302036` (GREEN E2),
`dc22ec48` (integration + example), `8a62fd7e` (tripwire rename), `082b1f07` (this SUMMARY).
