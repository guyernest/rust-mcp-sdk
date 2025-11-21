---
inclusion: always
---

# MCP Server Development - Product Overview

## What is the Model Context Protocol (MCP)?

The Model Context Protocol (MCP) is an open protocol that enables AI assistants like Kiro, Claude, and others to securely access external context and capabilities through standardized server interfaces.

### Core Capabilities

MCP servers provide four types of capabilities:

1. **Tools**: Functions that AI can invoke to perform actions
   - File operations (read, write, search)
   - API calls (HTTP requests, database queries)
   - Calculations and transformations
   - System operations (run commands, manage processes)

2. **Resources**: Data sources that AI can read
   - Files and directories
   - Database records
   - API responses
   - Configuration data
   - Real-time feeds

3. **Prompts**: Reusable templates for common AI tasks
   - Code review templates
   - Documentation generators
   - Analysis frameworks
   - Decision support workflows

4. **Workflows**: Multi-step orchestrated operations (NEW in pmcp 1.8.0+)
   - Sequential tool execution
   - Data transformation pipelines
   - Conditional logic and branching
   - Template bindings with array indexing

### Why MCP Matters

**For AI Assistants**:
- **Extended capabilities**: Access domain-specific knowledge and actions
- **Consistent interface**: Same protocol works across all MCP servers
- **Security**: Controlled access patterns with authentication
- **Scalability**: One assistant can use many specialized servers

**For Developers**:
- **Standardization**: Build once, works with all MCP clients
- **Specialization**: Focus on domain expertise, not protocol details
- **Reusability**: Share servers across projects and teams
- **Ecosystem**: Leverage existing MCP servers from community

## When to Build an MCP Server

### Use Case Decision Framework

| Scenario | Should Build MCP Server? | Recommended Approach |
|----------|-------------------------|---------------------|
| AI needs to access your database | ✅ Yes | Resource-heavy server (sqlite pattern) |
| AI needs to call external APIs | ✅ Yes | Tool-based server with API integration |
| AI needs to perform calculations | ✅ Yes | Simple tool server (calculator pattern) |
| AI needs to manage files | ✅ Yes | Tool + resource hybrid (file operations) |
| AI needs multi-step workflows | ✅ Yes | Workflow server with orchestration |
| Just sharing static data | ⚠️ Maybe | Consider simple file resources first |
| One-time script | ❌ No | Direct CLI tool is simpler |

### Build an MCP Server When You Need:

1. **Domain-Specific Expertise**: Your server encapsulates specialized knowledge
   - Medical diagnosis tools
   - Financial analysis systems
   - Legal research resources
   - Scientific computation workflows

2. **Secure Access Patterns**: Controlled interaction with sensitive systems
   - Database access with query validation
   - API calls with rate limiting
   - File operations with sandboxing
   - Authenticated service integration

3. **Standardized Interactions**: Multiple AI clients need same capabilities
   - Team using different AI tools (Claude, Kiro, custom agents)
   - Cross-project shared functionality
   - Third-party integrations

4. **Complex Orchestration**: Multi-step operations requiring coordination
   - Data transformation pipelines
   - Multi-API aggregation
   - Conditional workflow execution
   - Stateful conversation management

## MCP Server Patterns

### Pattern 1: Simple Tool Server (Calculator)

**Best For**: Basic operations, calculations, simple transformations

**Example Use Cases**:
- Mathematical operations
- Unit conversions
- String transformations
- Date/time calculations

**Template**: `calculator` (minimal template with tools)

**Characteristics**:
- 1-5 simple tools
- No state management
- Quick responses (<100ms)
- Minimal dependencies

**Example**:
```rust
// Tool: add two numbers
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct AddInput {
    pub a: f64,
    pub b: f64,
}

async fn add_tool(input: AddInput) -> Result<f64> {
    Ok(input.a + input.b)
}
```

### Pattern 2: Resource-Heavy Server (Database Explorer)

**Best For**: Data access, read-heavy operations, exploration

**Example Use Cases**:
- Database browsing
- File system navigation
- API documentation access
- Configuration inspection

**Template**: `sqlite_explorer`

**Characteristics**:
- Many resources (10-100+)
- Few tools (read-focused)
- Dynamic resource discovery
- URI-based access patterns

**Example**:
```rust
// Resource: sqlite://tables/{table_name}
// Returns: table schema and sample data
async fn get_table_resource(uri: &str) -> Result<String> {
    let table_name = extract_table_name(uri)?;
    let schema = db.get_schema(table_name).await?;
    let samples = db.query_limit(table_name, 10).await?;
    Ok(format_resource(schema, samples))
}
```

### Pattern 3: API Integration Server

**Best For**: External service interaction, data aggregation

**Example Use Cases**:
- Weather data fetching
- GitHub repository operations
- Payment processing
- Email sending

**Template**: `minimal` with API client tools

**Characteristics**:
- Tools wrapping API calls
- Error handling for network issues
- Authentication management
- Rate limiting and retries

**Example**:
```rust
// Tool: fetch weather forecast
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct WeatherInput {
    pub city: String,
    pub days: u8,
}

async fn get_forecast(input: WeatherInput) -> Result<Forecast> {
    let client = reqwest::Client::new();
    let response = client
        .get(&format!("https://api.weather.com/forecast/{}", input.city))
        .send()
        .await
        .context("Failed to fetch weather")?;

    if !response.status().is_success() {
        return Err(Error::validation("Invalid city name"));
    }

    response.json().await.context("Failed to parse forecast")
}
```

### Pattern 4: Workflow Orchestration Server (NEW)

**Best For**: Multi-step processes, data pipelines, conditional logic

**Example Use Cases**:
- Data transformation pipelines
- Multi-service aggregation
- Decision support workflows
- Automated testing scenarios

**Template**: `minimal` with workflow definitions

**Characteristics**:
- Workflow definitions (YAML/JSON)
- Step orchestration
- Data binding between steps
- Template variable substitution
- Array indexing support (v1.8.0+)

**Example**:
```yaml
# Workflow: Analyze GitHub repository
name: analyze_repo
steps:
  - name: fetch_repo
    tool: github_get_repo
    arguments:
      owner: "{input.owner}"
      repo: "{input.repo}"

  - name: get_languages
    tool: github_list_languages
    arguments:
      owner: "{input.owner}"
      repo: "{input.repo}"

  - name: analyze_top_language
    prompt: code_analysis
    bindings:
      language: "{languages.0.name}"     # Array indexing!
      repo_name: "{fetch_repo.full_name}"
      stars: "{fetch_repo.stargazers_count}"
```

## The pmcp SDK: Production-Grade Rust Implementation

### Why pmcp (vs. TypeScript SDK)?

**Performance**: 16x faster, 50x lower memory usage
- Cold start: <100ms (vs. >1s)
- Throughput: 10K+ msg/sec
- Memory: ~2MB per server

**Safety**: Rust's type system eliminates entire classes of bugs
- No null/undefined errors
- No race conditions
- Memory safety guaranteed
- Thread safety by default

**Quality**: Toyota Way principles built-in
- Zero tolerance for defects
- Comprehensive testing (80%+ coverage)
- Zero technical debt policy
- Continuous improvement metrics

**Production-Ready**: Proven patterns from 6 real servers
- Error handling best practices
- Authentication patterns
- Transport abstractions
- Testing frameworks

### pmcp vs. TypeScript SDK Feature Parity

| Feature | TypeScript SDK | pmcp SDK | Notes |
|---------|---------------|----------|-------|
| Tools | ✅ | ✅ | Fully compatible |
| Resources | ✅ | ✅ | Fully compatible |
| Prompts | ✅ | ✅ | Fully compatible |
| Workflows | ❌ | ✅ | pmcp exclusive feature |
| stdio Transport | ✅ | ✅ | Production standard |
| HTTP/SSE Transport | ✅ | ✅ | Development mode |
| WebSocket | ❌ | ✅ | pmcp exclusive |
| OAuth Support | Basic | ✅ Advanced | Full auth context pass-through |
| Type Safety | Partial | ✅ Complete | Compile-time guarantees |
| Performance | Baseline | 16x faster | Benchmarked |

## cargo-pmcp: Development Toolkit

### Philosophy

cargo-pmcp encodes **best practices from 6 production servers** into templates and tooling, following **Toyota Way principles**:

- **Jidoka**: Quality built-in, not bolted on
- **Kaizen**: Continuous improvement through measurable metrics
- **Genchi Genbutsu**: Real-world validation and evidence-based design

### Workflow (See mcp-workflow.md for Complete Details)

**CRITICAL**: ALWAYS use cargo-pmcp commands. NEVER create files manually.

The standard development cycle:

```bash
# 1. Create workspace (one-time setup)
cargo pmcp new my-mcp-workspace
cd my-mcp-workspace

# 2. Add server to workspace (scaffolds complete structure)
cargo pmcp add server weather --template minimal

# 3. Edit generated tool files (this is where you code)
# Edit: crates/mcp-weather-core/src/tools/*.rs
# Edit: crates/mcp-weather-core/src/lib.rs (register tools)

# 4. Develop with hot-reload
cargo pmcp dev --server weather

# 5. Generate test scenarios (in another terminal)
cargo pmcp test --server weather --generate-scenarios

# 6. Run automated tests
cargo pmcp test --server weather

# 7. Validate quality gates
cargo fmt --check && cargo clippy && cargo test

# 8. Deploy to production
cargo build --release
```

**Key Point**: Steps 1-2 scaffold everything. You only write code in step 3.

See **mcp-workflow.md** for detailed workflow, decision trees, and examples.

### Templates Available

**Minimal** (Quick Start):
- Basic server structure
- 1-2 example tools
- Ideal for learning or simple servers

**Calculator** (Educational):
- Complete arithmetic server
- Demonstrates tool patterns
- Comprehensive tests included

**SQLite Explorer** (Production):
- Full database browser
- Resource-heavy pattern
- Dynamic discovery
- Authentication example

**Complete Calculator** (Reference):
- All arithmetic operations
- Input validation
- Error handling patterns
- Testing best practices

## Decision: To Build or Not to Build?

### Build an MCP Server If:

✅ **Frequent AI interaction needed**: Your use case involves regular AI queries
✅ **Multiple clients**: Different teams/tools need same capabilities
✅ **Security required**: Need controlled access to sensitive systems
✅ **Complex operations**: Multi-step workflows or orchestration
✅ **Domain expertise**: You're encoding specialized knowledge
✅ **Reusability**: Server will be used across projects

### Don't Build MCP Server If:

❌ **One-time task**: Just write a script instead
❌ **Simple file access**: Use direct file resources in client
❌ **No standardization needed**: Custom integration is simpler
❌ **Performance critical**: Native SDK integration may be faster
❌ **Learning curve too steep**: Start with simpler approach first

### Alternatives to Consider:

**Option 1: Direct File Resources**
- Client directly accesses files via file:// URIs
- No server needed for static content

**Option 2: Custom CLI Tool**
- Standard command-line tool
- AI invokes via shell integration
- No protocol overhead

**Option 3: API Wrapper**
- Simple HTTP API
- Client uses generic HTTP tool
- More flexibility, less standardization

**Option 4: Embedded SDK**
- Integrate MCP client directly
- No server process needed
- Tighter integration, more complexity

## Quality Standards (Zero Tolerance)

When building MCP servers with pmcp and cargo-pmcp, we enforce:

### Code Quality
- **Complexity**: ≤25 cognitive complexity per function
- **Technical Debt**: 0 SATD comments allowed
- **Formatting**: 100% cargo fmt compliant
- **Linting**: 0 clippy warnings tolerated

### Testing
- **Coverage**: 80%+ test coverage required
- **Unit Tests**: Every function tested
- **Integration Tests**: Full client-server scenarios
- **Property Tests**: Invariant verification for complex logic
- **Fuzz Tests**: Robustness validation for parsers/validators

### Documentation
- **API Docs**: 100% public API documented
- **Examples**: All features have working examples
- **Patterns**: Complex patterns explained
- **Doctests**: All code examples are tested

### Performance
- **Cold Start**: <100ms server startup
- **Response Time**: <100ms for simple operations
- **Throughput**: 1K+ requests/second
- **Memory**: <10MB per server

## Getting Started Checklist

### Prerequisites
- [ ] Rust 1.70+ installed (`rustup update`)
- [ ] cargo-pmcp installed (`cargo install cargo-pmcp`)
- [ ] Understanding of async Rust (basic)
- [ ] Familiarity with JSON/serde (helpful)

### First Server (30 minutes)
- [ ] Create workspace: `cargo pmcp new learning-mcp`
- [ ] Add calculator server: `cargo pmcp add server calc --template calculator`
- [ ] Start dev server: `cargo pmcp dev --server calc`
- [ ] Connect MCP client (Claude Code, Kiro, or inspector)
- [ ] Test the add tool
- [ ] Read generated code in `crates/mcp-calc-core/`
- [ ] Run tests: `cargo pmcp test --server calc`

### Next Steps
- [ ] Modify calculator to add subtract tool
- [ ] Add input validation (no divide by zero)
- [ ] Generate test scenarios
- [ ] Pass all quality gates
- [ ] Build your first custom server

## Common Questions

### Q: Should I build one big server or many small servers?

**A**: Many small, focused servers following Unix philosophy:
- Each server does one thing well
- Easier to test and maintain
- Better security isolation
- Simpler deployment

**Exception**: Related capabilities can share a server (e.g., GitHub server with repo, issue, PR tools).

### Q: How do I handle authentication?

**A**: pmcp 1.8.0+ includes OAuth auth context pass-through:
```rust
// Tool receives auth context in metadata
async fn my_tool(input: Input, extra: RequestHandlerExtra) -> Result<Output> {
    let token = extra.metadata
        .get("oauth_token")
        .ok_or(Error::validation("Missing OAuth token"))?;

    // Use token for API calls
    api_client.with_token(token).fetch().await
}
```

### Q: What about error handling?

**A**: pmcp provides structured error types:
- `Error::validation(msg)`: Bad input from client
- `Error::internal(msg)`: Server-side issues
- `context("msg")`: Add context to any error

**Always** return meaningful errors to AI so it can correct and retry.

### Q: How do I test my server?

**A**: Three testing layers:
1. **Unit tests**: Test functions in isolation
2. **Integration tests**: `cargo pmcp test` with scenarios
3. **Manual testing**: Connect real MCP client and interact

### Q: Can I use async operations?

**A**: Yes! pmcp is fully async:
```rust
async fn fetch_data(input: Input) -> Result<Output> {
    let data = reqwest::get(input.url).await?;
    let json = data.json().await?;
    Ok(json)
}
```

### Q: How do I deploy to production?

**A**: Build release binary and run with stdio transport:
```bash
cargo build --release
./target/release/myserver-server
```

Configure in Claude Code/client:
```json
{
  "mcpServers": {
    "myserver": {
      "command": "/path/to/myserver-server",
      "args": []
    }
  }
}
```

## Success Metrics

### Your First Server Should:
- ✅ Start in <100ms
- ✅ Respond to tools/list
- ✅ Execute at least one tool successfully
- ✅ Pass all tests
- ✅ Pass clippy with 0 warnings
- ✅ Have 80%+ test coverage

### A Production Server Should:
- ✅ Handle errors gracefully
- ✅ Validate all inputs
- ✅ Log appropriately
- ✅ Support authentication (if needed)
- ✅ Document all capabilities
- ✅ Include comprehensive tests
- ✅ Meet performance targets
- ✅ Zero technical debt

## Resources

### Documentation
- [MCP Specification](https://modelcontextprotocol.io)
- [pmcp SDK Docs](https://docs.rs/pmcp)
- [cargo-pmcp README](https://github.com/paiml/rust-mcp-sdk/tree/main/cargo-pmcp)
- [pmcp Book](https://pmcp.rs) (comprehensive guide)

### Examples
- 200+ examples in `examples/` directory
- Production servers in `crates/` directory
- Template servers in cargo-pmcp

### Community
- GitHub Discussions
- Discord/Matrix chat
- Stack Overflow tag: `mcp`

---

**Next Steps**: Read `mcp-tech.md` for technical implementation details, and `mcp-structure.md` for project organization patterns.
