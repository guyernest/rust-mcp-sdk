::: exercise
id: ch11-02-test-scenarios
difficulty: intermediate
time: 45 minutes
:::

Write comprehensive test scenarios for mcp-tester. This bridges the gap
between manual Inspector testing and automated CI/CD testing.

::: objectives
thinking:
  - How to categorize tests (valid, invalid, edge cases)
  - The trade-off between strict and flexible assertions
  - What makes a test maintainable vs brittle
doing:
  - Generate baseline tests from server schema
  - Write custom scenarios for edge cases
  - Add assertions for response validation
  - Organize tests by category
:::

::: discussion
- What edge cases did you discover during Inspector testing?
- How much of your testing is currently manual vs automated?
- What bugs have you caught manually that automated tests could catch?
:::

## Step 1: Generate Baseline Tests

```bash
# Start your server first
cargo run --release &

# Generate test scenarios from schema
cargo pmcp test generate \
  --server http://localhost:3000 \
  --output tests/scenarios/
```

## Step 2: Review Generated Tests

Examine the generated YAML files:

```yaml
# tests/scenarios/list_tables_valid.yaml
name: "list_tables - Valid call"
description: "Auto-generated test for list_tables tool"

steps:
  - tool: list_tables
    input: {}
    expect:
      success: true
```

## Step 3: Write Edge Case Tests

Create a file `tests/scenarios/query_edge_cases.yaml`:

```yaml
name: "Query - Edge cases"
description: "Custom tests for query edge cases"
tags:
  - edge-case
  - query

steps:
  - name: "Empty result set"
    tool: execute_query
    input:
      sql: "SELECT * FROM users WHERE id = -999999"
    expect:
      success: true
      content:
        contains: "0 rows"

  - name: "SQL injection attempt blocked"
    tool: execute_query
    input:
      sql: "SELECT * FROM users; DROP TABLE users; --"
    expect:
      error:
        message_contains: "Only SELECT"

  - name: "Unicode in values"
    tool: execute_query
    input:
      sql: "SELECT * FROM messages WHERE content LIKE '%emoji%'"
    expect:
      success: true
```

## Step 4: Add Invalid Input Tests

Create `tests/scenarios/query_invalid.yaml`:

```yaml
name: "Query - Invalid inputs"
description: "Verify proper error handling"
tags:
  - error-handling
  - security

steps:
  - name: "Missing required field"
    tool: execute_query
    input: {}
    expect:
      error:
        code: -32602

  - name: "Empty SQL string"
    tool: execute_query
    input:
      sql: ""
    expect:
      error:
        message_contains: "empty"

  - name: "Non-SELECT statement"
    tool: execute_query
    input:
      sql: "DROP TABLE users"
    expect:
      error:
        message_contains: "Only SELECT"
```

## Step 5: Run Tests

```bash
# Run all tests
cargo pmcp test run --server http://localhost:3000

# Run specific category
cargo pmcp test run \
  --server http://localhost:3000 \
  --tag edge-case

# Output JUnit for CI
cargo pmcp test run \
  --server http://localhost:3000 \
  --format junit \
  --output test-results.xml
```

::: hints
level_1: "Use 'contains' assertions instead of exact matches - they're more maintainable."
level_2: "Organize tests by: tool_name_valid.yaml, tool_name_invalid.yaml, tool_name_edge.yaml."
level_3: "For security tests, verify that errors don't leak internal details like stack traces."
:::

## Success Criteria

- [ ] Generated baseline tests for all server tools
- [ ] Wrote at least 3 custom edge case scenarios
- [ ] Added response assertions to key tests
- [ ] Tests organized in logical file structure
- [ ] All tests pass against local server
- [ ] Can explain what each test verifies

---

*Next: [CI/CD Pipeline](../../part4-testing/ch12-remote-testing.md) for automated test integration.*
