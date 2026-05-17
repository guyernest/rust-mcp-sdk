# Example: Employee Directory (SQL)

Schema input:

```sql
CREATE TABLE employees (
    id          INTEGER PRIMARY KEY,
    name        TEXT NOT NULL,
    department  TEXT NOT NULL,
    salary      INTEGER NOT NULL
);
```

User intent (from the conversation): *"I want an agent that can look up
employees by id, find everyone in a department, and answer questions
that need ad-hoc SQL (like comparing average salaries across
departments)."*

That maps to two curated tools plus code-mode for the third. Resulting
`config.toml`:

```toml
[server]
name        = "employee-directory"
version     = "0.1.0"
description = "Query the employee directory."

# Curated tool 1: pure lookup. Pareto-justified — this is the single
# most common operation an agent will perform.
[[tools]]
name        = "get_employee_by_id"
description = "Look up a single employee by id."
sql         = "SELECT id, name, department, salary FROM employees WHERE id = :id"

[[tools.parameters]]
name = "id"
type = "integer"
description = "Employee id."
required = true

# Curated tool 2: bounded department listing. Bounded because we always
# include LIMIT :limit_n — never let an agent paginate an unbounded set.
[[tools]]
name        = "list_employees_by_department"
description = "List employees in a department, ordered by salary descending."
sql         = """
SELECT id, name, department, salary
FROM employees
WHERE department = :department
ORDER BY salary DESC
LIMIT :limit_n
"""

[[tools.parameters]]
name = "department"
type = "string"
description = "Department name."
required = true

[[tools.parameters]]
name = "limit_n"
type = "integer"
description = "Max rows to return."
required = false

# Code-mode handles the long tail (averages, percentile, group-by, etc.)
# Read-only — this is a directory, not a system of record.
[code_mode]
enabled        = true
allow_writes   = false
allow_deletes  = false
allow_ddl      = false
require_limit  = true
max_limit      = 1000
sensitive_columns = []
```

## What was deliberately NOT done

- We did NOT auto-generate a tool per column. The agent doesn't need
  "filter by salary range" as a tool — code-mode handles that.
- We did NOT expose `salary` as a sensitive column, because in this
  imaginary deployment the employee directory is non-confidential. In
  a real HR setting, `salary` would be in `sensitive_columns`.
- We did NOT set `token_secret` here — it comes from Secrets Manager
  via `${CODE_MODE_SECRET}` placeholder at deploy time.
