::: exercise
id: ch11-01-mcp-inspector
difficulty: intermediate
time: 30 minutes
:::

Master MCP Inspector for interactive debugging. Inspector is like browser
DevTools for MCP - you can see exactly what clients send and servers respond.

::: objectives
thinking:
  - How MCP Inspector reveals protocol-level communication
  - The JSON-RPC request/response cycle in MCP
  - When to use interactive debugging vs automated testing
doing:
  - Connect Inspector to a running MCP server
  - Explore server capabilities and tool schemas
  - Execute tools and examine responses
  - Debug a failing tool call step-by-step
:::

::: discussion
- When your MCP server returns unexpected results, how do you currently debug it?
- What information is lost when you only have server logs?
- How is Inspector different from just reading server logs?
:::

## Step 1: Install MCP Inspector

```bash
# Install globally
npm install -g @anthropic/mcp-inspector

# Or use directly with npx
npx @anthropic/mcp-inspector
```

## Step 2: Start Your Server

```bash
# Start your MCP server (example with db-explorer)
cargo run --release
# Server starts on http://localhost:3000
```

## Step 3: Connect Inspector

```bash
# For streamable-http transport (default)
npx @anthropic/mcp-inspector http://localhost:3000/mcp

# For SSE transport
npx @anthropic/mcp-inspector --transport sse http://localhost:3000/sse
```

## Step 4: Explore Capabilities

In the Inspector UI:
1. Click "Tools" to see available tools
2. Examine tool schemas - note required vs optional fields
3. Click "Resources" to see available resources
4. Look at prompts if your server provides them

## Step 5: Execute Tools

1. Select a tool (e.g., `list_tables`)
2. Fill in required parameters (if any)
3. Click "Execute"
4. Examine the response:
   - Is it a success or error?
   - What does the result contain?
   - What's the response time?

## Step 6: Debug an Error

1. Intentionally provide invalid input:
   - Missing required field
   - Wrong type (string instead of number)
   - Invalid SQL syntax
2. Examine the error response:
   - What error code is returned?
   - Is the message helpful?
   - Does it reveal too much internal detail?

::: hints
level_1: "If connection fails, verify your server is running on the expected port with 'curl http://localhost:3000/mcp'."
level_2: "Common error codes: -32600 (invalid request), -32601 (method not found), -32602 (invalid params)."
level_3: "Use Inspector's 'Raw' tab to see the exact JSON-RPC messages being exchanged."
:::

## Success Criteria

- [ ] Successfully connects Inspector to running server
- [ ] Lists all tools and examines their schemas
- [ ] Executes at least 3 different tool calls
- [ ] Intentionally triggers an error and explains the response
- [ ] Identifies at least one issue that logs didn't reveal

---

*Next: [Writing Test Scenarios](./test-scenarios.md) for automated testing.*
