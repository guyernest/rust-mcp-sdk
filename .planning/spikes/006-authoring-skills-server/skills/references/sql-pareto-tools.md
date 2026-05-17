# SQL — Pareto Tools

For SQL backends (Postgres, MySQL, Athena, Aurora, SQLite), each tool's
`sql` field is a parameterized query using the toolkit's canonical
`:name` placeholder syntax. The toolkit translates to dialect-native
placeholders at execute time.

## Tool design heuristics

1. **One tool = one user-visible operation.** "Get customer by id" is
   one tool. "Get customer with their last 5 orders" is a SEPARATE tool
   (it's a different intent, even if it shares a table).
2. **Parameters describe the question, not the SQL.** Name parameters
   after what the user thinks (`customer_id`, `since`, `min_amount`),
   not after column names (`c_id`, `created_at_gte`).
3. **Return what the agent needs to act, not raw rows.** If the agent's
   next likely step is "look at the order details", include order ids
   in the result so the agent can chain to the next tool.
4. **Always cap result size.** Use `LIMIT :limit_n` and make `limit_n`
   a parameter with a sensible default. Unbounded queries are a footgun.

## Dialect-aware authoring

The same `:name` syntax works across dialects. But you do need to write
SQL that runs on the target dialect:

| Dialect | LIMIT syntax | Common gotcha |
|---|---|---|
| Postgres | `LIMIT :n` | `RETURNING` available on INSERT/UPDATE |
| MySQL    | `LIMIT :n` | Backtick quoting for reserved words |
| Athena (Presto) | `LIMIT :n` | No transactions; no UPDATE/DELETE |
| SQLite   | `LIMIT :n` | `INTEGER PRIMARY KEY` autoincrement quirks |

For multi-dialect deployments, stick to ANSI SQL and avoid
dialect-specific functions. If you need dialect-specific SQL, the
toolkit lets each deployment specify its dialect in `[database]`.

## TOML shape

```toml
[[tools]]
name        = "get_customer_by_id"
description = "Look up a single customer by id."
sql         = "SELECT id, name, email FROM customers WHERE id = :customer_id"

[[tools.parameters]]
name        = "customer_id"
type        = "integer"
description = "Customer id (from the customers table)."
required    = true
```
