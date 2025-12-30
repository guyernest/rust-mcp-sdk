# Alternative AI Assistants

While Claude Code is our primary recommendation, the MCP developer knowledge can be adapted to other AI coding assistants. This chapter covers configuration for popular alternatives.

## Kiro

Kiro uses "steering files" - always-active knowledge that persists across conversations.

### Installation

```bash
# Create powers directory
mkdir -p ~/.kiro/powers

# Clone the MCP developer power
cd ~/.kiro/powers
git clone --depth 1 --filter=blob:none --sparse \
  https://github.com/paiml/rust-mcp-sdk.git temp
cd temp
git sparse-checkout set ai-agents/kiro/mcp-developer-power
mv ai-agents/kiro/mcp-developer-power ../mcp-developer
cd .. && rm -rf temp

# Restart Kiro
```

### Verify Installation

```bash
# Check files
ls ~/.kiro/powers/mcp-developer/steering/

# Should show:
# mcp-product.md
# mcp-tech.md
# mcp-structure.md
# mcp-workflow.md
# mcp-tool-patterns.md
```

### How It Works

Kiro's steering files are **always active** - Kiro reads them for every conversation:

```
steering/
├── mcp-product.md      # MCP concepts, use cases
├── mcp-tech.md         # Technology stack, patterns
├── mcp-structure.md    # Project organization
├── mcp-workflow.md     # CRITICAL: cargo-pmcp workflow
└── mcp-tool-patterns.md # Tool implementation patterns
```

### Usage

Simply ask Kiro to build an MCP server - it automatically knows the workflow:

```
You: Create a weather MCP server

Kiro: I'll create a weather server using cargo-pmcp.

$ cargo pmcp new weather-workspace
$ cd weather-workspace
$ cargo pmcp add server weather --template minimal

[Implements tools following patterns from steering files]
```

### Kiro vs Claude Code

| Aspect | Kiro | Claude Code |
|--------|------|-------------|
| **Knowledge type** | Always-active steering | On-demand agent |
| **Context size** | 10,000+ lines persistent | ~600 lines per invocation |
| **Best for** | Deep learning + building | Quick scaffolding |
| **MCP integration** | Native MCP client | Native MCP client |

## Cursor

Cursor uses `.cursorrules` for project-specific instructions.

### Configuration

Create `.cursorrules` in your project root:

```markdown
# MCP Server Development Rules

## CRITICAL: Always Use cargo-pmcp

NEVER create Cargo.toml, lib.rs, main.rs, or directories manually.
ALWAYS use cargo-pmcp commands:

```bash
# Create workspace (one-time)
cargo pmcp new <workspace-name>

# Add server
cargo pmcp add server <name> --template <template>

# Add tool to existing server
cargo pmcp add tool <tool-name> --server <server-name>
```

## Templates
- `minimal` - Empty structure for custom servers
- `calculator` - Simple arithmetic example
- `sqlite_explorer` - Database browser pattern

## Tool Implementation Pattern

```rust
use pmcp::{Result, TypedTool, RequestHandlerExtra, Error};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct MyInput {
    #[schemars(description = "Parameter description")]
    pub param: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct MyOutput {
    pub result: String,
}

async fn handler(input: MyInput, _: RequestHandlerExtra) -> Result<MyOutput> {
    // Validate
    if input.param.is_empty() {
        return Err(Error::validation("Param required"));
    }

    // Process
    Ok(MyOutput { result: input.param })
}

pub fn build_tool() -> TypedTool<MyInput, MyOutput> {
    TypedTool::new("my-tool", |input, extra| {
        Box::pin(handler(input, extra))
    })
    .with_description("Tool description")
}
```

## Quality Standards
- Run `cargo fmt --check` before committing
- Zero clippy warnings: `cargo clippy -- -D warnings`
- Minimum 80% test coverage
- Never use unwrap() in production code
```

### Usage

With `.cursorrules` in place, Cursor follows these rules automatically when editing Rust MCP code.

## GitHub Copilot

Copilot uses `.github/copilot-instructions.md` for repository-level guidance.

### Configuration

Create `.github/copilot-instructions.md`:

```markdown
# MCP Server Development Instructions

This repository contains MCP (Model Context Protocol) servers built with the
pmcp Rust SDK and cargo-pmcp toolkit.

## Development Workflow

1. **Scaffolding**: Always use `cargo pmcp` commands
   - `cargo pmcp new` for workspaces
   - `cargo pmcp add server` for new servers
   - Never create files manually

2. **Tool Pattern**: Use TypedTool with JsonSchema
   - Input types derive: Debug, Deserialize, JsonSchema
   - Output types derive: Debug, Serialize, JsonSchema
   - Handlers return Result<Output>

3. **Error Handling**: Use pmcp::Error types
   - Error::validation() for user errors
   - Error::internal() for server errors
   - Always add context with .context()

4. **Testing**: Use mcp-tester scenarios
   - `cargo pmcp test --generate-scenarios` to generate
   - `cargo pmcp test` to run
   - Minimum 80% coverage

## Code Style

- Format with `cargo fmt`
- Lint with `cargo clippy -- -D warnings`
- No unwrap() in production code
- Comprehensive error messages
```

## Aider

Aider uses `.aider.conf.yml` for configuration.

### Configuration

Create `.aider.conf.yml`:

```yaml
# Aider configuration for MCP development

## Model settings
model: claude-3-5-sonnet-20241022

## Convention files to always include
read:
  - .github/copilot-instructions.md
  - CONVENTIONS.md

## Auto-commit settings
auto-commits: false
dirty-commits: false

## Lint command (runs after edits)
lint-cmd: cargo fmt --check && cargo clippy -- -D warnings

## Test command
test-cmd: cargo test
```

Create `CONVENTIONS.md`:

```markdown
# MCP Development Conventions

## Scaffolding
ALWAYS use cargo-pmcp for project structure:
- `cargo pmcp new <workspace>` - Create workspace
- `cargo pmcp add server <name> --template minimal` - Add server

## Tool Structure
- Input: `#[derive(Debug, Deserialize, JsonSchema)]`
- Output: `#[derive(Debug, Serialize, JsonSchema)]`
- Handler: `async fn handler(input, extra) -> Result<Output>`
- Builder: `TypedTool::new("name", handler)`

## Error Handling
- Validation errors: `Error::validation("message")`
- Internal errors: `Error::internal("message")`
- Context: `.context("Failed to...")?`
- Never: `unwrap()`, `expect()`, `panic!()`

## Testing
- Unit tests in same file: `#[cfg(test)] mod tests { ... }`
- Integration: `cargo pmcp test --server <name>`
- Coverage: minimum 80%
```

## Continue.dev

Continue uses `.continuerc.json` for configuration.

### Configuration

Create `.continuerc.json`:

```json
{
  "customCommands": [
    {
      "name": "mcp-new",
      "description": "Create new MCP workspace",
      "prompt": "Create a new MCP server workspace using cargo-pmcp. Follow these steps:\n1. cargo pmcp new {workspace-name}\n2. cd {workspace-name}\n3. cargo pmcp add server {server-name} --template minimal"
    },
    {
      "name": "mcp-tool",
      "description": "Add MCP tool",
      "prompt": "Add a new tool to the MCP server. Use TypedTool with proper JsonSchema types. Include validation and error handling. Add unit tests."
    }
  ],
  "contextProviders": [
    {
      "name": "pmcp-docs",
      "type": "url",
      "url": "https://docs.rs/pmcp/latest/pmcp/"
    }
  ],
  "systemPrompt": "When working on MCP servers:\n- Always use cargo-pmcp commands for scaffolding\n- Follow TypedTool pattern with JsonSchema\n- Use Error::validation() and Error::internal()\n- Add .context() to all error paths\n- Write unit tests for all handlers"
}
```

## Windsurf

Windsurf uses agent configurations similar to Claude Code.

### Configuration

Create `.windsurf/agents/mcp-developer.md`:

```markdown
---
name: mcp-developer
description: MCP server developer using pmcp Rust SDK
triggers:
  - mcp
  - server
  - pmcp
  - tool
---

# MCP Development Agent

You are an expert MCP server developer using the pmcp Rust SDK.

## Critical Rules

1. **ALWAYS** use cargo-pmcp for scaffolding:
   - `cargo pmcp new <workspace>` for new projects
   - `cargo pmcp add server <name> --template minimal` for servers
   - NEVER create Cargo.toml or directory structure manually

2. **Tool Pattern**:
   - Input types: `#[derive(Debug, Deserialize, JsonSchema)]`
   - Output types: `#[derive(Debug, Serialize, JsonSchema)]`
   - Handlers: `async fn(Input, RequestHandlerExtra) -> Result<Output>`

3. **Quality Gates**:
   - `cargo fmt --check` - formatting
   - `cargo clippy -- -D warnings` - linting
   - `cargo test` - tests pass
   - 80%+ test coverage

4. **Error Handling**:
   - Never use unwrap() or expect()
   - Use Error::validation() for user errors
   - Use Error::internal() for server errors
   - Add .context() to error paths
```

## Creating Custom Configurations

### Core Knowledge to Include

Any AI assistant configuration should include:

1. **Workflow** (most critical):
   - Use cargo-pmcp commands
   - Never create files manually
   - Follow scaffold → implement → test → validate flow

2. **Type Patterns**:
   - JsonSchema derives for auto-schema generation
   - Proper input/output type definitions
   - TypedTool builder pattern

3. **Error Handling**:
   - Error types and when to use each
   - Context addition with anyhow
   - No unwrap/panic rules

4. **Quality Standards**:
   - Format, lint, test commands
   - Coverage requirements
   - Toyota Way principles

### Template

```markdown
# MCP Developer Configuration for [AI Tool]

## Workflow
- Scaffold: `cargo pmcp new` and `cargo pmcp add server`
- Implement: Edit `crates/mcp-*-core/src/tools/*.rs`
- Test: `cargo pmcp test --generate-scenarios && cargo pmcp test`
- Validate: `cargo fmt && cargo clippy && cargo test`

## Tool Pattern
[Include TypedTool example]

## Error Handling
[Include Error types and .context() usage]

## Quality Gates
[Include specific commands and thresholds]
```

## Summary

| AI Assistant | Configuration | Location |
|--------------|--------------|----------|
| Claude Code | Agent markdown | `~/.claude/agents/` |
| Kiro | Steering files | `~/.kiro/powers/` |
| Cursor | Rules file | `.cursorrules` |
| Copilot | Instructions | `.github/copilot-instructions.md` |
| Aider | YAML config | `.aider.conf.yml` |
| Continue | JSON config | `.continuerc.json` |
| Windsurf | Agent markdown | `.windsurf/agents/` |

All configurations encode the same core knowledge:
- cargo-pmcp workflow
- TypedTool patterns
- Error handling standards
- Quality gate requirements

Choose based on your preferred AI assistant, or contribute new configurations to the community.

---

*Continue to [Effective AI Collaboration](./ch16-collaboration.md) →*
