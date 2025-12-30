# Local Testing

Testing is what separates professional MCP servers from demos. This chapter covers comprehensive local testing strategies including Rust unit tests, MCP Inspector for interactive debugging, and mcp-tester for automated testing.

## Learning Objectives

By the end of this chapter, you will:
- Write effective unit tests for MCP tool logic
- Use MCP Inspector for interactive debugging
- Generate test scenarios from server schemas with mcp-tester
- Create comprehensive test suites covering happy paths, errors, and edge cases
- Integrate tests into your development workflow

## The Testing Pyramid for MCP Servers

```
┌─────────────────────────────────────────────────────────────────────┐
│                    MCP Testing Pyramid                              │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│                          ┌─────────┐                               │
│                         /  E2E     \         MCP Inspector         │
│                        /  Testing   \        Claude Desktop        │
│                       /─────────────\                              │
│                      /   mcp-tester  \       Scenario files        │
│                     /   Integration   \      API testing           │
│                    /───────────────────\                           │
│                   /    Rust Unit Tests  \    Tool logic            │
│                  /   Property Tests      \   Input validation      │
│                 /─────────────────────────\                        │
│                                                                     │
│  More tests at base, fewer at top                                  │
│  Base runs fastest, top runs slowest                               │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

## Rust Unit Tests

Before testing MCP protocol interactions, test your core tool logic with standard Rust tests.

### Testing Tool Logic

```rust
// src/tools/calculator.rs
pub fn add(a: f64, b: f64) -> f64 {
    a + b
}

pub fn divide(a: f64, b: f64) -> Result<f64, CalculatorError> {
    if b == 0.0 {
        return Err(CalculatorError::DivisionByZero);
    }
    Ok(a / b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_positive_numbers() {
        assert_eq!(add(2.0, 3.0), 5.0);
    }

    #[test]
    fn test_add_negative_numbers() {
        assert_eq!(add(-2.0, -3.0), -5.0);
    }

    #[test]
    fn test_add_mixed_signs() {
        assert_eq!(add(-2.0, 3.0), 1.0);
    }

    #[test]
    fn test_divide_normal() {
        assert_eq!(divide(10.0, 2.0).unwrap(), 5.0);
    }

    #[test]
    fn test_divide_by_zero() {
        assert!(matches!(
            divide(10.0, 0.0),
            Err(CalculatorError::DivisionByZero)
        ));
    }

    #[test]
    fn test_divide_zero_numerator() {
        assert_eq!(divide(0.0, 5.0).unwrap(), 0.0);
    }
}
```

### Testing Input Validation

```rust
// src/tools/query.rs
use regex::Regex;

#[derive(Debug, thiserror::Error)]
pub enum QueryError {
    #[error("Only SELECT queries are allowed")]
    NonSelectQuery,
    #[error("Limit must be between 1 and 1000, got {0}")]
    InvalidLimit(i32),
    #[error("Query cannot be empty")]
    EmptyQuery,
}

pub fn validate_query(query: &str, limit: Option<i32>) -> Result<(), QueryError> {
    if query.trim().is_empty() {
        return Err(QueryError::EmptyQuery);
    }

    let select_pattern = Regex::new(r"(?i)^\s*SELECT\b").unwrap();
    if !select_pattern.is_match(query) {
        return Err(QueryError::NonSelectQuery);
    }

    if let Some(l) = limit {
        if l < 1 || l > 1000 {
            return Err(QueryError::InvalidLimit(l));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_select_query() {
        assert!(validate_query("SELECT * FROM users", Some(100)).is_ok());
    }

    #[test]
    fn test_select_case_insensitive() {
        assert!(validate_query("select * from users", None).is_ok());
        assert!(validate_query("Select id From users", None).is_ok());
    }

    #[test]
    fn test_rejects_insert() {
        assert!(matches!(
            validate_query("INSERT INTO users VALUES (1)", None),
            Err(QueryError::NonSelectQuery)
        ));
    }

    #[test]
    fn test_rejects_drop() {
        assert!(matches!(
            validate_query("DROP TABLE users", None),
            Err(QueryError::NonSelectQuery)
        ));
    }

    #[test]
    fn test_limit_boundaries() {
        assert!(validate_query("SELECT 1", Some(1)).is_ok());
        assert!(validate_query("SELECT 1", Some(1000)).is_ok());
        assert!(matches!(
            validate_query("SELECT 1", Some(0)),
            Err(QueryError::InvalidLimit(0))
        ));
        assert!(matches!(
            validate_query("SELECT 1", Some(1001)),
            Err(QueryError::InvalidLimit(1001))
        ));
    }

    #[test]
    fn test_empty_query() {
        assert!(matches!(
            validate_query("", None),
            Err(QueryError::EmptyQuery)
        ));
        assert!(matches!(
            validate_query("   ", None),
            Err(QueryError::EmptyQuery)
        ));
    }
}
```

### Testing MCP Response Formatting

```rust
// src/mcp/response.rs
use serde_json::{json, Value};

pub fn format_tool_result(data: impl serde::Serialize) -> Value {
    json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string_pretty(&data).unwrap_or_default()
        }]
    })
}

pub fn format_error_result(message: &str) -> Value {
    json!({
        "content": [{
            "type": "text",
            "text": format!("Error: {}", message)
        }],
        "isError": true
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_tool_result_with_struct() {
        #[derive(serde::Serialize)]
        struct QueryResult {
            rows: Vec<String>,
            count: usize,
        }

        let result = QueryResult {
            rows: vec!["row1".to_string()],
            count: 1,
        };

        let formatted = format_tool_result(result);

        assert_eq!(formatted["content"][0]["type"], "text");
        let text = formatted["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("\"count\": 1"));
    }

    #[test]
    fn test_format_error_result() {
        let formatted = format_error_result("Division by zero");

        assert_eq!(formatted["isError"], true);
        let text = formatted["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("Division by zero"));
    }
}
```

### Property-Based Testing with proptest

For complex logic, property-based tests catch edge cases you might miss:

```rust
// src/tools/calculator.rs
#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn add_is_commutative(a in -1e10..1e10f64, b in -1e10..1e10f64) {
            prop_assert!((add(a, b) - add(b, a)).abs() < 1e-10);
        }

        #[test]
        fn add_zero_is_identity(a in -1e10..1e10f64) {
            prop_assert_eq!(add(a, 0.0), a);
        }

        #[test]
        fn divide_then_multiply_returns_original(
            a in -1e10..1e10f64,
            b in prop::num::f64::NORMAL.prop_filter("non-zero", |x| x.abs() > 1e-10)
        ) {
            let result = divide(a, b).unwrap();
            prop_assert!((result * b - a).abs() < 1e-6);
        }

        #[test]
        fn limit_validation_respects_bounds(limit in -100..2000i32) {
            let result = validate_query("SELECT 1", Some(limit));
            if limit >= 1 && limit <= 1000 {
                prop_assert!(result.is_ok());
            } else {
                prop_assert!(result.is_err());
            }
        }
    }
}
```

### Async Test Patterns

For database tools and async operations:

```rust
// src/tools/database.rs
#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::PgPool;

    // Use test fixtures
    async fn setup_test_db() -> PgPool {
        let pool = PgPool::connect("postgres://test:test@localhost/test_db")
            .await
            .expect("Failed to connect to test database");

        sqlx::query("CREATE TABLE IF NOT EXISTS test_users (id SERIAL, name TEXT)")
            .execute(&pool)
            .await
            .unwrap();

        pool
    }

    async fn teardown_test_db(pool: &PgPool) {
        sqlx::query("DROP TABLE IF EXISTS test_users")
            .execute(pool)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_query_returns_results() {
        let pool = setup_test_db().await;

        // Insert test data
        sqlx::query("INSERT INTO test_users (name) VALUES ('Alice'), ('Bob')")
            .execute(&pool)
            .await
            .unwrap();

        // Test the query function
        let result = execute_query(&pool, "SELECT * FROM test_users", 10).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 2);

        teardown_test_db(&pool).await;
    }

    #[tokio::test]
    async fn test_query_respects_limit() {
        let pool = setup_test_db().await;

        // Insert more data than limit
        for i in 0..20 {
            sqlx::query(&format!("INSERT INTO test_users (name) VALUES ('User{}')", i))
                .execute(&pool)
                .await
                .unwrap();
        }

        let result = execute_query(&pool, "SELECT * FROM test_users", 5).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 5);

        teardown_test_db(&pool).await;
    }
}
```

### Running Unit Tests

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific test module
cargo test tools::calculator

# Run tests matching a pattern
cargo test divide

# Run tests with coverage (requires cargo-tarpaulin)
cargo tarpaulin --out Html
```

## MCP Inspector: Interactive Testing

MCP Inspector is essential for development but not for automation. See [MCP Inspector Deep Dive](./ch11-01-inspector.md) for detailed coverage.

### Quick Start

```bash
# Install Inspector
npm install -g @anthropic/mcp-inspector

# Start your server
cargo run --release

# Connect Inspector (HTTP transport)
npx @anthropic/mcp-inspector http://localhost:3000/mcp

# Connect with SSE transport
npx @anthropic/mcp-inspector --transport sse http://localhost:3000/sse
```

### When to Use Inspector vs mcp-tester

| Task | Inspector | mcp-tester |
|------|-----------|------------|
| Debugging new tool | ✓ | |
| Exploring server capabilities | ✓ | |
| One-off manual testing | ✓ | |
| Automated test suites | | ✓ |
| CI/CD pipelines | | ✓ |
| Regression testing | | ✓ |
| Edge case coverage | | ✓ |
| Performance testing | | ✓ |

## mcp-tester: Automated Testing

mcp-tester is the core of PMCP's testing strategy. It generates test scenarios from your server's schema and executes them automatically.

### Core Workflow

```
┌─────────────────────────────────────────────────────────────────────┐
│                    mcp-tester Workflow                              │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  1. GENERATE                                                        │
│     cargo pmcp test generate                                        │
│           │                                                         │
│           ▼                                                         │
│     ┌─────────────┐     ┌─────────────┐                            │
│     │ MCP Server  │────▶│   Schema    │                            │
│     │ (running)   │     │  Introspect │                            │
│     └─────────────┘     └──────┬──────┘                            │
│                                │                                    │
│                                ▼                                    │
│     ┌──────────────────────────────────────────────────────┐       │
│     │              Generated Scenario Files                 │       │
│     │  tests/scenarios/                                     │       │
│     │  ├── tool_name_valid.yaml      (happy paths)         │       │
│     │  ├── tool_name_invalid.yaml    (error cases)         │       │
│     │  ├── tool_name_edge.yaml       (boundary values)     │       │
│     │  └── tool_name_types.yaml      (type validation)     │       │
│     └──────────────────────────────────────────────────────┘       │
│                                                                     │
│  2. EDIT (optional)                                                 │
│     Add custom scenarios, assertions, edge cases                   │
│                                                                     │
│  3. RUN                                                             │
│     cargo pmcp test run                                             │
│           │                                                         │
│           ▼                                                         │
│     ┌─────────────┐     ┌─────────────┐     ┌─────────────┐       │
│     │  Scenario   │────▶│ MCP Server  │────▶│   Assert    │       │
│     │   Files     │     │  Execute    │     │   Results   │       │
│     └─────────────┘     └─────────────┘     └─────────────┘       │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Basic Commands

```bash
# Generate test scenarios from running server
cargo pmcp test generate --server http://localhost:3000

# Run all generated tests
cargo pmcp test run --server http://localhost:3000

# Run specific scenario file
cargo pmcp test run --scenario tests/scenarios/query_valid.yaml

# Verbose output with timing
cargo pmcp test run --verbose

# JSON output for CI integration
cargo pmcp test run --format json --output results.json
```

See [mcp-tester Introduction](./ch11-02-mcp-tester.md) for comprehensive documentation.

## Schema-Driven Test Generation

The most powerful mcp-tester feature is automatic test generation from JSON Schema.

```bash
# Generate tests for all tools
cargo pmcp test generate --output tests/scenarios/

# Generate with edge case depth
cargo pmcp test generate --edge-cases deep

# Generate only for specific tools
cargo pmcp test generate --tools query,insert
```

See [Schema-Driven Test Generation](./ch11-03-schema-tests.md) for the complete guide including:
- How schema analysis works
- Generated test categories
- Customizing generated tests
- CI/CD integration

## Test Organization Best Practices

### Directory Structure

```
my-mcp-server/
├── src/
│   ├── tools/
│   │   ├── mod.rs
│   │   ├── calculator.rs      # Tool implementation
│   │   └── query.rs
│   └── lib.rs
├── tests/
│   ├── unit/                   # Rust unit tests
│   │   ├── calculator_test.rs
│   │   └── query_test.rs
│   ├── scenarios/              # mcp-tester scenarios
│   │   ├── generated/          # Auto-generated (gitignore)
│   │   │   ├── add_valid.yaml
│   │   │   └── add_invalid.yaml
│   │   └── custom/             # Hand-written tests
│   │       ├── complex_workflow.yaml
│   │       └── regression_123.yaml
│   └── integration/            # Full integration tests
│       └── client_test.rs
└── Cargo.toml
```

### Naming Conventions

```yaml
# tests/scenarios/custom/query_sql_injection_prevention.yaml
name: "Query - SQL injection prevention"
description: |
  Verify that the query tool properly rejects SQL injection attempts.
  This is a critical security test.
tags:
  - security
  - regression
  - critical

steps:
  - tool: query
    input:
      sql: "SELECT * FROM users WHERE id = '1; DROP TABLE users; --'"
    expect:
      error:
        message_contains: "Invalid SQL"
```

## Continuous Testing Workflow

```bash
# Development workflow with watch mode
cargo watch -x test -x "pmcp test run"

# Pre-commit testing
cargo test && cargo pmcp test run --fail-fast

# Full test suite before PR
cargo test --all-features && \
cargo pmcp test generate && \
cargo pmcp test run --format junit --output test-results.xml
```

## Summary

Effective MCP server testing combines:

1. **Rust Unit Tests** - Test tool logic in isolation
2. **Property Tests** - Catch edge cases with random inputs
3. **MCP Inspector** - Interactive debugging during development
4. **mcp-tester Scenarios** - Automated protocol-level testing
5. **Schema Generation** - Automatic test coverage from schemas

The key insight: most MCP bugs occur at the protocol level (wrong JSON format, missing fields, invalid responses), not in business logic. mcp-tester catches these automatically.

## Exercises

1. **Add unit tests** to an existing tool with 100% branch coverage
2. **Generate scenarios** for the db-explorer server and review them
3. **Write custom scenarios** for three edge cases the generator missed
4. **Set up watch mode** for continuous testing during development

---

*Continue to [MCP Inspector Deep Dive](./ch11-01-inspector.md) →*
