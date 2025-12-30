# mcp-tester: Automated MCP Testing

mcp-tester is the automated testing component of cargo-pmcp, designed to make MCP server testing as natural as unit testing in Rust. It generates test scenarios from your server's schema, executes them against running servers, and provides detailed assertions for both success and error cases.

## Learning Objectives

By the end of this lesson, you will:
- Understand the mcp-tester architecture and workflow
- Generate test scenarios from MCP server schemas
- Write comprehensive scenario files with assertions
- Execute tests locally and in CI/CD pipelines
- Debug test failures effectively

## Why mcp-tester?

### The Problem with Manual MCP Testing

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Manual MCP Testing Pain                          │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  1. Craft JSON-RPC request manually                                │
│     {                                                               │
│       "jsonrpc": "2.0",                                            │
│       "id": 1,                                                     │
│       "method": "tools/call",                                      │
│       "params": { "name": "query", "arguments": { ... } }          │
│     }                                                               │
│                                                                     │
│  2. Send via curl or Inspector                                     │
│     curl -X POST ... -d '...'                                      │
│                                                                     │
│  3. Manually verify response                                       │
│     - Check JSON structure                                         │
│     - Verify expected values                                       │
│     - Test error cases... repeat for each                         │
│                                                                     │
│  4. Repeat for every tool × every input combination                │
│     🔁 Tedious, error-prone, not repeatable                       │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### The mcp-tester Solution

```
┌─────────────────────────────────────────────────────────────────────┐
│                    mcp-tester Automation                            │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  1. Generate scenarios from schema                                 │
│     cargo pmcp test generate                                        │
│     → Creates YAML test files automatically                        │
│                                                                     │
│  2. Edit scenarios (optional)                                      │
│     → Add custom edge cases                                        │
│     → Tune assertions                                              │
│                                                                     │
│  3. Run tests automatically                                        │
│     cargo pmcp test run                                             │
│     → Executes all scenarios                                       │
│     → Reports pass/fail with details                               │
│                                                                     │
│  4. Integrate in CI/CD                                             │
│     → JUnit output for CI systems                                  │
│     → Fail builds on test failures                                 │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

## Installation and Setup

mcp-tester is included with cargo-pmcp:

```bash
# Install cargo-pmcp (includes mcp-tester)
cargo install cargo-pmcp

# Verify installation
cargo pmcp test --help
```

## Core Commands

### Generating Test Scenarios

```bash
# Generate from a running server
cargo pmcp test generate --server http://localhost:3000

# Generate to specific directory
cargo pmcp test generate --server http://localhost:3000 --output tests/scenarios

# Generate with deep edge cases
cargo pmcp test generate --server http://localhost:3000 --edge-cases deep

# Generate for specific tools only
cargo pmcp test generate --server http://localhost:3000 --tools query,insert,delete

# Generate with custom naming
cargo pmcp test generate --server http://localhost:3000 --prefix db_explorer
```

### Running Tests

```bash
# Run all scenarios in default directory
cargo pmcp test run --server http://localhost:3000

# Run specific scenario file
cargo pmcp test run --server http://localhost:3000 \
  --scenario tests/scenarios/query_valid.yaml

# Run all scenarios matching a pattern
cargo pmcp test run --server http://localhost:3000 \
  --pattern "*_security_*.yaml"

# Run with verbose output
cargo pmcp test run --server http://localhost:3000 --verbose

# Stop on first failure
cargo pmcp test run --server http://localhost:3000 --fail-fast

# Output in different formats
cargo pmcp test run --server http://localhost:3000 --format json
cargo pmcp test run --server http://localhost:3000 --format junit --output results.xml
cargo pmcp test run --server http://localhost:3000 --format tap
```

## Scenario File Format

Scenarios are YAML files that describe test steps and expected outcomes.

### Basic Structure

```yaml
# tests/scenarios/calculator_add.yaml

# Metadata
name: "Calculator Add Tool"
description: "Verify the add tool performs correct arithmetic"
version: "1.0"
tags:
  - calculator
  - arithmetic
  - regression

# Server configuration (optional, can be overridden by CLI)
server:
  url: http://localhost:3000
  transport: http
  timeout: 30s

# Setup steps (run before test steps)
setup:
  - tool: reset_calculator
    input: {}

# Test steps
steps:
  - name: "Add two positive numbers"
    tool: add
    input:
      a: 10
      b: 5
    expect:
      result: 15

  - name: "Add negative numbers"
    tool: add
    input:
      a: -10
      b: -5
    expect:
      result: -15

  - name: "Add with zero"
    tool: add
    input:
      a: 42
      b: 0
    expect:
      result: 42

# Teardown steps (run after test steps, even on failure)
teardown:
  - tool: cleanup
    input: {}
```

### Complete Step Options

```yaml
steps:
  - name: "Descriptive step name"           # Required
    description: "Longer description"       # Optional

    # Tool invocation
    tool: tool_name                         # Required
    input:                                  # Tool arguments
      param1: "value1"
      param2: 123
      nested:
        key: "value"

    # Timing
    timeout: 10s                            # Step-specific timeout
    delay_before: 500ms                     # Wait before execution
    delay_after: 100ms                      # Wait after execution

    # Retry configuration
    retry:
      count: 3                              # Number of retries
      delay: 1s                             # Delay between retries
      on_error: true                        # Retry on any error

    # Expectations (assertions)
    expect:
      # Success assertions
      success: true                         # Expect success (default)
      result: <exact_value>                 # Exact match
      contains:                             # Partial match
        key: "expected_value"
      type:                                 # Type checking
        result: number
        items: array
      matches:                              # Regex matching
        message: "Created item \\d+"
      comparison:                           # Numeric comparisons
        count:
          gte: 1
          lte: 100

      # Error assertions
      error:                                # Expect an error
        code: -32602                        # JSON-RPC error code
        message: "exact message"            # Exact message match
        message_contains: "partial"         # Partial message match

    # Capture values for later steps
    capture:
      item_id: "$.result.id"                # JSONPath expression
      all_items: "$.result.items[*]"        # Array capture
```

### Variable Substitution

Captured values can be used in subsequent steps:

```yaml
steps:
  - name: "Create a customer"
    tool: create_customer
    input:
      name: "Test Corp"
      email: "test@example.com"
    capture:
      customer_id: "$.result.id"
      created_at: "$.result.created_at"

  - name: "Retrieve the customer"
    tool: get_customer
    input:
      id: "${customer_id}"                  # Use captured value
    expect:
      contains:
        id: "${customer_id}"
        name: "Test Corp"

  - name: "Update the customer"
    tool: update_customer
    input:
      id: "${customer_id}"
      name: "Updated Corp"
    expect:
      success: true

  - name: "Delete the customer"
    tool: delete_customer
    input:
      id: "${customer_id}"
    expect:
      contains:
        deleted: true
```

### Environment Variables

```yaml
# Reference environment variables
server:
  url: "${MCP_SERVER_URL:-http://localhost:3000}"

steps:
  - name: "Query with credentials"
    tool: authenticated_query
    input:
      api_key: "${API_KEY}"                 # From environment
      query: "SELECT * FROM users"
```

## Assertion Types

### Exact Match

```yaml
expect:
  result: 42                                # Number
  message: "Success"                        # String
  items: [1, 2, 3]                          # Array
  user:                                     # Object
    name: "Alice"
    age: 30
```

### Partial Match (contains)

```yaml
expect:
  contains:
    status: "success"                       # Object must contain this
    # Other fields are ignored
```

### Type Checking

```yaml
expect:
  type:
    id: string
    count: number
    items: array
    metadata: object
    active: boolean
    optional_field: "null|string"           # Nullable
```

### Regex Matching

```yaml
expect:
  matches:
    id: "^[a-f0-9]{8}-[a-f0-9]{4}-4[a-f0-9]{3}-[89ab][a-f0-9]{3}-[a-f0-9]{12}$"  # UUID v4
    timestamp: "\\d{4}-\\d{2}-\\d{2}T\\d{2}:\\d{2}:\\d{2}"  # ISO datetime
    message: "Created (user|customer) \\d+"
```

### Numeric Comparisons

```yaml
expect:
  comparison:
    count:
      gt: 0                                 # Greater than
      gte: 1                                # Greater than or equal
      lt: 100                               # Less than
      lte: 100                              # Less than or equal
      eq: 50                                # Equal
      ne: 0                                 # Not equal
    response_time_ms:
      lt: 1000                              # Performance assertion
```

### Array Assertions

```yaml
expect:
  array:
    items:
      length: 5                             # Exact length
      min_length: 1                         # Minimum length
      max_length: 100                       # Maximum length
      contains: "admin"                     # Contains element
      all_match:                            # All elements match
        type: object
        contains:
          active: true
      any_match:                            # At least one matches
        contains:
          role: "admin"
```

### Error Assertions

```yaml
# Expect specific error
expect:
  error:
    code: -32602                            # Invalid params
    message: "Missing required field: query"

# Expect any error
expect:
  error: true

# Expect error containing text
expect:
  error:
    message_contains: "not found"

# Expect error matching pattern
expect:
  error:
    message_matches: "Item \\d+ not found"
```

## Test Categories

### Happy Path Tests

```yaml
# tests/scenarios/query_happy_path.yaml
name: "Query Tool - Happy Path"
description: "Normal usage patterns that should succeed"

steps:
  - name: "Simple SELECT query"
    tool: query
    input:
      sql: "SELECT * FROM users LIMIT 5"
    expect:
      type:
        rows: array
      array:
        rows:
          max_length: 5

  - name: "Query with parameters"
    tool: query
    input:
      sql: "SELECT * FROM users WHERE status = $1"
      params: ["active"]
    expect:
      success: true

  - name: "Empty result set"
    tool: query
    input:
      sql: "SELECT * FROM users WHERE 1=0"
    expect:
      contains:
        rows: []
        row_count: 0
```

### Error Handling Tests

```yaml
# tests/scenarios/query_errors.yaml
name: "Query Tool - Error Handling"
description: "Verify proper error responses for invalid inputs"

steps:
  - name: "Reject non-SELECT query"
    tool: query
    input:
      sql: "DROP TABLE users"
    expect:
      error:
        code: -32602
        message_contains: "Only SELECT queries allowed"

  - name: "Reject empty query"
    tool: query
    input:
      sql: ""
    expect:
      error:
        message_contains: "Query cannot be empty"

  - name: "Reject SQL injection attempt"
    tool: query
    input:
      sql: "SELECT * FROM users; DROP TABLE users; --"
    expect:
      error:
        message_contains: "Invalid SQL"

  - name: "Handle invalid table"
    tool: query
    input:
      sql: "SELECT * FROM nonexistent_table"
    expect:
      error:
        message_contains: "does not exist"
```

### Edge Case Tests

```yaml
# tests/scenarios/query_edge_cases.yaml
name: "Query Tool - Edge Cases"
description: "Boundary conditions and unusual inputs"

steps:
  - name: "Maximum limit value"
    tool: query
    input:
      sql: "SELECT * FROM users"
      limit: 1000
    expect:
      success: true

  - name: "Limit at boundary (1001 should fail)"
    tool: query
    input:
      sql: "SELECT * FROM users"
      limit: 1001
    expect:
      error:
        message_contains: "Limit must be between 1 and 1000"

  - name: "Unicode in query"
    tool: query
    input:
      sql: "SELECT * FROM users WHERE name = '日本語'"
    expect:
      success: true

  - name: "Very long query"
    tool: query
    input:
      sql: "SELECT * FROM users WHERE name IN ('a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z')"
    expect:
      success: true
```

### Security Tests

```yaml
# tests/scenarios/query_security.yaml
name: "Query Tool - Security"
description: "Security-focused test cases"
tags:
  - security
  - critical

steps:
  - name: "SQL injection - comment"
    tool: query
    input:
      sql: "SELECT * FROM users WHERE id = '1' --"
    expect:
      error:
        message_contains: "Invalid SQL"

  - name: "SQL injection - UNION"
    tool: query
    input:
      sql: "SELECT * FROM users UNION SELECT * FROM passwords"
    expect:
      error:
        message_contains: "UNION not allowed"

  - name: "SQL injection - subquery"
    tool: query
    input:
      sql: "SELECT * FROM users WHERE id = (SELECT password FROM users WHERE id = 1)"
    expect:
      # Either success (if subquery allowed) or specific error
      success: true

  - name: "Path traversal in table name"
    tool: query
    input:
      sql: "SELECT * FROM '../../../etc/passwd'"
    expect:
      error: true
```

### Performance Tests

```yaml
# tests/scenarios/query_performance.yaml
name: "Query Tool - Performance"
description: "Response time assertions"
tags:
  - performance

steps:
  - name: "Simple query under 100ms"
    tool: query
    input:
      sql: "SELECT 1"
    timeout: 100ms
    expect:
      success: true

  - name: "Complex query under 5s"
    tool: query
    input:
      sql: "SELECT * FROM large_table LIMIT 1000"
    timeout: 5s
    expect:
      success: true
```

## Multi-Step Workflows

### CRUD Workflow

```yaml
# tests/scenarios/customer_crud_workflow.yaml
name: "Customer CRUD Workflow"
description: "Complete create, read, update, delete cycle"

steps:
  - name: "Create customer"
    tool: create_customer
    input:
      name: "Acme Corp"
      email: "contact@acme.com"
      tier: "enterprise"
    capture:
      customer_id: "$.result.id"
    expect:
      contains:
        name: "Acme Corp"
        tier: "enterprise"

  - name: "Read customer"
    tool: get_customer
    input:
      id: "${customer_id}"
    expect:
      contains:
        id: "${customer_id}"
        name: "Acme Corp"

  - name: "Update customer"
    tool: update_customer
    input:
      id: "${customer_id}"
      name: "Acme Corporation"
      tier: "premium"
    expect:
      contains:
        name: "Acme Corporation"
        tier: "premium"

  - name: "Verify update"
    tool: get_customer
    input:
      id: "${customer_id}"
    expect:
      contains:
        name: "Acme Corporation"

  - name: "Delete customer"
    tool: delete_customer
    input:
      id: "${customer_id}"
    expect:
      contains:
        deleted: true

  - name: "Verify deletion"
    tool: get_customer
    input:
      id: "${customer_id}"
    expect:
      error:
        message_contains: "not found"
```

### Conditional Workflows

```yaml
# tests/scenarios/conditional_workflow.yaml
name: "Conditional Processing"
description: "Workflow with conditional steps"

steps:
  - name: "Check feature flag"
    tool: get_feature_flag
    input:
      flag: "new_pricing"
    capture:
      flag_enabled: "$.result.enabled"

  - name: "Apply new pricing (if enabled)"
    condition: "${flag_enabled} == true"
    tool: calculate_price
    input:
      product_id: "prod_123"
      pricing_version: "v2"
    expect:
      success: true

  - name: "Apply legacy pricing (if disabled)"
    condition: "${flag_enabled} == false"
    tool: calculate_price
    input:
      product_id: "prod_123"
      pricing_version: "v1"
    expect:
      success: true
```

## CI/CD Integration

### GitHub Actions

```yaml
# .github/workflows/test.yml
name: MCP Server Tests

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  test:
    runs-on: ubuntu-latest

    services:
      postgres:
        image: postgres:15
        env:
          POSTGRES_PASSWORD: postgres
        options: >-
          --health-cmd pg_isready
          --health-interval 10s
          --health-timeout 5s
          --health-retries 5
        ports:
          - 5432:5432

    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-action@stable

      - name: Install cargo-pmcp
        run: cargo install cargo-pmcp

      - name: Build server
        run: cargo build --release

      - name: Start server
        run: |
          ./target/release/my-mcp-server &
          sleep 5  # Wait for startup
        env:
          DATABASE_URL: postgres://postgres:postgres@localhost/test

      - name: Run mcp-tester
        run: |
          cargo pmcp test run \
            --server http://localhost:3000 \
            --format junit \
            --output test-results.xml

      - name: Upload test results
        uses: actions/upload-artifact@v3
        if: always()
        with:
          name: test-results
          path: test-results.xml

      - name: Publish test results
        uses: dorny/test-reporter@v1
        if: always()
        with:
          name: MCP Tests
          path: test-results.xml
          reporter: java-junit
```

### GitLab CI

```yaml
# .gitlab-ci.yml
stages:
  - build
  - test

variables:
  CARGO_HOME: $CI_PROJECT_DIR/.cargo

build:
  stage: build
  image: rust:1.75
  script:
    - cargo build --release
  artifacts:
    paths:
      - target/release/my-mcp-server

test:
  stage: test
  image: rust:1.75
  services:
    - postgres:15
  variables:
    DATABASE_URL: postgres://postgres:postgres@postgres/test
  script:
    - cargo install cargo-pmcp
    - ./target/release/my-mcp-server &
    - sleep 5
    - cargo pmcp test run --server http://localhost:3000 --format junit --output results.xml
  artifacts:
    reports:
      junit: results.xml
```

### Makefile Integration

```makefile
# Makefile

.PHONY: test test-unit test-mcp test-all

# Rust unit tests
test-unit:
	cargo test

# Start server and run mcp-tester
test-mcp: build
	@echo "Starting server..."
	@./target/release/my-mcp-server &
	@sleep 3
	@echo "Running mcp-tester..."
	@cargo pmcp test run --server http://localhost:3000 || (pkill my-mcp-server; exit 1)
	@pkill my-mcp-server

# Generate new test scenarios
test-generate:
	@./target/release/my-mcp-server &
	@sleep 3
	@cargo pmcp test generate --server http://localhost:3000 --output tests/scenarios/generated/
	@pkill my-mcp-server

# Run all tests
test-all: test-unit test-mcp

# CI target
ci: build
	cargo test --all-features
	./target/release/my-mcp-server &
	sleep 3
	cargo pmcp test run --server http://localhost:3000 --format junit --output test-results.xml
	pkill my-mcp-server
```

## Debugging Test Failures

### Verbose Output

```bash
# See detailed request/response
cargo pmcp test run --verbose

# Output:
# ════════════════════════════════════════════════════════════════
# Step: Add two positive numbers
# ════════════════════════════════════════════════════════════════
# Request:
#   Tool: add
#   Input: {"a": 10, "b": 5}
#
# Response:
#   Status: Success
#   Result: {"content": [{"type": "text", "text": "15"}]}
#   Duration: 12ms
#
# Assertions:
#   ✓ result equals 15
# ────────────────────────────────────────────────────────────────
```

### Debug Mode

```bash
# Maximum verbosity with JSON-RPC traces
cargo pmcp test run --debug

# Save raw responses for analysis
cargo pmcp test run --save-responses ./debug/
```

### Common Failure Patterns

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Common Test Failures                             │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  "Connection refused"                                               │
│  → Server not running or wrong port                                │
│  → Check: curl http://localhost:3000/health                        │
│                                                                     │
│  "Expected X but got Y"                                            │
│  → Response format changed                                         │
│  → Check: cargo pmcp test run --verbose                            │
│                                                                     │
│  "Timeout exceeded"                                                │
│  → Server too slow or hung                                         │
│  → Increase timeout or check server logs                           │
│                                                                     │
│  "Invalid JSON-RPC response"                                       │
│  → Server returning non-JSON or malformed response                 │
│  → Check server implementation                                     │
│                                                                     │
│  "Capture failed: path not found"                                  │
│  → JSONPath doesn't match response structure                       │
│  → Use --verbose to see actual response                            │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

## Best Practices

### Scenario Organization

```
tests/scenarios/
├── generated/              # Auto-generated (add to .gitignore)
│   ├── query_valid.yaml
│   └── query_invalid.yaml
├── custom/                 # Hand-written tests (commit these)
│   ├── query_security.yaml
│   ├── query_edge_cases.yaml
│   └── workflow_crud.yaml
└── regression/             # Bug fix verification tests
    ├── issue_123.yaml
    └── issue_456.yaml
```

### Test Independence

```yaml
# BAD: Tests depend on each other
steps:
  - name: "Create user"
    tool: create_user
    # Later tests assume this user exists

# GOOD: Each test is self-contained
setup:
  - tool: create_test_user
    input:
      id: "test_user_1"

steps:
  - name: "Get user"
    tool: get_user
    input:
      id: "test_user_1"

teardown:
  - tool: delete_user
    input:
      id: "test_user_1"
```

### Meaningful Assertions

```yaml
# BAD: Only checks success
expect:
  success: true

# GOOD: Verifies actual behavior
expect:
  contains:
    id: "${created_id}"
    status: "active"
  type:
    created_at: string
  comparison:
    items:
      gte: 1
```

## Summary

mcp-tester provides:

1. **Schema-driven generation** - Automatic test creation from tool schemas
2. **YAML scenarios** - Human-readable, version-controllable test definitions
3. **Rich assertions** - Exact match, partial match, regex, comparisons
4. **Multi-step workflows** - Variable capture and substitution
5. **CI/CD integration** - JUnit output, fail-fast mode, automation support

Key workflow:
```bash
# Generate initial tests
cargo pmcp test generate --server http://localhost:3000

# Add custom edge cases and security tests
vim tests/scenarios/custom/security.yaml

# Run all tests
cargo pmcp test run --server http://localhost:3000

# Integrate in CI
cargo pmcp test run --format junit --output results.xml
```

## Exercises

1. **Generate and review**: Generate tests for an existing server and review what edge cases it creates
2. **Write security tests**: Create a security-focused scenario file for SQL injection prevention
3. **Build a workflow**: Create a multi-step CRUD workflow with variable capture
4. **CI integration**: Set up GitHub Actions to run mcp-tester on every PR

---

*Continue to [Schema-Driven Test Generation](./ch11-03-schema-tests.md) →*
