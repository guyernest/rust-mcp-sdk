# Changelog

All notable changes to `pmcp-code-mode-derive` will be documented in this file.

## [0.2.0] - 2026-04-14

### Breaking Changes

- `language = "javascript"` now calls `validate_javascript_code_async` (async with policy
  evaluation) instead of `validate_javascript_code` (sync, no policy). This means Cedar
  and AVP policies are now enforced for JavaScript/OpenAPI servers using the derive macro.

  No code changes needed if you use `NoopPolicyEvaluator` — it allows all scripts.
  Custom evaluators must override `evaluate_script` (the trait default denies all scripts).

### Added

- `"sql"` and `"mcp"` language support — compile-time dispatch to `validate_sql_query` and
  `validate_mcp_composition` respectively.
- `"js"` accepted as alias for `"javascript"`.
- `CodeLanguage` sync test — CI enforces that derive macro's language dispatch stays aligned
  with the runtime `CodeLanguage` enum.
- `gen_validation_call` helper — centralized language-to-validation-method mapping with shared
  error handling. Adding a new language is a single match arm.
- Unknown language values produce a compile error listing all supported values.
- Invalid `context_from` values produce a compile error instead of panicking.

## [0.1.0] - 2026-04-13

### Breaking Changes

- `register_code_mode_tools` now returns `Result<ServerBuilder, pmcp_code_mode::TokenError>`
  instead of `ServerBuilder`. This catches misconfigured HMAC secrets at server startup instead
  of panicking at runtime.

  **Migration:** Add `?` or `.unwrap()` to your `register_code_mode_tools()` call:

  ```rust
  // Before (v0.0.x):
  let builder = server.register_code_mode_tools(builder);

  // After (v0.1.0):
  let builder = server.register_code_mode_tools(builder)?;
  ```

### Added

- `#[code_mode(context_from = "method_name")]` attribute for real `ValidationContext` binding.
- `#[code_mode(language = "...")]` attribute to parameterize validation path and tool metadata.
- Compile-fail trybuild tests for missing `token_secret` and `code_executor` fields.
- Improved error messages: missing field errors now explicitly mention `#[derive(CodeMode)]` as
  the source of the requirement and include type hints.
