# Server Composition

This chapter covers advanced patterns for building hierarchies of MCP servers in large organizations. These techniques become valuable when you have many domain-specific servers that share common functionality.

## When to Use Server Composition

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    Is This Chapter For You?                             │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  ⚠️  ADVANCED TOPIC - This chapter is OPTIONAL                          │
│                                                                         │
│  Skip this chapter if:                                                  │
│  ═══════════════════                                                    │
│  • You have fewer than 5 MCP servers                                    │
│  • Your servers don't share common functionality                        │
│  • You're still learning MCP basics                                     │
│  • Your organization hasn't standardized on MCP yet                     │
│                                                                         │
│  Read this chapter when:                                                │
│  ═════════════════════                                                  │
│  • You have 10+ MCP servers across teams                                │
│  • You see duplicated code in multiple servers                          │
│  • Teams are building similar tools independently                       │
│  • Discovery of available tools has become difficult                    │
│  • You need domain-specific server hierarchies                          │
│                                                                         │
│  The techniques here help large organizations:                          │
│  ✓ Reduce duplication with foundation servers                           │
│  ✓ Organize servers by business domain                                  │
│  ✓ Enable tool discovery across the organization                        │
│  ✓ Build complex workflows from simple components                       │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### The Problem at Scale

As organizations adopt MCP, they often encounter these challenges:

| Problem | Example | Impact |
|---------|---------|--------|
| **Code Duplication** | Every team implements their own "get-database-connection" tool | Inconsistent behavior, maintenance burden |
| **Discovery Difficulty** | "Does anyone have a tool that does X?" | Lost productivity, duplicate work |
| **Inconsistent Patterns** | Different error handling, naming, authentication | Hard to compose servers |
| **Domain Isolation** | Finance tools mixed with HR tools in one server | Hard to manage access control |

### The Three-Tier Solution

Server composition addresses these challenges with a hierarchical approach:

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    Server Composition Hierarchy                         │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│                        ┌─────────────────────┐                          │
│                        │   Orchestration     │  ← Complex workflows     │
│                        │      Servers        │    spanning domains      │
│                        └──────────┬──────────┘                          │
│                                   │                                     │
│            ┌──────────────────────┼──────────────────────┐              │
│            │                      │                      │              │
│            ▼                      ▼                      ▼              │
│   ┌─────────────────┐   ┌─────────────────┐   ┌─────────────────┐       │
│   │  Finance Domain │   │   HR Domain     │   │ Engineering     │       │
│   │     Server      │   │    Server       │   │ Domain Server   │       │
│   └────────┬────────┘   └────────┬────────┘   └────────┬────────┘       │
│            │                     │                     │                │
│            └──────────────────────┼──────────────────────┘              │
│                                   │                                     │
│                                   ▼                                     │
│                        ┌─────────────────────┐                          │
│                        │    Foundation       │  ← Shared capabilities:  │
│                        │      Servers        │    auth, database, files │
│                        └─────────────────────┘                          │
│                                                                         │
│  Layer Responsibilities:                                                │
│  ═══════════════════════                                                │
│                                                                         │
│  Foundation: Core building blocks used by all domains                   │
│  • Authentication tools (validate_token, get_user_info)                 │
│  • Database access (query, insert, update)                              │
│  • File operations (read, write, list)                                  │
│  • Logging and metrics infrastructure                                   │
│                                                                         │
│  Domain: Business-specific tools built on foundation                    │
│  • Finance: expense_report, invoice, budget_forecast                    │
│  • HR: employee_lookup, time_off_request, org_chart                     │
│  • Engineering: deploy, rollback, service_status                        │
│                                                                         │
│  Orchestration: Cross-domain workflows                                  │
│  • Onboarding workflow (HR + Engineering + Finance)                     │
│  • Quarterly review (HR + Finance)                                      │
│  • Incident response (Engineering + all affected domains)               │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### DRY Principles in MCP

**Don't Repeat Yourself** applies to MCP server development:

```rust
// ❌ WITHOUT composition: Every domain server duplicates auth
// finance_server.rs
async fn validate_token(token: &str) -> Result<User> {
    // 50 lines of auth code
}

// hr_server.rs
async fn validate_token(token: &str) -> Result<User> {
    // Same 50 lines copied
}

// engineering_server.rs
async fn validate_token(token: &str) -> Result<User> {
    // Same 50 lines copied again
}

// ✅ WITH composition: Foundation server provides auth
// foundation_auth_server.rs
pub struct AuthFoundation { /* ... */ }
impl AuthFoundation {
    pub async fn validate_token(&self, token: &str) -> Result<User> {
        // Auth logic written ONCE
    }
}

// Domain servers compose foundation
let finance_server = Server::builder()
    .name("finance-server")
    .with_foundation(auth_foundation.clone())  // Reuse!
    .tool("expense_report", expense_tool)      // Domain-specific
    .build()?;
```

### Discovery Benefits

With organized server hierarchies, AI clients can discover tools effectively:

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    Tool Discovery with Composition                      │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  AI Client: "I need to check an employee's expense status"              │
│                                                                         │
│  Without Composition:                  With Composition:                │
│  ══════════════════                    ═══════════════                  │
│                                                                         │
│  Client must search 50+ servers        Client queries domains:          │
│  for relevant tools                    1. HR → employee_lookup          │
│                                        2. Finance → expense_status      │
│  Hard to know which server             3. Orchestration → combines them │
│  has what capability                                                    │
│                                        Clear hierarchy makes            │
│  Tools may have conflicting            discovery straightforward        │
│  names across servers                                                   │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

## Chapter Contents

This chapter explores three aspects of server composition:

1. **[Foundation Servers](./ch19-01-foundations.md)** - Building reusable base capabilities that domain servers can compose
   - Authentication and authorization patterns
   - Shared data access components
   - Common utility tools

2. **[Domain Servers](./ch19-02-domains.md)** - Creating business-specific servers using foundation components
   - Composing foundation capabilities
   - Domain-specific tool organization
   - Cross-domain tool exposure

3. **[Orchestration Patterns](./ch19-03-orchestration.md)** - Building workflows that span multiple domains
   - Sequential workflows
   - Server-side execution
   - Data binding between steps

## Prerequisites

Before diving into this chapter, ensure you're comfortable with:

- Building basic MCP servers (Chapters 3-5)
- Typed tools with schema generation (Chapter 9)
- Resource providers (Chapter 10)
- Middleware patterns (Chapter 17)

## Key Concepts Preview

| Concept | What It Means | When to Use |
|---------|---------------|-------------|
| **Foundation Server** | Provides core capabilities other servers build on | When multiple servers need the same functionality |
| **Domain Server** | Business-specific server composing foundation components | When a department needs specialized tools |
| **Orchestration** | Workflows spanning multiple servers/domains | When tasks require coordination across boundaries |
| **Dynamic Resources** | URI-template-based resource providers | When resources follow patterns (users/{id}, files/{path}) |
| **Server-Side Execution** | Tools executed by server, not client | When workflows need deterministic execution |

---

*Continue to [Foundation Servers](./ch19-01-foundations.md) →*
