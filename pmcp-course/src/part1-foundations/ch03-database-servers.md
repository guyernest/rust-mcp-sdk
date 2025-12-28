# Database MCP Servers

Database access is the killer app for enterprise MCP. When employees can ask Claude "What were our top-selling products last quarter?" and get an instant, accurate answer from live data—that's transformative.

This chapter shows you how to build production-ready database MCP servers that are secure, performant, and enterprise-ready.

## What You'll Learn

| Section | Topics |
|---------|--------|
| [The Enterprise Data Access Problem](./ch03-01-data-access.md) | Why database access is MCP's killer app, the friction it eliminates |
| [Building db-explorer](./ch03-02-db-explorer.md) | Step-by-step server creation, query tools, schema introspection |
| [SQL Safety and Injection Prevention](./ch03-03-sql-safety.md) | Security patterns, parameterized queries, allowlisting |
| [Resource-Based Data Patterns](./ch03-04-resources.md) | When to use resources vs tools, structured access patterns |
| [Handling Large Results](./ch03-05-pagination.md) | Pagination, streaming, cursor-based navigation |

## Quick Preview

By the end of this chapter, you'll build a database server that lets Claude:

```
User: "Show me our top 10 customers by revenue"

Claude: I'll query the sales database for you.

[Calls list_tables tool]
[Calls query tool with: SELECT customer_name, SUM(order_total) as revenue 
 FROM orders GROUP BY customer_id ORDER BY revenue DESC LIMIT 10]

Here are your top 10 customers by revenue:
1. Acme Corp - $1,234,567
2. GlobalTech - $987,654
...
```

## The Architecture

```
┌─────────────────────────────────────────────────────────┐
│                     Claude / AI Client                   │
└─────────────────────────┬───────────────────────────────┘
                          │ MCP Protocol
                          ▼
┌─────────────────────────────────────────────────────────┐
│                   Database MCP Server                    │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐     │
│  │ list_tables │  │    query    │  │  Resources  │     │
│  │    Tool     │  │    Tool     │  │ (optional)  │     │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘     │
│         │                │                │             │
│         └────────────────┼────────────────┘             │
│                          ▼                              │
│              ┌───────────────────────┐                  │
│              │   Connection Pool     │                  │
│              │   (sqlx + Arc)        │                  │
│              └───────────┬───────────┘                  │
└──────────────────────────┼──────────────────────────────┘
                           │
                           ▼
                    ┌──────────────┐
                    │   Database   │
                    │  (SQLite,    │
                    │  PostgreSQL, │
                    │  MySQL)      │
                    └──────────────┘
```

## Prerequisites

Before starting this chapter, you should have:

- Completed [Chapter 2: Your First Production Server](./ch02-first-server.md)
- Basic familiarity with SQL
- A sample database to work with (we'll provide one)

## Sample Database

We'll use the [Chinook database](https://github.com/lerocha/chinook-database)—a sample database representing a digital media store with customers, invoices, tracks, and artists.

```bash
# Download the sample database
curl -L -o chinook.db https://github.com/lerocha/chinook-database/raw/master/ChinookDatabase/DataSources/Chinook_Sqlite.sqlite
```

## Chapter Sections

### 1. [The Enterprise Data Access Problem](./ch03-01-data-access.md)

Understand why database access is MCP's killer app for enterprises:
- The current friction in getting data to AI
- How MCP eliminates the copy-paste workflow
- Security considerations for enterprise data

### 2. [Building db-explorer](./ch03-02-db-explorer.md)

Build a complete database MCP server step-by-step:
- Creating the server with `cargo pmcp`
- Implementing `list_tables` and `query` tools
- Testing with MCP Inspector and Claude

### 3. [SQL Safety and Injection Prevention](./ch03-03-sql-safety.md)

Master security patterns for database access:
- SQL injection attacks and prevention
- Parameterized queries with sqlx
- Allowlisting vs blocklisting approaches
- Defense in depth strategies

### 4. [Resource-Based Data Patterns](./ch03-04-resources.md)

Learn when to use MCP resources instead of SQL tools:
- Resources for structured, predictable access
- Tools for flexible, ad-hoc queries
- Hybrid approaches for different use cases

### 5. [Handling Large Results](./ch03-05-pagination.md)

Handle enterprise-scale data volumes:
- Why OFFSET pagination fails at scale
- Cursor-based pagination patterns
- Streaming for very large results
- Memory-safe result handling

## Hands-On Exercises

After completing the lessons, practice with these exercises:

**[Chapter 3 Exercises](./ch03-exercises.md)**
- **Exercise 1: Database Query Basics** - Build list_tables and execute_query tools
- **Exercise 2: SQL Injection Review** - Find and fix security vulnerabilities
- **Exercise 3: Pagination Patterns** - Implement cursor-based pagination

## Security Checklist

Before deploying any database MCP server to production:

- [ ] Only SELECT queries allowed (no mutations)
- [ ] Parameterized queries for all user input
- [ ] Row limits enforced on all queries
- [ ] Sensitive columns filtered (SSN, passwords, PII)
- [ ] Connection pooling configured
- [ ] Query timeout set
- [ ] Audit logging enabled
- [ ] Authentication required (OAuth in production)

## Knowledge Check

Test your understanding after completing the chapter:

{{#quiz ../quizzes/ch03-database.toml}}

---

*Start with [The Enterprise Data Access Problem](./ch03-01-data-access.md) →*
