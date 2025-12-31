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
│  1. Craft JSON-RPC request manually                                 │
│     {                                                               │
│       "jsonrpc": "2.0",                                             │
│       "id": 1,                                                      │
│       "method": "tools/call",                                       │
│       "params": { "name": "query", "arguments": { ... } }           │
│     }                                                               │
│                                                                     │
│  2. Send via curl or Inspector                                      │
│     curl -X POST ... -d '...'                                       │
│                                                                     │
│  3. Manually verify response                                        │
│     - Check JSON structure                                          │
│     - Verify expected values                                        │
│     - Test error cases... repeat for each                           │
│                                                                     │
│  4. Repeat for every tool × every input combination                 │
│     🔁 Tedious, error-prone, not repeatable                         │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### The mcp-tester Solution

```
┌─────────────────────────────────────────────────────────────────────┐
│                    mcp-tester Automation                            │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  1. Generate scenarios from schema                                  │
│     cargo pmcp test generate                                        │
│     → Creates YAML test files automatically                         │
│                                                                     │
│  2. Edit scenarios (optional)                                       │
│     → Add custom edge cases                                         │
│     → Tune assertions                                               │
│                                                                     │
│  3. Run tests automatically                                         │
│     cargo pmcp test run                                             │
│     → Executes all scenarios                                        │
│     → Reports pass/fail with details                                │
│                                                                     │
│  4. Integrate in CI/CD                                              │
│     → JUnit output for CI systems                                   │
│     → Fail builds on test failures                                  │
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

Assertions are how you tell mcp-tester what to verify about the response. The right assertion type depends on how strict you need to be and what you're trying to prove.

**Choosing the right assertion:**
- **Exact match** when you need to verify the complete response (simple values, critical fields)
- **Partial match** when you only care about specific fields (response may include extra data)
- **Type checking** when the structure matters but values vary (IDs, timestamps)
- **Regex matching** when values follow a pattern (UUIDs, dates, formatted strings)
- **Numeric comparisons** when values should fall within a range (counts, scores)

### Exact Match

Use exact match when you need to verify the complete response or when specific values are critical. Be cautious with exact matching on complex objects—if the server adds a new field, the test breaks.

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

The most commonly used assertion. Use it when you want to verify specific fields exist with correct values, but you don't care about other fields in the response. This makes tests more resilient to API evolution—adding new fields won't break existing tests.

```yaml
expect:
  contains:
    status: "success"                       # Object must contain this
    # Other fields are ignored
```

### Type Checking

Use type checking when the structure matters more than specific values. This is ideal for fields that vary by call (like auto-generated IDs or timestamps) where you can't predict the exact value but know it should be a string, number, etc.

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

Use regex when values follow a predictable pattern but aren't exact. Common uses: UUIDs, timestamps, formatted IDs, or messages with dynamic content. Regex assertions prove the format is correct without knowing the specific value.

```yaml
expect:
  matches:
    id: "^[a-f0-9]{8}-[a-f0-9]{4}-4[a-f0-9]{3}-[89ab][a-f0-9]{3}-[a-f0-9]{12}$"  # UUID v4
    timestamp: "\\d{4}-\\d{2}-\\d{2}T\\d{2}:\\d{2}:\\d{2}"  # ISO datetime
    message: "Created (user|customer) \\d+"
```

### Numeric Comparisons

Use comparisons when you need to verify values fall within acceptable ranges rather than matching exact numbers. This is essential for counts (should be at least 1), scores (should be between 0-100), or any value where the exact number varies but should stay within bounds.

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

Use array assertions when working with collections. You often can't predict exact array contents, but you can verify: length constraints (pagination working?), presence of specific elements (admin user exists?), or that all elements meet certain criteria (all users have required fields?).

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

Error assertions verify that your server fails correctly. This is just as important as success testing—you need to prove that invalid input produces helpful errors, not crashes or security vulnerabilities.

**Levels of strictness:**
- `error: true` — just verify it fails (any error is acceptable)
- `error.code` — verify the JSON-RPC error code (for programmatic handling)
- `error.message` — verify the exact message (for user-facing errors)
- `error.message_contains` — verify the message includes key information

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

Testing isn't just about verifying your code works—it's about systematically proving your server handles all the situations it will encounter in production. Each test category targets a different dimension of quality. Think of them as layers of protection: happy path tests prove your server does what it should, error tests prove it fails gracefully, edge case tests prove it handles unusual inputs, and security tests prove it can't be exploited.

### Happy Path Tests

**What they test:** The normal, expected usage patterns—what happens when users use your tool correctly.

**Why they matter:** These tests form your baseline. If happy path tests fail, your server's core functionality is broken. They're also your documentation: anyone reading these tests can understand how your tool is supposed to work.

**What to include:**
- The most common use case (the one 80% of users will hit)
- Variations with different valid input combinations
- Empty results (a valid query that returns nothing is still a success)

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

**What they test:** How your server responds when given bad input or when something goes wrong.

**Why they matter:** In production, users *will* send invalid inputs—sometimes accidentally, sometimes deliberately. AI assistants may construct malformed requests. Error handling tests ensure your server:
1. Rejects invalid input clearly (not with cryptic crashes)
2. Returns helpful error messages that explain what went wrong
3. Uses appropriate error codes so clients can handle failures programmatically

**What to include:**
- Missing required fields
- Invalid field values (wrong type, out of range)
- Forbidden operations (like DROP TABLE in a read-only query tool)
- Malformed input that might cause parsing errors

**The key insight:** A good error message helps users fix their request. `"Query cannot be empty"` is actionable; `"Internal server error"` is not.

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

**What they test:** The boundary conditions and unusual-but-valid inputs at the extremes of what your tool accepts.

**Why they matter:** Bugs often hide at boundaries. If your limit is 1000, what happens at 999, 1000, and 1001? If you accept strings, what about empty strings, very long strings, or Unicode? Edge cases catch the "off-by-one errors" and "I didn't think about that" bugs before users find them.

**What to include:**
- Boundary values (minimum, maximum, just above/below limits)
- Empty inputs (empty string, empty array, null where allowed)
- Unicode and special characters
- Very large or very small values
- Unusual but valid combinations

**The mental model:** Imagine the valid input space as a rectangle. Happy path tests hit the middle; edge case tests probe the corners and edges where implementations often break.

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

**What they test:** Whether your server can be tricked into doing something dangerous through malicious input.

**Why they matter:** MCP servers often have access to databases, file systems, APIs, and other sensitive resources. An attacker who can exploit your server gains access to everything your server can access. Unlike other bugs that cause inconvenience, security bugs can cause data breaches, data loss, or system compromise.

**Common attack patterns to test:**
- **SQL Injection:** Can an attacker embed SQL commands in input fields?
- **Command Injection:** Can input escape to the shell?
- **Path Traversal:** Can `../../../etc/passwd` access files outside allowed directories?
- **Authorization Bypass:** Can users access data they shouldn't?

**The testing mindset:** Think adversarially. What would a malicious user try? What would happen if your tool was called by a compromised AI assistant?

**Important:** Security tests should be tagged (see `tags:` below) so you can run them separately and ensure they never regress.

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

**What they test:** Whether your server responds within acceptable time limits.

**Why they matter:** MCP servers are called by AI assistants that are interacting with users in real-time. If your tool takes 30 seconds to respond, the user experience suffers. Performance tests catch regressions early—that "small" code change that accidentally made queries 10x slower.

**What to include:**
- Simple operations (should be fast—under 100ms)
- Complex operations (acceptable latency—1-5 seconds)
- Timeout boundaries (verify the server doesn't hang indefinitely)

**Key considerations:**
- Set realistic thresholds based on what your users expect
- Performance can vary by environment (CI machines are often slower)
- Consider running performance tests separately from functional tests
- Track performance trends over time, not just pass/fail

**The timeout assertion:** Using `timeout: 100ms` doesn't just test speed—it proves your server will fail fast rather than hang when something goes wrong.

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

Single-tool tests verify individual operations work correctly. But real-world usage involves sequences of operations: create an item, update it, query it, delete it. Multi-step workflow tests verify that operations work correctly *in combination*—that the data from one step is correctly usable in the next.

**Why workflows matter:**
- They test the actual user journeys, not just isolated operations
- They catch state-related bugs (e.g., created record has wrong ID format)
- They verify that your API is coherent (create returns what get expects)
- They document real-world usage patterns

**Variable capture** is the key feature: `capture` extracts values from one step's response so you can use them in later steps. This mirrors how real users work—they create something, get back an ID, and use that ID for subsequent operations.

### CRUD Workflow

The most common workflow pattern tests the full lifecycle of a resource: **C**reate, **R**ead, **U**pdate, **D**elete. This is the minimum viable workflow test for any tool that manages persistent data.

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

Sometimes workflows need to branch based on runtime conditions—testing different paths depending on server state or configuration. Conditional steps let you write tests that adapt to the actual server response rather than assuming a fixed state.

**Use cases:**
- Testing feature flag behavior (if flag enabled, test new behavior; otherwise, test legacy)
- Handling optional features (if server supports X, test X)
- Testing different authorization levels

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

Tests are only valuable if they run consistently. Running mcp-tester in your CI/CD pipeline ensures every code change is verified before merge—catching bugs before they reach production.

**Key integration patterns:**
1. **Run on every PR** — catch issues before they're merged
2. **Use JUnit output** — integrates with standard CI reporting tools
3. **Fail the build** — don't allow merging if tests fail
4. **Archive results** — keep test output for debugging failed runs

The examples below show complete, copy-paste-ready configurations for common CI systems.

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
│  → Server not running or wrong port                                 │
│  → Check: curl http://localhost:3000/health                         │ 
│                                                                     │
│  "Expected X but got Y"                                             │
│  → Response format changed                                          │
│  → Check: cargo pmcp test run --verbose                             │
│                                                                     │
│  "Timeout exceeded"                                                 │
│  → Server too slow or hung                                          │
│  → Increase timeout or check server logs                            │
│                                                                     │
│  "Invalid JSON-RPC response"                                        │
│  → Server returning non-JSON or malformed response                  │
│  → Check server implementation                                      │
│                                                                     │
│  "Capture failed: path not found"                                   │
│  → JSONPath doesn't match response structure                        │
│  → Use --verbose to see actual response                             │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

## Best Practices

Good test suites are maintainable, reliable, and trustworthy. These practices help you avoid common pitfalls that make tests fragile, slow, or confusing.

### Scenario Organization

Keep your test files organized so you can find what you need. A well-organized test directory tells a story: what's generated vs. custom, what's for regression vs. exploration.

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

Tests should be self-contained—each scenario should set up its own data and clean up after itself. When tests depend on each other (or on pre-existing data), they become order-dependent and fragile. One failing test can cascade into many false failures.

**The rule:** A test that passes when run alone should pass when run with other tests. A test that fails should fail for one reason: the code under test is broken.

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

A test that only checks `success: true` proves very little—the server could return completely wrong data and the test would still pass. Good assertions verify the *behavior* you care about: the right data was returned, in the right structure, with the right values.

**Ask yourself:** "If this assertion passes but the code is broken, would I notice?" If the answer is no, add more specific assertions.

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

## Practice Ideas

These informal exercises help reinforce the concepts. For structured exercises with starter code and tests, see the chapter exercise pages.

1. **Generate and review**: Generate tests for an existing server and review what edge cases it creates
2. **Write security tests**: Create a security-focused scenario file for SQL injection prevention
3. **Build a workflow**: Create a multi-step CRUD workflow with variable capture
4. **CI integration**: Set up GitHub Actions to run mcp-tester on every PR

---

*Continue to [Schema-Driven Test Generation](./ch11-03-schema-tests.md) →*
