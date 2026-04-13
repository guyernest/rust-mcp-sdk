# Changelog

All notable changes to `pmcp-code-mode` will be documented in this file.

## [0.1.0] - Unreleased

### Breaking Changes

- `HmacTokenGenerator::new` and `new_from_bytes` now return `Result<Self, TokenError>` instead of
  panicking on short secrets. This catches misconfigured HMAC secrets at startup instead of
  panicking at runtime.

- `ValidationPipeline::new`, `from_token_secret`, `with_policy_evaluator`, and
  `from_token_secret_with_policy` now return `Result<Self, TokenError>` instead of `Self`.

  **Migration:** Add `?` or `.unwrap()` to your constructor calls:

  ```rust
  // Before (v0.0.x):
  let pipeline = ValidationPipeline::new(config, secret);

  // After (v0.1.0):
  let pipeline = ValidationPipeline::new(config, secret)?;
  // or: let pipeline = ValidationPipeline::new(config, secret).unwrap();
  ```

### Added

- `TokenError` enum for token generator construction errors (`TokenError::SecretTooShort`).
- `#[code_mode(context_from = "method_name")]` attribute for real `ValidationContext` binding.
- `#[code_mode(language = "...")]` attribute to parameterize tool metadata language.
- `PolicyEvaluator` trait wiring through `Arc<dyn PolicyEvaluator>` in `ValidationPipeline`.
- `from_token_secret_with_policy` constructor for derive macro use.
- Compile-fail trybuild tests for missing `token_secret` and `code_executor` fields.
