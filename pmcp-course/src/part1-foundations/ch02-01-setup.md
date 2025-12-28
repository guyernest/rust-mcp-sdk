# Development Environment Setup

Before building your first MCP server, let's set up your development environment. You'll need three things:

1. **Rust** - The programming language
2. **cargo-pmcp** - The PMCP development toolkit
3. **An MCP client** - To test and use your servers

## Installing Rust

If you don't have Rust installed, run:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Follow the prompts and select the default installation.

> **macOS users**: You may need to install Xcode command line tools first: `xcode-select --install`

After installation, restart your terminal and verify:

```bash
rustc --version
# Should output: rustc 1.82.0 or later

cargo --version
# Should output: cargo 1.82.0 or later
```

## Installing cargo-pmcp

Install the PMCP development toolkit:

```bash
cargo install cargo-pmcp
```

This provides several commands you'll use throughout this course:

| Command | Purpose |
|---------|---------|
| `cargo pmcp new` | Create a new MCP workspace |
| `cargo pmcp add` | Add servers and tools to your workspace |
| `cargo pmcp dev` | Run a server in development mode |
| `cargo pmcp test` | Run MCP-specific tests |
| `cargo pmcp deploy` | Deploy to cloud platforms |

Verify installation:

```bash
cargo pmcp --version
```

## Choosing an MCP Client

MCP servers need a client to connect to. Several developer-friendly MCP clients are available:

| Client | Best For | MCP Support |
|--------|----------|-------------|
| **Claude Code** | Terminal-based development, CLI workflows | Excellent |
| **Cursor** | AI-assisted coding in VS Code fork | Good |
| **Gemini Code Assist** | Google Cloud integrated development | Good |
| **Cline** | VS Code extension for AI coding | Good |
| **Kiro** | AWS-focused agentic IDE | Good |
| **Codex CLI** | OpenAI's terminal assistant | Basic |

For this course, we recommend **Claude Code**. It has excellent MCP support, works entirely in the terminal, and makes it easy to add and manage MCP servers.

## Installing Claude Code

### macOS and Linux

```bash
curl -fsSL https://claude.ai/install.sh | bash
```

### Windows

```powershell
irm https://claude.ai/install.ps1 | iex
```

After installation, verify it works:

```bash
claude --version
```

### First Run

The first time you run Claude Code, you'll need to authenticate:

```bash
claude
```

Follow the prompts to log in with your Anthropic account.

## Adding MCP Servers to Claude Code

Once your MCP server is running, you can add it to Claude Code with a single command:

```bash
claude mcp add <server-name> -t http <server-url>
```

For example:

```bash
claude mcp add calculator -t http http://localhost:3000
```

You can list your configured servers:

```bash
claude mcp list
```

And remove servers you no longer need:

```bash
claude mcp remove calculator
```

## MCP Inspector (Optional)

MCP Inspector is a debugging tool that lets you interact with MCP servers directly, without going through an AI client. It's useful for testing and troubleshooting.

No installation needed—just run with npx:

```bash
npx @modelcontextprotocol/inspector http://localhost:3000/mcp
```

This opens a web UI where you can browse tools, call them with test inputs, and see the raw JSON-RPC messages.

## Configuring Your IDE

For writing Rust code, configure your preferred IDE:

### VS Code

Install these extensions:
1. **rust-analyzer** - Rust language support (essential)
2. **Even Better TOML** - TOML syntax highlighting
3. **CodeLLDB** - Debugging support

### Cursor

Cursor includes rust-analyzer support. Enable it in settings and you're ready to go.

### RustRover

JetBrains RustRover works out of the box with Rust projects—no additional configuration needed.

### Zed

Zed has built-in Rust support with excellent performance.

## Enterprise Considerations

In enterprise environments, you may need to:

- Configure cargo to use an internal registry or mirror
- Set up proxy settings for cargo and rustup
- Use a corporate certificate authority

Consult your IT department's Rust setup guide for organization-specific instructions.

## Verify Your Setup

Let's confirm everything is working:

```bash
# Check Rust
rustc --version && cargo --version

# Check cargo-pmcp
cargo pmcp --version

# Check Claude Code
claude --version
```

If all three commands succeed, you're ready to build your first MCP server!

---

## Knowledge Check

Test your understanding of the setup process:

{{#quiz ../quizzes/ch02-01-setup.toml}}

---

*Continue to [Your First Production Server](./ch02-first-server.md) →*
