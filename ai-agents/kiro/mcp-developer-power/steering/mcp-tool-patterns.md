---
inclusion: fileMatch
fileMatchPattern: "**/tools/**/*.rs"
---

# MCP Tool Implementation Patterns

## Tool Anatomy

Every MCP tool consists of four parts:

1. **Input Type**: Deserializable struct defining tool arguments
2. **Output Type**: Serializable struct defining tool result
3. **Handler Function**: Async function implementing business logic
4. **Builder/Registration**: TypedTool instance for server registration

## Pattern 1: Simple Calculation Tool

**Use Case**: Pure functions, stateless operations, basic computations

### Complete Example

```rust
use pmcp::{Result, TypedTool, RequestHandlerExtra, Error};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// ============================================================================
// TYPE DEFINITIONS
// ============================================================================

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[schemars(deny_unknown_fields)]  // Reject unknown fields for safety
pub struct AddInput {
    /// First number to add
    #[schemars(description = "The first operand")]
    pub a: f64,

    /// Second number to add
    #[schemars(description = "The second operand")]
    pub b: f64,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct AddOutput {
    /// The sum of the two numbers
    pub result: f64,

    /// Human-readable description of the operation
    pub operation: String,
}

// ============================================================================
// HANDLER
// ============================================================================

async fn add_handler(
    input: AddInput,
    _extra: RequestHandlerExtra
) -> Result<AddOutput> {
    Ok(AddOutput {
        result: input.a + input.b,
        operation: format!("{} + {} = {}", input.a, input.b, input.a + input.b),
    })
}

// ============================================================================
// BUILDER
// ============================================================================

pub fn build_add_tool() -> TypedTool<AddInput, AddOutput> {
    TypedTool::new("add", |input, extra| {
        Box::pin(add_handler(input, extra))
    })
    .with_description("Add two numbers together")
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_add_positive_numbers() {
        let input = AddInput { a: 5.0, b: 3.0 };
        let result = add_handler(input, RequestHandlerExtra::default())
            .await
            .unwrap();

        assert_eq!(result.result, 8.0);
        assert_eq!(result.operation, "5 + 3 = 8");
    }

    #[tokio::test]
    async fn test_add_negative_numbers() {
        let input = AddInput { a: -5.0, b: -3.0 };
        let result = add_handler(input, RequestHandlerExtra::default())
            .await
            .unwrap();

        assert_eq!(result.result, -8.0);
    }
}
```

### Key Points

- ✅ **Explicit types**: Clear input/output contracts
- ✅ **JsonSchema derive**: Auto-generates schema for MCP clients
- ✅ **deny_unknown_fields**: Prevents typos and unexpected inputs
- ✅ **Description annotations**: Help AI understand tool parameters
- ✅ **Comprehensive tests**: Cover normal and edge cases

## Pattern 2: Tool with Input Validation

**Use Case**: Operations that can fail, need validation, error handling

### Complete Example

```rust
use pmcp::{Result, TypedTool, RequestHandlerExtra, Error};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use validator::Validate;

// ============================================================================
// TYPE DEFINITIONS
// ============================================================================

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, Validate)]
#[schemars(deny_unknown_fields)]
pub struct DivideInput {
    /// Dividend (number to be divided)
    #[validate(range(min = -1000000.0, max = 1000000.0))]
    #[schemars(description = "The dividend (number to be divided)")]
    pub a: f64,

    /// Divisor (number to divide by)
    #[validate(range(min = -1000000.0, max = 1000000.0))]
    #[schemars(description = "The divisor (must be non-zero)")]
    pub b: f64,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct DivideOutput {
    pub result: f64,
    pub operation: String,
}

// ============================================================================
// HANDLER WITH VALIDATION
// ============================================================================

async fn divide_handler(
    input: DivideInput,
    _extra: RequestHandlerExtra
) -> Result<DivideOutput> {
    // 1. Validate inputs using validator crate
    input.validate()
        .map_err(|e| Error::validation(format!("Invalid input: {}", e)))?;

    // 2. Business logic validation
    if input.b == 0.0 {
        return Err(Error::validation("Cannot divide by zero"));
    }

    // Edge case: Check for near-zero to avoid numerical instability
    if input.b.abs() < 1e-10 {
        return Err(Error::validation(
            "Divisor too close to zero (potential numerical instability)"
        ));
    }

    // 3. Perform operation
    let result = input.a / input.b;

    // 4. Check result validity
    if result.is_infinite() {
        return Err(Error::internal("Result is infinite"));
    }

    if result.is_nan() {
        return Err(Error::internal("Result is not a number"));
    }

    Ok(DivideOutput {
        result,
        operation: format!("{} / {} = {}", input.a, input.b, result),
    })
}

// ============================================================================
// BUILDER
// ============================================================================

pub fn build_divide_tool() -> TypedTool<DivideInput, DivideOutput> {
    TypedTool::new("divide", |input, extra| {
        Box::pin(divide_handler(input, extra))
    })
    .with_description("Divide two numbers (with validation)")
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_divide_success() {
        let input = DivideInput { a: 10.0, b: 2.0 };
        let result = divide_handler(input, RequestHandlerExtra::default())
            .await
            .unwrap();

        assert_eq!(result.result, 5.0);
    }

    #[tokio::test]
    async fn test_divide_by_zero() {
        let input = DivideInput { a: 10.0, b: 0.0 };
        let result = divide_handler(input, RequestHandlerExtra::default()).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Cannot divide by zero"));
    }

    #[tokio::test]
    async fn test_divide_by_near_zero() {
        let input = DivideInput { a: 10.0, b: 1e-15 };
        let result = divide_handler(input, RequestHandlerExtra::default()).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("too close to zero"));
    }

    #[tokio::test]
    async fn test_divide_out_of_range() {
        let input = DivideInput { a: 2000000.0, b: 2.0 };
        let result = divide_handler(input, RequestHandlerExtra::default()).await;

        assert!(result.is_err());  // Validator should reject out-of-range input
    }
}
```

### Validation Strategies

**Level 1: Type System** (compile-time)
```rust
// Use enums for constrained values
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Operation {
    Add,
    Subtract,
    Multiply,
    Divide,
}
```

**Level 2: Validator Crate** (runtime, declarative)
```rust
#[derive(Debug, Deserialize, Validate)]
pub struct EmailInput {
    #[validate(email)]
    pub email: String,

    #[validate(range(min = 18, max = 120))]
    pub age: u8,

    #[validate(length(min = 3, max = 50))]
    pub name: String,
}
```

**Level 3: Custom Validation** (runtime, business logic)
```rust
async fn handler(input: MyInput, _extra: RequestHandlerExtra) -> Result<MyOutput> {
    // Custom business rules
    if input.start_date > input.end_date {
        return Err(Error::validation("Start date must be before end date"));
    }

    if !is_valid_format(&input.data) {
        return Err(Error::validation(format!(
            "Invalid data format: expected JSON, got: {}",
            input.data
        )));
    }

    // Proceed with operation
    Ok(MyOutput { ... })
}
```

## Pattern 3: External API Call Tool

**Use Case**: Wrapping external HTTP APIs, third-party services

### Complete Example

```rust
use pmcp::{Result, TypedTool, RequestHandlerExtra, Error};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use reqwest::Client;
use anyhow::Context;

// ============================================================================
// TYPE DEFINITIONS
// ============================================================================

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct WeatherInput {
    /// City name to get weather for
    #[schemars(description = "Name of the city (e.g., 'London', 'Tokyo')")]
    pub city: String,

    /// Number of forecast days (1-5)
    #[schemars(description = "Number of days to forecast (1-5)")]
    pub days: Option<u8>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct WeatherOutput {
    pub city: String,
    pub temperature: f64,
    pub conditions: String,
    pub forecast: Vec<DayForecast>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct DayForecast {
    pub day: String,
    pub high: f64,
    pub low: f64,
    pub conditions: String,
}

// ============================================================================
// HANDLER WITH API CALL
// ============================================================================

async fn weather_handler(
    input: WeatherInput,
    extra: RequestHandlerExtra
) -> Result<WeatherOutput> {
    // 1. Validate inputs
    if input.city.is_empty() {
        return Err(Error::validation("City name cannot be empty"));
    }

    let days = input.days.unwrap_or(1);
    if !(1..=5).contains(&days) {
        return Err(Error::validation("Days must be between 1 and 5"));
    }

    // 2. Get API key from metadata (injected by middleware)
    let api_key = extra.metadata
        .get("weather_api_key")
        .ok_or_else(|| Error::internal("Weather API key not configured"))?;

    // 3. Build HTTP client
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .context("Failed to create HTTP client")?;

    // 4. Make API call with error handling
    let url = format!(
        "https://api.weatherapi.com/v1/forecast.json?key={}&q={}&days={}",
        api_key, input.city, days
    );

    let response = client
        .get(&url)
        .send()
        .await
        .context("Failed to connect to weather API")?;

    // 5. Check HTTP status
    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();

        return match status.as_u16() {
            404 => Err(Error::validation(format!("City '{}' not found", input.city))),
            401 => Err(Error::internal("Invalid API key")),
            429 => Err(Error::internal("API rate limit exceeded")),
            _ => Err(Error::internal(format!(
                "API error {}: {}", status, error_text
            ))),
        };
    }

    // 6. Parse response
    let weather_data: WeatherApiResponse = response
        .json()
        .await
        .context("Failed to parse weather API response")?;

    // 7. Transform to output format
    let forecast = weather_data.forecast.forecastday
        .into_iter()
        .map(|day| DayForecast {
            day: day.date,
            high: day.day.maxtemp_c,
            low: day.day.mintemp_c,
            conditions: day.day.condition.text,
        })
        .collect();

    Ok(WeatherOutput {
        city: weather_data.location.name,
        temperature: weather_data.current.temp_c,
        conditions: weather_data.current.condition.text,
        forecast,
    })
}

// ============================================================================
// API RESPONSE TYPES (from external API)
// ============================================================================

#[derive(Debug, Deserialize)]
struct WeatherApiResponse {
    location: LocationData,
    current: CurrentWeather,
    forecast: ForecastData,
}

#[derive(Debug, Deserialize)]
struct LocationData {
    name: String,
}

#[derive(Debug, Deserialize)]
struct CurrentWeather {
    temp_c: f64,
    condition: Condition,
}

#[derive(Debug, Deserialize)]
struct ForecastData {
    forecastday: Vec<ForecastDay>,
}

#[derive(Debug, Deserialize)]
struct ForecastDay {
    date: String,
    day: DayData,
}

#[derive(Debug, Deserialize)]
struct DayData {
    maxtemp_c: f64,
    mintemp_c: f64,
    condition: Condition,
}

#[derive(Debug, Deserialize)]
struct Condition {
    text: String,
}

// ============================================================================
// BUILDER
// ============================================================================

pub fn build_weather_tool() -> TypedTool<WeatherInput, WeatherOutput> {
    TypedTool::new("get-weather", |input, extra| {
        Box::pin(weather_handler(input, extra))
    })
    .with_description("Get current weather and forecast for a city")
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_empty_city_validation() {
        let input = WeatherInput {
            city: String::new(),
            days: Some(3),
        };

        let result = weather_handler(input, RequestHandlerExtra::default()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot be empty"));
    }

    #[tokio::test]
    async fn test_invalid_days_validation() {
        let input = WeatherInput {
            city: "London".to_string(),
            days: Some(10),
        };

        let result = weather_handler(input, RequestHandlerExtra::default()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("between 1 and 5"));
    }

    // Integration test (requires API key)
    #[tokio::test]
    #[ignore]  // Ignore by default, run with: cargo test -- --ignored
    async fn test_real_api_call() {
        use std::collections::HashMap;

        let mut metadata = HashMap::new();
        metadata.insert(
            "weather_api_key".to_string(),
            std::env::var("WEATHER_API_KEY").expect("WEATHER_API_KEY required")
        );

        let extra = RequestHandlerExtra { metadata };

        let input = WeatherInput {
            city: "London".to_string(),
            days: Some(2),
        };

        let result = weather_handler(input, extra).await.unwrap();
        assert_eq!(result.city, "London");
        assert_eq!(result.forecast.len(), 2);
    }
}
```

### API Integration Best Practices

**1. Timeouts**: Always set timeouts
```rust
let client = Client::builder()
    .timeout(Duration::from_secs(10))
    .build()?;
```

**2. Retries**: For transient failures
```rust
use tokio::time::sleep;

async fn with_retry<F, T>(mut f: F, max_attempts: u32) -> Result<T>
where
    F: FnMut() -> Pin<Box<dyn Future<Output = Result<T>>>>,
{
    let mut attempt = 0;
    loop {
        attempt += 1;
        match f().await {
            Ok(result) => return Ok(result),
            Err(e) if attempt < max_attempts => {
                sleep(Duration::from_millis(100 * attempt as u64)).await;
                continue;
            }
            Err(e) => return Err(e),
        }
    }
}
```

**3. Rate Limiting**: Respect API limits
```rust
use std::sync::Arc;
use tokio::sync::Semaphore;

lazy_static! {
    static ref API_SEMAPHORE: Arc<Semaphore> = Arc::new(Semaphore::new(10));
}

async fn rate_limited_call() -> Result<Response> {
    let _permit = API_SEMAPHORE.acquire().await;
    // Make API call
}
```

**4. Caching**: Avoid redundant calls
```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

lazy_static! {
    static ref CACHE: Arc<RwLock<HashMap<String, CachedData>>> =
        Arc::new(RwLock::new(HashMap::new()));
}

async fn cached_fetch(key: &str) -> Result<Data> {
    // Check cache
    {
        let cache = CACHE.read().await;
        if let Some(cached) = cache.get(key) {
            if !cached.is_expired() {
                return Ok(cached.data.clone());
            }
        }
    }

    // Fetch fresh data
    let data = fetch_from_api(key).await?;

    // Update cache
    {
        let mut cache = CACHE.write().await;
        cache.insert(key.to_string(), CachedData::new(data.clone()));
    }

    Ok(data)
}
```

## Pattern 4: Stateful Tool (Database Access)

**Use Case**: Tools that need shared state, database connections, configuration

### Tool with Shared State

```rust
use pmcp::{Result, TypedTool, RequestHandlerExtra, Error};
use sqlx::SqlitePool;
use std::sync::Arc;

// ============================================================================
// SHARED STATE
// ============================================================================

#[derive(Clone)]
pub struct DatabaseTools {
    pool: Arc<SqlitePool>,
}

impl DatabaseTools {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool: Arc::new(pool),
        }
    }

    // Method that creates tool closures with access to self
    pub fn build_query_tool(&self) -> TypedTool<QueryInput, QueryOutput> {
        let pool = self.pool.clone();

        TypedTool::new("query-database", move |input, _extra| {
            let pool = pool.clone();
            Box::pin(async move {
                query_handler(input, pool).await
            })
        })
        .with_description("Execute SQL query on database")
    }
}

// ============================================================================
// TYPE DEFINITIONS
// ============================================================================

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct QueryInput {
    #[schemars(description = "SQL query to execute (SELECT only)")]
    pub query: String,

    #[schemars(description = "Maximum rows to return (default: 100)")]
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct QueryOutput {
    pub rows: Vec<serde_json::Value>,
    pub row_count: usize,
}

// ============================================================================
// HANDLER
// ============================================================================

async fn query_handler(
    input: QueryInput,
    pool: Arc<SqlitePool>
) -> Result<QueryOutput> {
    // 1. Validate query (security!)
    let query_lower = input.query.to_lowercase();
    if !query_lower.trim().starts_with("select") {
        return Err(Error::validation(
            "Only SELECT queries are allowed"
        ));
    }

    if query_lower.contains("drop") ||
       query_lower.contains("delete") ||
       query_lower.contains("update") ||
       query_lower.contains("insert") {
        return Err(Error::validation(
            "Dangerous keywords detected in query"
        ));
    }

    // 2. Apply limit
    let limit = input.limit.unwrap_or(100).min(1000);
    let limited_query = format!("{} LIMIT {}", input.query, limit);

    // 3. Execute query
    let rows = sqlx::query(&limited_query)
        .fetch_all(pool.as_ref())
        .await
        .context("Failed to execute query")?;

    // 4. Convert to JSON
    let json_rows: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|row| {
            // Convert SqliteRow to JSON
            // (Implementation depends on schema)
            serde_json::json!({})  // Simplified
        })
        .collect();

    Ok(QueryOutput {
        row_count: json_rows.len(),
        rows: json_rows,
    })
}
```

## Pattern 5: Tool with OAuth Authentication

**Use Case**: Tools that call authenticated APIs using user's OAuth token

### OAuth-Enabled Tool (pmcp 1.8.0+)

```rust
use pmcp::{Result, TypedTool, RequestHandlerExtra, Error};

// ============================================================================
// HANDLER WITH OAUTH
// ============================================================================

async fn github_repos_handler(
    input: GitHubReposInput,
    extra: RequestHandlerExtra
) -> Result<GitHubReposOutput> {
    // 1. Extract OAuth token from metadata
    //    (Injected by transport layer after OAuth validation)
    let token = extra.metadata
        .get("oauth_token")
        .ok_or_else(|| Error::validation(
            "GitHub authentication required. Please authenticate first."
        ))?;

    // 2. Build authenticated HTTP client
    let client = reqwest::Client::new();

    // 3. Make authenticated API call
    let response = client
        .get(&format!("https://api.github.com/users/{}/repos", input.username))
        .header("Authorization", format!("Bearer {}", token))
        .header("User-Agent", "pmcp-github-server")
        .send()
        .await
        .context("Failed to connect to GitHub API")?;

    // 4. Handle auth failures
    if response.status() == 401 {
        return Err(Error::validation(
            "Invalid GitHub token. Please re-authenticate."
        ));
    }

    if response.status() == 403 {
        return Err(Error::validation(
            "GitHub API rate limit exceeded or insufficient permissions."
        ));
    }

    if !response.status().is_success() {
        return Err(Error::internal(format!(
            "GitHub API error: {}", response.status()
        )));
    }

    // 5. Parse and return
    let repos: Vec<GitHubRepo> = response
        .json()
        .await
        .context("Failed to parse GitHub response")?;

    Ok(GitHubReposOutput {
        repos: repos.into_iter().take(10).collect(),
    })
}
```

## Error Handling Patterns

### Error Types

```rust
use pmcp::Error;

// Validation errors (client's fault, 4xx)
Error::validation("Invalid email format")
Error::validation(format!("User '{}' not found", username))

// Internal errors (server's fault, 5xx)
Error::internal("Database connection failed")
Error::internal(format!("Failed to parse config: {}", err))

// Protocol errors (MCP protocol violations)
Error::protocol("Invalid request format")
```

### Adding Context

```rust
use anyhow::Context;

let data = fetch_data()
    .await
    .context("Failed to fetch data from API")?;

let parsed = serde_json::from_str(&data)
    .context("Failed to parse JSON response")?;
```

### Error Propagation

```rust
async fn complex_operation() -> Result<Output> {
    // Use ? for automatic error propagation
    let step1 = do_step1().await?;
    let step2 = do_step2(step1).await?;
    let step3 = do_step3(step2).await?;

    Ok(step3)
}
```

## Testing Patterns

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_success_case() {
        let input = MyInput { /* ... */ };
        let result = my_handler(input, RequestHandlerExtra::default())
            .await
            .unwrap();

        assert_eq!(result.field, expected_value);
    }

    #[tokio::test]
    async fn test_error_case() {
        let input = MyInput { /* invalid data */ };
        let result = my_handler(input, RequestHandlerExtra::default()).await;

        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("expected error text"));
    }
}
```

### Property-Based Tests

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_add_commutative(a: f64, b: f64) {
        let result1 = add(a, b);
        let result2 = add(b, a);
        assert_eq!(result1, result2);
    }
}
```

## Common Mistakes to Avoid

❌ **Using unwrap() in production code**
```rust
// BAD
let value = map.get("key").unwrap();

// GOOD
let value = map.get("key")
    .ok_or_else(|| Error::validation("Missing required key"))?;
```

❌ **Ignoring errors**
```rust
// BAD
let _ = risky_operation().await;

// GOOD
risky_operation()
    .await
    .context("Risky operation failed")?;
```

❌ **Not validating inputs**
```rust
// BAD
async fn divide(a: f64, b: f64) -> f64 {
    a / b  // Crashes on b=0
}

// GOOD
async fn divide(a: f64, b: f64) -> Result<f64> {
    if b == 0.0 {
        return Err(Error::validation("Cannot divide by zero"));
    }
    Ok(a / b)
}
```

❌ **Exposing internal errors to clients**
```rust
// BAD
return Err(Error::validation(format!("DB error: {:?}", db_error)));

// GOOD
tracing::error!("Database error: {:?}", db_error);
return Err(Error::internal("Failed to access database"));
```

## Tool Implementation Checklist

- [ ] **Input type** defined with `JsonSchema` and descriptive fields
- [ ] **Output type** defined with `Serialize`
- [ ] **Validation** implemented (type-level, declarative, custom)
- [ ] **Error handling** with proper `Error` types
- [ ] **Error context** added with `.context()`
- [ ] **Async handler** using `async fn`
- [ ] **TypedTool builder** with description
- [ ] **Unit tests** for success cases
- [ ] **Unit tests** for error cases
- [ ] **Documentation** in code comments
- [ ] **No unwrap/panic** in production code

---

**Next**: Read resource patterns in `mcp-resource-patterns.md` for data access implementations.
