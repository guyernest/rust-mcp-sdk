# Local Testing

Testing is what separates professional MCP servers from demos. This chapter covers local testing strategies using MCP Inspector and mcp-tester.

## The Testing Problem

MCP servers are challenging to test because:
- They communicate via JSON-RPC over HTTP/WebSocket
- Tools have complex input schemas
- Responses are structured but flexible
- Errors must be properly formatted
- Integration with AI clients is the real goal

Manual testing with copy-paste JSON is tedious and error-prone.

## MCP Inspector: Interactive Testing

MCP Inspector is the official debugging tool for MCP servers.

### Starting Inspector

```bash
# Connect to a local server
npx @anthropic-ai/mcp-inspector http://localhost:3000

# With SSE transport
npx @anthropic-ai/mcp-inspector --transport sse http://localhost:3000/sse
```

This opens a web UI at `http://localhost:5173` (or similar).

### Inspector Features

**Server Info Tab**
- Server name and version
- Capabilities (tools, resources, prompts)
- Protocol version

**Tools Tab**
- List of all registered tools
- Input schema visualization
- Interactive tool execution
- Response display

**Resources Tab**
- Available resources and templates
- Resource content preview
- URI pattern testing

**Messages Tab**
- Raw JSON-RPC message log
- Request/response pairs
- Error messages

### When to Use Inspector

✅ Good for:
- Initial development and debugging
- Schema exploration
- One-off testing
- Demonstrating functionality

❌ Not good for:
- Automated testing
- CI/CD pipelines
- Regression testing
- Load testing

## mcp-tester: Automated Testing

mcp-tester is part of cargo-pmcp and enables automated, reproducible testing.

### Basic Usage

```bash
# Run tests against a local server
cargo pmcp test run --server calculator --transport http

# Generate test scenarios from schema
cargo pmcp test generate --server calculator
```

### Test Scenario Format

Test scenarios are YAML files:

```yaml
# tests/calculator/add_numbers.yaml
name: "Add positive numbers"
description: "Verify addition of two positive numbers"

steps:
  - tool: add
    input:
      a: 10
      b: 5
    expect:
      result: 15
      description: "10 + 5 = 15"

  - tool: add
    input:
      a: 0
      b: 0
    expect:
      result: 0
```

### Schema-Driven Test Generation

The killer feature of mcp-tester is automatic test generation:

```bash
cargo pmcp test generate --server db-explorer --output tests/db-explorer/
```

This generates:
- **Happy path tests**: Valid inputs for each tool
- **Edge case tests**: Boundary values, empty inputs
- **Error tests**: Invalid inputs that should fail
- **Type tests**: Wrong types, missing required fields

### Example Generated Tests

For a `query` tool with this schema:

```json
{
  "type": "object",
  "properties": {
    "query": { "type": "string", "pattern": "^SELECT" },
    "limit": { "type": "integer", "minimum": 1, "maximum": 1000 }
  },
  "required": ["query"]
}
```

mcp-tester generates:

```yaml
# generated/query_valid.yaml
name: "query - valid inputs"
steps:
  - tool: query
    input:
      query: "SELECT * FROM users"
      limit: 100
    expect:
      success: true

# generated/query_invalid.yaml
name: "query - invalid inputs"
steps:
  - tool: query
    input:
      query: "DROP TABLE users"
    expect:
      error:
        code: -32602  # Invalid params

  - tool: query
    input:
      query: "SELECT * FROM users"
      limit: 0  # Below minimum
    expect:
      error:
        code: -32602

  - tool: query
    input:
      limit: 100  # Missing required 'query'
    expect:
      error:
        code: -32602
```

### Running Test Suites

```bash
# Run all tests for a server
cargo pmcp test run --server calculator

# Run specific test file
cargo pmcp test run --server calculator --scenario tests/calculator/add_numbers.yaml

# Verbose output
cargo pmcp test run --server calculator --verbose

# JSON output for CI
cargo pmcp test run --server calculator --format json
```

### Test Output

```
Running tests for calculator server...

✓ add_numbers (3 steps)
✓ subtract_numbers (2 steps)
✓ divide_by_zero_error (1 step)
✗ multiply_large_numbers (1 step)
  └── Expected result: 1000000000000
      Actual result:   999999999999.9999

3 passed, 1 failed
```

## Writing Effective Tests

### Test Categories

**1. Happy Path Tests**
Normal usage with valid inputs:

```yaml
name: "Query customers successfully"
steps:
  - tool: query
    input:
      query: "SELECT name, email FROM customers LIMIT 5"
    expect:
      row_count: 5
      truncated: false
```

**2. Error Handling Tests**
Verify proper error responses:

```yaml
name: "Query rejects dangerous SQL"
steps:
  - tool: query
    input:
      query: "DROP TABLE customers"
    expect:
      error:
        message: "Only SELECT queries are allowed"
```

**3. Edge Case Tests**
Boundary conditions and unusual inputs:

```yaml
name: "Query handles empty results"
steps:
  - tool: query
    input:
      query: "SELECT * FROM customers WHERE 1=0"
    expect:
      row_count: 0
      rows: []
```

**4. Integration Tests**
Multi-step workflows:

```yaml
name: "Create and retrieve customer"
steps:
  - tool: create_customer
    input:
      name: "Test Corp"
      email: "test@example.com"
    capture:
      customer_id: "$.id"

  - tool: get_customer
    input:
      id: "${customer_id}"
    expect:
      name: "Test Corp"
```

### Assertions

mcp-tester supports various assertion types:

```yaml
# Exact match
expect:
  result: 42

# Partial match (object contains these fields)
expect:
  contains:
    status: "success"

# Type checking
expect:
  type:
    result: number
    items: array

# Regex matching
expect:
  matches:
    message: "Created customer \\d+"

# Comparisons
expect:
  row_count:
    gte: 1
    lte: 100
```

## Property-Based Testing

For complex tools, consider property-based tests:

```rust
#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn add_is_commutative(a: f64, b: f64) {
            let result1 = add(a, b);
            let result2 = add(b, a);
            prop_assert_eq!(result1, result2);
        }

        #[test]
        fn query_always_respects_limit(limit in 1..1000i32) {
            let result = query("SELECT * FROM big_table", limit);
            prop_assert!(result.rows.len() <= limit as usize);
        }
    }
}
```

## Continuous Testing During Development

Set up automatic testing:

```bash
# Watch mode - rerun tests on file changes
cargo watch -x "pmcp test run --server calculator"

# Or use cargo-watch with specific files
cargo watch -w src/ -x "test" -x "pmcp test run --server calculator"
```

## Test Coverage

Track which tools and code paths are tested:

```bash
# Generate coverage report
cargo pmcp test run --server calculator --coverage

# Output
Tool Coverage:
  add:       100% (5 scenarios)
  subtract:  100% (3 scenarios)
  multiply:   80% (4 scenarios, missing: negative numbers)
  divide:    100% (6 scenarios, including error cases)

Overall: 95% tool coverage
```

## Exercises

1. **Generate tests for db-explorer**: Use `cargo pmcp test generate` and review the output

2. **Add custom scenarios**: Write tests for edge cases the generator missed

3. **Test error messages**: Verify error messages are helpful, not just error codes

4. **Test performance**: Add scenarios that measure response time

---

*Continue to [MCP Inspector Deep Dive](./ch11-01-inspector.md) →*
