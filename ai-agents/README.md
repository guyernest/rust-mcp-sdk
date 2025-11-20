# AI Agent Configurations for MCP Development

> **Transform your AI coding assistant into an MCP development expert**

## Vision

Building Model Context Protocol (MCP) servers should be as simple as describing what you want. This directory contains **AI agent configurations** that teach your favorite AI coding assistant how to build production-grade MCP servers using the pmcp Rust SDK and cargo-pmcp toolkit.

**The concept is simple**: Instead of manually learning every detail of MCP server development, your AI assistant learns it for you through curated knowledge files. You describe what you want to build, and your AI assistant scaffolds, implements, tests, and validates production-ready MCP servers following proven best practices.

**Why this matters**:
- **Lower the barrier**: MCP server development becomes accessible to developers at all skill levels
- **Enforce best practices**: AI assistants follow Toyota Way principles, cargo-pmcp workflow, and quality standards automatically
- **Accelerate development**: Scaffold complete servers in seconds, implement tools in minutes
- **Maintain quality**: Every server gets 80%+ test coverage, zero clippy warnings, comprehensive validation
- **Learn by doing**: As your AI builds servers, you learn MCP concepts through working examples

**Open ecosystem**: While we provide configurations for Kiro and Claude Code, the same knowledge can be adapted for any AI coding assistant. We encourage the community to create configurations for other AI agents (see [Community Implementations](#community-implementations)).

## Quick Start

Choose your AI assistant:

### For Kiro Users

**Prerequisites**: [Install Kiro](https://kiro.ai) first

**Install the MCP Developer Power**:

```bash
# Download and install
curl -fsSL https://raw.githubusercontent.com/paiml/rust-mcp-sdk/main/ai-agents/kiro/install.sh | bash

# Or manual install
mkdir -p ~/.kiro/powers
cd ~/.kiro/powers
git clone --depth 1 --filter=blob:none --sparse https://github.com/paiml/rust-mcp-sdk.git temp
cd temp
git sparse-checkout set ai-agents/kiro/mcp-developer-power
mv ai-agents/kiro/mcp-developer-power ../mcp-developer
cd .. && rm -rf temp

# Restart Kiro
```

**Verify**: Ask Kiro "How do I build an MCP server?"

### For Claude Code Users

**Prerequisites**: [Install Claude Code](https://claude.ai/code) first

**Install the MCP Developer Subagent**:

```bash
# User-level (available across all projects) - RECOMMENDED
curl -fsSL https://raw.githubusercontent.com/paiml/rust-mcp-sdk/main/ai-agents/claude-code/mcp-developer.md \
  -o ~/.claude/agents/mcp-developer.md

# OR project-level (current project only)
mkdir -p .claude/agents
curl -fsSL https://raw.githubusercontent.com/paiml/rust-mcp-sdk/main/ai-agents/claude-code/mcp-developer.md \
  -o .claude/agents/mcp-developer.md

# Restart Claude Code
```

**Verify**: Run `/agents` and look for "mcp-developer"

## What You Get

Both configurations teach your AI assistant to:

✅ **Always use cargo-pmcp** (never create files manually)
✅ **Follow proven workflow**: scaffold → implement → test → validate
✅ **Generate production-ready code**: type-safe, validated, tested
✅ **Enforce quality standards**: 80%+ coverage, zero warnings, Toyota Way
✅ **Implement MCP patterns**: tools, resources, workflows, prompts

## Example Usage

### With Kiro

```
You: "Create a weather forecast MCP server"

Kiro: [Reads steering files - always active]

I'll create a weather MCP server using cargo-pmcp.

Step 1: Create workspace
$ cargo pmcp new weather-mcp-workspace
$ cd weather-mcp-workspace

Step 2: Add server
$ cargo pmcp add server weather --template minimal

[Implements tools with type safety and validation]
[Generates tests and validates quality gates]

✅ Production-ready server complete!
```

### With Claude Code

```
You: "Create a weather forecast MCP server"

Claude Code: [Invokes mcp-developer subagent]

I'll use the cargo-pmcp workflow:

$ cargo pmcp new weather-mcp-workspace
$ cargo pmcp add server weather --template minimal

[Implements tools following best practices]
[Sets up testing and validation]

✅ Server ready for development!
```

## Prerequisites for MCP Development

Before using these AI assistants for MCP development:

```bash
# 1. Install Rust (1.70+)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup update

# 2. Install cargo-pmcp toolkit
cargo install cargo-pmcp

# 3. Verify installation
cargo pmcp --version
```

## What's Included

### Kiro MCP Developer Power

**Location**: `kiro/mcp-developer-power/`

**Type**: Steering files (always active knowledge base)

**Size**: 4,487 lines across 5 files

**Files**:
- `steering/mcp-product.md` - MCP concepts, use cases, decision frameworks
- `steering/mcp-tech.md` - Technology stack, patterns, async programming
- `steering/mcp-structure.md` - Project organization, naming conventions
- `steering/mcp-workflow.md` - **CRITICAL** cargo-pmcp workflow (never manual files)
- `steering/mcp-tool-patterns.md` - Tool implementation patterns

**How it works**: Kiro reads these files in every conversation, providing persistent MCP expertise.

**Learn more**: [kiro/mcp-developer-power/README.md](kiro/mcp-developer-power/README.md)

### Claude Code MCP Developer Subagent

**Location**: `claude-code/mcp-developer.md`

**Type**: Subagent (invoked on-demand)

**Size**: ~600 lines (self-contained)

**How it works**: Claude Code invokes this specialist when MCP development tasks are detected.

**Learn more**: [claude-code/README.md](claude-code/README.md)

## Key Differences

| Aspect | Kiro Power | Claude Code Subagent |
|--------|------------|---------------------|
| **Activation** | Always active (steering) | On-demand (subagent) |
| **Context** | Persistent across conversations | Task-specific context |
| **Size** | 4,487 lines (deep knowledge) | ~600 lines (focused) |
| **Format** | Multiple markdown files | Single markdown file |
| **Best for** | Learning + Building | Quick scaffolding |

Both enforce the same workflow and quality standards.

## The cargo-pmcp Workflow

Both AI assistants enforce this workflow (and will **never** create files manually):

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

**Key point**: Steps 1-2 scaffold the entire structure. You only write code in step 3.

## Templates Available

The AI assistants help you choose the right template:

| Template | Use Case | Example |
|----------|----------|---------|
| `minimal` | Custom servers | Weather API, GitHub, Slack |
| `calculator` | Learning MCP | Understanding basics |
| `complete_calculator` | Full reference | All MCP capabilities |
| `sqlite_explorer` | Database browsers | PostgreSQL, MySQL |

## Quality Standards Enforced

Both AI assistants enforce Toyota Way principles:

- **Complexity**: ≤25 per function
- **Technical Debt**: 0 SATD comments
- **Test Coverage**: ≥80%
- **Linting**: 0 clippy warnings
- **Formatting**: 100% cargo fmt compliant

## Updates

### Updating Kiro Power

```bash
cd ~/.kiro/powers/mcp-developer
git pull origin main
# Restart Kiro
```

### Updating Claude Code Subagent

```bash
curl -fsSL https://raw.githubusercontent.com/paiml/rust-mcp-sdk/main/ai-agents/claude-code/mcp-developer.md \
  -o ~/.claude/agents/mcp-developer.md
# Restart Claude Code
```

## Troubleshooting

### Kiro: Power not loading

```bash
# Check installation
ls ~/.kiro/powers/mcp-developer/steering/

# Should show 5 .md files
```

### Claude Code: Subagent not found

```bash
# Check installation
ls ~/.claude/agents/mcp-developer.md

# Verify with /agents command
```

### cargo-pmcp not found

```bash
cargo install cargo-pmcp
cargo pmcp --version
```

## Community Implementations

**We welcome and encourage implementations for other AI coding assistants!**

The MCP developer knowledge contained in this directory can be adapted to any AI coding assistant that supports:
- Loading external knowledge or instructions
- Executing shell commands (cargo-pmcp workflow)
- Reading and editing files (tool implementation)

### AI Assistants We'd Love to See

| AI Assistant | Status | How You Can Help |
|--------------|--------|------------------|
| **Kiro** | ✅ Available | [Use it](kiro/mcp-developer-power/) |
| **Claude Code** | ✅ Available | [Use it](claude-code/) |
| **GitHub Copilot** | ⚪ Not started | Create `.github/copilot-instructions.md` |
| **Cursor** | ⚪ Not started | Create `.cursorrules` configuration |
| **Cline (VSCode)** | ⚪ Not started | Create Cline custom instructions |
| **Aider** | ⚪ Not started | Create `.aider.conf.yml` with conventions |
| **Continue.dev** | ⚪ Not started | Create `.continuerc.json` configuration |
| **Windsurf** | ⚪ Not started | Create Windsurf agent configuration |
| **Your AI assistant?** | ⚪ We'd love your contribution! | See [Creating New Configurations](#creating-new-configurations) |

### Creating New Configurations

Want to create an MCP developer configuration for your favorite AI coding assistant? Here's how:

1. **Study the existing implementations**:
   - [Kiro Power](kiro/mcp-developer-power/) - Multi-file steering approach (4,487 lines)
   - [Claude Code Subagent](claude-code/mcp-developer.md) - Single-file approach (~600 lines)

2. **Core knowledge to include** (adapt to your AI's format):
   - **Critical workflow**: Always use cargo-pmcp, never create files manually
   - **MCP concepts**: Tools, resources, prompts, workflows
   - **Technology stack**: pmcp SDK, Tokio, type safety, error handling
   - **Project structure**: Workspace layout, naming conventions
   - **Tool patterns**: 5 implementation patterns with examples
   - **Testing workflow**: mcp-tester scenarios, quality gates
   - **Quality standards**: Toyota Way principles (complexity ≤25, 80%+ coverage, zero warnings)

3. **Adapt to your AI's capabilities**:
   - **Persistent knowledge**: AI reads files every conversation (like Kiro steering)
   - **On-demand knowledge**: AI loads context when needed (like Claude Code subagents)
   - **Instruction files**: AI follows pre-configured rules (like Cursor .cursorrules)
   - **System prompts**: AI receives instructions at session start

4. **Test thoroughly**:
   - Ask your AI: "Create a weather MCP server"
   - Verify it uses `cargo pmcp new` and `cargo pmcp add server` (not manual files)
   - Check quality: Does it generate tests? Enforce clippy? Follow patterns?

5. **Share with the community**:
   - Create a directory: `ai-agents/<your-ai-name>/`
   - Include README.md with installation and usage
   - Submit PR to this repository
   - Share your success story in Discussions

**Bonus**: If your AI assistant has unique capabilities (vision, voice, multi-agent coordination), we'd love to see those leveraged for MCP development!

## Contributing

Contributions are highly encouraged! There are several ways to help:

### Improve Existing Configurations

1. Test changes with Kiro or Claude Code
2. Update relevant files:
   - Kiro: `kiro/mcp-developer-power/steering/*.md`
   - Claude Code: `claude-code/mcp-developer.md`
3. Submit PR with examples of improved AI behavior
4. Share learnings in Discussions

### Create New AI Agent Configurations

Follow the [Creating New Configurations](#creating-new-configurations) guide above.

**High priority**: GitHub Copilot, Cursor, Cline - these have large user bases.

### Enhance Knowledge Content

- Add new tool implementation patterns
- Document advanced MCP features (OAuth, workflows)
- Add testing and observability best practices
- Share production deployment patterns

### Report AI Behavior

Found cases where your AI assistant doesn't follow best practices?

1. Document the issue (what you asked, what it did wrong)
2. Identify the knowledge gap
3. Propose improvements to steering/agent files
4. Test and submit PR

## Resources

- **pmcp SDK**: https://docs.rs/pmcp
- **cargo-pmcp**: https://github.com/paiml/rust-mcp-sdk/tree/main/cargo-pmcp
- **MCP Spec**: https://modelcontextprotocol.io
- **Examples**: https://github.com/paiml/rust-mcp-sdk/tree/main/examples

## Support

- **Issues**: https://github.com/paiml/rust-mcp-sdk/issues
- **Discussions**: https://github.com/paiml/rust-mcp-sdk/discussions

---

**Version**: 1.0.1 (Kiro Power) / 1.0.0 (Claude Code Subagent)
**Last Updated**: 2025-11-13
**Maintained by**: PMCP Team
