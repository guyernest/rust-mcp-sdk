# Regression Testing

Regression testing ensures that bug fixes stay fixed and new features don't break existing functionality. This chapter covers strategies for building and maintaining effective regression test suites for MCP servers.

## What is Regression Testing?

```
┌─────────────────────────────────────────────────────────────────────┐
│                   Regression Testing Purpose                         │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  Without Regression Tests:                                          │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │  v1.0: Bug found → Bug fixed ✓                              │   │
│  │  v1.1: New feature added                                    │   │
│  │  v1.2: Bug reappears! ✗                                     │   │
│  │  v1.3: Same bug fixed again...                              │   │
│  │  v1.4: Bug reappears again...                               │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  With Regression Tests:                                             │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │  v1.0: Bug found → Bug fixed + Regression test added ✓      │   │
│  │  v1.1: New feature added, regression test passes ✓          │   │
│  │  v1.2: Code change would reintroduce bug...                 │   │
│  │        → Regression test FAILS ✗                            │   │
│  │        → Developer catches issue before release             │   │
│  │        → Bug never reaches production again!                │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

## Creating Regression Tests

### From Bug Reports

When you fix a bug, immediately create a regression test:

```yaml
# tests/scenarios/regression/issue-42-empty-result.yaml
name: "Regression #42 - Empty query result handling"
description: |
  Bug: Server returned 500 when query returned empty results.
  Fixed in: v1.2.1 (commit abc123)
  Root cause: Missing null check in result formatting.

  This test ensures empty results are handled gracefully.

tags:
  - regression
  - issue-42
  - critical

# Link to original issue
metadata:
  issue_url: https://github.com/example/mcp-server/issues/42
  fixed_in: v1.2.1
  fixed_by: commit abc123

steps:
  - name: "Query returning empty results should succeed"
    tool: execute_query
    input:
      sql: "SELECT * FROM users WHERE id = -999999"
    expect:
      success: true
      content:
        type: text
        contains: "0 rows"

  - name: "Empty table query should succeed"
    tool: execute_query
    input:
      sql: "SELECT * FROM empty_table"
    expect:
      success: true
```

### From Production Incidents

After a production incident, capture the exact sequence that caused the problem:

```yaml
# tests/scenarios/regression/incident-2024-01-15.yaml
name: "Regression - Production incident 2024-01-15"
description: |
  Incident: Server crashed under specific query pattern.
  Impact: 15 minutes of downtime.
  Root cause: Memory exhaustion when joining large tables without LIMIT.

  This test reproduces the exact conditions that triggered the crash.

tags:
  - regression
  - incident
  - performance
  - critical

metadata:
  incident_date: "2024-01-15"
  postmortem_url: https://wiki.example.com/postmortems/2024-01-15

steps:
  - name: "Large join query with limit doesn't crash"
    tool: execute_query
    input:
      sql: "SELECT u.*, o.* FROM users u JOIN orders o ON u.id = o.user_id LIMIT 100"
    expect:
      success: true
      response_time_ms:
        less_than: 30000  # Should complete, not timeout

  - name: "Query without limit is rejected"
    tool: execute_query
    input:
      sql: "SELECT u.*, o.* FROM users u JOIN orders o ON u.id = o.user_id"
    expect:
      error:
        message_contains: "LIMIT required"
```

### From Edge Cases

Document edge cases discovered during development:

```yaml
# tests/scenarios/regression/unicode-handling.yaml
name: "Regression - Unicode edge cases"
description: |
  Various Unicode handling edge cases that have caused issues.

tags:
  - regression
  - unicode
  - i18n

steps:
  - name: "Emoji in query values"
    tool: execute_query
    input:
      sql: "SELECT * FROM messages WHERE content LIKE '%🎉%'"
    expect:
      success: true

  - name: "Chinese characters in table names"
    tool: execute_query
    input:
      sql: "SELECT * FROM \"用户表\" LIMIT 1"
    expect:
      success: true

  - name: "RTL text handling"
    tool: execute_query
    input:
      sql: "SELECT * FROM messages WHERE content = 'مرحبا'"
    expect:
      success: true

  - name: "Zero-width characters"
    tool: execute_query
    input:
      sql: "SELECT * FROM users WHERE name = 'test\u200B'"
    expect:
      success: true
```

## Organizing Regression Tests

### Directory Structure

```
tests/scenarios/regression/
├── README.md              # Overview and organization guide
├── by-issue/              # Organized by issue number
│   ├── issue-42.yaml
│   ├── issue-87.yaml
│   └── issue-123.yaml
├── by-component/          # Organized by affected component
│   ├── auth/
│   │   ├── oauth-token-refresh.yaml
│   │   └── session-expiry.yaml
│   ├── query/
│   │   ├── null-handling.yaml
│   │   └── unicode.yaml
│   └── transport/
│       ├── sse-reconnect.yaml
│       └── timeout-handling.yaml
├── by-severity/           # Organized by severity
│   ├── critical/
│   │   ├── data-loss-prevention.yaml
│   │   └── security-bypass.yaml
│   └── medium/
│       ├── display-issues.yaml
│       └── performance.yaml
└── incidents/             # Production incidents
    ├── 2024-01-15.yaml
    └── 2024-02-20.yaml
```

### Naming Conventions

```yaml
# Good: Descriptive, includes issue reference
name: "Regression #42 - Empty result set handling"

# Good: Includes component and behavior
name: "Regression - Query: NULL value comparison"

# Bad: Too vague
name: "Bug fix test"

# Bad: No context
name: "Test 1"
```

### Tagging Strategy

```yaml
tags:
  - regression          # All regression tests
  - issue-42           # Specific issue number
  - query              # Affected component
  - critical           # Severity level
  - fixed-v1.2.1       # Version where fixed
  - database           # Related system
```

Query tests by tags:
```bash
# Run all critical regressions
cargo pmcp test run --scenario tests/scenarios/regression/ --tag critical

# Run regressions for a specific component
cargo pmcp test run --scenario tests/scenarios/regression/ --tag query

# Run regressions fixed in a specific version
cargo pmcp test run --scenario tests/scenarios/regression/ --tag fixed-v1.2.1
```

## Maintenance Strategies

### Regular Review

Schedule periodic regression test reviews:

```yaml
# .github/workflows/regression-review.yml
name: Monthly Regression Review

on:
  schedule:
    - cron: '0 9 1 * *'  # First of each month

jobs:
  generate-report:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Count regression tests
        run: |
          echo "## Regression Test Report" >> $GITHUB_STEP_SUMMARY
          echo "" >> $GITHUB_STEP_SUMMARY
          echo "Total regression tests: $(find tests/scenarios/regression -name '*.yaml' | wc -l)" >> $GITHUB_STEP_SUMMARY
          echo "" >> $GITHUB_STEP_SUMMARY
          echo "### By Severity" >> $GITHUB_STEP_SUMMARY
          echo "- Critical: $(grep -r 'critical' tests/scenarios/regression --include='*.yaml' | wc -l)" >> $GITHUB_STEP_SUMMARY
          echo "- Medium: $(grep -r 'medium' tests/scenarios/regression --include='*.yaml' | wc -l)" >> $GITHUB_STEP_SUMMARY
          echo "" >> $GITHUB_STEP_SUMMARY
          echo "### Recent additions (last 30 days)" >> $GITHUB_STEP_SUMMARY
          find tests/scenarios/regression -name '*.yaml' -mtime -30 >> $GITHUB_STEP_SUMMARY

      - name: Check for stale tests
        run: |
          echo "### Tests without recent validation" >> $GITHUB_STEP_SUMMARY
          # Find tests not modified in 6 months
          find tests/scenarios/regression -name '*.yaml' -mtime +180 >> $GITHUB_STEP_SUMMARY
```

### Deprecation Process

When a regression test becomes obsolete:

```yaml
# tests/scenarios/regression/deprecated/issue-15.yaml
name: "DEPRECATED - Issue #15"
description: |
  This regression test is deprecated as of v2.0.0.

  Reason: The affected component (legacy auth) was completely replaced
  in v2.0.0 with a new OAuth implementation.

  Original issue: #15
  Deprecated in: v2.0.0
  Safe to remove after: v3.0.0

tags:
  - regression
  - deprecated
  - issue-15

# Skip this test but keep for documentation
skip: true
skip_reason: "Component replaced in v2.0.0"

steps:
  # Original test preserved for reference
  - name: "Legacy auth token refresh"
    tool: refresh_token
    input:
      token: "expired_token"
    expect:
      success: true
```

### Test Consolidation

Combine related tests to reduce maintenance:

```yaml
# Before: Multiple similar files
# - issue-45-null-string.yaml
# - issue-67-empty-string.yaml
# - issue-89-whitespace-string.yaml

# After: Consolidated test
# tests/scenarios/regression/string-edge-cases.yaml
name: "Regression - String edge cases"
description: |
  Consolidated test for string handling edge cases.
  Covers issues: #45, #67, #89

tags:
  - regression
  - issue-45
  - issue-67
  - issue-89
  - strings

steps:
  - name: "NULL string handling (#45)"
    tool: execute_query
    input:
      sql: "SELECT * FROM users WHERE name IS NULL"
    expect:
      success: true

  - name: "Empty string handling (#67)"
    tool: execute_query
    input:
      sql: "SELECT * FROM users WHERE name = ''"
    expect:
      success: true

  - name: "Whitespace-only string handling (#89)"
    tool: execute_query
    input:
      sql: "SELECT * FROM users WHERE name = '   '"
    expect:
      success: true
```

## Automated Regression Detection

### Schema Change Detection

Detect when schema changes might affect existing tests:

```yaml
# .github/workflows/schema-check.yml
name: Schema Change Detection

on:
  pull_request:
    paths:
      - 'src/tools/**'
      - 'src/schema/**'

jobs:
  check-schema:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Compare schemas
        run: |
          # Get schema from main branch
          git show origin/main:schema.json > /tmp/old-schema.json

          # Get current schema
          cargo run --release &
          sleep 5
          curl http://localhost:3000/schema > /tmp/new-schema.json

          # Compare
          if ! diff /tmp/old-schema.json /tmp/new-schema.json > /dev/null; then
            echo "::warning::Schema has changed. Review regression tests."
            diff /tmp/old-schema.json /tmp/new-schema.json
          fi

      - name: Run affected regression tests
        run: |
          # Identify changed tools
          CHANGED_TOOLS=$(diff /tmp/old-schema.json /tmp/new-schema.json | grep -oP '"name":\s*"\K[^"]+')

          # Run regression tests for those tools
          for tool in $CHANGED_TOOLS; do
            cargo pmcp test run \
              --scenario tests/scenarios/regression/ \
              --tag "$tool"
          done
```

### Performance Regression Detection

Track performance over time:

```yaml
# tests/scenarios/regression/performance/baseline.yaml
name: "Performance - Baseline measurements"
description: "Track performance to detect regressions"

tags:
  - regression
  - performance
  - baseline

steps:
  - name: "Simple query baseline"
    tool: execute_query
    input:
      sql: "SELECT 1"
    expect:
      success: true
      response_time_ms:
        less_than: 100

  - name: "Table listing baseline"
    tool: list_tables
    input: {}
    expect:
      success: true
      response_time_ms:
        less_than: 500

  - name: "Complex query baseline"
    tool: execute_query
    input:
      sql: "SELECT * FROM users JOIN orders ON users.id = orders.user_id LIMIT 100"
    expect:
      success: true
      response_time_ms:
        less_than: 2000
```

```bash
# Compare performance with baseline
cargo pmcp test run \
  --scenario tests/scenarios/regression/performance/ \
  --format json \
  --output current-perf.json

# Historical comparison
cargo pmcp test compare \
  --current current-perf.json \
  --baseline baseline-perf.json \
  --threshold 20%  # Fail if >20% slower
```

## Best Practices

### 1. Write Tests Before Merging Fixes

```bash
# Workflow for bug fixes
1. Reproduce bug locally
2. Write failing regression test
3. Fix the bug
4. Verify test passes
5. Create PR with both fix and test
```

### 2. Include Context

```yaml
# Good: Full context for future maintainers
name: "Regression #123 - SQL injection in table parameter"
description: |
  Bug: The `table` parameter in get_sample_rows was passed directly
  to SQL without sanitization, allowing SQL injection attacks.

  Example attack vector:
    table: "users; DROP TABLE secrets; --"

  Fix: Added input validation using allowed table list.

  Security impact: HIGH - Could leak or destroy data.
  Fixed by: @developer in PR #456

tags:
  - regression
  - security
  - critical
  - issue-123

steps:
  - name: "SQL injection attempt is blocked"
    tool: get_sample_rows
    input:
      table: "users; DROP TABLE secrets; --"
    expect:
      error:
        message_contains: "Invalid table name"
```

### 3. Keep Tests Fast

```yaml
# Good: Focused test
steps:
  - name: "Specific edge case"
    tool: execute_query
    input:
      sql: "SELECT * FROM users WHERE id = NULL"
    expect:
      success: true

# Bad: Slow, broad test
steps:
  - name: "Test everything"
    tool: execute_query
    input:
      sql: "SELECT * FROM large_table"  # Slow!
    expect:
      success: true
```

### 4. Make Tests Independent

```yaml
# Good: Self-contained test
steps:
  - name: "Create test data"
    tool: insert_record
    input:
      table: users
      data: { id: 99999, name: "test" }

  - name: "Test specific behavior"
    tool: execute_query
    input:
      sql: "SELECT * FROM users WHERE id = 99999"
    expect:
      success: true

  - name: "Clean up"
    tool: delete_record
    input:
      table: users
      id: 99999

# Bad: Depends on external state
steps:
  - name: "Assumes data exists"
    tool: execute_query
    input:
      sql: "SELECT * FROM users WHERE id = 1"  # Might not exist
```

## Summary

Effective regression testing:

1. **Create tests with every bug fix** - Never fix a bug without a test
2. **Include full context** - Future maintainers need to understand why
3. **Organize systematically** - By issue, component, or severity
4. **Maintain regularly** - Review, consolidate, and deprecate
5. **Automate detection** - Catch regressions before they ship

Regression tests are insurance against repeating past mistakes. The time spent creating them pays dividends in prevented bugs and faster debugging.

## Exercises

1. **Create a regression test** - Pick a bug from your issue tracker and write a test
2. **Organize existing tests** - Set up a tagging strategy for your regression suite
3. **Set up performance baselines** - Create baseline performance tests
4. **Automate schema detection** - Add a workflow to detect schema changes

---

*Return to [Remote Testing Overview](./ch12-remote-testing.md)*
