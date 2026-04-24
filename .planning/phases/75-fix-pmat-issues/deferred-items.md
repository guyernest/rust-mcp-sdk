# Phase 75 — Deferred Items (out of scope, Wave 0)

Items discovered during Wave 0 execution that are NOT in scope for this Wave
(per the executor's deviation-rules scope boundary). Logged here so later
waves can pick them up.

## 2026-04-23 — Pre-existing clippy errors in `crates/pmcp-code-mode/`

`cargo clippy -p pmcp-code-mode --features js-runtime --tests -- -D warnings`
fails with 18 lib errors + 28 lib-test errors. Verified to pre-exist Wave 0
Task 2 (reproduced on main with my changes stashed). Examples:

- `crates/pmcp-code-mode/src/eval.rs:1430` — `clippy::approx_constant` on
  `serde_json::json!(3.14)` (PI approximation) inside test block
- `crates/pmcp-code-mode/src/executor.rs:4614` — same `approx_constant` on
  `3.14` test literal
- `eval.rs:668` — `clippy::should_implement_trait` on `from_str` method
  (suggests implementing `std::str::FromStr` trait)
- multiple `clippy::redundant_closure` and `clippy::collapsible_match` lints

**Disposition:** Wave 3 (the pmcp-code-mode wave) inherits these as part of
its `evaluate_with_scope` cog 123 → ≤25 refactor. The semantic regression
baseline added in Task 2 (this file) does not introduce any new clippy
warnings in its own source — verified by grepping the clippy log for
`eval_semantic_regression` (zero hits).

**CLAUDE.md "zero tolerance" status:** technically already-violated on main
before Phase 75. Not surfaced earlier because `make quality-gate` is
typically run with `--features full` from root, not per-crate; the pmcp-code-mode-specific lints
only trip when isolating to that crate's test target. Recommended Wave 3
opens with a 1-task lint sweep before the cog refactor begins.

## 2026-04-23 — Pre-existing dead-code warnings in `crates/pmcp-code-mode/`

`cargo build -p pmcp-code-mode --features js-runtime --tests` produces 3
dead-code warnings:

- `MockHttpExecutor.mode` field never read
- `PlanExecutor::evaluate_with_binding` and `evaluate_with_two_bindings`
  methods never called

**Disposition:** Wave 3 — same justification as the clippy items above.
