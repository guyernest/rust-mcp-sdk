---
name: config-authoring
description: Help a developer design a config.toml for a PMCP schema-server toolkit deployment, applying the Pareto principle to curated tools and code-mode policy.
---

# PMCP Config Authoring

You are helping a developer design a `config.toml` for a PMCP schema-server
toolkit deployment. The config drives a runnable MCP server that exposes
curated "pareto" tools backed by SQL queries / GraphQL operations /
OpenAPI calls, plus a code-mode bootstrap for the long-tail surface.

## The Pareto Principle

A good config defines **the 20% of operations that handle 80% of the
real traffic** as named tools. The rest is handled by the code-mode
prompt + `validate_code` / `execute_code` tools that the toolkit ships
automatically.

Resist the urge to expose every table / endpoint / GraphQL operation as
a tool. Each tool is a **product surface** — name, description, JSON
Schema for inputs, output shape. Curating ten well-designed tools beats
auto-generating two hundred from a schema.

## Workflow

1. **Read the user's schema** (SQL DDL / OpenAPI YAML / GraphQL SDL).
2. **Ask what the agent should be able to do** in 1-2 sentences. Don't
   ask for tools yet — ask for *user intents*. ("Look up customers",
   "find unhappy customers from last week", etc.)
3. **Map intents to tools.** Each intent becomes 1-3 tools with concrete
   parameters and a representative example.
4. **Set safety policy.** Decide which tables / endpoints / operations
   are blocked, which fields are sensitive, which are read-only.
5. **Emit the `config.toml`.** Reference the per-backend guide for the
   exact TOML shape.

## When to use which reference

- For SQL backends → see `references/sql-pareto-tools.md`
- For OpenAPI backends → see `references/openapi-pareto-tools.md`
- For GraphQL backends → see `references/graphql-pareto-tools.md`
- For code-mode policy design → see `references/code-mode-policy.md`
- For a worked example → see `examples/employee-directory-sql.md`

## Output expectations

When you produce a `config.toml`, include:

- `[server]` section with `name`, `version`, `description`
- One `[[tools]]` block per curated tool, with `name`, `description`,
  the backend-specific execution field (`sql` / `query` / `path`+`method`),
  and `[[tools.parameters]]` for each input
- `[code_mode]` section with `enabled = true` and any policy fields the
  user agreed to
- Comments explaining non-obvious choices so a future reader (or the
  user's coworker) can see why each tool exists
