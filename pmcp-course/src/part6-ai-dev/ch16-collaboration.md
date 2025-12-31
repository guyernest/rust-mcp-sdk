# Effective AI Collaboration

With your AI assistant configured (Chapter 15), this chapter focuses on making your collaboration productive. We cover the cargo-pmcp workflow, effective prompting strategies, and quality assurance patterns.

## The Collaboration Model

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    Effective AI Collaboration                           │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  ┌─────────────────────────────────────────────────────────────────┐    │
│  │                        You (Developer)                          │    │
│  │                                                                 │    │
│  │  • Define WHAT to build (business requirements)                 │    │
│  │  • Provide domain knowledge (API constraints, data models)      │    │
│  │  • Make architectural decisions (transport, security)           │    │
│  │  • Review generated code (ownership and understanding)          │    │
│  └──────────────────────────┬──────────────────────────────────────┘    │
│                             │                                           │
│                    Clear Communication                                  │
│                             │                                           │
│                             ▼                                           │
│  ┌─────────────────────────────────────────────────────────────────┐    │
│  │                     AI Assistant                                │    │
│  │                                                                 │    │
│  │  • Generates HOW to build (code implementation)                 │    │
│  │  • Applies cargo-pmcp patterns (scaffolding, testing)           │    │
│  │  • Handles boilerplate (types, error handling, serialization)   │    │
│  │  • Iterates on compiler feedback (until quality gates pass)     │    │
│  └──────────────────────────┬──────────────────────────────────────┘    │
│                             │                                           │
│                      Quality Validation                                 │
│                             │                                           │
│                             ▼                                           │
│  ┌─────────────────────────────────────────────────────────────────┐    │
│  │                    Rust Compiler + Tooling                      │    │
│  │                                                                 │    │
│  │  • Type checking (catches errors at compile time)               │    │
│  │  • Borrow checking (memory safety guarantees)                   │    │
│  │  • Clippy linting (code quality enforcement)                    │    │
│  │  • Test runner (behavior verification)                          │    │
│  └─────────────────────────────────────────────────────────────────┘    │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

## The Three Pillars

### 1. Structured Workflow

A predictable workflow reduces ambiguity:

```
Request → Scaffold → Implement → Validate → Deploy
    │         │          │           │
    └─────────┴──────────┴───────────┘
         AI handles these steps
         You provide direction
```

### 2. Effective Communication

Good prompts lead to good code:

| Poor Prompt | Better Prompt |
|-------------|---------------|
| "Make an API server" | "Create an MCP server that queries the GitHub API" |
| "Add database stuff" | "Add a `list_tables` tool that returns table names and row counts" |
| "Fix the bug" | "The `get_user` tool returns 500 when the user doesn't exist. It should return a validation error." |

### 3. Quality Enforcement

Automated quality gates catch issues:

```bash
# AI iterates until all pass
cargo fmt --check     # ✓ Formatting
cargo clippy          # ✓ Linting
cargo test            # ✓ Unit tests
cargo pmcp test       # ✓ Integration tests
```

## What Makes This Different

### Traditional AI Code Generation

```
Prompt → Generate → Deploy → Runtime Errors → Debug → Repeat
                              ^                        |
                              |________________________|
                                    Slow feedback
```

### MCP Development with pmcp

```
Prompt → Scaffold → Generate → Compile → Fix → Validate → Deploy
            ^          |          |        |
            |          └──────────┴────────┘
            |              Fast iteration
            └── cargo-pmcp handles structure
```

Key differences:
1. **Structure is given** - cargo-pmcp scaffolds correctly
2. **Errors caught early** - Rust compiler prevents runtime bugs
3. **AI can self-correct** - Compiler feedback enables iteration
4. **Quality is enforced** - Gates prevent bad code from shipping

## Division of Responsibilities

### You Are Responsible For

1. **Requirements Definition**
   - What tools should the server provide?
   - What data should be accessible?
   - What are the error cases?

2. **Domain Knowledge**
   - API authentication methods
   - Data validation rules
   - Business logic constraints

3. **Architectural Decisions**
   - Transport mode (stdio vs HTTP)
   - Security requirements
   - Deployment target

4. **Code Review**
   - Understanding what was generated
   - Catching logical errors
   - Ensuring maintainability

### AI Is Responsible For

1. **Code Generation**
   - Type definitions
   - Handler implementations
   - Error handling boilerplate

2. **Pattern Application**
   - TypedTool structure
   - JsonSchema derives
   - cargo-pmcp conventions

3. **Iteration**
   - Fixing compiler errors
   - Addressing clippy warnings
   - Updating failing tests

4. **Documentation**
   - Inline comments
   - API documentation
   - Usage examples

## Working Sessions

### Short Sessions (15-30 minutes)

Good for:
- Adding a single tool
- Fixing a specific bug
- Updating existing functionality

Pattern:
```
"Add a search_users tool to the GitHub server that takes
a query string and returns matching usernames"
```

### Medium Sessions (1-2 hours)

Good for:
- Creating a new server
- Implementing a feature set
- Major refactoring

Pattern:
```
"Create a PostgreSQL MCP server with:
1. list_tables - returns table names
2. describe_table - returns column info
3. query - runs SELECT with row limit
4. explain - shows query plan

Use sqlx for async database access."
```

### Long Sessions (half day+)

Good for:
- Complex multi-server projects
- Full feature implementation
- Learning new patterns

Pattern:
```
"Build a complete CI/CD MCP server that:
1. Monitors GitHub Actions workflows
2. Triggers deployments
3. Provides status resources
4. Implements approval workflows

Break this into phases. Start with read-only
monitoring, then add write capabilities."
```

## Anti-Patterns to Avoid

### 1. Micromanaging Implementation

**Bad**:
```
"Create a struct called WeatherInput with a field city
of type String. Then create another struct called..."
```

**Good**:
```
"Create a weather tool that fetches current temperature
for a city. Return temperature in Celsius."
```

Let AI handle implementation details.

### 2. Vague Requirements

**Bad**:
```
"Make a database thing"
```

**Good**:
```
"Create a SQLite MCP server with list_tables and
execute_query tools. Limit queries to SELECT only."
```

Be specific about capabilities.

### 3. Ignoring Compiler Feedback

**Bad**:
```
User: "That doesn't work"
AI: "Let me try something else entirely"
```

**Good**:
```
User: "Here's the compiler error: [error message]"
AI: "I see the issue - the lifetime annotation is wrong.
     Let me fix that specific problem."
```

Share error messages for targeted fixes.

### 4. Skipping Quality Gates

**Bad**:
```
User: "Just make it compile, we'll fix warnings later"
```

**Good**:
```
User: "Run cargo clippy and fix all warnings before
      we consider this done"
```

Maintain quality throughout.

## Chapter Overview

This chapter covers three key topics:

### [The Development Workflow](./ch16-01-workflow.md)

The step-by-step cargo-pmcp workflow:
- Creating workspaces
- Adding servers
- Implementing tools
- Testing and validation
- Production deployment

### [Prompting for MCP Tools](./ch16-02-prompting.md)

Effective communication strategies:
- Describing tool requirements
- Specifying input/output types
- Handling error cases
- Iterating on generated code

### [Quality Assurance with AI](./ch16-03-qa.md)

Ensuring production-quality output:
- Automated quality gates
- Test generation
- Code review patterns
- Common issue resolution

## Summary

Effective AI collaboration requires:

1. **Clear communication** - Specific requirements, domain context
2. **Structured workflow** - cargo-pmcp patterns, predictable steps
3. **Quality enforcement** - Automated gates, compiler feedback
4. **Appropriate division** - You decide what, AI implements how

The goal is productive partnership: you provide direction and domain expertise, AI handles implementation details and iteration. The Rust compiler serves as an impartial referee, catching errors before they become bugs.

## Knowledge Check

Test your understanding of AI collaboration patterns:

{{#quiz ../quizzes/ch16-collaboration.toml}}

---

*Continue to [The Development Workflow](./ch16-01-workflow.md) →*
