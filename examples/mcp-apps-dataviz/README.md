# Data Visualization MCP App

Interactive Chinook SQLite Explorer with Chart.js charts and sortable data tables.

## Prerequisites

Download the Chinook sample database:

```bash
cd examples/mcp-apps-dataviz
curl -L -o Chinook.db https://github.com/lerocha/chinook-database/releases/download/v1.4.5/Chinook_Sqlite.sqlite
```

## Running

```bash
cargo run
```

The server starts at http://localhost:3002 by default.

## Available Tools

| Tool | Description |
|------|-------------|
| `execute_query` | Run SQL queries against the Chinook database |
| `list_tables` | List all tables in the database |
| `describe_table` | Get column metadata for a specific table |

## Widget

The interactive dashboard (`widgets/dashboard.html`) features:

- **Chart.js visualization** -- Bar, line, and pie charts from query results
- **Sortable data table** -- Click column headers to sort ascending/descending
- **Chart type switcher** -- Toggle between chart types without re-running queries
- **Table browser** -- Click table names to quickly explore their contents
- **Keyboard shortcut** -- Ctrl+Enter (or Cmd+Enter) to run queries
