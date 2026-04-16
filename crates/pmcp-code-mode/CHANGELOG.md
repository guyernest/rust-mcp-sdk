# Changelog

All notable changes to `pmcp-code-mode` will be documented in this file.

## [0.3.0] - 2026-04-14

### Breaking Changes

- `validate_javascript_code_async` added — the derive macro now calls this instead of the sync
  `validate_javascript_code`. This means **policy evaluation (Cedar/AVP) is now enforced** for
  JavaScript/OpenAPI servers using `#[derive(CodeMode)]`.

  The default `PolicyEvaluator::evaluate_script` implementation **denies all scripts**. If you
  have a custom evaluator that only implements `evaluate_operation`, override `evaluate_script`
  to allow scripts. `NoopPolicyEvaluator` already allows all scripts.

- Policy evaluation errors are **fail-closed** (matching the GraphQL path). A policy backend
  outage blocks JavaScript validation requests instead of silently allowing them.

### Added

- `validate_javascript_code_async` — async JavaScript validation with `PolicyEvaluator::evaluate_script`
  policy enforcement. Mirrors `validate_graphql_query_async` pattern.
- `validate_js_preamble` and `check_js_config_authorization` — shared helpers extracted from
  sync/async JavaScript validation (eliminates 55 lines of duplication).
- `CodeLanguage` enum — extensible runtime representation of supported languages with `from_attr`,
  `as_str`, `required_feature` methods.
- `JsCodeExecutor<H>` — standard adapter bridging `HttpExecutor` to `CodeExecutor` (Pattern B: JS+HTTP).
- `SdkCodeExecutor<S>` — standard adapter bridging `SdkExecutor` to `CodeExecutor` (Pattern C: JS+SDK).
- `McpCodeExecutor<M>` — standard adapter bridging `McpExecutor` to `CodeExecutor` (Pattern D: JS+MCP).
- `SdkExecutor` now exported from lib.rs (was previously missing).

## [0.2.0] - 2026-04-13

### Breaking Changes

- `HmacTokenGenerator::new` and `new_from_bytes` now return `Result<Self, TokenError>` instead of
  panicking on short secrets. This catches misconfigured HMAC secrets at startup instead of
  panicking at runtime.

- `ValidationPipeline::new`, `from_token_secret`, `with_policy_evaluator`, and
  `from_token_secret_with_policy` now return `Result<Self, TokenError>` instead of `Self`.

  **Migration:** Add `?` or `.unwrap()` to your constructor calls:

  ```rust
  // Before (v0.1.0):
  let pipeline = ValidationPipeline::new(config, secret);

  // After (v0.2.0):
  let pipeline = ValidationPipeline::new(config, secret)?;
  ```

- `ValidationPipeline::policy_evaluator` field changed from `Option<Box<dyn PolicyEvaluator>>`
  to `Option<Arc<dyn PolicyEvaluator>>`. `with_policy_evaluator` and `set_policy_evaluator`
  accept `Arc<dyn PolicyEvaluator>`.

- `language` attribute now selects the validation path at compile time, not just tool metadata.

### Added

- `TokenError` enum for token generator construction errors (`TokenError::SecretTooShort`).
- `#[code_mode(context_from = "method_name")]` attribute for real `ValidationContext` binding.
- `#[code_mode(language = "...")]` attribute — selects validation method and tool metadata.
- `PolicyEvaluator` trait wiring through `Arc<dyn PolicyEvaluator>` in `ValidationPipeline`.
- `from_token_secret_with_policy` constructor for derive macro use.
- Compile-fail trybuild tests for missing `token_secret` and `code_executor` fields.
