::: exercise
id: ch05-01-validation-errors
difficulty: intermediate
time: 25 minutes
:::

You're improving an MCP server that has poor validation. When AI clients send
invalid parameters, they get unhelpful errors like "Invalid input" and can't
self-correct. Your task is to implement AI-friendly validation with clear,
actionable error messages.

::: objectives
thinking:
  - Understand why validation errors are feedback for AI self-correction
  - Design error messages that tell AI what went wrong and how to fix it
  - Apply the four levels of validation: schema, format, business, security
doing:
  - Implement a ValidationError struct with helpful fields
  - Add validation for required fields, types, formats, and business rules
  - Return errors that enable AI retry with correct parameters
:::

::: discussion
- When an AI gets "Invalid input", what can it do? What about "expected: 2024-11-15, received: November 15"?
- Why is silent coercion (using defaults for invalid values) bad for AI clients?
- How might an AI use error codes like RATE_LIMITED vs NOT_FOUND differently?
:::

::: starter file="src/validation.rs"
```rust
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// A validation error with AI-friendly details
///
/// TODO: Add fields that help AI self-correct:
/// - code: Error code for programmatic handling
/// - field: Which field had the problem
/// - message: Human-readable explanation
/// - expected: What format/value was expected
/// - received: What was actually sent
#[derive(Debug, Serialize)]
pub struct ValidationError {
    // TODO: Add fields here
}

impl ValidationError {
    /// Create error for a missing required field
    pub fn missing_field(field: &str) -> Self {
        // TODO: Implement
        todo!()
    }

    /// Create error for wrong type (e.g., string instead of number)
    pub fn invalid_type(field: &str, expected: &str, received: &str) -> Self {
        // TODO: Implement
        todo!()
    }

    /// Create error for invalid format (e.g., wrong date format)
    pub fn invalid_format(field: &str, expected_format: &str, example: &str, received: &str) -> Self {
        // TODO: Implement
        todo!()
    }

    /// Create error for business rule violation
    pub fn business_rule(field: &str, rule: &str, received: &str) -> Self {
        // TODO: Implement
        todo!()
    }

    /// Convert to JSON for tool response
    pub fn to_json(&self) -> Value {
        serde_json::to_value(self).expect("Serialization should not fail")
    }
}

/// Input for the order query tool
#[derive(Debug, Deserialize)]
pub struct OrderQueryInput {
    pub customer_id: Option<String>,
    pub date_range: Option<DateRange>,
    pub status: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct DateRange {
    pub start: String,
    pub end: String,
}

/// Validate order query input with helpful errors
///
/// TODO: Implement validation that checks:
/// 1. At least one filter is provided (customer_id, date_range, or status)
/// 2. date_range dates are in ISO 8601 format (YYYY-MM-DD)
/// 3. date_range.end is not before date_range.start
/// 4. status is one of: pending, shipped, delivered, cancelled
/// 5. limit is between 1 and 1000
pub fn validate_order_query(input: &OrderQueryInput) -> Result<(), ValidationError> {
    // TODO: Implement validation
    // Remember: Return clear errors that help AI self-correct!

    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_missing_all_filters() {
        let input = OrderQueryInput {
            customer_id: None,
            date_range: None,
            status: None,
            limit: None,
        };

        let err = validate_order_query(&input).unwrap_err();
        // Error should explain that at least one filter is needed
        let json = err.to_json();
        assert!(json.get("code").is_some());
        assert!(json.get("message").is_some());
    }

    #[test]
    fn test_invalid_date_format() {
        let input = OrderQueryInput {
            customer_id: None,
            date_range: Some(DateRange {
                start: "November 15, 2024".to_string(),
                end: "November 20, 2024".to_string(),
            }),
            status: None,
            limit: None,
        };

        let err = validate_order_query(&input).unwrap_err();
        let json = err.to_json();

        // Error should show expected format and what was received
        assert!(json["expected"].as_str().unwrap().contains("2024-11-15"));
        assert!(json["received"].as_str().unwrap().contains("November"));
    }

    #[test]
    fn test_invalid_status() {
        let input = OrderQueryInput {
            customer_id: None,
            date_range: None,
            status: Some("in_progress".to_string()),
            limit: None,
        };

        let err = validate_order_query(&input).unwrap_err();
        let json = err.to_json();

        // Error should list valid options
        let expected = json["expected"].as_str().unwrap();
        assert!(expected.contains("pending") || expected.contains("shipped"));
    }

    #[test]
    fn test_limit_out_of_range() {
        let input = OrderQueryInput {
            customer_id: Some("CUST-001".to_string()),
            date_range: None,
            status: None,
            limit: Some(5000),
        };

        let err = validate_order_query(&input).unwrap_err();
        let json = err.to_json();

        assert_eq!(json["field"], "limit");
        assert!(json["message"].as_str().unwrap().contains("1000"));
    }

    #[test]
    fn test_valid_input_passes() {
        let input = OrderQueryInput {
            customer_id: Some("CUST-001".to_string()),
            date_range: None,
            status: None,
            limit: Some(100),
        };

        assert!(validate_order_query(&input).is_ok());
    }
}
```
:::

::: hint level=1 title="ValidationError structure"
Design the struct with fields that help AI understand and fix the error:
```rust
#[derive(Debug, Serialize)]
pub struct ValidationError {
    pub code: String,       // e.g., "MISSING_REQUIRED_FIELD"
    pub field: String,      // e.g., "date_range.start"
    pub message: String,    // Human-readable explanation
    pub expected: Option<String>,  // What was expected
    pub received: Option<String>,  // What was sent
}
```
:::

::: hint level=2 title="Constructor patterns"
Create constructors for common error types:
```rust
impl ValidationError {
    pub fn missing_field(field: &str) -> Self {
        Self {
            code: "MISSING_REQUIRED_FIELD".to_string(),
            field: field.to_string(),
            message: format!("At least one filter is required: {}", field),
            expected: Some("A value for one of the filter fields".to_string()),
            received: None,
        }
    }

    pub fn invalid_format(field: &str, expected_format: &str, example: &str, received: &str) -> Self {
        Self {
            code: "INVALID_FORMAT".to_string(),
            field: field.to_string(),
            message: format!("Field '{}' has invalid format", field),
            expected: Some(format!("{} (e.g., {})", expected_format, example)),
            received: Some(received.to_string()),
        }
    }
}
```
:::

::: hint level=3 title="Date validation"
For validating ISO 8601 dates:
```rust
fn is_valid_iso_date(s: &str) -> bool {
    // Simple check: YYYY-MM-DD format
    if s.len() != 10 {
        return false;
    }
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 3 {
        return false;
    }
    parts[0].len() == 4 && parts[1].len() == 2 && parts[2].len() == 2
        && parts.iter().all(|p| p.chars().all(|c| c.is_ascii_digit()))
}
```
:::

::: solution
```rust
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Serialize)]
pub struct ValidationError {
    pub code: String,
    pub field: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub received: Option<String>,
}

impl ValidationError {
    pub fn missing_field(field: &str) -> Self {
        Self {
            code: "MISSING_REQUIRED_FIELD".to_string(),
            field: field.to_string(),
            message: format!("Required field '{}' is missing or no filters provided", field),
            expected: Some(format!("A value for '{}'", field)),
            received: None,
        }
    }

    pub fn invalid_type(field: &str, expected: &str, received: &str) -> Self {
        Self {
            code: "INVALID_TYPE".to_string(),
            field: field.to_string(),
            message: format!("Field '{}' has wrong type", field),
            expected: Some(expected.to_string()),
            received: Some(received.to_string()),
        }
    }

    pub fn invalid_format(field: &str, expected_format: &str, example: &str, received: &str) -> Self {
        Self {
            code: "INVALID_FORMAT".to_string(),
            field: field.to_string(),
            message: format!("Field '{}' has invalid format", field),
            expected: Some(format!("{} (e.g., {})", expected_format, example)),
            received: Some(received.to_string()),
        }
    }

    pub fn business_rule(field: &str, rule: &str, received: &str) -> Self {
        Self {
            code: "BUSINESS_RULE_VIOLATION".to_string(),
            field: field.to_string(),
            message: rule.to_string(),
            expected: None,
            received: Some(received.to_string()),
        }
    }

    pub fn invalid_value(field: &str, message: &str, valid_options: &[&str], received: &str) -> Self {
        Self {
            code: "INVALID_VALUE".to_string(),
            field: field.to_string(),
            message: message.to_string(),
            expected: Some(format!("One of: {}", valid_options.join(", "))),
            received: Some(received.to_string()),
        }
    }

    pub fn out_of_range(field: &str, min: i64, max: i64, received: i64) -> Self {
        Self {
            code: "OUT_OF_RANGE".to_string(),
            field: field.to_string(),
            message: format!("Field '{}' must be between {} and {}", field, min, max),
            expected: Some(format!("{} to {}", min, max)),
            received: Some(received.to_string()),
        }
    }

    pub fn to_json(&self) -> Value {
        serde_json::to_value(self).expect("Serialization should not fail")
    }
}

#[derive(Debug, Deserialize)]
pub struct OrderQueryInput {
    pub customer_id: Option<String>,
    pub date_range: Option<DateRange>,
    pub status: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct DateRange {
    pub start: String,
    pub end: String,
}

fn is_valid_iso_date(s: &str) -> bool {
    if s.len() != 10 {
        return false;
    }
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 3 {
        return false;
    }
    parts[0].len() == 4
        && parts[1].len() == 2
        && parts[2].len() == 2
        && parts.iter().all(|p| p.chars().all(|c| c.is_ascii_digit()))
}

const VALID_STATUSES: &[&str] = &["pending", "shipped", "delivered", "cancelled"];

pub fn validate_order_query(input: &OrderQueryInput) -> Result<(), ValidationError> {
    // 1. Check at least one filter is provided
    if input.customer_id.is_none() && input.date_range.is_none() && input.status.is_none() {
        return Err(ValidationError {
            code: "MISSING_FILTER".to_string(),
            field: "customer_id, date_range, or status".to_string(),
            message: "At least one filter must be provided".to_string(),
            expected: Some("Provide customer_id, date_range, or status".to_string()),
            received: Some("No filters provided".to_string()),
        });
    }

    // 2. Validate date_range format
    if let Some(ref date_range) = input.date_range {
        if !is_valid_iso_date(&date_range.start) {
            return Err(ValidationError::invalid_format(
                "date_range.start",
                "ISO 8601 date (YYYY-MM-DD)",
                "2024-11-15",
                &date_range.start,
            ));
        }
        if !is_valid_iso_date(&date_range.end) {
            return Err(ValidationError::invalid_format(
                "date_range.end",
                "ISO 8601 date (YYYY-MM-DD)",
                "2024-11-20",
                &date_range.end,
            ));
        }

        // 3. Check end is not before start
        if date_range.end < date_range.start {
            return Err(ValidationError::business_rule(
                "date_range",
                "End date cannot be before start date",
                &format!("start: {}, end: {}", date_range.start, date_range.end),
            ));
        }
    }

    // 4. Validate status
    if let Some(ref status) = input.status {
        if !VALID_STATUSES.contains(&status.as_str()) {
            return Err(ValidationError::invalid_value(
                "status",
                "Invalid order status",
                VALID_STATUSES,
                status,
            ));
        }
    }

    // 5. Validate limit range
    if let Some(limit) = input.limit {
        if limit < 1 || limit > 1000 {
            return Err(ValidationError::out_of_range("limit", 1, 1000, limit));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_missing_all_filters() {
        let input = OrderQueryInput {
            customer_id: None,
            date_range: None,
            status: None,
            limit: None,
        };

        let err = validate_order_query(&input).unwrap_err();
        assert_eq!(err.code, "MISSING_FILTER");
    }

    #[test]
    fn test_invalid_date_format() {
        let input = OrderQueryInput {
            customer_id: None,
            date_range: Some(DateRange {
                start: "November 15, 2024".to_string(),
                end: "November 20, 2024".to_string(),
            }),
            status: None,
            limit: None,
        };

        let err = validate_order_query(&input).unwrap_err();
        assert_eq!(err.code, "INVALID_FORMAT");
        assert!(err.expected.as_ref().unwrap().contains("2024-11-15"));
        assert!(err.received.as_ref().unwrap().contains("November"));
    }

    #[test]
    fn test_end_before_start() {
        let input = OrderQueryInput {
            customer_id: None,
            date_range: Some(DateRange {
                start: "2024-11-20".to_string(),
                end: "2024-11-15".to_string(),
            }),
            status: None,
            limit: None,
        };

        let err = validate_order_query(&input).unwrap_err();
        assert_eq!(err.code, "BUSINESS_RULE_VIOLATION");
    }

    #[test]
    fn test_invalid_status() {
        let input = OrderQueryInput {
            customer_id: None,
            date_range: None,
            status: Some("in_progress".to_string()),
            limit: None,
        };

        let err = validate_order_query(&input).unwrap_err();
        assert_eq!(err.code, "INVALID_VALUE");
        assert!(err.expected.as_ref().unwrap().contains("pending"));
    }

    #[test]
    fn test_limit_too_high() {
        let input = OrderQueryInput {
            customer_id: Some("CUST-001".to_string()),
            date_range: None,
            status: None,
            limit: Some(5000),
        };

        let err = validate_order_query(&input).unwrap_err();
        assert_eq!(err.code, "OUT_OF_RANGE");
        assert_eq!(err.field, "limit");
    }

    #[test]
    fn test_valid_input() {
        let input = OrderQueryInput {
            customer_id: Some("CUST-001".to_string()),
            date_range: Some(DateRange {
                start: "2024-11-01".to_string(),
                end: "2024-11-30".to_string(),
            }),
            status: Some("shipped".to_string()),
            limit: Some(100),
        };

        assert!(validate_order_query(&input).is_ok());
    }
}
```

### Explanation

**ValidationError Design:**
The struct includes all information an AI needs to self-correct:
- `code`: Programmatic identifier (INVALID_FORMAT, OUT_OF_RANGE, etc.)
- `field`: Exact field path (date_range.start, not just "input")
- `message`: Human-readable explanation
- `expected`: What the AI should have sent (with examples!)
- `received`: What was actually sent (for comparison)

**Validation Levels:**
1. **Schema**: Missing required fields
2. **Format**: Date format validation
3. **Business**: End date must be after start date
4. **Range**: Limit between 1-1000

**AI Feedback Loop:**
When an AI sends `date_range.start: "November 15, 2024"`, it receives:
```json
{
  "code": "INVALID_FORMAT",
  "field": "date_range.start",
  "message": "Field 'date_range.start' has invalid format",
  "expected": "ISO 8601 date (YYYY-MM-DD) (e.g., 2024-11-15)",
  "received": "November 15, 2024"
}
```

The AI can now retry with `date_range.start: "2024-11-15"` - it learned from the error!
:::

::: reflection
- How would you handle validation for deeply nested objects?
- Should you return the first error or collect all errors?
- How might different error codes trigger different AI behaviors?
- What's the balance between helpful detail and information leakage?
:::
