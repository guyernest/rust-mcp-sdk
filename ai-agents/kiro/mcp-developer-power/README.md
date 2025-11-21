# MCP Developer Power for Kiro

**Build production-grade Model Context Protocol (MCP) servers with deep AI assistance**

Version: 1.0.0
Status: Phase 1 - Foundation Complete
License: MIT

## Overview

The MCP Developer Power transforms Kiro into an MCP development expert, providing comprehensive knowledge about building MCP servers using the pmcp Rust SDK and cargo-pmcp toolkit. With this Power, Kiro can:

- **Teach**: Explain MCP concepts with deep domain knowledge
- **Guide**: Provide decision frameworks for architecture choices
- **Scaffold**: Help generate production-ready server structures via cargo-pmcp
- **Implement**: Suggest code following proven patterns from 6 production servers
- **Validate**: Guide quality assurance and testing strategies
- **Test**: Directly test MCP servers via MCP protocol integration

## What's Included

### 📚 Comprehensive Steering Files (3,730+ lines)

#### Foundation Files (Always Active)
- **`mcp-product.md`** (555 lines): Product overview, use cases, decision frameworks, when to build MCP servers
- **`mcp-tech.md`** (789 lines): Technology stack, pmcp SDK patterns, async programming, error handling, authentication
- **`mcp-structure.md`** (784 lines): Project layout, workspace organization, naming conventions, file structure

#### Pattern Files (Context-Aware)
- **`mcp-tool-patterns.md`** (960 lines): Tool implementation patterns, validation strategies, API integration, OAuth, testing
- **`mcp-resource-patterns.md`** (Future): Resource patterns, URI schemes, data access
- **`mcp-workflow-patterns.md`** (Future): Multi-step workflows, data bindings, template patterns
- **`mcp-prompt-patterns.md`** (Future): Prompt templates, argument handling

#### Reference Files (On-Demand)
- **`mcp-examples.md`** (Future): Complete working examples
- **`mcp-troubleshooting.md`** (Future): Common issues and solutions

### 📖 Documentation

- **`DESIGN.md`**: Comprehensive design document explaining the Power architecture, vision, and roadmap
- **`QUICKSTART.md`** (Future): Getting started guide
- **`TESTING.md`** (Future): Testing MCP servers with Kiro
- **`PATTERNS.md`** (Future): Detailed pattern library

### 🔧 Future Components

- **Hooks**: Automated quality checks (formatting, linting, testing)
- **MCP Servers**: cargo-pmcp exposed as MCP server for programmatic access
- **Examples**: Reference implementations in markdown format

## Quick Start

### Installation (Manual - Kiro Powers System Pending)

1. **Clone or copy this directory**:
   ```bash
   # If part of pmcp SDK repo
   cd rust-mcp-sdk/ai-agents/kiro/mcp-developer-power

   # Or download/clone separately
   git clone https://github.com/paiml/rust-mcp-sdk
   cd rust-mcp-sdk/ai-agents/kiro/mcp-developer-power
   ```

2. **Copy to Kiro powers directory**:
   ```bash
   # Create powers directory if it doesn't exist
   mkdir -p ~/.kiro/powers

   # Copy the power
   cp -r . ~/.kiro/powers/mcp-developer

   # Or symlink for development
   ln -s $(pwd) ~/.kiro/powers/mcp-developer
   ```

3. **Enable in Kiro settings**:
   - Open Kiro
   - Go to Settings (Cmd/Ctrl + ,)
   - Search for "Powers" or "MCP Developer"
   - Enable the MCP Developer Power

4. **Verify installation**:
   - Ask Kiro: "How do I build an MCP server?"
   - Kiro should reference the steering files and provide comprehensive guidance

### Prerequisites

To use this Power effectively, you'll need:

- **Rust 1.70+**: `rustup update`
- **cargo-pmcp toolkit**: `cargo install cargo-pmcp`
- **Basic Rust knowledge**: Understanding of async, Result, traits
- **Optional**: PostgreSQL/SQLite for database servers

## Usage Examples

### Example 1: Understanding MCP

```
User: "What is MCP and when should I build an MCP server?"

Kiro: [Reads mcp-product.md steering file]

"MCP (Model Context Protocol) enables AI assistants to access external
capabilities through standardized servers. You should build an MCP server when:

1. AI needs domain-specific expertise (e.g., weather data, database access)
2. You need secure access patterns to sensitive systems
3. Multiple AI clients need the same capabilities
4. Complex orchestration or workflows are required

Based on your use case, I can help you choose the right pattern:
- Simple tools: calculator pattern
- Database access: sqlite_explorer pattern
- API integration: minimal template with HTTP client
- Workflows: Multi-step orchestration (NEW in pmcp 1.8.0+)

What type of server are you building?"
```

### Example 2: Scaffolding a Server

```
User: "I want to create a weather MCP server"

Kiro: [Reads mcp-product.md decision framework]
      [Reads mcp-structure.md for workspace layout]

"This is an external API integration use case. I recommend:

1. Template: minimal server with tools
2. Transport: HTTP for development, stdio for production
3. Tools: 2-3 weather tools (current, forecast, alerts)
4. Pattern: API call with retry and caching

Let's create the workspace:

$ cargo pmcp new weather-mcp-workspace
$ cd weather-mcp-workspace
$ cargo pmcp add server weather --template minimal

This creates:
- crates/server-common/ (HTTP transport helpers)
- crates/mcp-weather-core/ (your server implementation)
- crates/weather-server/ (binary entry point)
- scenarios/weather/ (test scenarios)

Next, I'll help you implement the weather tools following the
API integration pattern from mcp-tool-patterns.md..."
```

### Example 3: Implementing a Tool

```
User: "How do I add a tool that calls the OpenWeather API?"

Kiro: [Reads mcp-tool-patterns.md - Pattern 3: External API Call]

"I'll show you the complete pattern for an API integration tool.

[Provides code example following the pattern, including:]
- Input/Output type definitions with JsonSchema
- Input validation (city name, API key)
- HTTP client setup with timeout
- Error handling (404, 401, 429, network errors)
- Response parsing and transformation
- Comprehensive tests

This pattern includes:
✓ Timeout handling (10 seconds)
✓ HTTP status code checking
✓ Meaningful error messages for AI
✓ API key from metadata (OAuth-ready)
✓ Unit tests for validation
✓ Integration test (with #[ignore] for CI)

The code follows Toyota Way principles:
- Zero tolerance for unwrap()
- Comprehensive error context
- Input validation at multiple levels
- 80%+ test coverage

Would you like me to add retry logic or caching as well?"
```

### Example 4: Testing with Kiro

```
User: "How do I test this MCP server?"

Kiro: [Reads mcp-testing.md and TESTING.md]

"I can help you test in three ways:

1. **Unit Tests** (in code):
   $ cargo test

2. **Integration Tests** (mcp-tester):
   $ cargo pmcp test --server weather --generate-scenarios
   $ cargo pmcp test --server weather

3. **Interactive Testing** (with me directly):
   Let me start your server and connect to it:

   $ cargo pmcp dev --server weather

   Now I'll add it to my own MCP configuration and test the tools
   directly via the MCP protocol...

   [Kiro adds server to its .kiro/settings.json]
   [Kiro calls get-weather tool with 'London']

   ✓ Tool responds correctly
   ✓ Error handling works (tested with invalid city)
   ✓ Response format matches schema

   All tests passing! Your server is ready for production."
```

## Features by Phase

### Phase 1: Foundation (v1.0.0 - v1.0.1) ✅

- ✅ Comprehensive steering files (4,487 lines)
- ✅ Foundation knowledge (product, tech, structure, **workflow**)
- ✅ Tool implementation patterns
- ✅ cargo-pmcp integration and workflow enforcement
- ✅ Design documentation

**What Kiro Can Do**:
- Explain MCP concepts comprehensively
- Guide architectural decisions
- Suggest code following proven patterns
- **ALWAYS use cargo-pmcp (never create files manually)**
- Provide scaffolding and implementation guidance

### Phase 2: Testing & Observability (v1.1.0) ✅

- ✅ Comprehensive testing guide (mcp-testing.md, 3,389 lines)
- ✅ Production observability guide (mcp-observability.md, 3,000 lines)
- ✅ Updated Claude Code subagent with testing and observability
- ✅ AI agent ecosystem vision and multi-agent support
- ✅ pmcp-book chapter on AI-assisted development

**What Kiro Can Do Now**:
- Guide comprehensive testing strategies (unit, integration, property, fuzz)
- Implement structured logging with `tracing`
- Add metrics collection for production monitoring
- Configure deployment-specific observability (CloudWatch, Cloudflare)
- Generate test scenarios and validate quality gates
- Teach testing best practices and coverage requirements

**Total Content**: 10,876 lines across 7 steering files

### Phase 3: Advanced Patterns (Future - v1.2.0)

- ⏳ Resource patterns steering file
- ⏳ Workflow patterns steering file
- ⏳ Prompt patterns steering file
- ⏳ Examples and troubleshooting guides
- ⏳ Quick start documentation

### Phase 4: Quality Automation (Future - v1.3.0)

- ⏳ Quality enforcement integrated in cargo-pmcp
- ⏳ Pre-commit quality gates in cargo-pmcp
- ⏳ Post-generate test creation hooks
- ⏳ Continuous validation during development

### Phase 5: MCP Server Interface (Future - v2.0.0)

- ⏳ cargo-pmcp as MCP server
- ⏳ Programmatic scaffolding tools
- ⏳ Quality validation tools
- ⏳ Template access resources

## Steering File Architecture

### Inclusion Modes

**Always Included** (4 files, 2,885 lines):
- `mcp-product.md` - Core MCP knowledge (555 lines)
- `mcp-tech.md` - Technical implementation (789 lines)
- `mcp-structure.md` - Project organization (784 lines)
- `mcp-workflow.md` - **CRITICAL** cargo-pmcp workflow (757 lines)

**Conditional** (file-match based):
- `mcp-tool-patterns.md` - When editing `**/tools/**/*.rs` (960 lines) ✅
- `mcp-resource-patterns.md` - When editing `**/resources/**/*.rs` (planned)
- `mcp-workflow-patterns.md` - When editing `**/workflows/**/*.rs` (planned)
- `mcp-prompt-patterns.md` - When editing `**/prompts/**/*.rs` (planned)

**Manual** (reference on-demand):
- `mcp-testing.md` - Include with: `#mcp-testing` (3,389 lines) ✅
- `mcp-observability.md` - Include with: `#mcp-observability` (3,000 lines) ✅
- `mcp-examples.md` - Include with: `#mcp-examples` (planned)
- `mcp-troubleshooting.md` - Include with: `#mcp-troubleshooting` (planned)

### Front Matter Format

Each steering file includes YAML front matter:

```yaml
---
inclusion: always  # or fileMatch, or manual
fileMatchPattern: "**/tools/**/*.rs"  # for fileMatch mode
---
```

## Quality Standards (Toyota Way)

This Power encodes Toyota Way principles into all recommendations:

### Jidoka (Quality Built-In)
- All patterns include validation and error handling
- Testing is part of every example
- Quality gates integrated into workflow

### Kaizen (Continuous Improvement)
- Steering files will evolve based on community feedback
- Metrics tracked (coverage, complexity, quality)
- Regular updates with new patterns

### Genchi Genbutsu (Go and See)
- Patterns extracted from 6 real production servers
- All examples tested and validated
- Evidence-based recommendations

### Zero Tolerance Standards
- **Complexity**: ≤25 per function
- **Technical Debt**: 0 SATD comments
- **Test Coverage**: 80%+ required
- **Clippy Warnings**: 0 tolerated
- **Formatting**: 100% cargo fmt compliant

## Contributing

### Improving Steering Files

Found a gap in Kiro's MCP knowledge? Contribute improvements:

1. **Identify the gap**: What did Kiro not know or get wrong?
2. **Add to appropriate steering file**: Choose product/tech/structure or pattern file
3. **Include examples**: Code examples with explanations
4. **Test with Kiro**: Verify Kiro now handles the scenario correctly
5. **Submit PR**: Contribute back to the community

### Adding New Patterns

Discovered a useful pattern? Share it:

1. **Extract from working code**: Must be production-tested
2. **Document trade-offs**: Explain when to use vs. alternatives
3. **Provide complete example**: Full working code with tests
4. **Add to pattern file**: Or create new pattern file if substantial
5. **Update README**: Document the new pattern

### Guidelines

- **Quality first**: Patterns must follow Toyota Way principles
- **Completeness**: Examples should be copy-pasteable and working
- **Clarity**: Explain the "why" not just the "what"
- **Testing**: All patterns include test examples
- **Real-world**: Based on actual production experience

## Roadmap

### v1.0.0 (Current) - Foundation
- ✅ Core steering files
- ✅ Tool patterns
- ✅ Design documentation
- ✅ cargo-pmcp integration

### v1.1.0 - Complete Patterns
- Resource patterns
- Workflow patterns (array indexing, bindings)
- Prompt patterns
- Examples library
- Troubleshooting guide
- Quick start documentation

### v1.2.0 - Automation
- Pre-save formatting hooks
- Pre-commit quality gates
- Post-generate test scenarios
- Continuous validation

### v2.0.0 - MCP Server Interface
- cargo-pmcp as MCP server
- `create_workspace` tool
- `add_server/tool/resource` tools
- `validate_server` tool
- `generate_scenarios` tool
- Template and pattern resources

## Architecture

```
MCP Developer Power
│
├── Steering Files (Knowledge Layer)
│   ├── Foundation (always active)
│   │   ├── mcp-product.md      (555 lines)
│   │   ├── mcp-tech.md         (789 lines)
│   │   └── mcp-structure.md    (784 lines)
│   │
│   ├── Patterns (file-match)
│   │   ├── mcp-tool-patterns.md       (960 lines)
│   │   ├── mcp-resource-patterns.md   (future)
│   │   ├── mcp-workflow-patterns.md   (future)
│   │   └── mcp-prompt-patterns.md     (future)
│   │
│   └── Reference (manual)
│       ├── mcp-examples.md           (future)
│       └── mcp-troubleshooting.md    (future)
│
├── Documentation
│   ├── DESIGN.md        (architecture & vision)
│   ├── QUICKSTART.md    (future)
│   ├── TESTING.md       (future)
│   └── PATTERNS.md      (future)
│
├── Hooks (future)
│   ├── pre-save.yaml
│   ├── pre-commit.yaml
│   └── post-generate.yaml
│
├── MCP Servers (future)
│   └── cargo-pmcp-server/
│
└── Examples (future)
    ├── minimal-server.md
    ├── calculator.md
    └── api-integration.md
```

## Success Metrics

### Phase 1 Goals (Current)
- ✅ Kiro answers "How do I build an MCP server?" in <30 seconds
- ✅ Pattern recommendations >90% accurate
- ✅ Coverage of common MCP scenarios >90%
- 🎯 Kiro builds simple server without human intervention (in testing)

### Future Goals
- Scaffolding time <2 minutes for complete workspace
- Quality gate pass rate 100% for generated code
- Test coverage >80% from generation
- Zero human intervention for standard patterns

## Support & Resources

### Documentation
- [MCP Specification](https://modelcontextprotocol.io)
- [pmcp SDK Docs](https://docs.rs/pmcp)
- [cargo-pmcp README](https://github.com/paiml/rust-mcp-sdk/tree/main/cargo-pmcp)
- [pmcp Book](https://github.com/paiml/rust-mcp-sdk/tree/main/pmcp-book)

### Examples
- 200+ examples in pmcp SDK `examples/` directory
- Production servers in `crates/` directory
- Template servers in cargo-pmcp

### Community
- [GitHub Issues](https://github.com/paiml/rust-mcp-sdk/issues)
- [GitHub Discussions](https://github.com/paiml/rust-mcp-sdk/discussions)

### Feedback

This Power is evolving based on real usage. Please provide feedback:

- **What worked well**: Patterns that helped you build better servers
- **What was missing**: Gaps in knowledge or patterns
- **What was confusing**: Documentation that needs clarification
- **New patterns**: Useful patterns to add to the library

## License

MIT License - see [LICENSE](https://github.com/paiml/rust-mcp-sdk/blob/main/LICENSE)

## Credits

Built by the PMCP team following Toyota Way principles and PAIML quality standards.

Steering files extracted from:
- 6 production MCP servers
- 200+ examples
- pmcp SDK (16x faster than TypeScript)
- cargo-pmcp toolkit
- Community contributions

---

**Version**: 1.0.0
**Last Updated**: 2025-11-13
**Status**: Phase 1 Complete
**Next Milestone**: v1.1.0 - Complete Patterns
