# MCP Inspector Deep Dive

MCP Inspector is an interactive debugging and exploration tool for MCP servers. While mcp-tester handles automated testing, Inspector excels at manual exploration, debugging, and understanding server behavior during development.

## What is MCP Inspector?

Think of MCP Inspector as a "Postman for MCP"—it lets you interactively explore and test your server without writing code. While automated tests verify your server works correctly, Inspector helps you *understand* how it works and *debug* when it doesn't.

**When to reach for Inspector:**
- You're developing a new tool and want to see if it works
- Something is broken and you need to see the actual requests/responses
- You want to understand an unfamiliar server's capabilities
- You're reproducing a bug report from a user

MCP Inspector is a visual debugging tool that connects to MCP servers and provides:

- **Real-time protocol visibility** - See every message exchanged
- **Interactive tool execution** - Test tools with custom inputs
- **Schema exploration** - Browse available tools, resources, and prompts
- **Session management** - Test initialization and capability negotiation
- **Transport debugging** - Verify HTTP, SSE, and stdio transports

```
┌─────────────────────────────────────────────────────────────────────┐
│                     MCP Inspector Architecture                      │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌─────────────────┐     MCP Protocol      ┌─────────────────┐      │
│  │                 │──────────────────────▶│                 │      │
│  │  MCP Inspector  │   JSON-RPC over:      │   MCP Server    │      │
│  │    (Browser)    │   - HTTP POST         │  (Your Server)  │      │
│  │                 │◀──────────────────────│                 │      │
│  └────────┬────────┘   - SSE               └─────────────────┘      │
│           │            - stdio                                      │
│           │                                                         │
│           ▼                                                         │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │  Developer Features:                                        │    │
│  │  • Tool browser with schema display                         │    │
│  │  • Input form generation from JSON Schema                   │    │
│  │  • Response viewer with pretty-printing                     │    │
│  │  • Request/response history                                 │    │
│  │  • Error inspection and debugging                           │    │
│  │  • Session lifecycle management                             │    │
│  │  • Session management                                       │    │
│  └─────────────────────────────────────────────────────────────┘    │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

## Installation and Setup

### Installing MCP Inspector

```bash
# Install globally
npm install -g @anthropic/mcp-inspector

# Or run without installing
npx @anthropic/mcp-inspector

# Verify installation
mcp-inspector --version
```

### Starting Your MCP Server

Before connecting Inspector, start your MCP server:

```bash
# HTTP transport (recommended for development)
cargo run --release
# Server listening on http://localhost:3000

# With verbose logging for debugging
RUST_LOG=debug cargo run --release

# With specific configuration
cargo run --release -- --port 3001 --host 0.0.0.0
```

### Connecting Inspector

```bash
# Connect to HTTP transport
npx @anthropic/mcp-inspector http://localhost:3000/mcp

# Connect with SSE transport
npx @anthropic/mcp-inspector --transport sse http://localhost:3000/sse

# Connect to stdio-based server
npx @anthropic/mcp-inspector --transport stdio "cargo run --release"

# Connect with authentication
npx @anthropic/mcp-inspector \
  --header "Authorization: Bearer your-token" \
  http://localhost:3000/mcp

# Connect with custom timeout
npx @anthropic/mcp-inspector --timeout 30000 http://localhost:3000/mcp
```

## Inspector Interface Guide

### Main Dashboard

When you first connect, Inspector shows the main dashboard:

```
┌─────────────────────────────────────────────────────────────────────┐
│                        MCP Inspector                                │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  Connection: ● Connected to http://localhost:3000/mcp               │
│  Server: db-explorer v1.0.0                                         │
│  Protocol: MCP 2024-11-05                                           │
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │  CAPABILITIES                                                │   │
│  │  ├─ Tools: 3 available                                       │   │
│  │  │    ├─ list_tables                                         │   │
│  │  │    ├─ get_sample_rows                                     │   │
│  │  │    └─ execute_query                                       │   │
│  │  ├─ Resources: 0                                             │   │
│  │  └─ Prompts: 0                                               │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  [Tools] [Resources] [Prompts] [Messages] [Settings]                │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Tool Browser

Click on a tool to see its schema and test interface:

```
┌─────────────────────────────────────────────────────────────────────┐
│  Tool: execute_query                                                │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  Description: Execute a SELECT query on the database (read-only)    │
│                                                                     │
│  INPUT SCHEMA:                                                      │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │  {                                                           │   │
│  │    "type": "object",                                         │   │
│  │    "properties": {                                           │   │
│  │      "sql": {                                                │   │
│  │        "type": "string",                                     │   │
│  │        "description": "SQL SELECT query to execute"          │   │
│  │      }                                                       │   │
│  │    },                                                        │   │
│  │    "required": ["sql"]                                       │   │
│  │  }                                                           │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  INPUT FORM:                                                        │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │  sql*: [SELECT * FROM users LIMIT 5                       ]  │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                     │
│                                              [Execute Tool]         │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Response Viewer

After executing a tool, see the full response:

```
┌─────────────────────────────────────────────────────────────────────┐
│  Response: execute_query                           Duration: 23ms   │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  STATUS: Success                                                    │
│                                                                     │
│  CONTENT:                                                           │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │  [                                                           │   │
│  │    {                                                         │   │
│  │      "type": "text",                                         │   │
│  │      "text": "| id | name  | email           |\n..."         │   │
│  │    }                                                         │   │
│  │  ]                                                           │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  RAW JSON:                                                          │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │  {                                                           │   │
│  │    "jsonrpc": "2.0",                                         │   │
│  │    "id": 3,                                                  │   │
│  │    "result": {                                               │   │
│  │      "content": [...]                                        │   │
│  │    }                                                         │   │
│  │  }                                                           │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  [Copy Response] [Add to History] [Export]                          │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Message History

Track all protocol messages in the Messages tab:

```
┌─────────────────────────────────────────────────────────────────────┐
│  Message History                                   [Clear] [Export] │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  #1 [10:23:45] → initialize                                         │
│      Client info, capabilities request                              │
│                                                                     │
│  #2 [10:23:45] ← initialize (success)                               │
│      Server: db-explorer v1.0.0, Protocol: 2024-11-05               │
│                                                                     │
│  #3 [10:23:46] → tools/list                                         │
│      List available tools                                           │
│                                                                     │
│  #4 [10:23:46] ← tools/list (success)                               │
│      3 tools: list_tables, get_sample_rows, execute_query           │
│                                                                     │
│  #5 [10:24:12] → tools/call (execute_query)                         │
│      sql: "SELECT * FROM users LIMIT 5"                             │
│                                                                     │
│  #6 [10:24:12] ← tools/call (success, 23ms)                         │
│      5 rows returned                                                │
│                                                                     │
│  Click any message to see full JSON                                 │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

## Common Debugging Workflows

These workflows represent the most common debugging scenarios you'll encounter. Each follows a pattern: observe the problem, form a hypothesis, test with Inspector, and verify the fix.

### Workflow 1: Debugging a New Tool

When developing a new tool, use Inspector to validate behavior before writing automated tests. This "exploratory testing" phase helps you understand if your tool works as intended and catch obvious issues early.

```bash
# 1. Start server with debug logging
RUST_LOG=debug cargo run --release

# 2. Connect Inspector
npx @anthropic/mcp-inspector http://localhost:3000/mcp

# 3. In Inspector:
#    a. Go to Tools tab
#    b. Find your new tool
#    c. Verify the schema looks correct
#    d. Test with valid inputs
#    e. Test with invalid inputs
#    f. Check error messages are helpful
```

**Debugging checklist for new tools:**

1. **Schema validation**
   - Are all required fields marked as required?
   - Are descriptions clear and helpful?
   - Are types correct (string vs number)?
   - Are enums complete?

2. **Happy path testing**
   - Does valid input produce expected output?
   - Is the response format correct?
   - Are all fields present in the response?

3. **Error handling**
   - What happens with missing required fields?
   - What about wrong types?
   - Are error messages helpful?
   - Does isError flag get set?

### Workflow 2: Diagnosing Connection Issues

Connection problems are frustrating because the error messages are often generic ("connection refused", "timeout"). This workflow helps you systematically identify where the problem lies: Is the server running? Is it listening on the right port? Is it responding to MCP requests?

```bash
# Check server is running
curl http://localhost:3000/health

# Check MCP endpoint responds
curl -X POST http://localhost:3000/mcp \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}},"id":1}'

# Expected: JSON response with server info

# Check with Inspector verbose mode
npx @anthropic/mcp-inspector --verbose http://localhost:3000/mcp
```

**Common connection issues:**

| Symptom | Cause | Solution |
|---------|-------|----------|
| Connection refused | Server not running | Start server first |
| 404 on /mcp | Wrong endpoint | Check server route configuration |
| CORS error | Missing headers | Add CORS middleware |
| Timeout | Server not responding | Check for blocking code |
| Parse error | Invalid JSON | Check response format |

### Workflow 3: Testing Authentication

Authentication bugs are common and often subtle. Does your server reject requests without tokens? Does it accept expired tokens? Does it properly validate scopes? Inspector lets you test each scenario by manually controlling the headers.

```bash
# Test without auth (should fail)
npx @anthropic/mcp-inspector http://localhost:3000/mcp
# Expected: 401 Unauthorized

# Test with auth header
npx @anthropic/mcp-inspector \
  --header "Authorization: Bearer your-api-key" \
  http://localhost:3000/mcp

# Test with multiple headers
npx @anthropic/mcp-inspector \
  --header "Authorization: Bearer your-api-key" \
  --header "X-Request-ID: test-123" \
  http://localhost:3000/mcp
```

### Workflow 4: Reproducing Bug Reports

The first step in fixing any bug is reproducing it. Inspector lets you replay the exact sequence of operations a user performed, see the actual request/response data, and export the session for analysis or sharing with team members.

```bash
# 1. Start server with exact configuration
cargo run --release

# 2. Connect Inspector
npx @anthropic/mcp-inspector http://localhost:3000/mcp

# 3. Manually execute the reported sequence
#    - Use exact inputs from bug report
#    - Copy responses for analysis
#    - Export message history

# 4. Check Messages tab for:
#    - Request format
#    - Response format
#    - Error details
#    - Timing information
```

## Advanced Inspector Features

Beyond basic tool testing, Inspector provides advanced capabilities for edge case testing, security verification, and deep protocol debugging.

### Custom Request Builder

Sometimes you need to send requests that the normal UI can't construct—malformed JSON, missing fields, or injection attempts. The raw request builder lets you craft arbitrary JSON-RPC requests to test how your server handles unexpected input.

```
┌─────────────────────────────────────────────────────────────────────┐
│  Custom Request                                                     │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  METHOD: [tools/call                                    ▼]          │
│                                                                     │
│  PARAMS:                                                            │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │  {                                                           │   │
│  │    "name": "execute_query",                                  │   │
│  │    "arguments": {                                            │   │
│  │      "sql": "SELECT * FROM users; DROP TABLE users; --"      │   │
│  │    }                                                         │   │
│  │  }                                                           │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  [Send Request]                                                     │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

This allows testing:
- Malformed requests
- Invalid method names
- Missing required fields
- Injection attempts
- Boundary values

### Session Lifecycle Testing

Test the full session lifecycle:

```bash
# Start Inspector with session tracing
npx @anthropic/mcp-inspector --trace-session http://localhost:3000/mcp
```

Watch for:
1. **Initialize** - Client sends capabilities, server responds
2. **Initialized notification** - Client confirms ready
3. **Tool listing** - Client discovers available tools
4. **Tool execution** - Client calls tools
5. **Session end** - Clean shutdown

### Export and Share

Export debugging sessions for team sharing:

```bash
# Export message history
# In Inspector: Messages tab → Export → JSON

# The export includes:
{
  "session": {
    "server": "db-explorer",
    "version": "1.0.0",
    "connected_at": "2024-01-15T10:23:45Z"
  },
  "messages": [
    {
      "direction": "outgoing",
      "timestamp": "2024-01-15T10:23:45.123Z",
      "message": {
        "jsonrpc": "2.0",
        "method": "initialize",
        "params": {...},
        "id": 1
      }
    },
    ...
  ]
}
```

## Testing Different Transports

MCP supports multiple transport mechanisms, and Inspector can test all of them. Understanding transport differences helps you debug connectivity issues and choose the right transport for your deployment.

### HTTP POST Transport

The simplest and most common transport. Each request-response is a separate HTTP POST. Easy to debug with standard HTTP tools, but doesn't support server-initiated messages.

```bash
npx @anthropic/mcp-inspector http://localhost:3000/mcp

# Server implementation
async fn mcp_handler(
    Json(request): Json<JsonRpcRequest>,
) -> Json<JsonRpcResponse> {
    // Handle request and return response
}
```

### SSE Transport

Server-Sent Events enable the server to push updates to the client—useful for long-running operations or real-time notifications. More complex to debug because the connection is persistent.

```bash
npx @anthropic/mcp-inspector --transport sse http://localhost:3000/sse

# Server sends events like:
# event: message
# data: {"jsonrpc":"2.0","result":...}
```

Inspector will:
- Send requests via POST
- Receive responses via SSE stream
- Handle connection keep-alive
- Reconnect on disconnect

### Streamable HTTP Transport

The newest transport option, combining the simplicity of HTTP with streaming capabilities. Best for cloud deployments where you need both request-response and streaming patterns.

```bash
npx @anthropic/mcp-inspector --transport streamable http://localhost:3000/mcp

# This transport supports:
# - HTTP POST for requests
# - Streaming responses
# - Server-initiated notifications
```

### stdio Transport

For servers that run as local processes (like CLI tools), stdio transport communicates via standard input/output. Inspector spawns your server as a subprocess and manages the communication.

```bash
npx @anthropic/mcp-inspector --transport stdio "cargo run --release"

# Inspector will:
# - Spawn your server as a subprocess
# - Send JSON-RPC over stdin
# - Read responses from stdout
# - Display stderr as debug output
```

## Comparing Tools

### Inspector vs mcp-tester

| Feature | Inspector | mcp-tester |
|---------|-----------|------------|
| **Purpose** | Interactive debugging | Automated testing |
| **Interface** | Visual/GUI | CLI/YAML files |
| **Automation** | Manual only | Full CI/CD support |
| **Schema exploration** | Excellent | Basic |
| **Error debugging** | Detailed view | Pass/fail results |
| **Regression testing** | Not suitable | Designed for it |
| **Performance testing** | Basic timing | Detailed metrics |
| **Edge case discovery** | Manual | Auto-generated |

### Inspector vs Claude Desktop

| Feature | Inspector | Claude Desktop |
|---------|-----------|----------------|
| **Purpose** | Development/debugging | End-user experience |
| **Protocol view** | Full visibility | Hidden |
| **Custom requests** | Supported | Not available |
| **Authentication** | Configurable | Automatic |
| **Multi-server** | One at a time | Multiple servers |

### When to Use Each

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Testing Tool Selection                           │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  Development Phase:                                                 │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │  Writing new tool → Inspector                               │    │
│  │  Debugging issue  → Inspector                               │    │
│  │  Learning MCP     → Inspector                               │    │
│  └─────────────────────────────────────────────────────────────┘    │
│                                                                     │
│  Testing Phase:                                                     │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │  Unit tests      → cargo test                               │    │
│  │  Integration     → mcp-tester                               │    │
│  │  Edge cases      → mcp-tester (generated)                   │    │
│  │  Regression      → mcp-tester (CI/CD)                       │    │
│  └─────────────────────────────────────────────────────────────┘    │
│                                                                     │
│  Production Phase:                                                  │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │  Smoke tests     → mcp-tester (subset)                      │    │
│  │  User acceptance → Claude Desktop                           │    │
│  │  Bug reproduction→ Inspector                                │    │
│  └─────────────────────────────────────────────────────────────┘    │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

## Tips and Best Practices

### Effective Debugging

1. **Start simple** - Test basic functionality before complex scenarios
2. **Check schemas first** - Many issues are schema validation problems
3. **Read error messages** - Server errors usually explain the problem
4. **Export sessions** - Save message history before closing
5. **Compare working vs broken** - Diff message sequences

### Performance Investigation

Use Inspector to identify slow operations:

```
Message History with Timing:

#5 [10:24:12] → tools/call (execute_query)
#6 [10:24:12] ← tools/call (success, 23ms)     ← Fast

#7 [10:24:30] → tools/call (execute_query)
#8 [10:24:35] ← tools/call (success, 5023ms)   ← Slow!
```

When you see slow responses:
1. Check the query being executed
2. Look for missing indexes
3. Check for network latency
4. Review server-side logging

### Security Testing

Use Inspector to manually test security:

```bash
# Test SQL injection
Input: "SELECT * FROM users WHERE id = '1' OR '1'='1'"

# Test path traversal
Input: "../../../etc/passwd"

# Test command injection
Input: "test; rm -rf /"

# Test XSS (if output is HTML)
Input: "<script>alert('xss')</script>"
```

Verify your server:
- Rejects or sanitizes malicious input
- Returns appropriate error messages
- Doesn't expose sensitive data in errors

### Common Pitfalls

1. **Forgetting to restart server** - Code changes require restart
2. **Wrong port** - Server and Inspector on different ports
3. **Auth header issues** - Missing or malformed Bearer token
4. **JSON formatting** - Invalid JSON in custom requests
5. **CORS** - Browser-based Inspector blocked by CORS

## Integration with Development Workflow

### Development Cycle

```bash
# 1. Write code
vim src/tools/new_feature.rs

# 2. Build and run
cargo run --release &

# 3. Test with Inspector
npx @anthropic/mcp-inspector http://localhost:3000/mcp
# - Explore schema
# - Test happy paths
# - Test error cases

# 4. If issues found, check logs
# Server window shows RUST_LOG output

# 5. Fix and repeat
```

### Watch Mode Development

```bash
# Terminal 1: Watch for changes and rebuild
cargo watch -x run --release

# Terminal 2: Keep Inspector connected
npx @anthropic/mcp-inspector http://localhost:3000/mcp

# Workflow:
# 1. Edit code
# 2. cargo watch rebuilds automatically
# 3. Inspector reconnects (may need manual refresh)
# 4. Test immediately
```

## Summary

MCP Inspector is your primary tool for:
- **Understanding** how your server responds to requests
- **Debugging** issues during development
- **Exploring** server capabilities and schemas
- **Reproducing** reported bugs
- **Testing** authentication and security

Use Inspector during development, then codify working tests in mcp-tester for automation.

## Exercises

1. **Connect and explore** - Start the db-explorer server and use Inspector to list all tools
2. **Test error handling** - Send invalid SQL and verify error responses
3. **Export a session** - Execute several tools and export the message history
4. **Debug authentication** - Add auth to a server and test with Inspector headers
5. **Compare transports** - Test the same server with HTTP and SSE transports

---

*Continue to [mcp-tester Introduction](./ch11-02-mcp-tester.md) →*
