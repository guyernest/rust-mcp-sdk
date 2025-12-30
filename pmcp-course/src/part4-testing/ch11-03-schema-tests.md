# Schema-Driven Test Generation

The most powerful feature of mcp-tester is automatic test generation from your MCP server's JSON Schema definitions. This chapter explains how schema analysis works, what tests are generated, and how to customize the output for comprehensive coverage.

## Learning Objectives

By the end of this lesson, you will:
- Understand how mcp-tester analyzes tool schemas
- Generate comprehensive test suites automatically
- Customize generated tests for your specific needs
- Edit scenarios to add edge cases and assertions
- Integrate generated tests into CI/CD pipelines

## How Schema Analysis Works

### The Generation Process

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Schema Analysis Pipeline                         │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  1. INTROSPECT                                                      │
│     ┌─────────────────────────────────────────────────────────┐    │
│     │  mcp-tester connects to server                          │    │
│     │  Calls: initialize → tools/list                          │    │
│     │  Retrieves: tool names, descriptions, inputSchemas       │    │
│     └─────────────────────────────────────────────────────────┘    │
│                          │                                         │
│                          ▼                                         │
│  2. ANALYZE SCHEMA                                                  │
│     ┌─────────────────────────────────────────────────────────┐    │
│     │  For each tool's inputSchema:                           │    │
│     │  • Parse JSON Schema structure                          │    │
│     │  • Identify required vs optional properties              │    │
│     │  • Extract type constraints (string, number, etc.)      │    │
│     │  • Find validation rules (min, max, pattern, enum)      │    │
│     │  • Detect nested objects and arrays                     │    │
│     └─────────────────────────────────────────────────────────┘    │
│                          │                                         │
│                          ▼                                         │
│  3. GENERATE TEST CASES                                            │
│     ┌─────────────────────────────────────────────────────────┐    │
│     │  For each property and constraint:                      │    │
│     │  • Valid value tests (within constraints)               │    │
│     │  • Boundary value tests (min, max, at limits)           │    │
│     │  • Invalid value tests (violate constraints)            │    │
│     │  • Type violation tests (wrong types)                   │    │
│     │  • Required field tests (missing required)              │    │
│     └─────────────────────────────────────────────────────────┘    │
│                          │                                         │
│                          ▼                                         │
│  4. OUTPUT YAML FILES                                              │
│     ┌─────────────────────────────────────────────────────────┐    │
│     │  tests/scenarios/generated/                             │    │
│     │  ├── toolname_valid.yaml                                │    │
│     │  ├── toolname_invalid.yaml                              │    │
│     │  ├── toolname_edge.yaml                                 │    │
│     │  └── toolname_types.yaml                                │    │
│     └─────────────────────────────────────────────────────────┘    │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Schema Elements Analyzed

| Schema Element | Generated Tests |
|----------------|-----------------|
| `type: string` | Valid string, empty string, null |
| `type: number` | Valid number, zero, negative, float |
| `type: integer` | Valid int, float (should fail), boundaries |
| `type: boolean` | true, false, truthy strings (should fail) |
| `type: array` | Empty array, single item, multiple items |
| `type: object` | Valid object, empty object, nested |
| `required: [...]` | Missing each required field |
| `minimum/maximum` | Below min, at min, at max, above max |
| `minLength/maxLength` | Empty, at min, at max, over max |
| `pattern` | Matching, non-matching |
| `enum` | Each valid value, invalid value |
| `format` (email, uri, etc.) | Valid format, invalid format |

## Running the Generator

### Basic Generation

```bash
# Start your server
cargo run --release &

# Generate tests
cargo pmcp test generate --server http://localhost:3000

# Output:
# Connecting to server...
# Found 5 tools: query, insert, update, delete, get_schema
# Generating tests...
# ✓ query_valid.yaml (8 test steps)
# ✓ query_invalid.yaml (12 test steps)
# ✓ query_edge.yaml (6 test steps)
# ✓ query_types.yaml (4 test steps)
# ... (repeated for each tool)
# Generated 80 test scenarios in tests/scenarios/generated/
```

### Generation Options

```bash
# Specify output directory
cargo pmcp test generate \
  --server http://localhost:3000 \
  --output tests/scenarios/generated/

# Generate only for specific tools
cargo pmcp test generate \
  --server http://localhost:3000 \
  --tools query,insert

# Control edge case depth
cargo pmcp test generate \
  --server http://localhost:3000 \
  --edge-cases minimal    # Fewer edge cases
cargo pmcp test generate \
  --server http://localhost:3000 \
  --edge-cases deep       # More comprehensive

# Add prefix to generated files
cargo pmcp test generate \
  --server http://localhost:3000 \
  --prefix db_explorer

# Generate with descriptions from tool metadata
cargo pmcp test generate \
  --server http://localhost:3000 \
  --include-descriptions

# Dry run - show what would be generated
cargo pmcp test generate \
  --server http://localhost:3000 \
  --dry-run
```

## Generated Test Categories

### 1. Valid Input Tests (_valid.yaml)

Tests that should succeed with well-formed inputs:

```yaml
# Generated: query_valid.yaml
name: "query - Valid Inputs"
description: "Auto-generated tests for valid query tool inputs"
generated: true
schema_version: "2024-01-15"

steps:
  # Test with all required fields
  - name: "All required fields provided"
    tool: query
    input:
      sql: "SELECT * FROM users"
    expect:
      success: true

  # Test with optional fields
  - name: "With optional limit"
    tool: query
    input:
      sql: "SELECT * FROM users"
      limit: 100
    expect:
      success: true

  # Test each enum value
  - name: "Format: json"
    tool: query
    input:
      sql: "SELECT 1"
      format: "json"
    expect:
      success: true

  - name: "Format: csv"
    tool: query
    input:
      sql: "SELECT 1"
      format: "csv"
    expect:
      success: true
```

### 2. Invalid Input Tests (_invalid.yaml)

Tests that should fail with validation errors:

```yaml
# Generated: query_invalid.yaml
name: "query - Invalid Inputs"
description: "Auto-generated tests for invalid query tool inputs"
generated: true

steps:
  # Missing required field
  - name: "Missing required: sql"
    tool: query
    input:
      limit: 100
      # sql is missing
    expect:
      error:
        code: -32602
        message_contains: "sql"

  # Pattern violation
  - name: "Pattern violation: sql must start with SELECT"
    tool: query
    input:
      sql: "DROP TABLE users"
    expect:
      error:
        code: -32602

  # Enum violation
  - name: "Invalid enum value: format"
    tool: query
    input:
      sql: "SELECT 1"
      format: "invalid_format"
    expect:
      error:
        code: -32602
        message_contains: "format"

  # Below minimum
  - name: "Below minimum: limit"
    tool: query
    input:
      sql: "SELECT 1"
      limit: 0
    expect:
      error:
        code: -32602
        message_contains: "limit"

  # Above maximum
  - name: "Above maximum: limit"
    tool: query
    input:
      sql: "SELECT 1"
      limit: 10001
    expect:
      error:
        code: -32602
```

### 3. Edge Case Tests (_edge.yaml)

Boundary conditions and unusual but valid inputs:

```yaml
# Generated: query_edge.yaml
name: "query - Edge Cases"
description: "Auto-generated boundary and edge case tests"
generated: true

steps:
  # Boundary: at minimum
  - name: "Boundary: limit at minimum (1)"
    tool: query
    input:
      sql: "SELECT 1"
      limit: 1
    expect:
      success: true

  # Boundary: at maximum
  - name: "Boundary: limit at maximum (1000)"
    tool: query
    input:
      sql: "SELECT 1"
      limit: 1000
    expect:
      success: true

  # String length: at minLength
  - name: "String at minLength"
    tool: query
    input:
      sql: "S"  # If minLength: 1
    expect:
      success: true

  # String length: at maxLength
  - name: "String at maxLength"
    tool: query
    input:
      sql: "SELECT ... (very long)"  # At maxLength
    expect:
      success: true

  # Empty array (if minItems: 0)
  - name: "Empty array for columns"
    tool: query
    input:
      sql: "SELECT 1"
      columns: []
    expect:
      success: true

  # Array at minItems
  - name: "Array at minItems"
    tool: query
    input:
      sql: "SELECT 1"
      columns: ["id"]  # minItems: 1
    expect:
      success: true
```

### 4. Type Validation Tests (_types.yaml)

Tests that verify type constraints:

```yaml
# Generated: query_types.yaml
name: "query - Type Validation"
description: "Auto-generated type validation tests"
generated: true

steps:
  # Wrong type for string field
  - name: "Type error: sql should be string, got number"
    tool: query
    input:
      sql: 12345
    expect:
      error:
        code: -32602

  # Wrong type for number field
  - name: "Type error: limit should be integer, got string"
    tool: query
    input:
      sql: "SELECT 1"
      limit: "one hundred"
    expect:
      error:
        code: -32602

  # Wrong type for boolean field
  - name: "Type error: verbose should be boolean, got string"
    tool: query
    input:
      sql: "SELECT 1"
      verbose: "true"  # String, not boolean
    expect:
      error:
        code: -32602

  # Wrong type for array field
  - name: "Type error: columns should be array, got string"
    tool: query
    input:
      sql: "SELECT 1"
      columns: "id,name"  # String, not array
    expect:
      error:
        code: -32602

  # Null for non-nullable field
  - name: "Type error: sql cannot be null"
    tool: query
    input:
      sql: null
    expect:
      error:
        code: -32602
```

## Customizing Generated Tests

### Editing Generated Files

Generated tests are a starting point. Edit them to add:

```yaml
# tests/scenarios/generated/query_valid.yaml (edited)
name: "query - Valid Inputs"
description: "Auto-generated tests for valid query tool inputs"
generated: true
# Add: edited marker to prevent regeneration overwrite
edited: true

steps:
  # Keep generated steps...

  # ADD: Custom test for specific business logic
  - name: "Query with JOIN (business requirement)"
    tool: query
    input:
      sql: "SELECT u.name, o.total FROM users u JOIN orders o ON u.id = o.user_id"
    expect:
      success: true
      type:
        rows: array

  # ADD: Test for specific column selection
  - name: "Query specific columns"
    tool: query
    input:
      sql: "SELECT id, name, email FROM users"
      columns: ["id", "name", "email"]
    expect:
      contains:
        column_count: 3
```

### Override Files

Create override files that won't be replaced on regeneration:

```
tests/scenarios/
├── generated/              # Auto-generated
│   ├── query_valid.yaml
│   └── query_invalid.yaml
├── overrides/              # Manual overrides (higher priority)
│   └── query_valid.yaml    # Replaces generated version
└── custom/                 # Additional custom tests
    └── query_security.yaml
```

```yaml
# tests/scenarios/overrides/query_valid.yaml
name: "query - Valid Inputs (Custom)"
description: "Customized valid input tests with business-specific cases"

# Include steps from generated file
include:
  - ../generated/query_valid.yaml

# Add additional steps
steps:
  - name: "Complex business query"
    tool: query
    input:
      sql: "SELECT * FROM quarterly_reports WHERE year = 2024"
    expect:
      success: true
```

### Regeneration Strategy

```bash
# Regenerate only, don't overwrite edited files
cargo pmcp test generate \
  --server http://localhost:3000 \
  --skip-edited

# Force regenerate everything
cargo pmcp test generate \
  --server http://localhost:3000 \
  --force

# Regenerate and show diff
cargo pmcp test generate \
  --server http://localhost:3000 \
  --diff

# Merge new tests with existing
cargo pmcp test generate \
  --server http://localhost:3000 \
  --merge
```

## Advanced Schema Patterns

### Nested Object Schemas

```json
{
  "type": "object",
  "properties": {
    "user": {
      "type": "object",
      "properties": {
        "name": { "type": "string" },
        "address": {
          "type": "object",
          "properties": {
            "city": { "type": "string" },
            "zip": { "type": "string", "pattern": "^\\d{5}$" }
          },
          "required": ["city"]
        }
      },
      "required": ["name"]
    }
  },
  "required": ["user"]
}
```

Generated tests:

```yaml
steps:
  # Valid nested object
  - name: "Valid nested object"
    tool: create_user
    input:
      user:
        name: "Alice"
        address:
          city: "New York"
          zip: "10001"
    expect:
      success: true

  # Missing nested required field
  - name: "Missing nested required: user.name"
    tool: create_user
    input:
      user:
        address:
          city: "New York"
    expect:
      error:
        code: -32602

  # Missing deeply nested required
  - name: "Missing deeply nested required: user.address.city"
    tool: create_user
    input:
      user:
        name: "Alice"
        address:
          zip: "10001"
    expect:
      error:
        code: -32602

  # Pattern violation in nested field
  - name: "Pattern violation: user.address.zip"
    tool: create_user
    input:
      user:
        name: "Alice"
        address:
          city: "New York"
          zip: "invalid"
    expect:
      error:
        code: -32602
```

### Array Item Schemas

```json
{
  "type": "object",
  "properties": {
    "items": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "id": { "type": "integer" },
          "quantity": { "type": "integer", "minimum": 1 }
        },
        "required": ["id", "quantity"]
      },
      "minItems": 1,
      "maxItems": 100
    }
  },
  "required": ["items"]
}
```

Generated tests:

```yaml
steps:
  # Valid array
  - name: "Valid array with items"
    tool: process_order
    input:
      items:
        - id: 1
          quantity: 5
        - id: 2
          quantity: 3
    expect:
      success: true

  # Empty array (violates minItems)
  - name: "Empty array violates minItems"
    tool: process_order
    input:
      items: []
    expect:
      error:
        code: -32602

  # Array item missing required field
  - name: "Array item missing required: quantity"
    tool: process_order
    input:
      items:
        - id: 1
          # quantity missing
    expect:
      error:
        code: -32602

  # Array item constraint violation
  - name: "Array item constraint: quantity below minimum"
    tool: process_order
    input:
      items:
        - id: 1
          quantity: 0  # minimum is 1
    expect:
      error:
        code: -32602

  # Array exceeds maxItems
  - name: "Array exceeds maxItems (100)"
    tool: process_order
    input:
      items: [/* 101 items */]
    expect:
      error:
        code: -32602
```

### oneOf/anyOf/allOf Schemas

```json
{
  "type": "object",
  "properties": {
    "payment": {
      "oneOf": [
        {
          "type": "object",
          "properties": {
            "type": { "const": "credit_card" },
            "card_number": { "type": "string" }
          },
          "required": ["type", "card_number"]
        },
        {
          "type": "object",
          "properties": {
            "type": { "const": "bank_transfer" },
            "account_number": { "type": "string" }
          },
          "required": ["type", "account_number"]
        }
      ]
    }
  }
}
```

Generated tests:

```yaml
steps:
  # Valid: first oneOf option
  - name: "Valid oneOf: credit_card"
    tool: process_payment
    input:
      payment:
        type: "credit_card"
        card_number: "4111111111111111"
    expect:
      success: true

  # Valid: second oneOf option
  - name: "Valid oneOf: bank_transfer"
    tool: process_payment
    input:
      payment:
        type: "bank_transfer"
        account_number: "123456789"
    expect:
      success: true

  # Invalid: matches neither oneOf
  - name: "Invalid oneOf: unknown type"
    tool: process_payment
    input:
      payment:
        type: "cash"
    expect:
      error:
        code: -32602

  # Invalid: missing field for matched oneOf
  - name: "Invalid oneOf: credit_card missing card_number"
    tool: process_payment
    input:
      payment:
        type: "credit_card"
        # card_number missing
    expect:
      error:
        code: -32602
```

## CI/CD Pipeline Integration

### Complete GitHub Actions Workflow

```yaml
# .github/workflows/mcp-tests.yml
name: MCP Server Tests

on:
  push:
    branches: [main, develop]
  pull_request:
    branches: [main]
  schedule:
    - cron: '0 6 * * *'  # Daily at 6 AM

jobs:
  generate-and-test:
    runs-on: ubuntu-latest

    services:
      postgres:
        image: postgres:15
        env:
          POSTGRES_PASSWORD: postgres
          POSTGRES_DB: test
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

      - name: Cache cargo
        uses: actions/cache@v3
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}

      - name: Install cargo-pmcp
        run: cargo install cargo-pmcp

      - name: Build server
        run: cargo build --release

      - name: Start server
        run: |
          ./target/release/my-mcp-server &
          echo $! > server.pid
          sleep 5
        env:
          DATABASE_URL: postgres://postgres:postgres@localhost/test

      - name: Generate tests from schema
        run: |
          cargo pmcp test generate \
            --server http://localhost:3000 \
            --output tests/scenarios/generated/ \
            --edge-cases deep

      - name: Check for schema changes
        run: |
          if git diff --exit-code tests/scenarios/generated/; then
            echo "No schema changes detected"
          else
            echo "::warning::Schema changes detected - generated tests updated"
          fi

      - name: Run all tests
        run: |
          cargo pmcp test run \
            --server http://localhost:3000 \
            --format junit \
            --output test-results.xml

      - name: Stop server
        if: always()
        run: |
          if [ -f server.pid ]; then
            kill $(cat server.pid) || true
          fi

      - name: Upload test results
        uses: actions/upload-artifact@v3
        if: always()
        with:
          name: test-results
          path: |
            test-results.xml
            tests/scenarios/generated/

      - name: Publish test report
        uses: dorny/test-reporter@v1
        if: always()
        with:
          name: MCP Test Results
          path: test-results.xml
          reporter: java-junit
          fail-on-error: true
```

### Schema Change Detection

```yaml
# .github/workflows/schema-check.yml
name: Schema Change Detection

on:
  pull_request:
    paths:
      - 'src/**'

jobs:
  check-schema:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Install tools
        run: cargo install cargo-pmcp

      - name: Build and start server
        run: |
          cargo build --release
          ./target/release/my-mcp-server &
          sleep 5

      - name: Generate current schema tests
        run: |
          cargo pmcp test generate \
            --server http://localhost:3000 \
            --output tests/scenarios/current/

      - name: Compare with committed tests
        run: |
          if ! diff -r tests/scenarios/generated/ tests/scenarios/current/; then
            echo "::error::Schema has changed! Update tests with: cargo pmcp test generate"
            exit 1
          fi
```

## Best Practices

### 1. Version Control Strategy

```
tests/scenarios/
├── generated/              # Add to .gitignore OR commit baseline
│   └── .gitkeep
├── custom/                 # Always commit
│   ├── security/
│   ├── performance/
│   └── workflows/
└── regression/             # Always commit
    └── issue_fixes/
```

**.gitignore option** (regenerate in CI):
```
tests/scenarios/generated/
!tests/scenarios/generated/.gitkeep
```

**Commit baseline option** (track schema changes):
```
# Commit generated tests, regenerate on schema changes
# Use PR checks to detect drift
```

### 2. Test Organization

```yaml
# Use tags for filtering
tags:
  - smoke         # Quick sanity tests
  - regression    # Bug fix verification
  - security      # Security-focused
  - performance   # Performance requirements
  - integration   # Multi-step workflows

# Run subsets
cargo pmcp test run --tags smoke
cargo pmcp test run --tags security,regression
```

### 3. Maintenance Workflow

```bash
# Weekly: regenerate and review
cargo pmcp test generate --diff

# On schema change: update baseline
cargo pmcp test generate --force
git add tests/scenarios/generated/
git commit -m "Update generated tests for schema change"

# On bug fix: add regression test
vim tests/scenarios/regression/issue_123.yaml
git add tests/scenarios/regression/
git commit -m "Add regression test for issue #123"
```

## Summary

Schema-driven test generation provides:

1. **Automatic coverage** - Every schema constraint gets tested
2. **Maintenance reduction** - Tests update with schema changes
3. **Edge case discovery** - Boundary values automatically identified
4. **Type safety verification** - Type constraints validated
5. **CI/CD integration** - Detect schema drift automatically

Key commands:
```bash
# Generate tests
cargo pmcp test generate --server http://localhost:3000

# Generate with deep edge cases
cargo pmcp test generate --server http://localhost:3000 --edge-cases deep

# Check for changes
cargo pmcp test generate --diff

# Run generated tests
cargo pmcp test run --server http://localhost:3000
```

## Exercises

1. **Generate and analyze**: Generate tests for an existing server and identify what edge cases it covers
2. **Customize tests**: Edit generated tests to add business-specific assertions
3. **Schema change workflow**: Make a schema change and observe how generated tests update
4. **CI integration**: Set up a GitHub Action that regenerates tests and fails on drift

---

*Continue to [Remote Testing](../part4-testing/ch12-remote-testing.md) →*
