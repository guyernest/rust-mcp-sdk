# AI-Assisted MCP Development

> **Advanced Topic**: Accelerate MCP server development with AI coding assistants

## Overview

This chapter introduces **AI-assisted MCP development** - using AI coding assistants like Kiro and Claude Code to accelerate building production-grade MCP servers with the pmcp Rust SDK and cargo-pmcp toolkit.

**Important**: This is an advanced topic. Before using AI assistance, you should understand:
- MCP protocol basics (Tools, Resources, Prompts, Workflows)
- cargo-pmcp workflow
- Rust async programming fundamentals
- Testing and quality standards

**Why read this chapter**:
- Learn how to configure AI assistants for MCP development
- Accelerate scaffolding and implementation
- Maintain quality standards with AI assistance
- Understand what AI can and cannot do well

## The Vision

Building MCP servers should be as simple as describing what you want:

**You**: "Create a weather forecast MCP server"

**AI Assistant**:
```
I'll create a production-ready weather MCP server using cargo-pmcp.

Step 1: Creating workspace
$ cargo pmcp new weather-mcp-workspace

Step 2: Adding server with minimal template
$ cargo pmcp add server weather --template minimal

Step 3: Implementing get_forecast tool
[Generates type-safe tool with validation and error handling]

Step 4: Adding tests
[Generates unit tests and integration scenarios]

Step 5: Validating quality
[Runs fmt, clippy, tests]

✅ Production-ready server complete with 85% test coverage!
```

This is now possible with **AI agent configurations** that teach your coding assistant how to build MCP servers correctly.

## What AI Assistants Can Do

✅ **Excellent at**:
- Scaffolding servers with `cargo pmcp new` and `cargo pmcp add server`
- Generating type-safe tool implementations following patterns
- Writing comprehensive unit and integration tests
- Adding proper error handling and validation
- Following cargo-pmcp workflow consistently
- Generating test scenarios and running quality checks

⚠️ **Good with guidance**:
- Choosing the right template for your use case
- Implementing complex business logic
- Integrating with external APIs
- Performance optimization

❌ **Not suitable for**:
- Understanding your domain/business requirements (you must specify)
- Making architectural decisions without context
- Security decisions (review all authentication/authorization code)
- Production deployment decisions (you must understand implications)

## Supported AI Assistants

### Kiro (Steering Files)

**What it is**: AI coding assistant with persistent knowledge via "steering files"

**How it works**: Kiro reads markdown files in every conversation, providing constant MCP expertise

**Best for**: Learning MCP while building servers

**Installation**:
```bash
mkdir -p ~/.kiro/powers
cd ~/.kiro/powers
git clone --depth 1 --filter=blob:none --sparse https://github.com/paiml/rust-mcp-sdk.git temp
cd temp
git sparse-checkout set ai-agents/kiro/mcp-developer-power
mv ai-agents/kiro/mcp-developer-power ../mcp-developer
cd .. && rm -rf temp
# Restart Kiro
```

**Knowledge included** (10,876 lines across 7 steering files):
- MCP concepts and decision frameworks
- Technology stack and patterns
- Project structure and naming conventions
- **cargo-pmcp workflow** (critical - prevents manual file creation)
- Tool implementation patterns
- Testing strategies (comprehensive guide)
- Observability best practices (production-ready)

**Example usage**:
```
You: "Create a calculator MCP server"

Kiro: [Reads steering files automatically]

I'll use the cargo-pmcp workflow to create a calculator server.

Step 1: Create workspace
$ cargo pmcp new calc-mcp-workspace
$ cd calc-mcp-workspace

This creates the workspace structure with proper dependencies.

Step 2: Add calculator server
$ cargo pmcp add server calculator --template calculator

This scaffolds:
- mcp-calculator-core/ (library crate with tools)
- calculator-server/ (binary crate)
- Complete directory structure

The calculator template includes pre-built arithmetic tools.

Step 3: Review generated structure
[Shows what was created]

The server is ready to use! Let me show you how to start it...

$ cargo pmcp dev --server calculator

Next steps:
1. Test the tools: cargo pmcp test --server calculator
2. Customize: Add more mathematical operations
3. Deploy: cargo build --release

Would you like me to add more advanced operations like trigonometry or statistics?
```

### Claude Code (Subagent)

**What it is**: Official Anthropic CLI with subagent system

**How it works**: Invokes specialist subagents for specific tasks

**Best for**: Quick scaffolding and implementation

**Installation**:
```bash
# User-level (recommended - works across all projects)
curl -fsSL https://raw.githubusercontent.com/paiml/rust-mcp-sdk/main/ai-agents/claude-code/mcp-developer.md \
  -o ~/.claude/agents/mcp-developer.md

# Restart Claude Code
```

**Knowledge included** (~750 lines, self-contained):
- cargo-pmcp workflow enforcement
- MCP patterns and best practices
- Type-safe implementation examples
- Testing and observability basics
- Quality standards

**Example usage**:
```
You: "Create a GitHub API MCP server"

Claude Code: [Invokes mcp-developer subagent]

I'll build a GitHub API MCP server using the minimal template.

$ cargo pmcp new github-mcp-workspace
$ cd github-mcp-workspace
$ cargo pmcp add server github --template minimal

Now I'll implement the GitHub API tools...

[Implements get_repository, list_issues, create_pr tools]
[Adds proper authentication handling]
[Generates tests]
[Validates quality]

✅ GitHub MCP server ready!

Quality metrics:
- Test coverage: 82%
- Clippy warnings: 0
- All tests passing

Next: Add your GitHub token to .env and run:
$ cargo pmcp dev --server github
```

### Community Implementations

The AI agent configurations can be adapted for any AI coding assistant:

- **GitHub Copilot**: Via `.github/copilot-instructions.md` (community contribution welcome)
- **Cursor**: Via `.cursorrules` file (community contribution welcome)
- **Cline**: Via custom instructions (community contribution welcome)
- **Others**: See [ai-agents/README.md](../ai-agents/README.md) for contribution guide

## The cargo-pmcp Workflow (Critical)

AI assistants are configured to **ALWAYS** use cargo-pmcp commands and **NEVER** create files manually.

### Why This Matters

cargo-pmcp encodes best practices from 6 production MCP servers. Manual file creation:
- ❌ Misses proven patterns
- ❌ Creates inconsistent structure
- ❌ No hot-reload dev server
- ❌ No test scaffolding

### AI-Enforced Workflow

```bash
# 1. Create workspace (AI does this first)
cargo pmcp new my-mcp-workspace
cd my-mcp-workspace

# 2. Add server (AI scaffolds everything)
cargo pmcp add server myserver --template minimal

# 3. Implement tools (AI writes code here)
# Only edits: crates/mcp-myserver-core/src/tools/*.rs

# 4. Start dev server (AI uses for testing)
cargo pmcp dev --server myserver

# 5. Generate tests (AI creates scenarios)
cargo pmcp test --server myserver --generate-scenarios

# 6. Run tests (AI validates)
cargo pmcp test --server myserver

# 7. Quality gates (AI enforces)
cargo fmt --check && cargo clippy && cargo test
```

**Key insight**: AI assistants only write code in step 3. Everything else is cargo-pmcp commands.

## Working with AI Assistants

### Effective Prompts

**❌ Vague**:
```
"Build me an MCP server"
```

**✅ Clear**:
```
"Create an MCP server for weather forecasts with two tools:
- get_forecast: Takes city and days (1-5), returns temperature and conditions
- get_alerts: Takes city, returns weather alerts
Use the minimal template and include comprehensive tests."
```

**✅ Even better**:
```
"Create a weather MCP server using the minimal template.

Tools needed:
1. get_forecast
   - Input: city (string, required), days (number, 1-5, optional, default 1)
   - Output: {temperature: number, conditions: string, forecast: array}
   - Validation: city cannot be empty, days must be 1-5
   - API: https://api.weather.com/forecast/{city}?days={days}

2. get_alerts
   - Input: city (string, required)
   - Output: {alerts: array of {severity: string, message: string}}
   - API: https://api.weather.com/alerts/{city}

Error handling:
- Return validation error if city is empty
- Return validation error if city not found (404)
- Return internal error for API failures

Testing:
- Unit tests for validation
- Integration tests for happy path and errors
- Mock external API calls

Include structured logging with tracing and request metrics."
```

### Iterative Development

Work with AI in iterations:

**Iteration 1: Scaffold**
```
You: "Create weather MCP server with get_forecast tool"
AI: [Scaffolds server, implements basic tool]
```

**Iteration 2: Add features**
```
You: "Add caching for 5 minutes to reduce API calls"
AI: [Adds caching logic with proper expiration]
```

**Iteration 3: Improve quality**
```
You: "Add property tests for temperature ranges"
AI: [Adds proptest tests]
```

**Iteration 4: Add observability**
```
You: "Add structured logging and metrics"
AI: [Adds tracing and metrics collection]
```

### Reviewing AI-Generated Code

Always review code for:

1. **Security**:
   - [ ] No hardcoded secrets
   - [ ] Proper input validation
   - [ ] Authentication implemented correctly
   - [ ] No SQL injection (if using databases)

2. **Correctness**:
   - [ ] Logic matches requirements
   - [ ] Error handling is comprehensive
   - [ ] Edge cases are handled

3. **Quality**:
   - [ ] No `unwrap()` in production code
   - [ ] Tests actually test the logic
   - [ ] Code is readable and maintainable

4. **Performance**:
   - [ ] No blocking operations in async functions
   - [ ] Appropriate use of caching
   - [ ] Resource cleanup (connections, files)

### Common Pitfalls

**Pitfall 1: AI creates files manually**

If your AI tries to create `Cargo.toml` or `lib.rs` manually:

```
Stop! Please use cargo-pmcp to scaffold the server:
$ cargo pmcp add server myserver --template minimal
```

This should be rare with properly configured AI agents, but if it happens, redirect immediately.

**Pitfall 2: AI skips tests**

Always insist on tests:

```
Please add comprehensive tests:
1. Unit tests for all validation logic
2. Integration scenarios with cargo pmcp test --generate-scenarios
3. Error case tests
```

**Pitfall 3: AI uses unwrap()**

If AI generates code with `unwrap()`:

```
Please replace unwrap() with proper error handling using the ? operator
```

**Pitfall 4: AI doesn't add logging**

Request observability:

```
Please add structured logging with tracing:
- Log tool invocations at INFO level
- Log errors with context
- Add request duration metrics
```

## Quality Standards (AI-Enforced)

AI assistants are configured to enforce Toyota Way principles:

### Code Quality
- **Complexity**: ≤25 per function
- **Technical Debt**: 0 SATD comments (no TODO/FIXME/HACK)
- **Formatting**: 100% `cargo fmt` compliant
- **Linting**: 0 `cargo clippy` warnings

### Testing
- **Coverage**: ≥80%
- **Unit tests**: Every tool handler
- **Integration**: mcp-tester scenarios
- **Property tests**: For complex logic

### Error Handling
- **Never**: `unwrap()`, `expect()`, `panic!()`
- **Always**: `?` operator, `.context()`, comprehensive errors
- **Validation**: Client errors (4xx) vs internal errors (5xx)

### Observability
- **Logging**: Structured with `tracing`
- **Metrics**: Request counts, durations, errors
- **Production-ready**: JSON logs, proper log levels

## Example: End-to-End with AI

Let's build a complete MCP server with AI assistance:

### Step 1: Initial Request

**You**:
```
Create an MCP server for managing TODO lists with these tools:
- add_todo: Add a new TODO item
- list_todos: List all TODOs
- complete_todo: Mark a TODO as complete
- delete_todo: Delete a TODO

Use SQLite for storage. Include comprehensive tests.
```

### Step 2: AI Scaffolds

**AI**:
```
I'll create a TODO manager MCP server using SQLite.

$ cargo pmcp new todo-mcp-workspace
$ cd todo-mcp-workspace
$ cargo pmcp add server todo --template sqlite_explorer

The sqlite_explorer template provides database patterns we can adapt.

Now I'll implement the TODO tools...

[Implements 4 tools with proper database operations]
[Adds connection pooling]
[Adds input validation]
[Generates migration for TODO table]
```

### Step 3: Review and Iterate

**You review the code and request improvements**:
```
Great start! Please add:
1. Priority field to TODOs (low/medium/high)
2. Filter list_todos by priority
3. Add due dates
4. Property tests for priority validation
```

**AI adds features**:
```
Adding priority support...

[Updates schema]
[Updates tools with priority parameter]
[Adds validation for priority values]
[Adds property tests]
```

### Step 4: Testing

**You**:
```
Generate and run tests
```

**AI**:
```
$ cargo pmcp test --server todo --generate-scenarios

Generated scenarios for:
- Adding TODOs with different priorities
- Listing and filtering
- Completing and deleting
- Error cases (invalid priority, non-existent ID)

$ cargo pmcp test --server todo

✅ All 23 scenarios passing
```

### Step 5: Quality Validation

**AI automatically**:
```
Running quality checks...

$ cargo fmt --check
✅ Formatting: PASS

$ cargo clippy
✅ Linting: PASS (0 warnings)

$ cargo test
✅ Tests: PASS (47/47)

$ cargo tarpaulin
✅ Coverage: 84%

All quality gates passed!
```

### Step 6: Production Readiness

**You**:
```
Add observability for production deployment
```

**AI**:
```
Adding structured logging and metrics...

[Adds tracing to all tool handlers]
[Adds metrics for request counts and durations]
[Adds database query metrics]
[Configures JSON logging for production]

Setup in main.rs:
- Tracing subscriber with env filter
- Prometheus metrics on :9090/metrics
- Health check endpoint

Run with:
$ RUST_LOG=info ./todo-server

Metrics available at: http://localhost:9090/metrics
```

### Result

**Time**: ~15-20 minutes with AI vs. ~2-3 hours manual

**Quality**:
- ✅ Production-ready code
- ✅ 84% test coverage
- ✅ Comprehensive error handling
- ✅ Structured logging
- ✅ Metrics collection
- ✅ Zero clippy warnings
- ✅ Type-safe tool implementations

## Limitations and Considerations

### What AI Cannot Do

1. **Understand your domain**: You must specify business logic clearly
2. **Make security decisions**: Review all auth/authz code
3. **Optimize for your scale**: You must understand performance requirements
4. **Handle edge cases you don't specify**: Be comprehensive in requirements
5. **Deploy to production**: You must understand deployment implications

### When to Use Manual Development

Consider manual development when:
- **Learning MCP**: First server should be manual to understand concepts
- **Novel patterns**: Doing something not covered in AI training
- **Complex state management**: Requires deep architectural thinking
- **Performance-critical**: Needs careful optimization

### Hybrid Approach (Recommended)

Best results come from combining AI and manual work:

1. **AI scaffolds**: Use cargo-pmcp workflow
2. **You architect**: Make structural decisions
3. **AI implements**: Write boilerplate and patterns
4. **You review**: Ensure correctness and quality
5. **AI tests**: Generate comprehensive test suites
6. **You validate**: Verify tests actually test the right things

## Configuration Details

### Kiro Configuration

**Steering file architecture**:
```
~/.kiro/powers/mcp-developer/
├── steering/
│   ├── mcp-product.md      # Always active - MCP concepts
│   ├── mcp-tech.md          # Always active - Technology stack
│   ├── mcp-structure.md     # Always active - Project structure
│   ├── mcp-workflow.md      # Always active - cargo-pmcp workflow
│   ├── mcp-tool-patterns.md # Conditional - Tool patterns
│   ├── mcp-testing.md       # Manual - Testing strategies
│   └── mcp-observability.md # Manual - Logging and metrics
├── power.json               # Metadata
└── README.md                # Documentation
```

**Total knowledge**: 10,876 lines of MCP expertise

**Inclusion modes**:
- `always`: Read in every conversation (core knowledge)
- `fileMatch`: Read when editing matching files (patterns)
- `manual`: Read when user requests (advanced topics)

### Claude Code Configuration

**Subagent architecture**:
```
~/.claude/agents/
└── mcp-developer.md  # Single self-contained file (~750 lines)
```

**YAML frontmatter**:
```yaml
---
name: mcp-developer
description: Expert MCP server developer. Use PROACTIVELY for MCP tasks.
tools: Read, Write, Edit, Bash, Grep, Glob, Task
model: sonnet
---
```

**Invocation**: Automatically when MCP development is detected, or explicitly:
```
> Use mcp-developer to create a weather server
```

## Future Enhancements

### Planned AI Features

1. **Multi-agent workflows**: Specialized agents for testing, observability, deployment
2. **Quality enforcement**: Integrated into cargo-pmcp (not hooks)
3. **Deployment assistance**: AI-guided deployment to AWS, Cloudflare, etc.
4. **Performance optimization**: AI suggests caching, batching, etc.
5. **Documentation generation**: Auto-generate API docs from code

### Community Contributions

We welcome:
- Configurations for other AI assistants (Cursor, Copilot, etc.)
- Improved prompts and patterns
- Additional tool implementation examples
- Real-world case studies

See [ai-agents/README.md](../ai-agents/README.md) for contribution guidelines.

## Getting Help

### AI Assistant Not Following Workflow

If your AI tries to create files manually instead of using cargo-pmcp:

1. **Interrupt immediately**: "Stop, please use cargo-pmcp to scaffold"
2. **Reference steering files**: "Review the mcp-workflow.md steering file"
3. **Report issue**: File issue at rust-mcp-sdk repo with example

### AI Generates Poor Quality Code

1. **Be more specific**: Provide detailed requirements
2. **Iterate**: Request improvements incrementally
3. **Review carefully**: Don't accept code blindly
4. **Learn patterns**: Understand what good code looks like

### AI Doesn't Know Latest Features

AI training has a cutoff date. For new features:

1. **Provide documentation**: Paste relevant docs in your prompt
2. **Show examples**: Share code snippets of correct usage
3. **Iterate**: AI will learn from your corrections in the conversation

## Best Practices

1. **Start with clear requirements**: Detailed, specific, unambiguous
2. **Review all code**: Never deploy AI code without review
3. **Iterate incrementally**: Small changes are easier to verify
4. **Use quality gates**: Always run fmt, clippy, test before accepting
5. **Test thoroughly**: Verify tests actually test what they claim
6. **Learn MCP first**: Understand concepts before using AI assistance
7. **Contribute learnings**: Share patterns that work well

## Resources

- **AI Agent Configurations**: [/ai-agents/](../ai-agents/)
- **Kiro Power**: [/ai-agents/kiro/mcp-developer-power/](../ai-agents/kiro/mcp-developer-power/)
- **Claude Code Subagent**: [/ai-agents/claude-code/](../ai-agents/claude-code/)
- **cargo-pmcp**: [/cargo-pmcp/](../cargo-pmcp/)
- **Examples**: [/examples/](../examples/)
- **Community**: [GitHub Discussions](https://github.com/paiml/rust-mcp-sdk/discussions)

## Conclusion

AI-assisted MCP development combines:
- **Human expertise**: Domain knowledge, architectural decisions, quality review
- **AI execution**: Scaffolding, pattern implementation, test generation
- **cargo-pmcp automation**: Proven workflows and best practices

The result: Production-ready MCP servers in minutes instead of hours, with quality enforced automatically.

**Remember**: AI is a powerful tool, but you remain responsible for understanding your code, validating correctness, and ensuring security.

**Next steps**:
1. Install AI agent configuration for your preferred assistant
2. Build your first MCP server with AI assistance
3. Review this chapter's examples
4. Share your experience with the community

---

**This chapter is part of the pmcp SDK documentation.**
**For foundational MCP concepts, see earlier chapters.**
**For cargo-pmcp details, see the cargo-pmcp chapter.**
