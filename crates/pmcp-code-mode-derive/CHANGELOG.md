# Changelog

All notable changes to `pmcp-code-mode-derive` will be documented in this file.

## [0.1.0] - Unreleased

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
  // or: let builder = server.register_code_mode_tools(builder).unwrap();
  ```

### Added

- `#[code_mode(context_from = "method_name")]` attribute for real `ValidationContext` binding.
- `#[code_mode(language = "...")]` attribute to parameterize tool metadata language.
- Compile-fail trybuild tests for missing `token_secret` and `code_executor` fields.
- Improved error messages: missing field errors now explicitly mention `#[derive(CodeMode)]` as
  the source of the requirement and include type hints.
