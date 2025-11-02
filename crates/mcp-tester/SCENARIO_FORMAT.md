# MCP Server Tester - Test Scenario Format Documentation

## Overview

The MCP Server Tester supports defining test scenarios in YAML or JSON format. These scenarios allow you to:
- Define sequences of MCP operations to test
- Set up test data and variables
- Make assertions about server responses
- Create reproducible test suites for any MCP server

## Running Test Scenarios

```bash
# Run a scenario file
mcp-tester scenario <SERVER_URL> <SCENARIO_FILE> [--detailed]

# Examples
mcp-tester scenario http://localhost:8080 scenarios/basic-test.yaml
mcp-tester scenario stdio scenarios/tool-validation.yaml --detailed
```

## Scenario File Structure

### Basic Structure (YAML)

```yaml
name: Scenario Name                    # Required: Name of the test scenario
description: What this tests           # Optional: Description
timeout: 60                           # Optional: Overall timeout in seconds (default: 60)
stop_on_failure: true                 # Optional: Stop on first failure (default: true)

variables:                            # Optional: Variables for use in the scenario
  key1: value1
  key2: value2

setup:                               # Optional: Setup steps to run first
  - name: Setup step
    operation: ...

steps:                               # Required: Main test steps
  - name: Test step
    operation: ...
    assertions: ...

cleanup:                             # Optional: Cleanup steps (always run)
  - name: Cleanup step
    operation: ...
```

### Basic Structure (JSON)

```json
{
  "name": "Scenario Name",
  "description": "What this tests",
  "timeout": 60,
  "stop_on_failure": true,
  "variables": {
    "key1": "value1",
    "key2": "value2"
  },
  "setup": [...],
  "steps": [...],
  "cleanup": [...]
}
```

## Test Steps

Each test step has the following structure:

```yaml
- name: Step name                    # Required: Name of this step
  operation:                         # Required: The operation to perform
    type: operation_type
    # ... operation-specific fields
  timeout: 30                        # Optional: Step timeout in seconds
  continue_on_failure: false         # Optional: Continue if this fails
  store_result: variable_name        # Optional: Store result in variable
  assertions:                        # Optional: List of assertions
    - type: assertion_type
      # ... assertion-specific fields
```

## Operation Types

### 1. Tool Call
Call an MCP tool with arguments:

```yaml
operation:
  type: tool_call
  tool: tool_name
  arguments:
    param1: value1
    param2: value2
```

### 2. List Tools
Get the list of available tools:

```yaml
operation:
  type: list_tools
```

### 3. List Resources
Get the list of available resources:

```yaml
operation:
  type: list_resources
```

### 4. Read Resource
Read a specific resource:

```yaml
operation:
  type: read_resource
  uri: resource://path/to/resource
```

### 5. List Prompts
Get the list of available prompts:

```yaml
operation:
  type: list_prompts
```

### 6. Get Prompt
Get a specific prompt with arguments:

```yaml
operation:
  type: get_prompt
  name: prompt_name
  arguments:
    key: value
```

### 7. Custom Request
Send a custom JSON-RPC request:

```yaml
operation:
  type: custom
  method: custom.method
  params:
    key: value
```

### 8. Wait
Wait for a specified duration:

```yaml
operation:
  type: wait
  seconds: 2.5
```

### 9. Set Variable
Set a variable for use in later steps:

```yaml
operation:
  type: set_variable
  name: my_variable
  value: some_value
```

## Assertion Types

### 1. Success/Failure
Check if the operation succeeded or failed:

```yaml
assertions:
  - type: success    # Expects no error
  - type: failure    # Expects an error
```

### 2. Equals
Check if a field equals a specific value:

```yaml
assertions:
  - type: equals
    path: result.status
    value: "active"
    ignore_case: false  # Optional: case-insensitive comparison
```

### 3. Contains
Check if a field contains a substring:

```yaml
assertions:
  - type: contains
    path: result.message
    value: "success"
    ignore_case: true   # Optional
```

### 4. Matches
Check if a field matches a regex pattern:

```yaml
assertions:
  - type: matches
    path: result.id
    pattern: "^[a-f0-9]{8}-[a-f0-9]{4}-.*"
```

### 5. Exists/Not Exists
Check if a field exists or doesn't exist:

```yaml
assertions:
  - type: exists
    path: result.data
  
  - type: not_exists
    path: result.error
```

### 6. Array Length
Check the length of an array:

```yaml
assertions:
  - type: array_length
    path: result.items
    equals: 5
    # OR
    greater_than: 3
    # OR
    less_than_or_equal: 10
    # OR
    between:
      min: 2
      max: 8
```

### 7. Numeric
Check numeric values:

```yaml
assertions:
  - type: numeric
    path: result.count
    greater_than_or_equal: 100
    # Same comparison options as array_length
```

### 8. Path Expressions (JSONPath-style)
Use simple path expressions to navigate JSON responses:

```yaml
assertions:
  - type: jsonpath
    expression: "result.items[0].id"
    expected: "abc-123"  # Optional: if omitted, only checks presence

  - type: jsonpath
    expression: "data.user.profile.email"  # Dot notation
    expected: "test@example.com"

  - type: jsonpath
    expression: "results[0]"  # Array indexing
```

**Note**: The `jsonpath` type uses **simple path expressions** with:
- Dot notation: `user.profile.name`
- Array indexing: `items[0]` or `data.users[5].email`
- Combined: `result.items[0].metadata.tags[2]`

**Not supported** (use other assertion types instead):
- `$` root notation
- `..` recursive descent
- `*` wildcards
- `@` current object
- Filter expressions: `[?(@.price < 10)]`
- Array slicing: `[0:5]`

## Variables and Substitution

Variables can be defined and used throughout the scenario:

```yaml
variables:
  user_id: "test_123"
  message: "Hello, World!"

steps:
  - name: Use variables
    operation:
      type: tool_call
      tool: send_message
      arguments:
        user: "${user_id}"        # Variable substitution
        text: "${message}"
```

Store results from operations for later use:

```yaml
steps:
  - name: Create item
    operation:
      type: tool_call
      tool: create_item
      arguments:
        name: "Test Item"
    store_result: created_item    # Store the result
  
  - name: Update item
    operation:
      type: tool_call
      tool: update_item
      arguments:
        id: "${created_item.result.id}"  # Use stored result
        status: "active"
```

## Path Syntax for Assertions

Paths use dot notation to access nested fields:

- `result` - Top-level result field
- `result.status` - Nested field
- `result.items[0]` - Array index
- `result.items[0].name` - Field in array element

## Complete Examples

### Example 1: Basic Tool Testing

```yaml
name: Basic Tool Test
description: Test basic tool functionality
timeout: 30

steps:
  - name: List tools
    operation:
      type: list_tools
    assertions:
      - type: success
      - type: array_length
        path: tools
        greater_than: 0
  
  - name: Call echo tool
    operation:
      type: tool_call
      tool: echo
      arguments:
        message: "Test message"
    assertions:
      - type: success
      - type: contains
        path: result
        value: "Test message"
```

### Example 2: Complex Workflow

```yaml
name: User Workflow Test
description: Test complete user creation and management workflow
timeout: 60

variables:
  test_email: "test@example.com"

setup:
  - name: Clean up existing test user
    operation:
      type: tool_call
      tool: delete_user
      arguments:
        email: "${test_email}"
    continue_on_failure: true

steps:
  - name: Create user
    operation:
      type: tool_call
      tool: create_user
      arguments:
        email: "${test_email}"
        name: "Test User"
    store_result: new_user
    assertions:
      - type: success
      - type: exists
        path: result.id
  
  - name: Verify user exists
    operation:
      type: tool_call
      tool: get_user
      arguments:
        id: "${new_user.result.id}"
    assertions:
      - type: equals
        path: result.email
        value: "test@example.com"
  
  - name: Update user
    operation:
      type: tool_call
      tool: update_user
      arguments:
        id: "${new_user.result.id}"
        status: "active"
    assertions:
      - type: success

cleanup:
  - name: Delete test user
    operation:
      type: tool_call
      tool: delete_user
      arguments:
        id: "${new_user.result.id}"
    continue_on_failure: true
```

### Example 3: Performance Testing

```yaml
name: Performance Test
description: Test server performance with rapid requests
timeout: 30
stop_on_failure: false

steps:
  - name: Rapid request 1
    operation:
      type: list_tools
    timeout: 2
    
  - name: Rapid request 2
    operation:
      type: list_tools
    timeout: 2
    
  - name: Large payload test
    operation:
      type: tool_call
      tool: echo
      arguments:
        message: "Very long message..."  # 1KB+ of text
    timeout: 5
    assertions:
      - type: success
```

## Best Practices

1. **Use meaningful step names** - Makes test results easier to understand
2. **Set appropriate timeouts** - Prevent tests from hanging
3. **Use setup/cleanup sections** - Ensure consistent test environment
4. **Store and reuse results** - Test data dependencies and workflows
5. **Use continue_on_failure** - For non-critical steps or when testing error handling
6. **Group related tests** - Create separate scenarios for different features
7. **Use variables** - Make scenarios reusable with different data
8. **Add descriptions** - Document what each scenario tests

## Error Handling

- Steps fail if any assertion fails
- Use `continue_on_failure: true` to continue after failures
- Use `stop_on_failure: false` at scenario level to run all steps
- Cleanup steps always run, even after failures
- Timeout errors are treated as failures

## Output

The tester provides detailed output including:
- Step-by-step execution status
- Assertion results with actual vs expected values
- Timing information for each step
- Overall pass/fail status
- JSON output format available with `--format json`
