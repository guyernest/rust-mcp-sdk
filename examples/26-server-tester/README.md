# MCP Server Tester

A comprehensive testing tool for Model Context Protocol (MCP) servers, providing protocol compliance validation, capability testing, and diagnostic features.

## Features

### Core Testing
- **Protocol Compliance Testing**: Validates JSON-RPC 2.0 and MCP protocol compliance
- **Multi-Transport Support**: Tests HTTP, HTTPS, WebSocket, and stdio transports
- **OAuth 2.0 Authentication (NEW!)**: Interactive browser-based OAuth flow with PKCE, token caching, and automatic middleware injection across all transport types
- **Comprehensive Diagnostics**: Layer-by-layer connection troubleshooting
- **Server Comparison**: Compare capabilities and performance between servers
- **CI/CD Ready**: JSON output for automated testing pipelines

### Discovery & Validation (NEW!)
- **Tool Schema Validation**: Automatically validates JSON schemas and warns about incomplete definitions
- **Resource Testing**: Tests resource discovery, reading, and metadata validation
- **Prompt Testing**: Validates prompt templates, arguments, and metadata
- **Metadata Validation**: Checks for missing descriptions, MIME types, and other essential metadata

### Test Automation (NEW!)
- **Automated Scenario Generation**: Generates test scenarios from discovered server capabilities
- **Smart Schema Analysis**: Creates appropriate test values based on JSON schema definitions
- **Tool Testing**: Discover and test individual tools with custom arguments
- **Scenario Testing**: Define and run complex test scenarios from YAML/JSON files
- **Assertion Framework**: Validate server responses with powerful assertions

### Reporting
- **Multiple Output Formats**: Pretty, JSON, minimal, and verbose outputs
- **Schema Validation Reports**: Detailed warnings about tool schema completeness
- **Color-Coded Results**: Visual feedback for test status and warnings

## Installation

```bash
# From the examples directory
cd examples/26-server-tester
cargo build --release

# The binary will be at target/release/mcp-tester
```

## Usage

### Quick Start

```bash
# Test a local HTTP server
mcp-tester test http://localhost:8080

# Test with tools validation
mcp-tester test http://localhost:8080 --with-tools

# Test a stdio server
mcp-tester test stdio

# Quick connectivity check
mcp-tester quick http://localhost:8080

# Test OAuth-protected server with access token
mcp-tester test https://api.example.com/mcp --api-key YOUR_ACCESS_TOKEN
```

### Testing OAuth-Protected MCP Servers (Interactive OAuth)

The MCP Tester now supports **interactive OAuth 2.0 authentication** with automatic browser-based login and token management.

#### Interactive OAuth Flow

For servers that require OAuth authentication, the tester provides a seamless authentication experience:

```bash
# Interactive OAuth with automatic browser login (OIDC discovery)
mcp-tester test https://your-oauth-server.com/mcp \
  --oauth-client-id "your-client-id" \
  --oauth-issuer "https://auth.example.com" \
  --oauth-scopes openid,email,profile
```

**What happens:**
1. ✅ Tester generates secure PKCE challenge
2. 🌐 Opens your browser to the OAuth provider login page
3. 🔐 You authenticate with your credentials
4. ✅ Tester receives the authorization code via local callback server
5. 🎫 Exchanges code for access token
6. 💾 **Caches token locally** for future requests
7. 🚀 Automatically injects `Authorization: Bearer` header into all MCP requests

#### Token Caching

Tokens are cached in `~/.mcp-tester/tokens.json`:
- Cached tokens are automatically reused until expiration
- No need to re-authenticate for every test run
- Tokens are stored securely with expiration timestamps
- Manual refresh is handled automatically when tokens expire

#### AWS Cognito Example (OIDC discovery)

Testing an MCP server protected by AWS Cognito:

```bash
mcp-tester test https://your-api.execute-api.us-west-2.amazonaws.com/mcp \
  --oauth-issuer "https://your-pool.auth.us-west-2.amazoncognito.com" \
  --oauth-client-id "your-cognito-client-id" \
  --oauth-scopes openid \
  --oauth-redirect-port 8080
```

**Important for AWS Cognito:**
- Add `http://localhost:8765/callback` to your Cognito App Client's allowed callback URLs
- Ensure your App Client has "Authorization code grant" enabled
- The tester automatically handles PKCE (no client secret needed)

#### Multi-Transport OAuth Support

OAuth middleware now works seamlessly across **all transport types**:

- ✅ **JSON-RPC HTTP** (AWS Lambda, API Gateway, standard HTTP servers)
- ✅ **StreamableHTTP** (SSE-based streaming servers)
- ✅ **Automatic Transport Detection** - OAuth works regardless of transport

The tester automatically detects the correct transport and applies OAuth middleware appropriately.

#### Manual Token Usage (Alternative)

If you already have an access token from another source:

```bash
# Pass token directly
mcp-tester test https://your-oauth-server.com/mcp --api-key "YOUR_ACCESS_TOKEN"

# Or via environment variable
export MCP_API_KEY="YOUR_ACCESS_TOKEN"
mcp-tester test https://your-oauth-server.com/mcp
```

The tester will automatically add the `Authorization: Bearer YOUR_ACCESS_TOKEN` header to all requests.

### Commands

#### `test` - Run Full Test Suite

```bash
mcp-tester test <URL> [OPTIONS]

Options:
  --with-tools                Test all discovered tools
  --tool <NAME>               Test specific tool
  --args <JSON>               Tool arguments as JSON
  --format <FORMAT>           Output format (pretty|json|minimal|verbose)
  --timeout <SECONDS>         Connection timeout (default: 30)
  --insecure                  Skip TLS certificate verification

  # OAuth 2.0 Authentication (OIDC discovery)
  --oauth-issuer <URL>        OAuth/OIDC issuer URL (for discovery)
  --oauth-client-id <ID>      OAuth client ID
  --oauth-scopes <SCOPES>     Comma-separated scopes (default: openid)
  --oauth-redirect-port <N>   Local redirect port (default: 8080)
  --oauth-no-cache            Disable token cache (~/.mcp-tester/tokens.json)
  --api-key <TOKEN>           Manual bearer token (alternative)
```

#### `compliance` - Protocol Compliance Validation

```bash
mcp-tester compliance <URL> [OPTIONS]

Options:
  --strict              Treat warnings as failures
```

#### `tools` - Discover and Test Tools with Schema Validation

```bash
mcp-tester tools <URL> [OPTIONS]

Options:
  --test-all            Test each tool with sample data
  --verbose             Show detailed schema validation warnings
```

**NEW: Schema Validation Output Example:**
```
✓ Found 10 tools:
  • search_wikipedia - Search for Wikipedia articles by query
    ✓ Schema properly defined
  • get_article - Retrieve full Wikipedia article content
    ⚠ Tool 'get_article' missing 'properties' field for object type
  • get_summary - Get a summary of a Wikipedia article
    ⚠ Tool 'get_summary' has empty input schema - consider defining parameters

Schema Validation Summary:
⚠ 3 total warnings found
  - 1 tools with empty schema
  - 2 tools missing 'properties' in schema
```

#### `resources` - Test Resources (NEW!)

```bash
mcp-tester resources <URL>

Discovers and validates all available resources, checking for:
- Missing MIME types
- Invalid URIs
- Metadata completeness
```

#### `prompts` - Test Prompts (NEW!)

```bash
mcp-tester prompts <URL>

Discovers and validates all available prompts, checking for:
- Missing descriptions
- Undefined arguments
- Argument schema validation
```

#### `generate-scenario` - Generate Test Scenarios (NEW!)

```bash
mcp-tester generate-scenario <URL> [OPTIONS]

Options:
  -o, --output <FILE>        Output file path (default: generated_scenario.yaml)
  --all-tools                Include all discovered tools
  --with-resources           Include resource testing
  --with-prompts             Include prompt testing

Examples:
  # Generate basic scenario
  mcp-tester generate-scenario http://localhost:8080 -o test.yaml

  # Generate comprehensive scenario
  mcp-tester generate-scenario http://localhost:8080 -o full_test.yaml \
    --all-tools --with-resources --with-prompts
```

#### `diagnose` - Connection Diagnostics

```bash
mcp-tester diagnose <URL> [OPTIONS]

Options:
  --network             Include network-level diagnostics
```

#### `compare` - Compare Two Servers

```bash
mcp-tester compare <SERVER1> <SERVER2> [OPTIONS]

Options:
  --with-perf           Include performance comparison
```

#### `health` - Server Health Check

```bash
mcp-tester health <URL>
```

#### `scenario` - Run Test Scenarios

```bash
mcp-tester scenario <URL> <SCENARIO_FILE> [OPTIONS]

Options:
  --detailed       Show detailed step-by-step output

Examples:
# Run a basic test scenario
mcp-tester scenario http://localhost:8080 scenarios/basic-test.yaml

# Run with detailed output
mcp-tester scenario http://localhost:8080 scenarios/complex-workflow.json --detailed

# Run performance tests
mcp-tester scenario http://localhost:8080 scenarios/performance-test.yaml
```

Test scenarios allow you to define complex test sequences with variables, assertions, and workflows. See [SCENARIO_FORMAT.md](SCENARIO_FORMAT.md) for detailed documentation on creating test scenarios.

## Scenario Generation (NEW!)

The MCP Tester can automatically generate test scenarios from your server's discovered capabilities. This feature analyzes tool schemas and creates comprehensive test templates.

### Generated Scenario Example

When you run `mcp-tester generate-scenario`, it creates a YAML file like this:

```yaml
name: wikipedia-mcp-server Test Scenario
description: Automated test scenario for server
timeout: 60
stop_on_failure: false
variables:
  test_id: test_123
  test_value: sample_value

steps:
  - name: List available capabilities
    operation:
      type: list_tools
    store_result: available_tools
    assertions:
      - type: success
      - type: exists
        path: tools

  - name: Test tool: search_wikipedia
    operation:
      type: tool_call
      tool: search_wikipedia
      arguments:
        query: "TODO: query"  # Automatically generated from schema
    timeout: 30
    continue_on_failure: true
    store_result: search_wikipedia_result
    assertions:
      - type: success

  - name: Test tool: get_article
    operation:
      type: tool_call
      tool: get_article
      arguments:
        title: "TODO: title"  # Placeholder based on schema type
    store_result: get_article_result
    assertions:
      - type: success
```

### Smart Value Generation

The scenario generator creates appropriate placeholder values based on JSON schema types:

| Schema Type | Example Generated Value |
|------------|------------------------|
| `string` with format `uri` | `"https://example.com"` |
| `string` with format `email` | `"test@example.com"` |
| `string` with format `date` | `"2024-01-01"` |
| `string` with format `uuid` | `"550e8400-e29b-41d4-a716-446655440000"` |
| `string` with description containing "path" | `"/path/to/file"` |
| `string` with description containing "id" | `"test_id_123"` |
| `number` with minimum | The minimum value |
| `boolean` | `false` |
| `array` | Sample array with one item |
| `object` | Nested object with all properties |
| Unknown types | `"TODO: field_name"` |

### Workflow

1. **Generate the scenario:**
   ```bash
   mcp-tester generate-scenario https://api.example.com/mcp -o my_test.yaml --all-tools
   ```

2. **Edit the generated file to replace TODOs with actual test data:**
   ```yaml
   arguments:
     query: "artificial intelligence"  # Was: "TODO: query"
     limit: 10
   ```

3. **Add custom assertions:**
   ```yaml
   assertions:
     - type: success
     - type: array_length
       path: results
       greater_than: 0
     - type: contains
       path: results[0].title
       value: "AI"
   ```

4. **Run the scenario:**
   ```bash
   mcp-tester scenario https://api.example.com/mcp my_test.yaml --detailed
   ```

## Examples

### Testing an OAuth-enabled Server

```bash
# Test the OAuth example server (local development)
cd ../25-oauth-basic
make run-http &  # Start server in background

# Run tests with OAuth middleware
mcp-tester test http://localhost:8080 --with-tools

# Test specific tool with arguments
mcp-tester test http://localhost:8080 \
  --tool admin_action \
  --args '{"action": "test"}'

# Test AWS Lambda MCP server with interactive OAuth (recommended)
mcp-tester test https://your-api.execute-api.us-west-2.amazonaws.com/mcp \
  --oauth-issuer "https://your-pool.auth.us-west-2.amazoncognito.com" \
  --oauth-client-id "your-cognito-client-id" \
  --with-tools

# Or with manual token (if you already have one)
mcp-tester test https://your-api.execute-api.us-west-2.amazonaws.com/mcp \
  --api-key "YOUR_ACCESS_TOKEN" \
  --with-tools

# Subsequent runs will use cached token (no re-authentication needed!)
mcp-tester test https://your-api.execute-api.us-west-2.amazonaws.com/mcp \
  --oauth-client-id "your-cognito-client-id" \
  --with-tools
```

### CI/CD Integration

```bash
# Output JSON for automated testing
mcp-tester test $SERVER_URL --format json > test-results.json

# Check exit code
if [ $? -eq 0 ]; then
  echo "All tests passed"
else
  echo "Tests failed"
  exit 1
fi
```

### Debugging Connection Issues

```bash
# Run comprehensive diagnostics
mcp-tester diagnose http://localhost:8080 --network

# This will test:
# - URL validation
# - DNS resolution
# - TCP connectivity
# - TLS/SSL certificates (for HTTPS)
# - HTTP response
# - MCP protocol handshake
```

### Running Test Scenarios

```bash
# Run a basic test scenario
mcp-tester scenario http://localhost:8080 scenarios/basic-test.yaml

# Create a custom scenario for your server
cat > my-test.yaml << EOF
name: My Server Test
steps:
  - name: List available tools
    operation:
      type: list_tools
    assertions:
      - type: success
      - type: array_length
        path: tools
        greater_than: 0
        
  - name: Test my custom tool
    operation:
      type: tool_call
      tool: my_tool
      arguments:
        param: "test"
    assertions:
      - type: success
      - type: exists
        path: result
EOF

# Run the custom scenario
mcp-tester scenario http://localhost:8080 my-test.yaml --verbose
```

### Comparing Server Implementations

```bash
# Compare two servers
mcp-tester compare http://server1.example.com http://server2.example.com --with-perf

# This compares:
# - Protocol versions
# - Capabilities
# - Available tools
# - Performance metrics
```

## Output Formats

### Pretty (Default)

Color-coded terminal output with symbols:
- ✓ Passed (green)
- ✗ Failed (red)  
- ⚠ Warning (yellow)
- ○ Skipped (gray)

### JSON

Structured output for programmatic processing:

```json
{
  "tests": [
    {
      "name": "Initialize",
      "category": "Core",
      "status": "Passed",
      "duration": { "secs": 0, "nanos": 123456 },
      "error": null,
      "details": "Server: my-server v1.0.0, Protocol: 2024-11-05"
    }
  ],
  "summary": {
    "total": 10,
    "passed": 8,
    "failed": 1,
    "warnings": 1,
    "skipped": 0
  },
  "duration": { "secs": 2, "nanos": 500000000 }
}
```

### Minimal

Single-line summary:
```
PASS: 8 passed, 0 failed, 1 warnings in 2.50s
```

### Verbose

Detailed output with full error messages and protocol exchanges.

## Test Categories

### Core Tests
- Connection establishment
- Server initialization
- Capability discovery
- Health endpoints

### Protocol Tests
- JSON-RPC 2.0 compliance
- MCP protocol version validation
- Required method implementation
- Error code standards

### Tool Tests
- Tool discovery (tools/list)
- Tool invocation
- Input validation
- Response format validation

### Performance Tests
- Connection latency
- Response times
- Throughput measurements

## Deployment-Specific Testing

### AWS Lambda

```bash
# Test Lambda function via API Gateway
mcp-tester test https://abc123.execute-api.us-east-1.amazonaws.com/prod

# Lambda cold starts may timeout - increase timeout
mcp-tester test <LAMBDA_URL> --timeout 60
```

### Docker Containers

```bash
# Test containerized server
docker run -p 8080:8080 my-mcp-server

mcp-tester test http://localhost:8080
```

### Kubernetes

```bash
# Port-forward to test in-cluster service
kubectl port-forward service/mcp-server 8080:80

mcp-tester test http://localhost:8080
```

## Troubleshooting

### Connection Refused

```bash
# Run diagnostics to identify the issue
mcp-tester diagnose http://localhost:8080 --network

# Common solutions:
# - Verify server is running
# - Check port is correct
# - Review firewall settings
```

### TLS Certificate Errors

```bash
# For self-signed certificates
mcp-tester test https://localhost:8443 --insecure
```

### Protocol Version Mismatch

The tester supports protocol versions:
- 2024-11-05 (current)
- 2025-03-26
- 2025-06-18

### Timeout Issues

```bash
# Increase timeout for slow servers
mcp-tester test <URL> --timeout 120
```

## Integration with CI/CD

### GitHub Actions

```yaml
- name: Test MCP Server
  run: |
    cargo run --bin mcp-tester -- test ${{ env.SERVER_URL }} --format json > results.json
    
- name: Upload Test Results
  uses: actions/upload-artifact@v2
  with:
    name: test-results
    path: results.json
```

### Jenkins

```groovy
stage('Test MCP Server') {
  steps {
    sh 'mcp-tester test ${SERVER_URL} --format json > results.json'
    archiveArtifacts 'results.json'
  }
}
```

## Contributing

The server tester is part of the Rust MCP SDK. Contributions are welcome!

### Adding New Tests

1. Add test logic to `src/tester.rs`
2. Add validators to `src/validators.rs`
3. Update test categories in `src/report.rs`
4. Add CLI options in `src/main.rs`

### Testing the Tester

```bash
# Run against known good server
cargo run -- test http://localhost:8080

# Run against test fixtures
cargo test
```

## License

MIT - See LICENSE file in the repository root.
