::: exercise
id: ch03-02-sql-injection-review
type: code-review
difficulty: intermediate
time: 25 minutes
prerequisites: ch03-01-db-query-basics, ch02-03-code-review-basics
:::

You've been asked to review a database query tool before it goes to production. The developer is new to security and made several classic mistakes. SQL injection vulnerabilities can lead to data breaches, data loss, and complete system compromise.

This exercise builds on your code review skills from Chapter 2, now with a security focus. SQL injection is consistently in the OWASP Top 10 - it's one of the most common and dangerous vulnerabilities in web applications.

**Your task:** Identify ALL security vulnerabilities, categorize them by severity, and propose secure alternatives using parameterized queries.

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
- How does SQL injection work? What allows it to happen?
- Why is checking for "DROP" and "DELETE" not sufficient protection?
- What's the fundamental problem with string concatenation in SQL?
- How do parameterized queries prevent injection?
:::

::: starter file="src/main.rs" language=rust
//! User Search MCP Server - CODE REVIEW EXERCISE
//!
//! Review this code for security vulnerabilities.
//! You should find at least 7 issues.
//!
//! Severity Guide:
//! - Critical: Direct SQL injection, data breach possible
//! - High: Indirect injection, privilege escalation possible
//! - Medium: Information disclosure, DoS potential
//! - Low: Missing best practices, defense in depth gaps

use pmcp::{Server, ServerCapabilities, ToolCapabilities};
use pmcp::server::TypedTool;
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use sqlx::{Pool, Sqlite};
use std::sync::Arc;

type DbPool = Arc<Pool<Sqlite>>;

#[derive(Deserialize, JsonSchema)]
struct SearchUsersInput {
    /// Name to search for
    name: Option<String>,
    /// Email domain to filter (e.g., "company.com")
    email_domain: Option<String>,
    /// Minimum user ID
    min_id: Option<i64>,
    /// Sort column
    sort_by: Option<String>,
    /// Sort direction
    sort_order: Option<String>,
}

#[derive(Serialize)]
struct User {
    id: i64,
    name: String,
    email: String,
    role: String,
}

async fn search_users(pool: &DbPool, input: SearchUsersInput) -> anyhow::Result<Vec<User>> {
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

    // Add minimum ID filter
    if let Some(min_id) = input.min_id {
        query.push_str(&format!(" AND id >= {}", min_id));
    }

    // Add sorting
    if let Some(sort_by) = &input.sort_by {
        query.push_str(&format!(" ORDER BY {}", sort_by));

        if let Some(order) = &input.sort_order {
            if order == "desc" || order == "DESC" {
                query.push_str(" DESC");
            }
        }
    }

    // Execute the query
    let rows: Vec<(i64, String, String, String)> = sqlx::query_as(&query)
        .fetch_all(pool.as_ref())
        .await?;

    Ok(rows.into_iter().map(|(id, name, email, role)| {
        User { id, name, email, role }
    }).collect())
}

#[derive(Deserialize, JsonSchema)]
struct GetUserInput {
    /// User ID to retrieve
    user_id: String,
}

async fn get_user(pool: &DbPool, input: GetUserInput) -> anyhow::Result<User> {
    let query = format!(
        "SELECT id, name, email, role FROM users WHERE id = {}",
        input.user_id
    );

    let row: (i64, String, String, String) = sqlx::query_as(&query)
        .fetch_one(pool.as_ref())
        .await?;

    Ok(User {
        id: row.0,
        name: row.1,
        email: row.2,
        role: row.3,
    })
}

#[derive(Deserialize, JsonSchema)]
struct UpdateNicknameInput {
    user_id: i64,
    nickname: String,
}

async fn update_nickname(pool: &DbPool, input: UpdateNicknameInput) -> anyhow::Result<String> {
    // This is a read-only server, but we have this for admin use
    let query = format!(
        "UPDATE users SET nickname = '{}' WHERE id = {}",
        input.nickname, input.user_id
    );

    sqlx::query(&query)
        .execute(pool.as_ref())
        .await?;

    Ok("Nickname updated".to_string())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let pool: DbPool = Arc::new(
        sqlx::sqlite::SqlitePoolOptions::new()
            .connect("sqlite:users.db")
            .await?
    );

    let pool_search = pool.clone();
    let pool_get = pool.clone();
    let pool_update = pool.clone();

    let server = Server::builder()
        .name("user-search")
        .version("1.0.0")
        .capabilities(ServerCapabilities {
            tools: Some(ToolCapabilities::default()),
            ..Default::default()
        })
        .tool("search_users", TypedTool::new("search_users", move |input: SearchUsersInput| {
            let pool = pool_search.clone();
            Box::pin(async move {
                let users = search_users(&pool, input).await?;
                Ok(serde_json::to_value(users)?)
            })
        }))
        .tool("get_user", TypedTool::new("get_user", move |input: GetUserInput| {
            let pool = pool_get.clone();
            Box::pin(async move {
                let user = get_user(&pool, input).await?;
                Ok(serde_json::to_value(user)?)
            })
        }))
        .tool("update_nickname", TypedTool::new("update_nickname", move |input: UpdateNicknameInput| {
            let pool = pool_update.clone();
            Box::pin(async move {
                let result = update_nickname(&pool, input).await?;
                Ok(serde_json::to_value(result)?)
            })
        }))
        .build()?;

    println!("User search server ready!");
    Ok(())
}
:::

::: hint level=1
Look for string concatenation patterns like `format!()` or `push_str()` that include user input directly in SQL queries.
:::

::: hint level=2
The blocklist approach (checking for "DROP" and "DELETE") can be bypassed. Consider: `'; SELECT * FROM users WHERE role='admin' --`
:::

::: hint level=3
Issues to find:
1. Name filter: SQL injection via string concatenation
2. Email domain filter: SQL injection (no validation)
3. Sort column: SQL injection (arbitrary column/expression)
4. Sort order: Injection possible (only checks exact match)
5. get_user: user_id is String, concatenated without validation
6. update_nickname: Direct string concatenation
7. Architecture: UPDATE tool on "read-only" server
:::

::: solution reveal=on-demand
```rust
// Secure implementation of search_users using parameterized queries
async fn search_users(pool: &DbPool, input: SearchUsersInput) -> anyhow::Result<Vec<User>> {
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

    // For ORDER BY, use an allowlist - can't parameterize column names
    let allowed_columns = ["id", "name", "email"];
    let order_clause = match &input.sort_by {
        Some(col) if allowed_columns.contains(&col.as_str()) => {
            let direction = match &input.sort_order {
                Some(o) if o.to_lowercase() == "desc" => "DESC",
                _ => "ASC",
            };
            format!(" ORDER BY {} {}", col, direction)
        }
        _ => String::new(),
    };

    let query = format!(
        "SELECT id, name, email, role FROM users WHERE {} LIMIT 100{}",
        conditions.join(" AND "),
        order_clause
    );

    // Build query with dynamic binding
    let mut query_builder = sqlx::query_as::<_, (i64, String, String, String)>(&query);
    for param in &params {
        query_builder = query_builder.bind(param);
    }

    let rows = query_builder.fetch_all(pool.as_ref()).await?;

    Ok(rows.into_iter().map(|(id, name, email, role)| {
        User { id, name, email, role }
    }).collect())
}

// Key security principles:
// - Never use string concatenation for SQL with user input
// - Blocklists can always be bypassed - use allowlists instead
// - Parameterized queries separate SQL structure from data
// - Defense in depth: read-only connections, least privilege, audit logging
// - Code comments don't enforce security - "read-only server" with UPDATE tool
```
:::

::: reflection
- Why can't you parameterize ORDER BY column names?
- What's the difference between escaping quotes and parameterized queries?
- If the database user only has SELECT permission, is SQL injection still dangerous?
- How would you test for SQL injection in an automated way?
:::
