# Exercise: SQL Injection Code Review

::: exercise
id: ch03-02-sql-injection-review
difficulty: intermediate
time: 25 minutes
prerequisites: [ch03-01-db-query-basics, ch02-03-code-review-basics]
:::

Security is critical for database servers. Review this code for SQL injection vulnerabilities.

You've been asked to review a database query tool before production. Find at least **7 issues**, categorize by severity, and propose fixes.

::: objectives
thinking:
  - How SQL injection attacks work and why they're dangerous
  - Why string concatenation for SQL is always wrong
  - The difference between blocklisting and allowlisting

doing:
  - Identify multiple SQL injection vulnerabilities
  - Propose fixes using parameterized queries
  - Recognize insufficient security controls
:::

::: discussion
- What's the OWASP ranking for SQL injection?
- Can prepared statements prevent all SQL injection?
- What other layers of defense should exist beyond parameterized queries?
:::

## Code to Review

```rust
async fn search_users(pool: &DbPool, input: SearchUsersInput) -> Result<Vec<User>> {
    let mut query = String::from("SELECT id, name, email, role FROM users WHERE 1=1");

    // Add name filter
    if let Some(name) = &input.name {
        // Filter out obvious SQL injection attempts
        if !name.contains("DROP") && !name.contains("DELETE") {
            query.push_str(&format!(" AND name LIKE '%{}%'", name));
        }
    }

    // Add email domain filter
    if let Some(domain) = &input.email_domain {
        query.push_str(&format!(" AND email LIKE '%@{}'", domain));
    }

    // Add sorting
    if let Some(sort_by) = &input.sort_by {
        query.push_str(&format!(" ORDER BY {}", sort_by));
    }

    let rows = sqlx::query_as(&query).fetch_all(pool.as_ref()).await?;
    Ok(rows)
}

async fn get_user(pool: &DbPool, input: GetUserInput) -> Result<User> {
    let query = format!(
        "SELECT id, name, email, role FROM users WHERE id = {}",
        input.user_id  // user_id is a String, not i64!
    );
    let row = sqlx::query_as(&query).fetch_one(pool.as_ref()).await?;
    Ok(row)
}

async fn update_nickname(pool: &DbPool, input: UpdateNicknameInput) -> Result<String> {
    // This is a read-only server, but we have this for admin use
    let query = format!(
        "UPDATE users SET nickname = '{}' WHERE id = {}",
        input.nickname, input.user_id
    );
    sqlx::query(&query).execute(pool.as_ref()).await?;
    Ok("Nickname updated".to_string())
}
```

::: hint level=1 title="The Blocklist is Ineffective"
What about `drop` (lowercase)? `DR/**/OP`? There are infinite bypasses.
:::

::: hint level=2 title="ORDER BY Injection"
ORDER BY takes identifiers, not strings. You can't parameterize it - use an allowlist instead.
:::

::: hint level=3 title="Key Issues"
1. **Name filter** - Direct SQL injection via string concatenation
2. **Blocklist bypass** - Case variations, encoding, SQL comments
3. **Email domain** - No validation at all
4. **ORDER BY** - Identifier injection (blind SQL injection possible)
5. **get_user** - String user_id allows `1 OR 1=1`
6. **update_nickname** - Write operation in "read-only" server
7. **No LIMIT** - Memory exhaustion possible
8. **No authorization** - Anyone can query any user
:::

::: solution
**Fix with parameterized queries:**
```rust
async fn search_users(pool: &DbPool, input: SearchUsersInput) -> Result<Vec<User>> {
    let mut conditions = vec!["1=1".to_string()];
    let mut params: Vec<String> = vec![];

    if let Some(name) = &input.name {
        conditions.push("name LIKE ?".to_string());
        params.push(format!("%{}%", name));
    }

    if let Some(domain) = &input.email_domain {
        conditions.push("email LIKE ?".to_string());
        params.push(format!("%@{}", domain));
    }

    // ORDER BY must use allowlist - can't parameterize identifiers
    let allowed_columns = ["id", "name", "email"];
    let order_clause = match &input.sort_by {
        Some(col) if allowed_columns.contains(&col.as_str()) => {
            format!(" ORDER BY {}", col)
        }
        _ => String::new(),
    };

    let query = format!(
        "SELECT id, name, email, role FROM users WHERE {} LIMIT 100{}",
        conditions.join(" AND "),
        order_clause
    );

    let mut query_builder = sqlx::query_as::<_, User>(&query);
    for param in &params {
        query_builder = query_builder.bind(param);
    }

    let rows = query_builder.fetch_all(pool.as_ref()).await?;
    Ok(rows)
}
```

**Fix get_user with proper typing:**
```rust
async fn get_user(pool: &DbPool, input: GetUserInput) -> Result<User> {
    // Parse user_id as i64 first - fails fast if invalid
    let user_id: i64 = input.user_id.parse()
        .map_err(|_| anyhow!("Invalid user ID"))?;

    let row = sqlx::query_as::<_, User>(
        "SELECT id, name, email, role FROM users WHERE id = ?"
    )
    .bind(user_id)
    .fetch_one(pool.as_ref())
    .await?;

    Ok(row)
}
```

### Key Fixes

1. **Parameterized queries** - Separate SQL structure from data
2. **Allowlist for ORDER BY** - Can't parameterize identifiers
3. **Type validation** - Parse user_id as i64 before using
4. **LIMIT clause** - Prevent memory exhaustion
5. **Remove write operation** - Or add proper authorization
:::

::: reflection
- Why can't you parameterize ORDER BY column names?
- What's the difference between escaping quotes and parameterized queries?
- If the database user only has SELECT permission, is SQL injection still dangerous?
:::
