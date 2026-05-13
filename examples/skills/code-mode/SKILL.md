---
name: code-mode
description: Generate validated GraphQL queries against this server's schema
---

# Code Mode

This server exposes `validate_code` and `execute_code` tools for running
LLM-generated GraphQL queries with cryptographically signed approval tokens.

## Before you generate a query

1. Read `skill://code-mode/references/schema.graphql` for available types.
2. Read `skill://code-mode/references/examples.md` for canonical patterns.
3. Read `skill://code-mode/references/policies.md` for what's allowed.

## Round-trip

1. Generate a GraphQL query that satisfies the user's request.
2. Call `validate_code(code: "<your query>")`. You'll get back an
   `approval_token` plus a human-readable explanation. Show the explanation
   to the user.
3. After user approval, call `execute_code(code, token)`. Any modification
   to `code` between validate and execute invalidates the token.

## When NOT to use code mode

For simple lookups that match a curated tool (e.g. `get_user_by_id`),
prefer that tool. Code mode is for the long tail of compositions that
don't have dedicated tools.
