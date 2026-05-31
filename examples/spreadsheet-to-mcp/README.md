# Spreadsheet → SQLite → MCP server

Companion files for the article
[*From a Spreadsheet to a Secure MCP Server in 10 Minutes*](../../docs/articles/spreadsheet-to-mcp-server.md).

A two-sheet sales workbook (customers + quotes) becomes a queryable, governed
MCP server with no query code written by hand.

| File | Role |
|------|------|
| `customers.csv` | The "Customers" sheet, exported as CSV |
| `quotes.csv` | The "Quotes" sheet, exported as CSV |
| `schema.sql` | Typed `CREATE TABLE`s — used to build the DB *and* as the `--schema` resource |
| `config.toml` | `pmcp-sql-server` config: SQLite backend, Code Mode policy, two curated tools |

`company.db` is **generated** (gitignored) — rebuild it from the CSVs.

## Build the database

```bash
rm -f company.db
sqlite3 company.db < schema.sql
sqlite3 company.db ".mode csv" \
  ".import --skip 1 customers.csv customers" \
  ".import --skip 1 quotes.csv quotes"

# sanity check
sqlite3 company.db "SELECT COUNT(*) FROM customers; SELECT COUNT(*) FROM quotes;"
```

## Serve it

```bash
# from the repo root:
cargo build -p pmcp-sql-server
./target/debug/pmcp-sql-server \
  --config examples/spreadsheet-to-mcp/config.toml \
  --schema examples/spreadsheet-to-mcp/schema.sql
# (run from inside this directory, or set [database] file_path to an absolute path)
```

The server prints its bound address and serves over streamable HTTP. Connect any
MCP client, or smoke-test it with `cargo pmcp test conformance <url>`. You get two
curated tools (`list_customers`, `quotes_for_customer`) plus Code Mode
(`validate_code` / `execute_code`) for everything else.

See the article for the full walkthrough and the deployment note.
