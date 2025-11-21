# MCP Developer Subagent for Claude Code

**Expert MCP server development assistant using pmcp Rust SDK and cargo-pmcp toolkit**

## Overview

This Claude Code subagent provides specialized expertise for building production-grade Model Context Protocol (MCP) servers using the pmcp Rust SDK and cargo-pmcp toolkit. It enforces Toyota Way quality principles and the proper cargo-pmcp workflow.

## What This Subagent Does

- ✅ **Scaffolds MCP servers** using cargo-pmcp commands (never manual file creation)
- ✅ **Implements production-ready tools** with type safety and validation
- ✅ **Enforces quality standards** (80%+ test coverage, zero clippy warnings)
- ✅ **Guides testing workflow** (generate scenarios, run tests, validate)
- ✅ **Provides architectural guidance** (template selection, pattern recommendations)
- ✅ **Teaches MCP concepts** while building servers

## Installation

### Option 1: User-Level (Available Across All Projects)

```bash
# Copy to user agents directory
mkdir -p ~/.claude/agents
cp mcp-developer.md ~/.claude/agents/
```

### Option 2: Project-Level (Current Project Only)

```bash
# Copy to project agents directory
mkdir -p .claude/agents
cp mcp-developer.md .claude/agents/
```

### Option 3: Using Claude Code Interface

1. Open Claude Code
2. Run command: `/agents`
3. Select "Create New Agent"
4. Choose scope (user or project)
5. Copy the content from `mcp-developer.md`
6. Save

## Prerequisites

Before using this subagent, ensure you have:

```bash
# Rust 1.70+
rustup update

# cargo-pmcp toolkit
cargo install cargo-pmcp

# Verify installation
cargo pmcp --version
```

## Usage

### Automatic Invocation

The subagent is automatically invoked when you mention MCP server development:

```
> Create an MCP server for weather forecasts
> Build a calculator MCP server
> Help me scaffold a GitHub API MCP server
```

**The subagent will automatically**:
- Use `cargo pmcp new` for workspace creation
- Use `cargo pmcp add server` for scaffolding
- Only edit generated tool files (never create manually)
- Follow the complete cargo-pmcp workflow

### Explicit Invocation

You can also invoke it explicitly:

```
> Use the mcp-developer subagent to create a weather server
> Have mcp-developer help me build a database MCP server
```

### Example Workflow

**You**: "Create a weather forecast MCP server"

**Subagent Response**:

```
I'll create a weather MCP server using the cargo-pmcp workflow.

Step 1: Create workspace
$ cargo pmcp new weather-mcp-workspace
$ cd weather-mcp-workspace

Step 2: Add weather server with minimal template
$ cargo pmcp add server weather --template minimal

This scaffolds:
- mcp-weather-core/ (library crate)
- weather-server/ (binary crate)
- Complete structure with tools/, resources/, workflows/

Step 3: Implementing get_forecast tool

[Provides type-safe tool implementation]

Editing: crates/mcp-weather-core/src/tools/get_forecast.rs
Editing: crates/mcp-weather-core/src/lib.rs

Step 4: Start dev server
$ cargo pmcp dev --server weather

Step 5: Generate & run tests
$ cargo pmcp test --server weather --generate-scenarios
$ cargo pmcp test --server weather

✅ All quality gates passing!
```

## What the Subagent Knows

### Core Knowledge

- **MCP Protocol**: Tools, resources, prompts, workflows
- **pmcp SDK 1.8.3+**: Rust implementation (16x faster than TypeScript)
- **cargo-pmcp toolkit**: Scaffolding, dev server, testing
- **Type safety**: JsonSchema, validation, error handling
- **OAuth**: pmcp 1.8.0+ auth context pass-through
- **Quality standards**: Toyota Way principles

### Workflow Expertise

The subagent enforces the **correct cargo-pmcp workflow**:

1. `cargo pmcp new <workspace>` - Create workspace
2. `cargo pmcp add server <name> --template <type>` - Scaffold server
3. Edit generated tool files only
4. `cargo pmcp dev --server <name>` - Hot-reload dev server
5. `cargo pmcp test --generate-scenarios` - Generate tests
6. `cargo pmcp test --server <name>` - Run tests
7. Quality gates (fmt, clippy, test)
8. `cargo build --release` - Production build

### Templates Available

| Template | Use Case | When to Use |
|----------|----------|-------------|
| `minimal` | Custom servers | GitHub API, Slack, custom tools |
| `calculator` | Learning | Understanding MCP basics |
| `complete_calculator` | Reference | See all MCP capabilities |
| `sqlite_explorer` | Databases | PostgreSQL, MySQL browsers |

### Tool Implementation Patterns

1. **Simple calculation tools** - Stateless operations
2. **External API calls** - HTTP clients, error handling
3. **Database access** - Connection pooling, query validation
4. **Stateful tools** - Shared state management
5. **OAuth tools** - Authentication token handling

## Key Features

### 1. Never Creates Files Manually

The subagent is trained to **ALWAYS** use cargo-pmcp commands and **NEVER** create Cargo.toml, lib.rs, main.rs, or directory structures manually.

**Before** (wrong approach):
```bash
mkdir -p crates/mcp-weather-core
touch Cargo.toml
# Manual file creation...
```

**After** (subagent's approach):
```bash
cargo pmcp add server weather --template minimal
# Everything scaffolded correctly!
```

### 2. Type-Safe Implementation

Generates code with:
- `JsonSchema` derives for auto-generated schemas
- Input validation at multiple levels
- Comprehensive error handling
- No `unwrap()` in production code

### 3. Testing Integration

Automatically:
- Generates test scenarios with `--generate-scenarios`
- Runs comprehensive test suites
- Validates quality gates
- Ensures 80%+ test coverage

### 4. Quality Enforcement

Enforces Toyota Way standards:
- Complexity: ≤25 per function
- Technical Debt: 0 SATD comments
- Formatting: 100% cargo fmt
- Linting: 0 clippy warnings
- Testing: ≥80% coverage

## Configuration

The subagent is pre-configured with optimal settings:

```yaml
name: mcp-developer
description: Expert MCP server developer using pmcp Rust SDK and cargo-pmcp toolkit. Use PROACTIVELY when user asks to build, scaffold, or develop MCP servers.
tools: Read, Write, Edit, Bash, Grep, Glob, Task
model: sonnet
```

### Why These Tools?

- **Read/Grep/Glob**: Explore existing code and templates
- **Write/Edit**: Implement tool code in generated files
- **Bash**: Run cargo-pmcp commands, build, test
- **Task**: Delegate subtasks if needed

### Why Sonnet?

Sonnet provides the best balance of:
- Code quality and accuracy
- Understanding of Rust patterns
- Following complex workflows
- Cost-effectiveness

## Common Use Cases

### 1. Create New MCP Server

```
> Create an MCP server for GitHub repository operations
```

**Subagent will**:
- Create workspace with `cargo pmcp new`
- Add server with appropriate template
- Implement tools following patterns
- Set up testing and quality gates

### 2. Add Tool to Existing Server

```
> Add a 'create_issue' tool to my GitHub server
```

**Subagent will**:
- Edit `crates/mcp-github-core/src/tools/create_issue.rs`
- Implement type-safe input/output
- Add validation and error handling
- Update lib.rs to register tool
- Add tests

### 3. Fix Quality Issues

```
> My server has clippy warnings, help me fix them
```

**Subagent will**:
- Run `cargo clippy`
- Analyze warnings
- Fix issues following best practices
- Validate quality gates pass

### 4. Improve Error Handling

```
> My weather tool needs better error handling
```

**Subagent will**:
- Review current error handling
- Add validation for edge cases
- Improve error messages
- Add context to errors
- Add error case tests

## Best Practices

### For Best Results

1. **Be Specific**: "Create a weather server with forecast and alerts tools"
2. **Mention MCP**: Triggers automatic subagent invocation
3. **Ask for Tests**: "Create a server and include comprehensive tests"
4. **Request Quality**: "Build this following Toyota Way standards"

### What to Expect

The subagent will:
- ✅ Use cargo-pmcp commands
- ✅ Generate production-ready code
- ✅ Include tests and validation
- ✅ Follow quality standards
- ✅ Explain each step clearly
- ✅ Provide next steps

The subagent will NOT:
- ❌ Create files manually
- ❌ Skip testing
- ❌ Use unwrap() in production
- ❌ Ignore quality gates
- ❌ Suggest manual scaffolding

## Troubleshooting

### Subagent Not Loading

```bash
# Verify file location
ls ~/.claude/agents/mcp-developer.md
# or
ls .claude/agents/mcp-developer.md

# Check file format (YAML frontmatter + markdown)
head -20 ~/.claude/agents/mcp-developer.md
```

### Subagent Not Invoked Automatically

Try explicit invocation:
```
> Use the mcp-developer subagent to create a calculator server
```

Or check the description matches your request:
- Keywords: "MCP server", "build", "create", "scaffold"

### cargo-pmcp Commands Failing

```bash
# Ensure cargo-pmcp is installed
cargo install cargo-pmcp

# Update to latest
cargo install cargo-pmcp --force

# Verify version
cargo pmcp --version
```

## Example Sessions

### Session 1: Simple Calculator

**User**: "Build a calculator MCP server"

**Subagent**:
1. Creates workspace with `cargo pmcp new calc-workspace`
2. Adds server with `cargo pmcp add server calculator --template calculator`
3. Explains generated structure
4. Starts dev server
5. Generates and runs tests
6. Validates quality gates

**Time**: ~2 minutes
**Output**: Production-ready calculator server

### Session 2: Weather API Integration

**User**: "Create a weather forecast MCP server"

**Subagent**:
1. Creates workspace
2. Adds server with `minimal` template (API integration pattern)
3. Implements `get_forecast` tool with:
   - Input validation (city, days)
   - External API call with timeout
   - Error handling (404, 401, network)
   - Type-safe response parsing
4. Adds comprehensive tests
5. Shows how to configure API key

**Time**: ~5 minutes
**Output**: Production-ready weather server

### Session 3: Database Explorer

**User**: "Build a PostgreSQL database browser MCP server"

**Subagent**:
1. Creates workspace
2. Adds server with `sqlite_explorer` template
3. Customizes for PostgreSQL:
   - Connection pooling
   - Query validation (SELECT only)
   - Dynamic resource discovery
4. Implements tools:
   - `list_tables`
   - `get_schema`
   - `execute_query`
5. Adds comprehensive tests with mocked DB

**Time**: ~10 minutes
**Output**: Production-ready database browser

## Updating the Subagent

To update to a newer version:

```bash
# Backup current version
cp ~/.claude/agents/mcp-developer.md ~/.claude/agents/mcp-developer.md.backup

# Copy new version
cp path/to/new/mcp-developer.md ~/.claude/agents/

# Restart Claude Code
```

## Integration with Kiro Power

This Claude Code subagent is based on the same knowledge as the **Kiro MCP Developer Power** (`ai-agents/kiro/mcp-developer-power`). Both share:

- Same workflow principles
- Same quality standards
- Same patterns and examples
- Same Toyota Way enforcement

**Differences**:
- **Kiro Power**: Uses steering files (always active, 4,487 lines)
- **Claude Code Subagent**: Self-contained agent (invoked on-demand, ~600 lines)

Both approaches ensure AI builds MCP servers **the right way** using cargo-pmcp.

## Contributing

Found an issue or have a suggestion?

1. Test the improvement with the subagent
2. Update `mcp-developer.md`
3. Submit PR to rust-mcp-sdk repository
4. Share learnings with community

## Resources

- **pmcp SDK**: https://docs.rs/pmcp
- **cargo-pmcp**: https://github.com/paiml/rust-mcp-sdk/tree/main/cargo-pmcp
- **MCP Spec**: https://modelcontextprotocol.io
- **Examples**: https://github.com/paiml/rust-mcp-sdk/tree/main/examples
- **Kiro Power**: https://github.com/paiml/rust-mcp-sdk/tree/main/ai-agents/kiro/mcp-developer-power

## License

MIT - Part of the pmcp SDK ecosystem

---

**Version**: 1.0.0
**Compatible with**: Claude Code (all versions with subagent support)
**Based on**: pmcp SDK 1.8.3+, cargo-pmcp 0.1.0+
**Last Updated**: 2025-11-13
