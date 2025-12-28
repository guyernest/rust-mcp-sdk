# Your First Production Server

> **Prerequisites**: Make sure you've completed the [Development Environment Setup](./ch02-01-setup.md) before continuing. You'll need Rust, cargo-pmcp, and Claude Code installed.

Let's build your first MCP server. We'll get it running and connected to Claude in under 5 minutes—then we'll explore how it works.

## Quick Start: From Zero to Working Server

### Step 1: Create the Workspace

```bash
cargo pmcp new my-mcp-servers
cd my-mcp-servers
```

This creates a workspace structure for building MCP servers.

### Step 2: Add a Calculator Server

```bash
cargo pmcp add server calculator --template calculator
```

This generates a complete, working MCP server with example tools.

### Step 3: Build and Run

```bash
cargo pmcp dev calculator
```

You should see:

```
INFO Starting MCP server "calculator" v1.0.0
INFO Listening on http://0.0.0.0:3000
```

Your server is running.

### Step 4: Connect to Claude Code

In a new terminal, add the server to Claude Code:

```bash
claude mcp add calculator -t http http://0.0.0.0:3000
```

That's it—Claude Code now knows about your server.

### Step 5: Try It!

Start Claude Code and ask:

> "What is 1234 + 5678?"

Claude will call your `add` tool and respond with the result. You just built an MCP server!

Try a few more:
- "Calculate 100 divided by 7"
- "What's 15 times 23?"
- "Divide 10 by 0" (watch the error handling)

## What Just Happened?

In those 5 steps, you created a production-ready MCP server that:

| Feature | What It Does |
|---------|--------------|
| **Type-safe inputs** | Invalid inputs are rejected automatically |
| **Structured outputs** | Results include both values and descriptions |
| **Error handling** | Division by zero returns a proper error, not a crash |
| **JSON Schema** | Claude knows exactly what parameters each tool accepts |
| **HTTP transport** | Ready for cloud deployment |

This isn't a toy example—it's the same foundation you'll use for enterprise servers.

## Testing with MCP Inspector

Before connecting to Claude, you can test your server interactively using MCP Inspector:

```bash
npx @modelcontextprotocol/inspector http://localhost:3000/mcp
```

This opens a web UI where you can:
- Browse available tools and their schemas
- Call tools with test inputs
- See the raw JSON-RPC messages

Try the `divide` tool with `divisor: 0` to see how errors are handled.

## Project Structure

Let's look at what `cargo pmcp` generated:

```
my-mcp-servers/
├── Cargo.toml              # Workspace manifest
├── pmcp.toml               # PMCP configuration
├── server-common/          # Shared HTTP bootstrap code
│   ├── Cargo.toml
│   └── src/lib.rs
└── servers/
    └── calculator/         # Your calculator server
        ├── Cargo.toml
        └── src/
            ├── main.rs     # Entry point
            └── tools/
                ├── mod.rs
                └── calculator.rs
```

**Why a workspace?** As you build more servers, they'll share the `server-common` code for HTTP handling, authentication, and other infrastructure. This keeps each server focused on business logic.

## Your Turn: Build Your First Server

You've seen the calculator server in action. Now build your own MCP server from scratch.

**[Chapter 2 Exercises](./ch02-exercises.md)** - Start with Exercise 1: Your First MCP Server

## Next Steps

Now that you have a working server, the following sections will cover:

1. **[Building and Running](./ch02-02-workspace.md)** - Understanding the workspace structure
2. **[The Calculator Server](./ch02-03-calculator.md)** - Deep dive into the generated code
3. **[Understanding the Code](./ch02-04-code-walkthrough.md)** - Rust patterns and PMCP conventions
4. **[Testing with MCP Inspector](./ch02-05-inspector.md)** - Advanced debugging techniques

---

*Continue to [Building and Running](./ch02-02-workspace.md) →*
