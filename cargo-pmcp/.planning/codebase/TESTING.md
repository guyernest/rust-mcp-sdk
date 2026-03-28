# Testing Patterns

**Analysis Date:** 2026-02-26

## Test Framework

**Runner:**
- Rust built-in test runner (no external test framework dependency)
- Config: No explicit config file; uses Cargo's default test configuration
- `#[test]` attribute for synchronous tests
- `#[tokio::test]` for async tests (16 instances across codebase)

**Assertion Library:**
- Standard `assert!` and `assert_eq!` macros
- No external assertion library (proptest, quickcheck not detected)

**Run Commands:**
```bash
cargo test                 # Run all tests
cargo test --lib          # Run library tests only
cargo test -- --nocapture # Show println! output
cargo test --doc          # Run doctests
```

## Test File Organization

**Location:**
- Co-located in same file as implementation using `#[cfg(test)] mod tests { }`
- Tests live in the module they test
- 20 files contain test modules identified with `mod tests { }`

**Naming:**
- Test functions: `test_*` convention (camelCase suffix)
- Examples: `test_glob_match()`, `test_local_provider_get()`, `test_next_available_port()`, `test_registry_default_providers()`
- Module name: `tests` (lowercase, singular)

**Structure:**
```
src/
├── secrets/
│   ├── providers/local.rs
│   │   └── mod tests { #[test] fn test_glob_match() { ... } }
│   ├── registry.rs
│   │   └── mod tests { #[test] fn test_registry_default_providers() { ... } }
│   └── value.rs
│       └── mod tests { ... }
└── utils/
    └── config.rs
        └── mod tests { #[test] fn test_next_available_port() { ... } }
```

## Test Structure

**Suite Organization:**
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_feature_name() {
        // Arrange
        let temp_dir = TempDir::new().unwrap();
        let provider = LocalSecretProvider::new(temp_dir.path().join("secrets"));

        // Act
        provider.set("server/KEY", SecretValue::new("value".to_string()), defaults).await.unwrap();

        // Assert
        let value = provider.get("server/KEY").await.unwrap();
        assert_eq!(value.expose(), "value");
    }
}
```

**Patterns:**
- **Setup**: Arrange phase creates test fixtures (TempDir, provider instances)
- **Execution**: Act phase performs operation being tested
- **Assertion**: Assert phase verifies expected behavior
- Helper functions within test module for repeated setup
- Example from `src/secrets/providers/local.rs`:
  ```rust
  #[tokio::test]
  async fn test_local_provider_set_and_get() {
      let temp_dir = TempDir::new().unwrap();
      let provider = LocalSecretProvider::new(temp_dir.path().join("secrets"));

      provider.set("test-server/API_KEY", SecretValue::new("secret-value".to_string()), SetOptions::default())
          .await.unwrap();
      let value = provider.get("test-server/API_KEY").await.unwrap();
      assert_eq!(value.expose(), "secret-value");
  }
  ```

## Mocking

**Framework:**
- No mocking framework detected (mockito, mock_derive not in Cargo.toml)
- Manual fake/test implementations used instead

**Patterns:**
- Trait-based design enables testing via concrete test implementations
- Example: `SecretProvider` trait has test implementations like `LocalSecretProvider`
- Test data structures created directly rather than mocked
- From `src/publishing/landing.rs`:
  ```rust
  fn sample_project() -> ProjectInfo {
      ProjectInfo {
          name: "demo-app".to_string(),
          description: "A demo application".to_string(),
          logo: None,
          widgets: vec![WidgetInfo { ... }],
      }
  }
  ```

**What to Mock:**
- Filesystem operations: Mocked via `tempfile::TempDir` for isolation
- HTTP/network calls: Test implementations would provide fake responses
- Time-based operations: Not observed in codebase

**What NOT to Mock:**
- Core business logic: Always test real implementations
- Traits: Use real implementations instead
- Configuration: Use test config structures

## Fixtures and Factories

**Test Data:**
```rust
fn sample_project() -> ProjectInfo {
    ProjectInfo {
        name: "demo-app".to_string(),
        description: "A demo application".to_string(),
        logo: None,
        widgets: vec![WidgetInfo {
            name: "hello".to_string(),
            uri: "ui://app/hello".to_string(),
            html: r#"<html>...</html>"#.to_string(),
        }],
    }
}

fn sample_mock_data() -> HashMap<String, serde_json::Value> {
    let mut data = HashMap::new();
    data.insert("key".to_string(), json!("value"));
    data
}
```

**Location:**
- Defined as helper functions within `mod tests { }` blocks
- Lightweight builders for common test scenarios
- Example from `src/secrets/providers/local.rs`: Setup pattern uses builder methods on providers

## Coverage

**Requirements:**
- Not enforced by CI (no minimum coverage detection)
- Project standards in CLAUDE.md require 80%+ coverage for new features
- No coverage reporting tool detected in Cargo.toml

**View Coverage:**
Not configured. Manual testing approach used.

## Test Types

**Unit Tests:**
- **Scope**: Individual functions and methods
- **Approach**: Direct function calls with assertions
- **Frequency**: Most abundant test type (16 async + many sync tests)
- Examples:
  - `test_glob_match()` - Tests glob pattern matching function
  - `test_next_available_port()` - Tests port allocation logic
  - `test_parse_secret_name_valid()` - Tests string parsing

**Integration Tests:**
- **Scope**: Multiple components working together
- **Approach**: Create provider, set/get/list operations, verify state
- Example from `src/secrets/providers/local.rs`:
  ```rust
  #[tokio::test]
  async fn test_local_provider_list() {
      let provider = LocalSecretProvider::new(temp_dir.path().join("secrets"));
      provider.set("server-a/KEY1", ...).await.unwrap();
      provider.set("server-a/KEY2", ...).await.unwrap();
      let result = provider.list(ListOptions::default()).await.unwrap();
      assert_eq!(result.secrets.len(), 3);
  }
  ```

**E2E Tests:**
- **Framework**: Not detected in codebase
- **Status**: No end-to-end tests found
- Integration tests serve as closest equivalent

## Common Patterns

**Async Testing:**
```rust
#[tokio::test]
async fn test_async_operation() {
    let provider = create_test_provider().await;
    let result = provider.get("key").await;
    assert!(result.is_ok());
}
```
- Uses `#[tokio::test]` attribute
- Directly await async calls (16 instances)
- Error handling with `.unwrap()` for test setup, assertion-based checks for behavior

**Error Testing:**
```rust
#[tokio::test]
async fn test_local_provider_no_overwrite() {
    let provider = LocalSecretProvider::new(...);

    // Set initial value
    provider.set("server/EXISTING", SecretValue::new("original".to_string()), defaults).await.unwrap();

    // Try to set with no_overwrite flag
    let result = provider.set(
        "server/EXISTING",
        SecretValue::new("new-value".to_string()),
        SetOptions { no_overwrite: true, ..Default::default() },
    ).await;

    // Assert error occurred
    assert!(matches!(result, Err(SecretError::AlreadyExists { .. })));

    // Verify original value unchanged
    let value = provider.get("server/EXISTING").await.unwrap();
    assert_eq!(value.expose(), "original");
}
```
- Pattern: Set state, attempt invalid operation, verify error type and original state
- Use `matches!` macro for error variants
- Test both error occurrence and side effects

**Sync Tests:**
```rust
#[test]
fn test_glob_match() {
    assert!(glob_match("*", "anything"));
    assert!(glob_match("server-*", "server-a"));
    assert!(glob_match("*KEY", "API_KEY"));
    assert!(glob_match("exact", "exact"));
    assert!(!glob_match("server-*", "other-a"));
}
```
- Simple assertion pattern for pure functions
- Multiple assertions per test for related cases
- Direct function calls without setup

## Test Dependencies

**Dev Dependencies:**
- `tempfile = "3"` - Temporary directory creation for filesystem tests
- From `src/secrets/providers/local.rs`: `let temp_dir = TempDir::new().unwrap();`

**Async Dependencies:**
- `tokio` runtime with `tokio::test` macro
- No external async testing utilities

## Testing Async Trait Methods

```rust
#[async_trait]
impl SecretProvider for LocalSecretProvider {
    async fn get(&self, name: &str) -> SecretResult<SecretValue> { ... }
    async fn list(&self, options: ListOptions) -> SecretResult<ListResult> { ... }
}
```

Tests call these as normal async methods:
```rust
#[tokio::test]
async fn test_list_with_filter() {
    let provider = LocalSecretProvider::new(...);
    let result = provider.list(ListOptions { ... }).await.unwrap();
    assert_eq!(result.secrets.len(), 2);
}
```

## No Integration Test Framework

- No separate `tests/` directory detected
- No integration-specific setup or fixtures
- Integration tests co-located with implementation tests
- This approach sufficient for cargo-pmcp's scope (CLI tool)

---

*Testing analysis: 2026-02-26*
