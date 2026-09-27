# Phase 128: Secure-by-default input validation for config-driven servers - Pattern Map

**Mapped:** 2026-09-26
**Files analyzed:** 27 (9 new, 18 modified)
**Analogs found:** 24 / 27
**Tree:** `15a52795` (branch `fix/oauth-discovery-optional-fields`)

> **How to read this.** RESEARCH.md already quotes most *target* code verbatim (the code this phase
> CHANGES). This document supplies the missing half: for each file, the closest existing in-tree
> **analog** — a file that already solves the same shape — with excerpts to copy. Where RESEARCH.md
> already carries the verbatim excerpt, this document points at it by Finding number instead of
> re-pasting it.
>
> **Every path below was verified tracked** (`git ls-files`). No `.gsd/` mirror paths appear.
>
> ⚠ **One RESEARCH.md correction is recorded in § Corrections — read it before planning D-03(b).**

## File Classification

### New files

| New file | Role | Data Flow | Closest Analog | Match |
|---|---|---|---|---|
| `src/server/schema_validation.rs` | service (validator) | request-response (pre-dispatch guard) | `src/server/output_validation.rs` | exact |
| `crates/pmcp-server-toolkit/tests/input_validation_acceptance.rs` | test (integration) | request-response + mock I/O | `crates/pmcp-server-toolkit/tests/http_executor.rs` | exact |
| `crates/pmcp-server-toolkit/tests/curated_path_injection.rs` | test (integration) | request-response + mock I/O | `crates/pmcp-server-toolkit/tests/http_auth.rs` | exact (same gate: `http` only) |
| `crates/pmcp-server-toolkit/tests/request_policy.rs` | test (integration) | event-driven (hook) | `crates/pmcp-server-toolkit/tests/http_auth.rs` | role-match |
| `crates/pmcp-server-toolkit/tests/path_placeholder_props.rs` | test (property) | transform | `crates/pmcp-server-toolkit/tests/http_connector_props.rs` + `tests/log_emitter.rs:160-170` | split analog (see §P4) |
| `tests/typed_tool_garde.rs` (root) | test (integration) | transform | `tests/log_emitter.rs` (marker convention) + `src/server/typed_tool.rs` tests | role-match |
| `fuzz/fuzz_targets/fuzz_input_schema_enforcement.rs` | test (fuzz) | transform | `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs` | exact |
| `examples/s57_*.rs` | example | request-response | `examples/s16_typed_tools.rs` / `s20_typed_tool_v2.rs`; stanza from `Cargo.toml:714-718` | role-match |
| `crates/pmcp-server-toolkit/examples/e05_input_validation.rs` | example | request-response | `crates/pmcp-server-toolkit/examples/e01_toolkit_minimal.rs` | exact |

### Modified files

| Modified file | Role | Data Flow | Analog for the change | Match |
|---|---|---|---|---|
| `src/server/output_validation.rs` (18 cfg renames, widen helpers) | service | — | itself (mechanical) | n/a |
| `src/server/mod.rs` (`pub mod schema_validation;`, deprecate `validation`) | module decl | — | `src/server/mod.rs:83-88` (the `output_validation` cfg pair) | exact |
| `src/server/validation.rs` (deprecate + `doc(hidden)`; harvest `validate_safe_path`) | utility | transform | its own `ValidationError::elicit` shape (`:23-40`) | exact |
| `src/server/typed_tool.rs` (E3) | service | transform | RESEARCH Finding 4c shape A | role-match |
| `crates/pmcp-code-mode/src/executor.rs` (D-09 trait + 2 error wraps) | service (dispatch) | request-response | RESEARCH Finding 3c/3d verbatim | exact |
| `crates/pmcp-server-toolkit/src/code_mode.rs` (drop step 1, add E1 between 2 and 3) | service | request-response | RESEARCH Finding 3e verbatim | exact |
| `crates/pmcp-server-toolkit/src/http/client.rs` (D4 at `substitute_path`/`render_scalar`) | service | request-response | `render_scalar` `:354-372` (the SC-7 convention) | exact |
| `crates/pmcp-server-toolkit/src/tools.rs` (D1 call, D2 keywords, SC-6 comments) | service (synthesizer) | transform | `build_param_property` `:216-243` + its two asserting tests | exact |
| `crates/pmcp-server-toolkit/src/config.rs` (D2 fields, `[server.validation]`, `lint()`) | model/config | transform | the 15 `validate_*` unit tests `:1095-1371` | exact |
| `crates/pmcp-server-toolkit/src/error.rs` (new `ConfigValidationError` variants) | model | — | `error.rs:145-169` (thiserror enum) | exact |
| `cargo-pmcp/src/commands/validate.rs` (SC-3 half) | CLI command | file-I/O | `validate_deploy` `:584-607` | role-match |
| `Cargo.toml` (`schema-validation`, 2.21.0) | config | — | `Cargo.toml:216-220` comment discipline | exact |
| `crates/pmcp-server-toolkit/Cargo.toml` (+ 7 consumer manifests) | config | — | RESEARCH Finding 7b pin table | exact |
| `Makefile` (`:587`, `:596`, new `test-code-mode` leg) | build config | — | `Makefile:584-616` `test-server-toolkit` | exact |
| `.planning/codebase/ARCHITECTURE.md:199-200` | docs | — | — | no analog needed |
| `CHANGELOG.md` (D-15 rollout note) | docs | — | existing entries | n/a |

## Pattern Assignments

---

### `src/server/schema_validation.rs` (NEW — service, pre-dispatch guard)

**Analog:** `src/server/output_validation.rs` (2155 lines) — read it at `:553-650` only. Do **not**
read the whole file; the rest is tests and the `fuzz_support` seam.

**Module-declaration pattern to copy** — `src/server/mod.rs:83-88`:
```rust
#[cfg(not(feature = "fuzzing"))]
pub(crate) mod output_validation;
/// Warn-only emit-time validation of `structuredContent` against a declared
/// `outputSchema` (no-op unless the `validation` feature is enabled).
#[cfg(feature = "fuzzing")]
pub mod output_validation;
```
New module is unconditionally `pub` (it ships `validate_input`), but gated
`#[cfg(not(target_arch = "wasm32"))]` the way `pub mod validation;` is at `:204-206`:
```rust
/// Validation helpers for typed tools.
#[cfg(not(target_arch = "wasm32"))]
pub mod validation;
```

**Compile pattern to CALL (not copy)** — `output_validation.rs:574-592` `compile_2020_12`, reached
through the three-function split documented at `:598-605`. RESEARCH Finding 2a quotes both verbatim.
Planner rule from that Finding: `validate_input` is a **fourth sibling**, never a fifth branch inside
the existing three.

**Cache pattern** — `output_validation.rs:631-650` `cached_validator`, keyed `(Era, String)`.
RESEARCH Finding 1i measures the key at 540 ns vs a 10.4 µs compile; Q6 is open on whether to reuse
it or use a per-handler `OnceLock<Arc<Validator>>`.

**Error-rendering pattern — the ANTI-analog.** `output_validation.rs:128-131`:
```rust
.map(|e| format!("{} (at {})", e, e.instance_path()))
```
**Do not copy this line.** Copy instead the `ValidationErrorKind` matcher in RESEARCH.md
§ Architecture Patterns → Pattern 1 (measured against jsonschema 0.49.2).

**Value-free error-shape analog that IS safe to copy** — `src/server/validation.rs:23-40`
`ValidationError::elicit`, which builds `format!("Validation failed for field '{}'", &field_str)`
plus a machine-readable `{code, field, expected, elicit}` object. It names the *expected*, never the
rejected value — the same discipline SC-7 locks. This is the in-core precedent for `InputViolation`'s
shape.

---

### `crates/pmcp-server-toolkit/src/config.rs` (D2 `ParamDecl` + `lint()`)

**Analog for the new fields:** the struct itself at `:913-949` (RESEARCH Finding 10d, verbatim).

**Analog for the config-validation tests — 15 of them, all one shape.** `config.rs:1129-1148`:
```rust
    fn validate_rejects_empty_tool_name() {
        let toml = r#"
            [server]
            name = "demo"
            version = "0.1.0"

            [[tools]]
            name = "ok"
            description = "first"

            [[tools]]
            name = ""
            description = "second-is-empty"
        "#;
        let cfg = ServerConfig::from_toml(toml).expect("parse");
        match cfg.validate() {
            Err(ConfigValidationError::EmptyToolName(1)) => {},
            other => panic!("expected EmptyToolName(1), got {other:?}"),
        }
    }
```
Copy this shape verbatim for `validate_rejects_uncompilable_pattern` and every D2 round-trip test:
inline TOML literal → `from_toml(...).expect("parse")` → `match cfg.validate()` on the **exact**
error variant → `other => panic!` with the variant name. The full roster is at `:1095, 1101, 1115,
1129, 1151, 1174, 1195, 1216, 1236, 1256, 1278, 1304, 1331, 1353, 1371`.

**Analog for the new error variants** — `crates/pmcp-server-toolkit/src/error.rs:145-169`:
```rust
pub enum ConfigValidationError {
    /// `[server] name` is missing or whitespace-only.
    #[error("server.name must be non-empty")]
    EmptyServerName,
    …
    /// `[[tools]]` entry at `index` has an empty / whitespace-only `name`.
    #[error("[[tools]] entry at index {0} has empty name")]
    EmptyToolName(usize),
    …
    /// Per Phase 83 Plan 06 review R9: `[code_mode].token_secret` was given as
    /// an inline literal … Inline literals in committed configs leak HMAC
    /// signing keys; the toolkit defaults to rejecting them.
    #[error(
        "[code_mode].token_secret is an inline literal; use 'env:VAR_NAME' \
         or set allow_inline_token_secret_for_dev=true (NEVER in production)"
    )]
    InlineSecretRejected,
```
`thiserror`; every variant carries a `///` doc naming the config key, and a security-motivated
variant carries the *why* in the doc. `UncompilableParamSchema { tool, position, detail }` follows
this exactly. Note `detail` is author-facing at config time, so echoing `e.to_string()` there is
safe — the SC-7 no-echo rule is about the *client-facing* call path only.

**`lint()` has no analog** — `ServerConfig::validate` is first-error-wins with no warning channel
(RESEARCH Finding 11a). **No in-tree analog for a warning-returning config check exists in this
crate.** The closest *conceptual* precedent is `cargo-pmcp`'s `crate::deployment::iam::validate`,
which returns warnings (`cargo-pmcp/src/commands/validate.rs:584-607` calls it as
`let warnings = crate::deployment::iam::validate(&config.iam)`). Different crate, different document
— borrow the `-> Vec<Warning>` signature idea, not the code.

---

### `crates/pmcp-server-toolkit/src/tools.rs` (D1 wiring, D2 emission, SC-6)

**Target code:** RESEARCH Finding 10b (`build_input_schema` `:198-208`), 10c (`build_param_property`
`:216-243`), 10a (the three false claims verbatim), 10e (the six `synthesize*` entry points).

**Analog for the schema-assertion tests D2 must extend** — two already exist in the same file's
test module, and they are the template for asserting new keywords:
```rust
// crates/pmcp-server-toolkit/src/tools.rs ~:745-751
        let schema = &info.input_schema;
        assert_eq!(schema["type"], Value::String("object".to_string()));
        assert_eq!(schema["properties"], serde_json::json!({}));
        assert_eq!(schema["required"], serde_json::json!([]));
        assert_eq!(schema["additionalProperties"], Value::Bool(false));
```
```rust
// …and ~:1033-1036, the http-synthesis twin
        let schema = &info.input_schema;
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["required"], json!(["id"]));
        assert_eq!(schema["additionalProperties"], Value::Bool(false));
```
Index-into-`input_schema`-and-`assert_eq!`-against-a-`json!` is the convention. The D2 object-form
`items` assertion (`schema["properties"]["tags"]["items"], json!({"type":"string"})`) goes here —
array-form `items` fails the 2020-12 pin (RESEARCH Finding 1h).

**SC-6 comment-rewrite analog:** claims 1–3 become TRUE after this phase, so the edit restates and
points at the enforcer. The in-tree model for "a comment that names its enforcing function" is
`crates/pmcp-server-toolkit/src/code_mode.rs`'s numbered step comments (Finding 3e), e.g.
`// (1) Path-param substitution from the body object. A non-scalar {key} value is rejected (WR-03)`.
Per RESEARCH's threat-model discipline note, each rewritten `T-*` must also gain an acceptance row
that fails when the enforcement is removed.

---

### `crates/pmcp-server-toolkit/src/http/client.rs` (D4, curated surface)

**Analog is in the same file, 200 lines down:** `render_scalar` at `:354-372` (RESEARCH Finding 6
quotes it verbatim). It is the in-tree statement of the SC-7 convention:
```rust
        // Object OR Array: non-scalar in a path/query/header position is rejected
        // rather than silently JSON-stringified. Name the param ONLY (Pitfall 5).
        serde_json::Value::Object(_) | serde_json::Value::Array(_) => {
            Err(HttpConnectorError::Backend(format!(
                "param '{param_name}' must be a scalar (non-scalar values are \
                 not supported in path/query/header position)"
            )))
        },
```
The D4 refusal goes in this function's neighbourhood and uses **this wording style**: name the
declared param, state the declared expectation, never the value. Its twin on the Code Mode surface is
`code_mode.rs::scalar_str` (Finding 3e) whose doc comment says
`/// Per Pitfall 5 the message names the KEY only — never the value.` Keep the two parallel — they
differ only in error type (`HttpConnectorError::Backend` vs `ExecutionError::RuntimeError`).

**Position oracle:** `operation.path_parameters()` already drives `substitute_path`'s loop
(`:143-163`), so D-05's path/query/body position needs no new config surface.

---

### `crates/pmcp-code-mode/src/executor.rs` (D-09)

**No analog needed — RESEARCH Finding 3 is the complete map** (trait at `:2421-2432`, four
implementors, two library call sites at `:2843`/`:3016`, the two error wraps at `:2846`/`:3018`, and
the layer-1 vs layer-2 resolution split). The one pattern to preserve is the free-helper-for-cog-25
idiom, stated in `code_mode.rs::resolve_path`'s own doc:
`/// (kept a free helper so the trait method stays under the cog ≤25 budget)`.

---

### `crates/pmcp-server-toolkit/tests/input_validation_acceptance.rs` (NEW — integration)

**Analog:** `crates/pmcp-server-toolkit/tests/http_executor.rs`, which RESEARCH Finding 12c quotes
(header at `:11-14`, body at `:27-44`). Copy three things verbatim from it:

1. **The file-level gate line**, adapted: `#![cfg(all(feature = "http", feature = "input-validation"))]`
   (http_executor.rs:16 is `#![cfg(feature = "openapi-code-mode")]`).
2. **The header block recording the run incantation and the fn-naming rule** (`:11-14`) — the test
   fns are prefixed with the binary name so the positional verify filter resolves.
3. **The wiremock arrange block** — `MockServer::start().await` → `Mock::given(method("GET"))
   .and(path(...)).respond_with(...).mount(&server).await`.

**Zero-upstream-request idiom — use idiom 2**, from
`crates/pmcp-server-toolkit/tests/script_tool_engine_parity.rs:85-87`:
```rust
        .received_requests()
        .await
        .expect("wiremock records requests")
```
RESEARCH Finding 12b prefers it over `.expect(0)` (which only fires on `MockServer` drop, making a
failure hard to attribute to a matrix row). Both idioms exist in-tree; this one lets one test body
prove zero-requests *and* the no-echo assertions together. The full assembled row is in RESEARCH.md
§ Code Examples → "Zero-upstream-request acceptance row".

---

### `crates/pmcp-server-toolkit/tests/curated_path_injection.rs` (NEW — integration, NO JS engine)

**Analog:** `crates/pmcp-server-toolkit/tests/http_auth.rs:1-16` — because it is gated on `http`
alone, which is exactly the build SC-1 is about:
```rust
//! Integration coverage for the outbound HTTP auth providers (OAPI-03 / D-05 / H1).
//!
//! Named `http_auth` so the plan's `cargo test ... http_auth` verify filter
//! resolves to a dedicated test binary. The exhaustive per-variant unit tests
//! live in `src/http/auth.rs`; these assert the public crate-root-reachable
//! surface (`http::auth::*`) behaves end-to-end through the published API.

#![cfg(feature = "http")]
```
Copy the header's three-part structure (what/why-this-name/where-the-unit-tests-live) and the
single-feature gate. ⚠ This binary must **not** require `openapi-code-mode`, or SC-1's
"the two rows that need no JS engine" is untested on the build it is about.

---

### `crates/pmcp-server-toolkit/tests/path_placeholder_props.rs` (NEW — property)

**Split analog — neither half alone is sufficient:**

- **proptest body shape:** `crates/pmcp-server-toolkit/tests/http_connector_props.rs` (existing
  toolkit proptest binary; also `tool_synthesis_props.rs`, `http_auth.rs`).
- **Gate-visibility marker:** `tests/log_emitter.rs:160-170`, verbatim —
```rust
/// Property arm for `make test-property` (`cargo test --features "full" --
/// --ignored property_`).
///
/// The exhaustive fence above is the load-bearing half; this arm exists so the
/// CLAUDE.md ALWAYS-property requirement is discharged by a test the
/// `validate-always` target actually SELECTS, rather than by an always-run test
/// that target never runs.
#[test]
#[ignore = "property arm — selected by `make test-property` (--ignored property_)"]
fn property_log_level_ordering_is_a_total_order_over_declaration_index() {
    use proptest::prelude::*;

    proptest!(|(a in 0usize..8, b in 0usize..8, c in 0usize..8)| {
```
⚠ **Caveat the planner must not miss:** `make test-property` runs
`cargo test --features "full" -- --ignored property_` (`Makefile:797-800`) — a **root-package**
selector. A *toolkit* property test carrying this marker is still never reached by that leg; it lands
in `make test-server-toolkit`. So the marker is necessary for root-crate arms and merely
convention-preserving for toolkit arms. Put at least one SC-7 property arm in **root** `tests/` if
the ALWAYS-property leg is to select anything for this phase.

---

### `fuzz/fuzz_targets/fuzz_input_schema_enforcement.rs` (NEW — fuzz)

**Analog:** `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs` — same subsystem, same seam question. Copy
its **header discipline** (`:1-45`), which is the most valuable part:

```rust
//! Fuzz target for the era-branched `outputSchema` validation path.
//!
//! CLAUDE.md ALWAYS / FUZZ Testing:
//!
//! ```bash
//! cd fuzz && cargo +nightly fuzz run fuzz_schema_draft_pin
//! ```
//!
//! **`+nightly` is REQUIRED and is not a style choice.** `cargo fuzz` passes
//! `-Zsanitizer=address`, which stable rustc refuses … The repo's
//! `make test-fuzz` invokes the PLAIN form … and pipes every non-zero exit into
//! `|| echo "… completed"`, so on a stable default toolchain that target reports
//! success having fuzzed NOTHING. Do not cite `make test-fuzz` as evidence for
//! this target; run the `+nightly` command above …
//!
//! # What is being fuzzed, and why
//! …
//! # Input layout
//! … Splitting raw bytes at an arbitrary point makes BOTH halves fail to parse as
//! JSON on nearly every iteration … A length prefix fixes that, and — being
//! writable by hand — is what makes a committed seed corpus possible:
//!
//! ```text
//! byte 0                  : case selector — 0 = RAW family, non-zero = JSON family
//! bytes 1..5              : u32 little-endian schema_len
//! bytes 5..5+schema_len   : schema bytes
//! bytes 5+schema_len..    : instance bytes (the remainder)
```
The **length-prefixed two-JSON-document layout** is directly reusable: this phase's target also needs
a `(schema, arguments)` pair, and the same degeneracy argument applies.

**Registration stanza** — `fuzz/Cargo.toml:267-271`:
```toml
name = "fuzz_schema_draft_pin"
path = "fuzz_targets/fuzz_schema_draft_pin.rs"
test = false
doc = false
bench = false
```
Root fuzz features are at `fuzz/Cargo.toml:60` with a load-bearing comment at `:41-47` explaining
that `fuzzing` widens the MODULE and `validation` supplies its CONTENTS. **After the D-04 split that
comment needs updating** — `schema-validation` is what supplies the contents now. Note
`validate_input` is already `pub`, so the new target needs **no** `fuzz_support` seam.

---

### `examples/s57_*.rs` and `crates/pmcp-server-toolkit/examples/e05_*.rs` (NEW)

**Root-example analogs:** `examples/s16_typed_tools.rs`, `s20_typed_tool_v2.rs`,
`s34_typed_tools_workflow.rs` — the typed-tool family. ⚠ **`s19_wasm_typed_tools.rs` is NOT an
analog** for a native example: its whole body sits inside `#[cfg(target_arch = "wasm32")] mod
wasm_example` with a stub `main()` for native builds. See § Corrections.

**Stanza analog** — `Cargo.toml:714-718`:
```toml
# taken by `s45_tool_as_task_lifecycle` below, hence `s56`.
[[example]]
name = "s56_workflow_skill_projection"
path = "examples/s56_workflow_skill_projection.rs"
required-features = ["skills", "full"]
```
Next free slot is `s57_` (RESEARCH Finding 9c).

**Toolkit-example analog** — `crates/pmcp-server-toolkit/examples/e01_toolkit_minimal.rs:1-22`:
```rust
//! Toolkit minimal example — demonstrates Shape C ≤15-line `main.rs` usage.
//!
//! Build with:
//! ```sh
//! cargo run -p pmcp-server-toolkit --example e01_toolkit_minimal --features code-mode
//! ```
//!
//! Per CLAUDE.md ALWAYS requirements + Phase 83 review R3:
//! the imports below are ONE block, crate-root only. NO module-path
//! qualification (`pmcp_server_toolkit::auth::*` etc.). If those need to be
//! added, the D-15 headline DX promise is broken — fix the missing
//! re-export in `lib.rs`, never qualify the import here.

use std::sync::Arc;

use pmcp::Server;
use pmcp_server_toolkit::{
    ServerBuilderExt, ServerConfig, StaticAuthProvider, StaticResourceHandler,
};
```
**Binding constraint to carry forward:** a toolkit example's imports must be a single crate-root
block. If E1's `RequestPolicy` or E2's `ArgumentValidator` cannot be imported from
`pmcp_server_toolkit::{…}` directly, the fix is a `lib.rs` re-export — never a qualified path in the
example. The `[[example]]` stanza shape is `crates/pmcp-server-toolkit/Cargo.toml:150-152` with
`required-features` naming toolkit features only.

---

### `Makefile` (`:587`, `:596`, new `test-code-mode` leg)

**Analog is the target being edited** — `Makefile:584-616`, the `test-server-toolkit` leg. It is the
in-tree reference implementation of "prove a nonzero test count":
```make
test-server-toolkit:
	@out=$$(… $(CARGO) test -p pmcp-server-toolkit --features http -- --test-threads=1 2>&1); \
	status=$$?; echo "$$out"; if [ $$status -ne 0 ]; then exit $$status; fi; \
	ran=$$(echo "$$out" | awk '/^test result:/ { total += $$4 } END { print total+0 }'); \
	if [ "$$ran" -eq 0 ]; then \
		echo "$(RED)✗ pmcp-server-toolkit reported 0 tests — the gate is not reaching this crate$(NC)"; \
		exit 1; \
	fi; \
	REQUIRED_TEST_BINARIES="env_ref_grammar_parity base_url_expansion"; \
	for b in $$REQUIRED_TEST_BINARIES; do \
		n=$$(printf '%s\n' "$$out" | awk -v want="tests/$$b.rs" -f scripts/named-test-binary-count.awk); \
		case "$$n" in \
		-1) echo "… never RAN — cargo printed no 'Running tests/$$b.rs' target line."; exit 1;; \
		-2) echo "… printed a target line but NO 'test result:' line followed it."; exit 1;; \
		 0) echo "… RAN but passed ZERO tests — a #[cfg] gate turned false …"; exit 1;; \
		''|*[!0-9]*) echo "… the count extractor produced no usable reading ('$$n')."; exit 1;; \
		 *) echo "$(GREEN)  ✓ $$b passed $$n tests$(NC)";; \
		esac; \
	done; \
	echo "$(GREEN)✓ pmcp-server-toolkit tests passed ($$ran tests)$(NC)"
```
Two edits and one new target:
- `:587` — `--features http` → `--features http,input-validation` (RESEARCH Pitfall 2).
- `:596` — `REQUIRED_TEST_BINARIES` gains `input_validation_acceptance curated_path_injection
  request_policy path_placeholder_props`.
- A new `test-code-mode` leg: **copy this entire target verbatim**, swapping `-p pmcp-server-toolkit
  --features http` for `-p pmcp-code-mode`, and chain it into `test-all` (`Makefile:1481`). Today
  `pmcp-code-mode` has no `quality-gate` leg at all, so D-09's tests would be gate-invisible.

---

## Shared Patterns

### SP-1 — Value-free refusal (SC-7)
**Sources:** `src/server/validation.rs:23-40` (`ValidationError::elicit` — declared `expected`, never
the value); `crates/pmcp-server-toolkit/src/http/client.rs:354-372` and
`crates/pmcp-server-toolkit/src/code_mode.rs::scalar_str` (both carry
`// Name the param ONLY (Pitfall 5)`).
**Anti-source:** `src/server/output_validation.rs:128-131`.
**Apply to:** every D1/D4/E1/E2 refusal, on both HTTP surfaces and in core.

### SP-2 — Nonzero-test-count proof for any feature-gated test file
**Source:** `Makefile:584-616` (quoted above), plus the recorded measurement at `Makefile:576-583`.
**Apply to:** all five new test binaries. A `#![cfg(feature = …)]` file the gate's feature list does
not enable compiles to `running 0 tests` and exits **0**.

### SP-3 — The `#[ignore = "property arm …"]` marker
**Source:** `tests/log_emitter.rs:169` (and `:2224`).
**Apply to:** every new `property_*` fn in **root** `tests/`. Toolkit property arms are reached by
`make test-server-toolkit` instead — carry the marker for consistency but do not rely on
`make test-property` to select them.

### SP-4 — Cognitive-complexity discipline (cog ≤ 25, PMAT 3.15.0, CI-gated)
**Sources:** `output_validation.rs:598-605` ("Keep this function separate — do not inline it back");
`code_mode.rs::resolve_path`'s `/// (kept a free helper so the trait method stays under the cog ≤25
budget)`.
**Apply to:** `schema_validation.rs`, `build_param_property` (already 7 `if let` arms; D2 adds 5 —
RESEARCH assumption A4 is unmeasured, so re-run `pmat analyze complexity` after the edit),
`HttpCodeExecutor::execute_request`, `ServerConfig::lint`.

### SP-5 — Load-bearing manifest comments
**Source:** `Cargo.toml:216-220`:
```toml
# `optional` and `default-features = false` are LOAD-BEARING and must survive verbatim:
# jsonschema's defaults are ["resolve-http", "resolve-file", "tls-aws-lc-rs"], which pull
# reqwest + rustls, break the wasm build, and turn an external `$ref` from a hard error
# into a live network fetch (SEP-2106).
jsonschema = { version = "0.49", optional = true, default-features = false }
```
**Apply to:** the `schema-validation` feature line and the toolkit's `input-validation` rewrite. Every
new feature edge in this phase gets a `# Why:` comment in the same style. Same for
`fuzz/Cargo.toml:41-47`, whose comment must be updated for the D-04 split.

### SP-6 — Threat-comment discipline
**Source:** the phase's own P0 sub-goal + RESEARCH § Threat-model discipline note.
**Apply to:** every `T-*` comment written or rewritten. It must name the enforcing function and be
backed by an acceptance row that fails when the enforcement is removed. `script_tool.rs:185` is the
counter-example: it asserts *binding*, not *validation*.

## Corrections to RESEARCH.md

> ⚠ **RESEARCH Finding 5c / Pitfall 6 is WRONG, and it makes D-03(b) look more expensive than it is.**
>
> Finding 5c claims `#[deprecated]` on `pub mod validation` breaks `make lint` because
> `examples/s19_wasm_typed_tools.rs` calls `validation::validate_*` at eight sites under
> `RUSTFLAGS = -D warnings`. Measured this session:
>
> 1. `s19`'s import is `use pmcp::server::wasm_typed_tool::{validation, SimpleWasmTool,
>    WasmTypedTool};` (`examples/s19_wasm_typed_tools.rs:25`). That resolves to
>    **`src/server/wasm_typed_tool.rs:278` `pub mod validation {`** — a *different, inline* module,
>    not `src/server/validation.rs`.
> 2. Every one of those eight call sites is inside `#[cfg(target_arch = "wasm32")] mod wasm_example`
>    (`:19-20`); the native build compiles only a stub `main()` that eprintlns
>    (`:12-17`). `cargo check --features "full" --examples` on a native host never compiles them.
> 3. `grep -rn "server::validation" examples/ src/ tests/ crates/` returns **only the 11 doctest
>    lines inside `src/server/validation.rs` itself** (`:54, 80, 111, 137, 180, 214, 246, 290, 312,
>    355, 377`). There are **zero** non-doctest consumers, tree-wide.
>
> **Consequences for the planner:**
> - Finding 5c's remediation table (options a/b/c) is unnecessary. Do **not** plan a task to add
>   `#[allow(deprecated)]` to `s19`, and do **not** plan Finding 9c's suggestion to rewrite `s19` as
>   the `s57` replacement — `s19` teaches a wasm-only surface this phase does not touch.
> - The **only** remaining deprecation-warning risk is RESEARCH assumption A1: the 11 doctests inside
>   `validation.rs` under `make test-doc`. That task stands; keep it and measure it.
> - The `s57` example should be built fresh against `examples/s16_typed_tools.rs` /
>   `s20_typed_tool_v2.rs` (native typed-tool examples) rather than derived from `s19`.
>
> This does not change any locked decision — D-03 still deprecates + hides — it removes a phantom cost
> and redirects the example task.

## No Analog Found

| File | Role | Data Flow | Reason |
|---|---|---|---|
| `crates/pmcp-server-toolkit/src/config.rs` → `ServerConfig::lint()` | config lint | transform | No warning-returning validator exists in this crate. `validate` is first-error-wins with no warning channel (RESEARCH Finding 11a). The `cargo-pmcp` IAM validator's `-> Vec<Warning>` shape is a cross-crate idea, not a copyable analog. |
| `cargo-pmcp` → a `ServerConfig` lint surface (SC-3 half) | CLI command | file-I/O | `cargo-pmcp` has **no** dependency on `pmcp-server-toolkit` and `validate.rs` never reads a `ServerConfig` (RESEARCH Finding 11b). Whatever route Q5 picks, it is new wiring. `validate_deploy` (`cargo-pmcp/src/commands/validate.rs:584-607`) is the structural analog for the *subcommand*, not for the config reading. |
| E1 `RequestPolicy` trait + builder registration | middleware | event-driven | No policy/interceptor hook exists on the toolkit builder. The nearest structural neighbour is the `AuthProvider` trait (`crates/pmcp-server-toolkit/src/http/auth.rs`, exercised by `tests/http_auth.rs`) — same async-trait-injected-into-the-request-path shape, different responsibility. Treat it as a weak analog only. |

## Metadata

**Analog search scope:** `src/server/`, `crates/pmcp-server-toolkit/{src,tests,examples}/`,
`crates/pmcp-code-mode/src/`, `crates/pmcp-openapi-server/tests/`, `cargo-pmcp/src/commands/`,
`tests/`, `fuzz/fuzz_targets/`, `examples/`, `Makefile`, root + 9 `Cargo.toml`.
**Tracked-source check:** every analog path verified with `git ls-files`; zero `.gsd/` mirror paths.
**Files read this session (beyond RESEARCH.md's quotes):** `crates/pmcp-server-toolkit/src/config.rs`
(1095–1155), `src/error.rs`-equivalent `crates/pmcp-server-toolkit/src/error.rs` (145–169),
`crates/pmcp-server-toolkit/src/tools.rs` (745–760, 1030–1045), `tests/log_emitter.rs` (160–180),
`Makefile` (584–616), `fuzz/fuzz_targets/fuzz_schema_draft_pin.rs` (1–45), `fuzz/Cargo.toml`
(41–47, 267–271), `examples/s19_wasm_typed_tools.rs` (1–30, 205–235),
`crates/pmcp-server-toolkit/examples/e01_toolkit_minimal.rs` (1–35),
`crates/pmcp-server-toolkit/tests/http_auth.rs` (1–40), `src/server/wasm_typed_tool.rs` (278).
**Pattern extraction date:** 2026-09-26
