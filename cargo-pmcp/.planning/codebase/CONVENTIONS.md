# Coding Conventions

**Analysis Date:** 2026-02-26

## Naming Patterns

**Files:**
- Lowercase with underscores (snake_case)
- Module files: `provider.rs`, `config.rs`, `error.rs`
- Modules organized by feature: `secrets/`, `deployment/`, `commands/`, `templates/`, `publishing/`
- Files match their primary type name: `value.rs` contains `SecretValue`, `config.rs` contains `SecretsConfig`

**Functions:**
- snake_case for all function names
- Helper/internal functions prefixed with `_` when appropriate: rarely used in this codebase
- Public functions documented with triple-slash comments
- Async functions use `async fn` with `async_trait` for trait methods (41 files use async patterns)
- Examples: `execute()`, `parse_secret_name()`, `generate_random()`, `ensure_dir()`

**Variables:**
- snake_case for local variables and parameters
- Prefix-based naming for special patterns:
  - `temp_dir` for temporary directories
  - `config` for configuration objects
  - `provider` for trait implementations
  - `result` for Result values
- Examples from `src/secrets/providers/local.rs`: `secret_path()`, `ensure_server_dir()`, `secret_name`

**Types:**
- PascalCase for all struct, enum, trait names
- Trait names often end in "Provider", "Config", "Error" suffix to indicate category
- Examples: `SecretProvider`, `SecretValue`, `ProviderRegistry`, `SecretError`, `ProjectInfo`
- Generic types use uppercase single letters: `T`, `E`, etc.

## Code Style

**Formatting:**
- Standard Rust formatting (implicitly enforced by context of project)
- 4-space indentation (Rust convention)
- Opening braces on same line
- Comments for complex logic blocks

**Linting:**
- Uses `#[allow(dead_code)]` for fields/methods that are conditionally compiled or intentionally unused
- Examples in `src/publishing/detect.rs` line 18, `src/secrets/providers/aws.rs` lines 19, 37
- Attributes placed directly above the item they modify
- No clippy warnings enforced (per project CLAUDE.md standards)

## Import Organization

**Order:**
1. External crates (anyhow, serde, etc.)
2. Standard library (std::*)
3. Project crates (internal modules)

**Path Aliases:**
Not detected. Direct imports used throughout. Examples:
- `use anyhow::{Context, Result};` - Standard error handling
- `use crate::secrets::{...};` - Internal module imports
- `use std::path::PathBuf;` - Stdlib paths

## Error Handling

**Patterns:**
- `anyhow::Result<T>` for most function returns - primary error type across codebase
- Custom error enums with `thiserror::Error` derive for domain-specific errors
  - Example: `SecretError` in `src/secrets/error.rs` with variants like `NotFound`, `InvalidName`, `AuthenticationFailed`
- Error context propagation with `.context()` method
- `.with_context()` for lazy error messages
- Examples from `src/publishing/detect.rs`:
  ```rust
  fs::read_to_string(&cargo_toml_path)
      .with_context(|| "No Cargo.toml found. Are you in a Rust project directory?")?
  ```
- Type aliases like `SecretResult<T> = Result<T, SecretError>` for domain-specific result types
- No panic-driven error handling; all errors propagate with `?` operator

## Logging

**Framework:** `println!` and `eprintln!` for user-facing output

**Patterns:**
- `println!` for normal output, often with color via `colored` crate
- `eprintln!` for verbose debugging when PMCP_VERBOSE env var set
- Status indicators using emoji and colored output
  - Examples: `"✓".green()`, `"📥"`, `"✅"`, `"🔧"`
  - From `src/templates/server.rs`: `println!("  {} Created {}", "✓".green(), core_name.bright_yellow());`
- Verbose mode checking: `if std::env::var("PMCP_VERBOSE").is_ok()`
- From `src/secrets/providers/pmcp_run.rs`: `eprintln!("[VERBOSE] GraphQL request to: {}", self.api_url);`

## Comments

**When to Comment:**
- Module-level documentation (//!) explaining purpose and design
- Public items (functions, structs, enums) have doc comments with examples
- Complex algorithm explanations (e.g., glob matching in `src/secrets/providers/local.rs`)
- Design decisions that aren't obvious from code
- TODO/FIXME/HACK comments not found - zero technical debt comments enforced by project standards

**JSDoc/TSDoc:**
- Uses Rust doc comments (///) for public APIs
- Format: Description, optional # Examples section, optional # Panics, # Errors
- Examples from `src/deployment/metadata.rs`:
  ```rust
  /// Complete MCP server metadata for deployment.
  ///
  /// This structure is extracted from server configuration files and injected
  /// into deployment artifacts for platforms to consume.
  ```

## Function Design

**Size:**
- Functions typically 10-50 lines
- Larger functions (100+ lines) split into helper functions
- Examples:
  - `src/commands/deploy/init.rs`: 1705 lines (large command with nested helpers)
  - `src/secrets/providers/local.rs`: Well-decomposed into ~30-50 line methods
  - `generate_*` functions split into discrete steps

**Parameters:**
- Minimal: 1-3 parameters preferred
- Self or &self for trait implementations
- Path/string parameters passed as `&str` or `&Path` when possible
- Builder pattern used for complex construction: `ServerCoreBuilder` in parent SDK
- Options struct pattern for many optional parameters
  - Example: `ListOptions`, `SetOptions` in secrets module

**Return Values:**
- `Result<T>` for fallible operations (most functions)
- Direct type `T` for infallible operations
- Tuple returns rare; structs preferred for multiple return values
- Iterator types used where appropriate
- Example from `src/secrets/registry.rs`:
  ```rust
  pub async fn check_all_health(&self) -> Vec<(String, ProviderHealth)>
  ```

## Module Design

**Exports:**
- Explicit public items using `pub` keyword
- Modules themselves marked `pub mod` only if needed by external consumers
- Re-exports common types at module level
- Example from `src/secrets/mod.rs`: Public re-export of `SecretValue`, `ProviderRegistry`, `SecretError`

**Barrel Files:**
- Not heavily used; most imports are direct
- `mod.rs` files exist only for organizational reasons
- Example: `src/secrets/mod.rs` organizes submodules but doesn't re-export everything

## Async Patterns

**Framework:** tokio runtime for async operations
- `#[tokio::test]` for async tests (16 instances found)
- `async_trait` crate for async trait methods
- Uses `.await` extensively for async operations
- From `src/secrets/providers/local.rs`:
  ```rust
  #[tokio::test]
  async fn test_local_provider_get() {
      let provider = LocalSecretProvider::new(...);
      let value = provider.get("test-server/API_KEY").await.unwrap();
  }
  ```

## Type Conversions

**From/Into traits:**
- `From<String>` and `From<&str>` implemented for domain types
- Example from `src/secrets/value.rs`:
  ```rust
  impl From<String> for SecretValue
  impl From<&str> for SecretValue
  ```
- Custom parsing with `FromStr` trait
- Example from `src/secrets/value.rs`: `SecretCharset` implements `FromStr`

## Trait Implementations

**Standard traits:**
- `Debug` manually implemented to redact secrets
  - From `src/secrets/value.rs`: `impl fmt::Debug for SecretValue` returns `[REDACTED]`
- `Display` for user-facing output with redaction
- `Default` for configuration objects
- `Clone` preferred over `Copy` for most types
- `Serialize/Deserialize` with `#[serde(rename_all = "camelCase")]` pattern seen in parent SDK

## Feature Flags

**Conditional compilation:**
- `#[cfg(test)]` for test modules
- `#[cfg(unix)]` for platform-specific code (e.g., file permissions)
  - From `src/secrets/providers/local.rs`: Unix-specific permission setting
- `#[cfg(feature = "aws-secrets")]` for optional AWS provider

## Visibility and Encapsulation

**Private by default:**
- Helper methods marked as private or pub(crate)
- Example from `src/secrets/providers/local.rs`:
  - Private: `ensure_dir()`, `ensure_gitignore()`, `secret_path()`, `ensure_server_dir()`
  - Public: `new()`, trait method implementations
- Internal trait methods not exposed in public API

---

*Convention analysis: 2026-02-26*
