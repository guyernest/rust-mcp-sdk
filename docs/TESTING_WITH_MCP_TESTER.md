# Testing with MCP Tester

The MCP SDK now includes comprehensive integration with the MCP Tester tool, providing automated testing capabilities that replace manual curl commands and ad-hoc testing scripts.

## Why Use MCP Tester?

### Problems with Manual Testing
- **Forgotten headers**: Easy to forget `mcp-protocol-version`, `Accept`, session headers
- **Manual JSON parsing**: Repetitive jq commands and error-prone manual validation
- **No protocol compliance**: Manual testing doesn't verify MCP protocol compliance
- **Inconsistent testing**: Different approaches across different examples
- **No regression testing**: Hard to ensure examples keep working

### Benefits of MCP Tester Integration
- **Automated validation**: Protocol compliance, schema validation, capability checking
- **Consistent approach**: Same testing framework for all examples
- **CI/CD ready**: Automated testing in GitHub Actions
- **Scenario-based testing**: Define complex test workflows
- **Schema validation**: Automatic validation of tool schemas with warnings

## Quick Start

### 1. Build the MCP Tester
```bash
make build-tester
```

### 2. Test All Examples
```bash
make test-with-tester
```

### 3. Test Specific Example
```bash
make test-example-server EXAMPLE=t04_streamable_http_stateful
```

### 4. Generate Test Scenario
```bash
# Start your server first
cargo run --example t04_streamable_http_stateful --features streamable-http &

# Generate scenario
make generate-test-scenario URL=http://localhost:8080

# Stop server
kill %1
```

## Testing Workflows

### Development Workflow

When developing a new MCP server or example:

1. **Start your server**:
   ```bash
   cargo run --example my_server --features streamable-http
   ```

2. **Generate initial scenario**:
   ```bash
   ./target/release/examples/26-server-tester generate-scenario \
     http://localhost:8080 \
     -o my_test.yaml \
     --all-tools \
     --with-resources \
     --with-prompts
   ```

3. **Edit the scenario** to add specific test values and assertions

4. **Run the test**:
   ```bash
   ./target/release/examples/26-server-tester scenario \
     http://localhost:8080 \
     my_test.yaml \
     --detailed
   ```

### CI/CD Workflow

The MCP Tester is integrated into GitHub Actions:

```yaml
# Automatically runs on PR and push to main
# See .github/workflows/mcp-tester-validation.yml
```

Tests run automatically for:
- All example servers
- Schema validation
- Protocol compliance
- Tool functionality

## Test Scenarios

### Scenario Structure

Test scenarios are YAML files that define:
- Test steps to execute
- Expected results
- Assertions to validate

Example scenario:
```yaml
name: My Server Test
description: Validates my server implementation
timeout: 30
stop_on_failure: false

variables:
  test_value: "hello"

steps:
  - name: List tools
    operation:
      type: list_tools
    assertions:
      - type: success
      - type: exists
        path: tools[?(@.name=='my_tool')].description

  - name: Call tool
    operation:
      type: tool_call
      tool: my_tool
      arguments:
        input: "{{test_value}}"
    assertions:
      - type: success
      - type: equals
        path: result
        value: "processed: hello"
```

### Available Operations

- `list_tools` - List available tools
- `tool_call` - Call a specific tool
- `list_resources` - List available resources
- `read_resource` - Read a specific resource
- `list_prompts` - List available prompts
- `get_prompt` - Get a specific prompt

### Available Assertions

- `success` - Operation succeeded
- `error` - Operation failed (for negative testing)
- `equals` - Exact value match
- `contains` - String contains substring
- `exists` - Path exists in response
- `array_length` - Array has specific length
- `range` - Numeric value in range

## Programmatic Testing

Use MCP Tester in unit tests:

```rust
#[tokio::test]
async fn test_my_server() {
    // Start server
    let server = create_my_server();
    let handle = start_server(server, "127.0.0.1:0").await;

    // Create tester
    let mut tester = ServerTester::new("http://127.0.0.1:8080");

    // Initialize
    let init_result = tester.test_initialize().await;
    assert_eq!(init_result.status, TestStatus::Passed);

    // Test tools
    let tools_result = tester.test_tools_list().await;
    assert_eq!(tools_result.status, TestStatus::Passed);

    // Validate schemas
    let tools = tester.get_tools().unwrap();
    for tool in tools {
        let warnings = tester.validate_tool_schema(&tool);
        assert!(warnings.is_empty(), "Tool {} has schema warnings", tool.name);
    }
}
```

## Best Practices

### 1. Always Create Scenarios for Examples
Every example should have a corresponding test scenario in `examples/scenarios/`.

### 2. Validate Tool Schemas
Ensure all tools have complete schemas with descriptions:
```rust
let tool = SimpleTool::new("my_tool", handler)
    .with_description("Clear description of what this tool does")
    .with_schema(json!({
        "type": "object",
        "properties": {
            "input": {
                "type": "string",
                "description": "The input to process"
            }
        },
        "required": ["input"]
    }));
```

### 3. Test Error Cases
Include negative tests in scenarios:
```yaml
- name: Test invalid input
  operation:
    type: tool_call
    tool: my_tool
    arguments:
      invalid_field: "value"
  continue_on_failure: true
  assertions:
    - type: error
    - type: contains
      path: error.message
      value: "validation"
```

### 4. Use Variables for Reusability
```yaml
variables:
  test_id: "test_123"
  expected_result: "success"

steps:
  - name: Test with variable
    operation:
      type: tool_call
      tool: process
      arguments:
        id: "{{test_id}}"
    assertions:
      - type: equals
        path: status
        value: "{{expected_result}}"
```

### 5. Store Results for Chaining
```yaml
- name: Create resource
  operation:
    type: tool_call
    tool: create_resource
    arguments:
      name: "test"
  store_result: created_resource

- name: Use created resource
  operation:
    type: read_resource
    uri: "{{created_resource.uri}}"
  assertions:
    - type: success
```

## Integration with Development Tools

### VS Code Integration
```json
// .vscode/tasks.json
{
  "tasks": [
    {
      "label": "Test with MCP Tester",
      "type": "shell",
      "command": "make test-example-server EXAMPLE=${input:example}",
      "problemMatcher": []
    }
  ]
}
```

### Pre-commit Hook
```bash
#!/bin/sh
# .git/hooks/pre-commit
if [ -f "Makefile" ]; then
  make test-with-tester || exit 1
fi
```

## Troubleshooting

### Server Not Starting
- Check port availability: `lsof -i :8080`
- Increase startup delay in test scripts
- Check server logs for errors

### Session Management Issues
- Stateful servers require session headers
- Use `--stateless` flag for stateless servers
- Check `x-mcp-session-id` header in responses

### Schema Validation Warnings
- Run with `--verbose` to see detailed warnings
- Ensure all tools have descriptions
- Check for missing `properties` in object schemas

### Timeout Issues
- Increase timeout: `--timeout 60`
- Check for blocking operations in handlers
- Verify async handlers are properly implemented

## Examples

### Complete Testing Example
```bash
# 1. Start the example server
cargo run --example t04_streamable_http_stateful --features streamable-http &
SERVER_PID=$!

# 2. Wait for startup
sleep 2

# 3. Run quick test
./target/release/examples/26-server-tester quick http://localhost:8080

# 4. Run compliance test
./target/release/examples/26-server-tester compliance http://localhost:8080

# 5. List and validate tools
./target/release/examples/26-server-tester tools http://localhost:8080 --verbose

# 6. Run scenario test
./target/release/examples/26-server-tester scenario \
  http://localhost:8080 \
  examples/scenarios/22_http_stateful_test.yaml \
  --detailed

# 7. Stop server
kill $SERVER_PID
```

## Contributing

When adding new examples or features:
1. Create a test scenario in `examples/scenarios/`
2. Add the example to the test matrix in `.github/workflows/mcp-tester-validation.yml`
3. Update `scripts/test_examples_with_tester.sh`
4. Run `make test-with-tester` to verify

## Summary

The MCP Tester integration provides a robust, automated testing framework that:
- Replaces error-prone manual testing
- Ensures protocol compliance
- Validates schemas and capabilities
- Enables CI/CD automation
- Improves developer experience

Use it consistently across all MCP server development for better quality and maintainability.