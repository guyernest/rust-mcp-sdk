---
inclusion: manual
---

# MCP Testing - Comprehensive Testing Strategy

This guide covers testing MCP servers built with pmcp SDK and cargo-pmcp. Testing is **mandatory** - every MCP server must achieve ≥80% test coverage with zero defects.

## Testing Philosophy (Toyota Way)

### Zero Tolerance for Defects

Testing is not optional. It's how we prove quality:

- **Build quality in**: Write tests as you implement features
- **Stop and fix**: Never proceed with failing tests
- **Visual management**: Test coverage must be ≥80%
- **Respect for people**: Good tests make maintenance easier for everyone

### Test Pyramid for MCP Servers

```
     ╱────────────╲
    ╱  Integration ╲     ← mcp-tester scenarios (10-20% of tests)
   ╱────────────────╲
  ╱   Unit Tests     ╲   ← Tool/resource logic (70-80% of tests)
 ╱────────────────────╲
╱  Property/Fuzz Tests ╲ ← Invariant testing (10% of tests)
```

## Testing Layers

### 1. Unit Tests (Majority of Tests)

**Purpose**: Test individual tool/resource implementations in isolation.

**Coverage target**: 70-80% of total test suite

**Pattern**:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use pmcp::Error;

    #[tokio::test]
    async fn test_add_success() {
        let input = AddInput { a: 5.0, b: 3.0 };
        let extra = RequestHandlerExtra::default();

        let result = handler(input, extra).await.unwrap();

        assert_eq!(result.result, 8.0);
    }

    #[tokio::test]
    async fn test_weather_invalid_city() {
        let input = WeatherInput {
            city: String::new(),
            days: Some(1)
        };
        let extra = RequestHandlerExtra::default();

        let result = handler(input, extra).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            Error::Validation(msg) => {
                assert!(msg.contains("empty"));
            }
            _ => panic!("Expected validation error"),
        }
    }

    #[tokio::test]
    async fn test_weather_invalid_days() {
        let input = WeatherInput {
            city: "London".to_string(),
            days: Some(10) // Out of range
        };
        let extra = RequestHandlerExtra::default();

        let result = handler(input, extra).await;

        assert!(result.is_err());
    }
}
```

**Key practices**:
- One test per behavior/edge case
- Use descriptive test names: `test_<function>_<scenario>_<expected>`
- Test both success and error cases
- Use `#[tokio::test]` for async functions
- Test all validation logic
- Test error messages contain useful information

### 2. Integration Tests (mcp-tester Scenarios)

**Purpose**: Test the complete MCP server through the protocol.

**Coverage target**: 10-20% of total test suite

**When to use**: After implementing tools, test end-to-end behavior.

#### Generating Test Scenarios

```bash
# Generate scenarios from server introspection
cargo pmcp test --server myserver --generate-scenarios

# Creates: scenarios/myserver/generated.yaml
```

**Generated scenario example**:
```yaml
# scenarios/weather/generated.yaml
scenarios:
  - name: "get-weather happy path"
    steps:
      - tool_call:
          name: "get-weather"
          arguments:
            city: "London"
            days: 3
          expect:
            success: true
            output_contains: "temperature"

  - name: "get-weather validation error"
    steps:
      - tool_call:
          name: "get-weather"
          arguments:
            city: ""
            days: 1
          expect:
            success: false
            error_contains: "empty"
```

#### Running Scenarios

```bash
# Run all scenarios for a server
cargo pmcp test --server myserver

# Run specific scenario file
cargo pmcp test --server myserver --scenario scenarios/myserver/custom.yaml

# Run with verbose output
cargo pmcp test --server myserver --verbose
```

#### Custom Scenario Files

Create `scenarios/myserver/custom.yaml`:

```yaml
scenarios:
  - name: "Weather workflow - plan trip"
    description: "Multi-step weather checking for trip planning"
    steps:
      # Step 1: Check departure city
      - tool_call:
          name: "get-weather"
          arguments:
            city: "London"
            days: 5
          expect:
            success: true
          store_output_as: "departure_weather"

      # Step 2: Check destination city
      - tool_call:
          name: "get-weather"
          arguments:
            city: "Paris"
            days: 5
          expect:
            success: true
          store_output_as: "destination_weather"

      # Step 3: Compare (using stored outputs)
      - assertion:
          description: "Both cities have weather data"
          conditions:
            - "$departure_weather.temperature is not null"
            - "$destination_weather.temperature is not null"

  - name: "Error handling - API timeout"
    steps:
      - tool_call:
          name: "get-weather"
          arguments:
            city: "InvalidCity12345"
          expect:
            success: false
            error_type: "validation"
            error_contains: "not found"

  - name: "Resource listing"
    steps:
      - list_resources:
          expect:
            count_gt: 0
            contains_uri: "weather://forecast/London"

  - name: "Resource reading"
    steps:
      - read_resource:
          uri: "weather://forecast/London"
          expect:
            success: true
            content_type: "application/json"
```

**Scenario capabilities**:
- `tool_call`: Call a tool with arguments
- `list_resources`: List available resources
- `read_resource`: Read a specific resource
- `list_prompts`: List available prompts
- `get_prompt`: Get a specific prompt
- `assertion`: Custom assertions on stored data
- `store_output_as`: Save output for later use
- `expect`: Validation of results

### 3. Property Tests (Invariant Testing)

**Purpose**: Test properties that should always hold true, regardless of input.

**Coverage target**: 10% of total test suite

**When to use**: For complex logic, calculations, or state management.

```rust
#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_add_commutative(a in -1000.0..1000.0, b in -1000.0..1000.0) {
            let input1 = AddInput { a, b };
            let input2 = AddInput { a: b, b: a };
            let extra = RequestHandlerExtra::default();

            let result1 = tokio_test::block_on(handler(input1, extra.clone())).unwrap();
            let result2 = tokio_test::block_on(handler(input2, extra)).unwrap();

            prop_assert!((result1.result - result2.result).abs() < 0.0001);
        }

        #[test]
        fn test_weather_days_in_range(
            city in "[a-zA-Z]{3,20}",
            days in 1u8..=5u8
        ) {
            let input = WeatherInput {
                city: city.clone(),
                days: Some(days)
            };
            let extra = RequestHandlerExtra::default();

            // Should not panic with valid range
            let _ = tokio_test::block_on(handler(input, extra));
        }
    }
}
```

**Add to Cargo.toml**:
```toml
[dev-dependencies]
proptest = "1.0"
tokio-test = "0.4"
```

### 4. Fuzz Testing (Robustness)

**Purpose**: Find edge cases and crashes with random inputs.

**When to use**: For parsing, validation, or complex input handling.

**Setup** (optional but recommended):
```bash
# Install cargo-fuzz
cargo install cargo-fuzz

# Create fuzz target
cargo fuzz init

# Add fuzz target for weather tool
cat > fuzz/fuzz_targets/weather_input.rs << 'EOF'
#![no_main]
use libfuzzer_sys::fuzz_target;
use mcp_weather_core::tools::weather::WeatherInput;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = serde_json::from_str::<WeatherInput>(s);
    }
});
EOF

# Run fuzzer
cargo fuzz run weather_input
```

## Testing Workflow with cargo-pmcp

### Step 1: Implement Tool with Tests

When implementing a tool, write tests immediately:

```rust
// crates/mcp-myserver-core/src/tools/weather.rs

async fn handler(input: WeatherInput, extra: RequestHandlerExtra) -> Result<WeatherOutput> {
    // Implementation
}

pub fn build_tool() -> TypedTool<WeatherInput, WeatherOutput> {
    // Builder
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_success() {
        // Test implementation
    }

    #[tokio::test]
    async fn test_validation_errors() {
        // Test validation
    }

    #[tokio::test]
    async fn test_edge_cases() {
        // Test edge cases
    }
}
```

### Step 2: Run Unit Tests

```bash
# Run all tests
cargo test

# Run tests for specific tool
cargo test weather

# Run with output
cargo test -- --nocapture

# Run with threads=1 (race condition prevention)
cargo test --test-threads=1
```

### Step 3: Generate Integration Scenarios

```bash
# Start dev server
cargo pmcp dev --server myserver

# In another terminal, generate scenarios
cargo pmcp test --server myserver --generate-scenarios

# Review generated scenarios
cat scenarios/myserver/generated.yaml
```

### Step 4: Customize Scenarios

Edit `scenarios/myserver/generated.yaml` or create `scenarios/myserver/custom.yaml`:

```yaml
scenarios:
  - name: "Real-world workflow"
    steps:
      # Add multi-step workflows
      # Add edge cases
      # Add error conditions
```

### Step 5: Run Integration Tests

```bash
# Run all scenarios
cargo pmcp test --server myserver

# Run specific scenario
cargo pmcp test --server myserver --scenario scenarios/myserver/custom.yaml

# Run with detailed output
cargo pmcp test --server myserver --verbose
```

### Step 6: Measure Coverage

```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Generate coverage report
cargo tarpaulin --out Html --output-dir coverage

# View report
open coverage/index.html

# Verify ≥80% coverage
```

### Step 7: Validate Quality Gates

```bash
# All quality checks must pass
cargo fmt --check
cargo clippy -- -D warnings
cargo test --test-threads=1

# Or use Makefile
make quality-gate
```

## Test Organization

### Directory Structure

```
crates/mcp-myserver-core/
├── src/
│   ├── tools/
│   │   ├── weather.rs          # Implementation + unit tests
│   │   └── forecast.rs         # Implementation + unit tests
│   ├── resources/
│   │   └── weather_data.rs     # Implementation + unit tests
│   └── lib.rs
└── tests/
    ├── integration_test.rs     # Cross-tool integration tests
    └── common/
        └── mod.rs              # Shared test utilities

scenarios/
└── myserver/
    ├── generated.yaml          # Auto-generated scenarios
    ├── custom.yaml             # Hand-written scenarios
    └── edge_cases.yaml         # Edge case scenarios
```

### Test Utilities

Create `crates/mcp-myserver-core/tests/common/mod.rs`:

```rust
use pmcp::RequestHandlerExtra;
use tokio::runtime::Runtime;

pub fn create_test_extra() -> RequestHandlerExtra {
    RequestHandlerExtra::default()
}

pub fn run_async<F, T>(future: F) -> T
where
    F: std::future::Future<Output = T>,
{
    Runtime::new().unwrap().block_on(future)
}

pub fn assert_validation_error<T>(result: pmcp::Result<T>, expected_msg: &str) {
    match result {
        Err(pmcp::Error::Validation(msg)) => {
            assert!(msg.contains(expected_msg),
                "Expected '{}' in error message, got '{}'", expected_msg, msg);
        }
        Ok(_) => panic!("Expected validation error, got success"),
        Err(e) => panic!("Expected validation error, got {:?}", e),
    }
}
```

Use in tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::common::*;

    #[tokio::test]
    async fn test_invalid_input() {
        let input = WeatherInput { city: String::new(), days: Some(1) };
        let extra = create_test_extra();

        let result = handler(input, extra).await;

        assert_validation_error(result, "empty");
    }
}
```

## Testing Best Practices

### 1. Test Names Should Be Descriptive

```rust
// ❌ Bad
#[tokio::test]
async fn test1() { }

// ✅ Good
#[tokio::test]
async fn test_weather_handler_returns_error_when_city_is_empty() { }
```

### 2. Test One Thing Per Test

```rust
// ❌ Bad - tests multiple things
#[tokio::test]
async fn test_weather() {
    // Test valid input
    // Test invalid city
    // Test invalid days
    // Test API errors
}

// ✅ Good - focused tests
#[tokio::test]
async fn test_weather_success_with_valid_input() { }

#[tokio::test]
async fn test_weather_error_when_city_empty() { }

#[tokio::test]
async fn test_weather_error_when_days_out_of_range() { }
```

### 3. Use Arrange-Act-Assert Pattern

```rust
#[tokio::test]
async fn test_add_two_numbers() {
    // Arrange
    let input = AddInput { a: 5.0, b: 3.0 };
    let extra = RequestHandlerExtra::default();

    // Act
    let result = handler(input, extra).await.unwrap();

    // Assert
    assert_eq!(result.result, 8.0);
}
```

### 4. Test Error Cases Thoroughly

```rust
#[tokio::test]
async fn test_all_validation_errors() {
    let test_cases = vec![
        (WeatherInput { city: String::new(), days: Some(1) }, "empty"),
        (WeatherInput { city: "London".into(), days: Some(0) }, "1-5"),
        (WeatherInput { city: "London".into(), days: Some(10) }, "1-5"),
    ];

    for (input, expected_msg) in test_cases {
        let extra = RequestHandlerExtra::default();
        let result = handler(input, extra).await;
        assert_validation_error(result, expected_msg);
    }
}
```

### 5. Mock External Dependencies

```rust
// For testing tools that call external APIs
#[tokio::test]
async fn test_weather_with_mock_api() {
    let mut server = mockito::Server::new();

    let mock = server.mock("GET", "/weather/London")
        .with_status(200)
        .with_body(r#"{"temperature": 15.5, "conditions": "Cloudy"}"#)
        .create();

    let input = WeatherInput {
        city: "London".into(),
        days: Some(1)
    };
    let extra = RequestHandlerExtra::default();

    // Override API URL to use mock server
    std::env::set_var("WEATHER_API_URL", &server.url());

    let result = handler(input, extra).await.unwrap();

    assert_eq!(result.temperature, 15.5);
    mock.assert();
}
```

**Add to Cargo.toml**:
```toml
[dev-dependencies]
mockito = "1.0"
```

## Coverage Requirements

### Minimum Coverage: 80%

```bash
# Generate coverage report
cargo tarpaulin --out Html --output-dir coverage

# Check coverage percentage
cargo tarpaulin | grep "Coverage"
# Must show: Coverage: ≥80%
```

### What to Cover

✅ **Must have tests**:
- All public functions
- All tool handlers
- All resource handlers
- All validation logic
- All error paths
- All branches (if/else, match arms)

⚠️ **Can skip**:
- Generated code (build.rs output)
- Simple getters/setters (if trivial)
- Debug/Display impls (unless complex)

### Coverage by Module

Aim for:
- **Tools**: 90%+ coverage (critical business logic)
- **Resources**: 85%+ coverage
- **Workflows**: 80%+ coverage
- **Utilities**: 80%+ coverage

## Continuous Testing

### During Development

```bash
# Watch mode - re-run tests on file changes
cargo watch -x test

# Watch specific module
cargo watch -x 'test weather'
```

### Before Committing

```bash
# Run all quality gates
make quality-gate

# Or manually:
cargo fmt --check
cargo clippy -- -D warnings
cargo test --test-threads=1
cargo tarpaulin
```

### In CI/CD

```yaml
# .github/workflows/test.yml
name: Tests
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - name: Run tests
        run: cargo test --test-threads=1
      - name: Check coverage
        run: |
          cargo install cargo-tarpaulin
          cargo tarpaulin --out Xml
      - name: Upload coverage
        uses: codecov/codecov-action@v2
```

## Common Testing Patterns

### Pattern 1: Testing Async Functions

```rust
#[tokio::test]
async fn test_async_operation() {
    let result = async_function().await;
    assert!(result.is_ok());
}
```

### Pattern 2: Testing with Timeouts

```rust
#[tokio::test]
async fn test_with_timeout() {
    let timeout = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        slow_operation()
    );

    assert!(timeout.await.is_ok(), "Operation timed out");
}
```

### Pattern 3: Testing Parallel Operations

```rust
#[tokio::test]
async fn test_concurrent_calls() {
    let futures = vec![
        handler(input1.clone(), extra.clone()),
        handler(input2.clone(), extra.clone()),
        handler(input3.clone(), extra.clone()),
    ];

    let results = futures::future::join_all(futures).await;

    assert!(results.iter().all(|r| r.is_ok()));
}
```

### Pattern 4: Testing State Management

```rust
#[tokio::test]
async fn test_stateful_tool() {
    let state = Arc::new(Mutex::new(ServerState::new()));

    // First call modifies state
    let result1 = handler_with_state(input1, state.clone()).await.unwrap();

    // Second call sees modified state
    let result2 = handler_with_state(input2, state.clone()).await.unwrap();

    assert_eq!(result2.counter, 2);
}
```

## Debugging Failed Tests

### Enable Logging in Tests

```rust
#[tokio::test]
async fn test_with_logging() {
    let _ = env_logger::builder()
        .is_test(true)
        .try_init();

    log::debug!("Running test");
    let result = handler(input, extra).await;
    log::debug!("Result: {:?}", result);

    assert!(result.is_ok());
}
```

### Run Single Test with Output

```bash
cargo test test_weather_handler -- --nocapture --test-threads=1
```

### Use `dbg!()` Macro

```rust
#[tokio::test]
async fn test_debug() {
    let input = dbg!(create_input());
    let result = dbg!(handler(input, extra).await);
    assert!(result.is_ok());
}
```

## Quality Metrics

### Test Quality Checklist

- [ ] ≥80% code coverage
- [ ] All tools have unit tests
- [ ] All error paths tested
- [ ] Integration scenarios exist
- [ ] Property tests for complex logic
- [ ] No flaky tests (run 10x to verify)
- [ ] Tests run in <10 seconds (unit tests)
- [ ] Clear test names
- [ ] Tests are independent (can run in any order)
- [ ] No hardcoded dependencies (use mocks)

### Signs of Good Tests

✅ **Green flags**:
- Tests fail when code is broken
- Tests pass when code is correct
- Tests run fast (<1s for unit tests)
- Tests are easy to understand
- Tests don't depend on external services
- Tests can run in parallel

❌ **Red flags**:
- Flaky tests (pass/fail randomly)
- Slow tests (>5s for unit test)
- Tests that depend on order
- Tests that require manual setup
- Tests that ignore errors
- Tests with unclear purpose

## Resources

- **mcp-tester docs**: https://github.com/paiml/rust-mcp-sdk/tree/main/mcp-tester
- **cargo-pmcp testing**: https://github.com/paiml/rust-mcp-sdk/tree/main/cargo-pmcp#testing
- **Proptest guide**: https://proptest-rs.github.io/proptest/
- **Tokio testing**: https://tokio.rs/tokio/topics/testing

---

**Remember**: Testing is not extra work - it's how we build quality in. Every tool you implement should have tests before you call it "done".
