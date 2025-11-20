# MCP Developer Power - Design Document

## Overview

The **MCP Developer Power** is a comprehensive Kiro Power bundle that provides deep knowledge, patterns, and tooling for building production-grade Model Context Protocol (MCP) servers using the pmcp Rust SDK and cargo-pmcp toolkit.

## Vision

Enable Kiro to autonomously build, test, and deploy production-ready MCP servers following Toyota Way principles and PAIML quality standards, while teaching developers MCP concepts through interactive scaffolding.

## Power Components

### 1. Steering Files (Knowledge Layer)

Provides persistent knowledge about MCP development that Kiro references across all conversations.

#### Foundation Files (Always Included)
- **`mcp-product.md`**: What MCP is, when to build servers, ecosystem context, decision frameworks
- **`mcp-tech.md`**: Technology stack, pmcp SDK patterns, Rust conventions, async patterns, error handling
- **`mcp-structure.md`**: Project layout, workspace organization, file structure, naming conventions

#### Pattern Files (Conditional - File Type Based)
- **`mcp-tool-patterns.md`**: Tool implementation patterns, input validation, error handling, testing
- **`mcp-resource-patterns.md`**: Resource patterns, URI schemes, data access patterns
- **`mcp-workflow-patterns.md`**: Multi-step workflows, data bindings, template patterns, array indexing
- **`mcp-prompt-patterns.md`**: Prompt templates, argument handling, response formatting

#### Reference Files (Manual Inclusion)
- **`mcp-examples.md`**: Complete working examples (calculator, sqlite, weather API)
- **`mcp-troubleshooting.md`**: Common issues, debugging strategies, error patterns
- **`mcp-testing.md`**: mcp-tester scenarios, test generation, quality gates

### 2. Hooks (Quality Automation)

**Future Enhancement**: Automated quality checks on file save/commit.

Potential hooks:
- **Pre-save**: `cargo fmt` formatting
- **Pre-commit**: Quality gates (clippy, tests, coverage)
- **Post-generate**: Automatic test scenario generation

### 3. MCP Servers (Tool Interface)

**Future Enhancement**: cargo-pmcp exposed as MCP server for programmatic access.

Potential tools:
- `create_mcp_workspace`: Scaffold new workspace
- `add_mcp_server`: Add server to workspace
- `add_tool/resource/workflow`: Add components
- `validate_server`: Run quality gates
- `generate_scenarios`: Create test scenarios

### 4. Documentation (Learning Resources)

- **Design document** (this file): Architecture and vision
- **Quick start guide**: Getting started with the Power
- **Pattern library**: Detailed pattern explanations
- **Integration guide**: Testing MCP servers with Kiro

### 5. Examples (Reference Implementations)

- **Minimal server**: Basic tool-only server
- **Complete calculator**: Full-featured example with tests
- **API integration**: External API pattern (weather, GitHub)
- **Database server**: Resource-heavy pattern (sqlite)
- **Workflow server**: Multi-step orchestration example

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Kiro (AI Agent)                          │
│                                                             │
│  ┌──────────────────────────────────────────────────────┐  │
│  │         MCP Developer Power (Bundle)                  │  │
│  │                                                       │  │
│  │  ┌─────────────┐  ┌──────────┐  ┌────────────────┐  │  │
│  │  │  Steering   │  │  Hooks   │  │  MCP Servers   │  │  │
│  │  │   Files     │  │ (Future) │  │    (Future)    │  │  │
│  │  │             │  │          │  │                │  │  │
│  │  │ • Product   │  │ • Format │  │ • Scaffold     │  │  │
│  │  │ • Tech      │  │ • Quality│  │ • Validate     │  │  │
│  │  │ • Structure │  │ • Test   │  │ • Test Gen     │  │  │
│  │  │ • Patterns  │  │          │  │                │  │  │
│  │  └─────────────┘  └──────────┘  └────────────────┘  │  │
│  │                                                       │  │
│  │  ┌─────────────┐  ┌──────────┐                       │  │
│  │  │    Docs     │  │ Examples │                       │  │
│  │  │             │  │          │                       │  │
│  │  │ • Design    │  │ • Minimal│                       │  │
│  │  │ • Quickstart│  │ • Calc   │                       │  │
│  │  │ • Patterns  │  │ • API    │                       │  │
│  │  └─────────────┘  └──────────┘                       │  │
│  └──────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                           │
                           ▼
              ┌─────────────────────────┐
              │   cargo-pmcp toolkit    │
              │                         │
              │  • new workspace        │
              │  • add server/tool      │
              │  • dev mode             │
              │  • test scenarios       │
              │  • quality validation   │
              └─────────────────────────┘
                           │
                           ▼
              ┌─────────────────────────┐
              │   pmcp SDK (Rust)       │
              │                         │
              │  • MCP protocol impl    │
              │  • Async handlers       │
              │  • Macros (#[tool])     │
              │  • Transports (HTTP)    │
              └─────────────────────────┘
```

## Workflow: Building an MCP Server with Kiro

### Phase 1: Design & Planning (Kiro guided)

```
User: "I need an MCP server for weather forecasts"

Kiro: [Reads mcp-product.md for decision framework]
      [Analyzes use case: external API integration]

"This is an API integration use case. I recommend:
- Template: minimal server with tools
- Pattern: external API call with error handling
- Features: 2-3 weather tools (current, forecast, alerts)
- Transport: HTTP for development, stdio for production

Would you like me to create this structure?"
```

### Phase 2: Scaffolding (cargo-pmcp)

```
User: "Yes, create it"

Kiro: [Uses cargo-pmcp via terminal]

$ cargo pmcp new weather-mcp-workspace
$ cd weather-mcp-workspace
$ cargo pmcp add server weather --template minimal

"Workspace created with structure:
- crates/server-common/     (HTTP transport)
- crates/mcp-weather-core/  (server implementation)
- crates/weather-server/    (binary entry point)
- scenarios/weather/        (test scenarios)

Next: I'll add the weather tools..."
```

### Phase 3: Implementation (Kiro + Steering Patterns)

```
Kiro: [Reads mcp-tool-patterns.md for API call pattern]
      [References mcp-tech.md for error handling]
      [Generates tool code following patterns]

"Added 3 tools to crates/mcp-weather-core/src/tools/:
1. get_current.rs - Current weather by city
2. get_forecast.rs - 5-day forecast
3. get_alerts.rs - Weather alerts

All tools include:
✓ Input validation (city name, API key)
✓ Error handling (network, API errors)
✓ Comprehensive tests
✓ Type-safe input/output structs

I followed the 'external API call' pattern from steering,
including retry logic and timeout handling."
```

### Phase 4: Testing with Kiro (Direct Integration)

```
Kiro: [Starts dev server]

$ cargo pmcp dev --server weather

"Server running on http://0.0.0.0:3000

Now I'll add it as an MCP server to my own configuration
to test it interactively..."

[Kiro adds to .kiro/settings.json]:
{
  "mcpServers": {
    "weather": {
      "command": "node",
      "args": ["/path/to/mcp-proxy.js"],
      "env": {
        "MCP_SERVER_URL": "http://0.0.0.0:3000"
      }
    }
  }
}

"Testing the server now..."

[Kiro calls weather tools directly via MCP]

"✓ get_current tool works (tested with 'London')
✓ get_forecast tool works (tested with 'Tokyo')
✓ Error handling works (tested with invalid city)

All tools responding correctly! Generating test scenarios..."

$ cargo pmcp test --server weather --generate-scenarios

"Test scenarios created in scenarios/weather/generated.yaml
Running automated tests..."

$ cargo pmcp test --server weather

"✓ All 8 scenarios passing
✓ Coverage: 87%
✓ Quality gates: PASS

Server is production-ready!"
```

### Phase 5: Quality Validation & Deployment

```
Kiro: [Runs quality gates]

$ cargo fmt --check
$ cargo clippy -- -D warnings
$ cargo test

"Quality report:
✓ Formatting: clean
✓ Clippy: 0 warnings
✓ Tests: 15 passing
✓ Coverage: 87% (target: 80%)
✓ Cognitive complexity: max 18 (limit: 25)

Next steps:
1. Add your OpenWeather API key to .env
2. Deploy with: cargo pmcp build --release
3. Connect to Claude Code/Cursor via stdio transport

Documentation generated at docs/weather-server.md"
```

## Immediate Features (Phase 1)

### 1. Comprehensive Steering Files

**Status**: Implementing now

Create 8 steering files covering:
- Foundation knowledge (3 files)
- Implementation patterns (4 files)
- Reference materials (1 file)

**Benefit**: Kiro understands MCP deeply without re-explanation

### 2. cargo-pmcp Integration via Terminal

**Status**: Already works

Kiro uses existing cargo-pmcp CLI:
```bash
cargo pmcp new <workspace>
cargo pmcp add server <name> --template <template>
cargo pmcp dev --server <name>
cargo pmcp test --server <name>
```

**Benefit**: Full development workflow available today

### 3. Direct MCP Server Testing

**Status**: New capability to document

**Workflow**:
1. Kiro scaffolds MCP server with cargo-pmcp
2. Kiro starts dev server: `cargo pmcp dev --server myserver`
3. Kiro adds server to its own MCP configuration
4. Kiro tests tools interactively via MCP protocol
5. Kiro validates responses and generates scenarios
6. Kiro runs automated tests: `cargo pmcp test --server myserver`

**Benefit**: Immediate validation without Claude Code dependency

### 4. Power Bundle Structure

**Status**: Creating now

```
mcp-developer-power/
├── README.md                          # Power overview & quick start
├── power.json                         # Power metadata (for Kiro team)
├── steering/                          # Steering files
│   ├── mcp-product.md                # ✓ Always included
│   ├── mcp-tech.md                   # ✓ Always included
│   ├── mcp-structure.md              # ✓ Always included
│   ├── mcp-tool-patterns.md          # ✓ File match: **/tools/**
│   ├── mcp-resource-patterns.md      # ✓ File match: **/resources/**
│   ├── mcp-workflow-patterns.md      # ✓ File match: **/workflows/**
│   ├── mcp-prompt-patterns.md        # ✓ File match: **/prompts/**
│   └── mcp-examples.md               # ✓ Manual: #mcp-examples
├── docs/                              # Documentation
│   ├── DESIGN.md                     # This file
│   ├── QUICKSTART.md                 # Getting started guide
│   ├── TESTING.md                    # Testing MCP servers with Kiro
│   └── PATTERNS.md                   # Detailed pattern library
├── examples/                          # Reference implementations
│   ├── minimal-server.md             # Basic tool server
│   ├── calculator.md                 # Complete example
│   └── api-integration.md            # External API pattern
├── hooks/                             # Future: quality automation
│   └── README.md                     # Placeholder
└── mcp-servers/                       # Future: cargo-pmcp as MCP
    └── README.md                     # Placeholder
```

**Benefit**: Clean, organized Power ready for Kiro team integration

## Advanced Features (Future Phases)

### Phase 2: Hooks (Quality Automation)

**Pre-save Hook**: Auto-format on save
```yaml
# .kiro/hooks/pre-save.yaml
- name: "Format Rust files"
  pattern: "**/*.rs"
  command: "cargo fmt --files {file}"
```

**Pre-commit Hook**: Quality gates
```yaml
# .kiro/hooks/pre-commit.yaml
- name: "MCP Quality Gates"
  command: "cargo clippy -- -D warnings && cargo test"
  blocking: true
```

### Phase 3: MCP Server Interface

**cargo-pmcp exposed as MCP server**:

```bash
# Terminal 1: Start cargo-pmcp MCP server
cargo pmcp serve --port 3030

# Kiro connects via MCP
# Now has programmatic access to all cargo-pmcp functionality
```

**Tools exposed**:
- `create_workspace`: Structured scaffolding
- `add_server`: Template-based generation
- `validate_server`: Quality gates as tool
- `generate_scenarios`: Test generation

**Resources exposed**:
- `templates://{name}`: Access template code
- `patterns://{type}`: Pattern documentation
- `quality://{workspace}`: Quality metrics

### Phase 4: Advanced AI Features

**Intelligent Template Selection**:
```
Kiro: [Analyzes user's use case]
      [Compares against template capabilities]
      [Recommends best-fit template with reasoning]
```

**Code Analysis & Improvement**:
```
Kiro: [Scans existing MCP server code]
      [Identifies anti-patterns, security issues]
      [Suggests refactoring following steering patterns]
```

**Migration Assistance**:
```
Kiro: [Detects SDK version mismatch]
      [Generates migration steps]
      [Updates code to new patterns]
```

## Toyota Way Principles

### Jidoka (Quality Built-In)

- **Steering enforces patterns**: AI follows best practices by default
- **Quality gates integrated**: Validation happens during development
- **Stop-the-line**: Kiro stops on quality gate failures

### Kaizen (Continuous Improvement)

- **Steering files evolve**: Community feedback improves patterns
- **Metrics tracked**: Coverage, complexity, quality scores
- **Iterative refinement**: Each server built improves the Power

### Genchi Genbutsu (Go and See)

- **Real templates**: Extracted from 6 production servers
- **Tested patterns**: Every pattern validated in real usage
- **Evidence-based**: Metrics and test results guide decisions

## Success Metrics

### Phase 1 (Steering Files)
- **Time to answer "How do I build an MCP server?"**: <30 seconds
- **Pattern recommendation accuracy**: >90%
- **Coverage of common scenarios**: >90%
- **Kiro autonomy**: Build simple server without human intervention

### Phase 2 (Hooks)
- **Auto-fix rate**: >80% of formatting issues
- **Quality gate pass rate**: >95% before commit
- **Time saved on manual checks**: >50%

### Phase 3 (MCP Server Interface)
- **Scaffolding time**: <2 minutes for complete workspace
- **Quality gate pass rate**: 100% (generated code)
- **Test coverage**: >80% from generation
- **Human intervention**: 0% for standard patterns

## Integration with Kiro Team

### Power Metadata (power.json)

```json
{
  "name": "mcp-developer",
  "version": "1.0.0",
  "displayName": "MCP Developer Power",
  "description": "Build production-grade MCP servers with pmcp SDK",
  "author": "PMCP Team",
  "license": "MIT",
  "repository": "https://github.com/paiml/rust-mcp-sdk",

  "capabilities": {
    "steering": {
      "enabled": true,
      "path": "steering/"
    },
    "hooks": {
      "enabled": false,
      "path": "hooks/",
      "note": "Future enhancement"
    },
    "mcpServers": {
      "enabled": false,
      "path": "mcp-servers/",
      "note": "Future enhancement"
    }
  },

  "requirements": {
    "rust": ">=1.70.0",
    "cargo": "*",
    "cargo-pmcp": ">=0.1.0"
  },

  "keywords": [
    "mcp",
    "model-context-protocol",
    "rust",
    "pmcp",
    "code-generation",
    "scaffolding"
  ]
}
```

### Installation Instructions

**For Kiro Users**:
```bash
# Future: One-click install via Kiro marketplace
# For now: Manual installation

1. Copy mcp-developer-power/ to ~/.kiro/powers/
2. Enable in Kiro settings
3. Restart Kiro
4. Test: Ask "How do I build an MCP server?"
```

**For Power Development**:
```bash
# Clone pmcp SDK
git clone https://github.com/paiml/rust-mcp-sdk
cd rust-mcp-sdk/ai-agents/kiro/mcp-developer-power

# Symlink to Kiro (for development)
ln -s $(pwd) ~/.kiro/powers/mcp-developer

# Test in Kiro
# Open Kiro, ask MCP-related questions
```

## Testing Strategy

### 1. Steering File Testing

**Manual Testing**:
- Ask Kiro to build MCP server without additional context
- Verify it references steering files correctly
- Check pattern recommendations match steering
- Validate code follows documented patterns

**Success Criteria**:
- Kiro can explain MCP concepts from steering
- Kiro generates code matching patterns
- Kiro validates quality gates correctly

### 2. Integration Testing

**Scenario**: Weather API Server
```
1. User: "Build weather MCP server"
2. Kiro: Uses cargo-pmcp to scaffold
3. Kiro: Generates tools following patterns
4. Kiro: Starts dev server
5. Kiro: Adds to own MCP config
6. Kiro: Tests tools via MCP
7. Kiro: Runs quality gates
8. Result: Production-ready server
```

**Success Criteria**:
- Complete workflow without human intervention
- All quality gates pass
- Server works when tested via MCP
- Generated code matches steering patterns

### 3. Quality Validation

**Metrics**:
- Test coverage: >80%
- Clippy warnings: 0
- Cognitive complexity: <25
- Build time: <30 seconds
- Documentation: 100% public API

## Release Plan

### v1.0.0 - Foundation (Current Sprint)

**Features**:
- ✓ 8 comprehensive steering files
- ✓ Power bundle structure
- ✓ Documentation (design, quickstart, testing)
- ✓ Example implementations
- ✓ Integration guide

**Timeline**: 2-3 days

**Success Criteria**:
- Kiro can build simple MCP server autonomously
- All steering files validated with real usage
- Documentation complete and tested
- Power bundle ready for Kiro team integration

### v1.1.0 - Hooks (Future)

**Features**:
- Pre-save formatting hooks
- Pre-commit quality gates
- Post-generate test creation

**Timeline**: 1-2 days (after Kiro team finalizes hook system)

### v1.2.0 - MCP Server Interface (Future)

**Features**:
- cargo-pmcp as MCP server
- Programmatic scaffolding tools
- Quality validation tools
- Template access resources

**Timeline**: 2-3 days (after Phase 1 validation)

### v2.0.0 - Advanced AI Features (Future)

**Features**:
- Intelligent template selection
- Code analysis and improvement
- Migration assistance
- Performance profiling

**Timeline**: 1-2 weeks (after community feedback)

## Community & Feedback

### Feedback Channels

- **GitHub Issues**: Bug reports, feature requests
- **Discussions**: Pattern suggestions, use cases
- **Discord**: Real-time help, community patterns
- **PRs**: Community-contributed patterns

### Contribution Guidelines

**Adding Patterns**:
1. Extract from real, working code
2. Provide comprehensive example
3. Document trade-offs and alternatives
4. Include tests and validation
5. Submit PR with rationale

**Improving Steering**:
1. Identify gaps in AI understanding
2. Document missing patterns
3. Test with Kiro before submitting
4. Include examples of improved output

## Conclusion

The MCP Developer Power transforms Kiro from a general-purpose AI into a specialized MCP development expert, capable of:

1. **Teaching**: Explaining MCP concepts with deep knowledge
2. **Scaffolding**: Generating production-ready server structures
3. **Implementing**: Writing code following proven patterns
4. **Testing**: Validating servers via direct MCP integration
5. **Quality**: Enforcing Toyota Way zero-tolerance standards

By starting with comprehensive steering files and leveraging existing cargo-pmcp tooling, we achieve immediate value while laying the foundation for advanced features like hooks and MCP server interfaces.

**The meta-insight**: This Power doesn't just help developers build MCP servers - it teaches AI to teach developers to build MCP servers, creating a self-reinforcing cycle of knowledge transfer and quality improvement.

---

**Document Version**: 1.0.0
**Last Updated**: 2025-11-13
**Status**: Phase 1 Implementation In Progress
**Next Review**: After v1.0.0 release
