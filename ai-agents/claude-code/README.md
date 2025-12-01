# MCP Developer Agent for Claude Code

**Transform Claude Code into an MCP server development expert**

## Overview

This directory contains a Claude Code agent configuration that teaches Claude Code how to build production-grade MCP (Model Context Protocol) servers using the pmcp Rust SDK and cargo-pmcp toolkit.

## Quick Start

### Installation

**Option 1: User-Level (Recommended - Available Across All Projects)**

```bash
# Create agents directory if it doesn't exist
mkdir -p ~/.claude/agents

# Download the agent
curl -fsSL https://raw.githubusercontent.com/paiml/rust-mcp-sdk/main/ai-agents/claude-code/mcp-developer.md \
  -o ~/.claude/agents/mcp-developer.md

# Restart Claude Code
```

**Option 2: Project-Level (Current Project Only)**

```bash
# Create project agents directory
mkdir -p .claude/agents

# Download the agent
curl -fsSL https://raw.githubusercontent.com/paiml/rust-mcp-sdk/main/ai-agents/claude-code/mcp-developer.md \
  -o .claude/agents/mcp-developer.md

# Restart Claude Code
```

**Option 3: Manual Copy**

1. Copy `mcp-developer.md` from this directory
2. Place it in `~/.claude/agents/` (user-level) or `.claude/agents/` (project-level)
3. Restart Claude Code

### Verify Installation

Run `/agents` in Claude Code and look for "mcp-developer" in the list.

### Prerequisites

Before using this agent, install the required tools:

```bash
# 1. Install Rust (1.70+)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup update

# 2. Install cargo-pmcp toolkit
cargo install cargo-pmcp

# 3. Verify installation
cargo pmcp --version
```

## What This Agent Does

When you ask Claude Code to build an MCP server, this agent:

1. **Uses cargo-pmcp commands** (never creates files manually)
2. **Follows proven workflow**: scaffold -> implement -> test -> validate
3. **Generates production-ready code**: type-safe, validated, tested
4. **Enforces quality standards**: 80%+ test coverage, zero clippy warnings
5. **Implements MCP patterns**: tools, resources, workflows, prompts

## Usage Examples

### Create a New MCP Server

```
You: Create a weather forecast MCP server

Claude Code: I'll use the cargo-pmcp workflow to create your weather server.

$ cargo pmcp new weather-mcp-workspace
$ cd weather-mcp-workspace
$ cargo pmcp add server weather --template minimal

[Implements tools with type safety and validation]
[Generates tests and validates quality gates]

Server ready for development!
```

### Add a Tool to Existing Server

```
You: Add a get_alerts tool to my weather server

Claude Code: I'll add the tool following the established patterns.

[Edits crates/mcp-weather-core/src/tools/get_alerts.rs]
[Updates lib.rs to register the tool]
[Adds tests for the new tool]
```

### Fix Quality Issues

```
You: My server has clippy warnings

Claude Code: Let me analyze and fix them.

$ cargo clippy
[Fixes all warnings following best practices]
$ cargo clippy  # Verify clean
```

## Agent Configuration

The agent is configured with:

```yaml
name: mcp-developer
description: Expert MCP server developer using pmcp Rust SDK
tools: Read, Write, Edit, Bash, Grep, Glob, Task
model: sonnet
```

### Why These Tools?

- **Read/Grep/Glob**: Explore existing code and templates
- **Write/Edit**: Implement tool code in generated files
- **Bash**: Run cargo-pmcp commands, build, test
- **Task**: Delegate subtasks if needed

## The cargo-pmcp Workflow

This agent enforces the proper workflow:

```bash
# 1. Create workspace (one-time)
cargo pmcp new my-mcp-workspace
cd my-mcp-workspace

# 2. Add server (scaffolds everything)
cargo pmcp add server myserver --template minimal

# 3. Implement tools (this is where you code)
# Edit: crates/mcp-myserver-core/src/tools/*.rs

# 4. Start dev server (hot-reload)
cargo pmcp dev --server myserver

# 5. Generate test scenarios
cargo pmcp test --server myserver --generate-scenarios

# 6. Run tests
cargo pmcp test --server myserver

# 7. Validate quality gates
cargo fmt --check && cargo clippy && cargo test

# 8. Build for production
cargo build --release
```

**Key Principle**: Steps 1-2 scaffold the entire structure. You only write code in step 3.

## Templates Available

| Template | Use Case | Example |
|----------|----------|---------|
| `minimal` | Custom servers | Weather API, GitHub, Slack |
| `calculator` | Learning MCP | Understanding basics |
| `complete_calculator` | Full reference | All MCP capabilities |
| `sqlite_explorer` | Database browsers | PostgreSQL, MySQL |

## Quality Standards Enforced

The agent enforces Toyota Way principles:

- **Complexity**: <= 25 per function
- **Technical Debt**: 0 SATD comments
- **Test Coverage**: >= 80%
- **Linting**: 0 clippy warnings
- **Formatting**: 100% cargo fmt compliant

## Updating

To update to the latest version:

```bash
# User-level
curl -fsSL https://raw.githubusercontent.com/paiml/rust-mcp-sdk/main/ai-agents/claude-code/mcp-developer.md \
  -o ~/.claude/agents/mcp-developer.md

# Project-level
curl -fsSL https://raw.githubusercontent.com/paiml/rust-mcp-sdk/main/ai-agents/claude-code/mcp-developer.md \
  -o .claude/agents/mcp-developer.md

# Restart Claude Code
```

## Troubleshooting

### Agent Not Found

```bash
# Verify file exists
ls ~/.claude/agents/mcp-developer.md

# Check file has correct YAML frontmatter
head -10 ~/.claude/agents/mcp-developer.md
```

### Agent Not Invoked Automatically

Try explicit invocation:
```
Use the mcp-developer agent to create a calculator server
```

Or mention keywords like "MCP server", "build", "create", "scaffold".

### cargo-pmcp Commands Failing

```bash
# Ensure cargo-pmcp is installed
cargo install cargo-pmcp

# Update to latest
cargo install cargo-pmcp --force

# Verify version
cargo pmcp --version
```

## Comparison with Kiro Power

This Claude Code agent is based on the same knowledge as the [Kiro MCP Developer Power](../kiro/mcp-developer-power/). Both share:

- Same workflow principles
- Same quality standards
- Same patterns and examples
- Same Toyota Way enforcement

**Differences**:

| Aspect | Kiro Power | Claude Code Agent |
|--------|------------|-------------------|
| **Activation** | Always active (steering) | On-demand (agent) |
| **Context** | Persistent across conversations | Task-specific context |
| **Size** | 10,000+ lines (deep knowledge) | ~600 lines (focused) |
| **Format** | Multiple steering files | Single markdown file |
| **Best for** | Learning + Building | Quick scaffolding |

## Resources

- **pmcp SDK**: https://docs.rs/pmcp
- **cargo-pmcp**: https://github.com/paiml/rust-mcp-sdk/tree/main/cargo-pmcp
- **MCP Spec**: https://modelcontextprotocol.io
- **Examples**: https://github.com/paiml/rust-mcp-sdk/tree/main/examples

## Contributing

Found an issue or have a suggestion?

1. Test the improvement with the agent
2. Update `mcp-developer.md`
3. Submit PR to rust-mcp-sdk repository
4. Share learnings with community

## License

MIT - Part of the pmcp SDK ecosystem

---

**Version**: 1.1.0
**Compatible with**: Claude Code (all versions with agent support)
**Based on**: pmcp SDK 1.8.6+, cargo-pmcp
**Last Updated**: 2025-11-30
