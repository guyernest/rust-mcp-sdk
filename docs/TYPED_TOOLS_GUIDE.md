# Type-Safe Tools with Schema Generation Guide

## Overview

The type-safe tool feature in PMCP v1.5.5+ provides automatic JSON schema generation from Rust types, offering:

1. **Security**: Type-safe argument validation at compile time and runtime
2. **Simplicity**: Define schemas using Rust structs instead of manual JSON
3. **Documentation**: Field descriptions from doc comments appear in schemas
4. **Flexibility**: Support for optional fields, defaults, enums, and nested structures

## Key Benefits

### 1. Security Through Type Safety

Instead of accepting arbitrary JSON that could contain injection attacks or unexpected fields:

```rust
// ❌ OLD: Vulnerable to arbitrary JSON
SimpleTool::new("tool", |args, _| {
    // args is raw JSON - could contain anything
    let query = args["query"].as_str()?; // Could be SQL injection
})

// ✅ NEW: Type-safe with automatic validation
TypedTool::new("tool", |args: QueryArgs, _| {
    // args is validated Rust struct - only expected fields
    let query = args.query; // Type-checked String
})
```

### 2. Field Descriptions

Use doc comments to add descriptions that appear in the JSON schema:

```rust
#[derive(JsonSchema, Deserialize, Serialize)]
struct UserArgs {
    /// The user's email address
    /// This description appears in the schema
    email: String,

    /// Age in years (must be 18+)
    age: u32,
}
```

Generated schema includes:
```json
{
  "properties": {
    "email": {
      "type": "string",
      "description": "The user's email address\nThis description appears in the schema"
    },
    "age": {
      "type": "integer",
      "description": "Age in years (must be 18+)"
    }
  }
}
```

### 3. Optional Fields

Use `Option<T>` for optional fields:

```rust
#[derive(JsonSchema, Deserialize, Serialize)]
struct SearchArgs {
    /// Required search query
    query: String,

    /// Optional limit (can be omitted)
    limit: Option<u32>,

    /// Optional with default (uses default if omitted)
    #[serde(default)]
    include_archived: bool,
}
```

### 4. Default Values

Provide defaults using serde attributes:

```rust
#[derive(JsonSchema, Deserialize, Serialize)]
struct ConfigArgs {
    /// Timeout in seconds
    #[serde(default = "default_timeout")]
    timeout: u32,

    /// Use verbose output
    #[serde(default)] // Uses Default::default() for bool (false)
    verbose: bool,
}

fn default_timeout() -> u32 {
    30
}
```

### 5. Enum Values (String Matching)

Enums automatically match string values:

```rust
#[derive(JsonSchema, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum Operation {
    Create,   // Matches "create"
    Update,   // Matches "update"
    Delete,   // Matches "delete"
}

#[derive(JsonSchema, Deserialize, Serialize)]
struct RequestArgs {
    operation: Operation,
}
```

Client sends: `{"operation": "create"}` → Parsed as `Operation::Create`

### 6. Runtime Validation

While `schemars` doesn't support validation attributes like regex/range, you perform validation in the handler:

```rust
TypedTool::new("process", |args: ProcessArgs, _| {
    Box::pin(async move {
        // Validate email format
        if !args.email.contains('@') {
            return Err(pmcp::Error::Validation(
                "Invalid email format".to_string()
            ));
        }

        // Validate numeric ranges
        if args.age < 18 || args.age > 120 {
            return Err(pmcp::Error::Validation(
                "Age must be between 18 and 120".to_string()
            ));
        }

        // Validate string patterns
        if !args.phone.starts_with('+') {
            return Err(pmcp::Error::Validation(
                "Phone must start with country code".to_string()
            ));
        }

        // Validate enum-like strings
        let valid_countries = ["US", "UK", "CA", "AU"];
        if !valid_countries.contains(&args.country.as_str()) {
            return Err(pmcp::Error::Validation(
                format!("Country must be one of: {:?}", valid_countries)
            ));
        }

        // Process the validated data
        Ok(json!({"success": true}))
    })
})
```

## Complete Example: Payment Processing

Here's a comprehensive example showing all features:

```rust
use pmcp::{TypedTool, ServerBuilder};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// Main payment request with all field types
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct PaymentRequest {
    /// Amount in cents (e.g., 1000 = $10.00)
    amount_cents: u32,

    /// ISO 4217 currency code (e.g., "USD")
    currency: String,

    /// Customer email for receipt
    email: String,

    /// Optional invoice number
    invoice_id: Option<String>,

    /// Send receipt (defaults to true)
    #[serde(default = "default_send_receipt")]
    send_receipt: bool,

    /// Payment method details
    method: PaymentMethod,

    /// Additional metadata
    #[serde(default)]
    metadata: Vec<Metadata>,
}

fn default_send_receipt() -> bool {
    true
}

// Enum with multiple variants
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
enum PaymentMethod {
    /// Credit card payment
    CreditCard {
        /// Last 4 digits
        last_four: String,
        /// Cardholder name
        holder: String,
    },
    /// Bank transfer
    BankTransfer {
        /// Account number
        account: String,
    },
    /// Digital wallet
    Wallet {
        /// Provider name
        provider: WalletProvider,
    }
}

// Simple string enum
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum WalletProvider {
    Paypal,
    ApplePay,
    GooglePay,
}

// Nested structure
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct Metadata {
    /// Metadata key
    key: String,
    /// Metadata value
    value: String,
}

// Create the tool with validation
let tool = TypedTool::new("process_payment", |args: PaymentRequest, _| {
    Box::pin(async move {
        // Validate amount (min $1, max $10,000)
        if args.amount_cents < 100 || args.amount_cents > 1_000_000 {
            return Err(pmcp::Error::Validation(
                "Amount must be between $1 and $10,000".to_string()
            ));
        }

        // Validate currency
        let valid_currencies = ["USD", "EUR", "GBP"];
        if !valid_currencies.contains(&args.currency.as_str()) {
            return Err(pmcp::Error::Validation(
                format!("Currency must be one of: {:?}", valid_currencies)
            ));
        }

        // Validate email
        if !args.email.contains('@') {
            return Err(pmcp::Error::Validation(
                "Invalid email format".to_string()
            ));
        }

        // Validate metadata count
        if args.metadata.len() > 10 {
            return Err(pmcp::Error::Validation(
                "Maximum 10 metadata entries".to_string()
            ));
        }

        // Validate payment method specifics
        match &args.method {
            PaymentMethod::CreditCard { last_four, holder } => {
                if last_four.len() != 4 || !last_four.chars().all(|c| c.is_ascii_digit()) {
                    return Err(pmcp::Error::Validation(
                        "Last four must be 4 digits".to_string()
                    ));
                }
                if holder.is_empty() {
                    return Err(pmcp::Error::Validation(
                        "Cardholder name required".to_string()
                    ));
                }
            }
            PaymentMethod::BankTransfer { account } => {
                if account.len() < 10 {
                    return Err(pmcp::Error::Validation(
                        "Invalid account number".to_string()
                    ));
                }
            }
            PaymentMethod::Wallet { provider } => {
                // Provider is already validated by enum
                match provider {
                    WalletProvider::Paypal => { /* Paypal-specific validation */ }
                    WalletProvider::ApplePay => { /* Apple Pay validation */ }
                    WalletProvider::GooglePay => { /* Google Pay validation */ }
                }
            }
        }

        // Process the payment
        Ok(json!({
            "transaction_id": "txn_12345",
            "amount": args.amount_cents,
            "currency": args.currency,
            "status": "success"
        }))
    })
})
.with_description("Process secure payments with validation");
```

## Validation Patterns

### 1. String Format Validation
```rust
// Email
if !email.contains('@') || !email.contains('.') { /* error */ }

// Phone with regex-like check
if !phone.starts_with('+') || phone.len() < 10 { /* error */ }

// URL
if !url.starts_with("http://") && !url.starts_with("https://") { /* error */ }
```

### 2. Numeric Range Validation
```rust
// Age range
if age < 18 || age > 120 { /* error */ }

// Percentage
if percentage < 0.0 || percentage > 100.0 { /* error */ }

// Array size
if items.len() > 100 { /* error */ }
```

### 3. Enum-like String Validation
```rust
// Fixed set of values
let valid_values = ["option1", "option2", "option3"];
if !valid_values.contains(&value.as_str()) { /* error */ }

// Case-insensitive
let valid = ["USD", "EUR", "GBP"];
if !valid.iter().any(|&v| v.eq_ignore_ascii_case(&currency)) { /* error */ }
```

### 4. Complex Pattern Validation
```rust
// SQL injection prevention
if !query.to_uppercase().trim_start().starts_with("SELECT") {
    return Err(pmcp::Error::Validation("Only SELECT queries allowed".into()));
}

// Path traversal prevention
if path.contains("..") || !path.starts_with("/allowed/") {
    return Err(pmcp::Error::Validation("Invalid path".into()));
}

// Custom business logic
if amount > user.balance {
    return Err(pmcp::Error::Validation("Insufficient funds".into()));
}
```

## Best Practices

1. **Use Doc Comments**: Always document fields with `///` comments - they become schema descriptions

2. **Validate Early**: Perform validation at the start of your handler before any processing

3. **Specific Error Messages**: Provide clear, actionable error messages for validation failures

4. **Use Enums for Fixed Values**: Instead of validating strings against lists, use enums when possible

5. **Combine Optional and Default**:
   - Use `Option<T>` when field can be completely absent
   - Use `#[serde(default)]` when field should have a default value
   - Use both for optional fields with defaults when present

6. **Security First**: Always validate untrusted input, even with type safety:
   - Check string lengths to prevent DoS
   - Validate numeric ranges
   - Sanitize paths and queries
   - Verify enum-like strings

7. **Test Your Schemas**: Write tests to verify schema generation:
```rust
#[test]
fn test_schema() {
    let schema = schemars::schema_for!(MyArgs);
    let json = serde_json::to_value(&schema).unwrap();

    // Verify structure
    assert!(json["properties"]["field_name"].is_object());

    // Test with sample data
    let sample = json!({"field": "value"});
    let parsed: MyArgs = serde_json::from_value(sample).unwrap();
}
```

## Migration from SimpleTool

Before:
```rust
SimpleTool::new("mytool", |args, _| {
    Box::pin(async move {
        let name = args["name"].as_str().ok_or("name required")?;
        let age = args["age"].as_u64().unwrap_or(0);
        // Manual validation...
        Ok(json!({"result": "success"}))
    })
})
```

After:
```rust
#[derive(JsonSchema, Deserialize, Serialize)]
struct MyToolArgs {
    /// User name (required)
    name: String,
    /// User age (optional)
    #[serde(default)]
    age: u32,
}

TypedTool::new("mytool", |args: MyToolArgs, _| {
    Box::pin(async move {
        // args.name and args.age are already typed and validated
        Ok(json!({"result": "success"}))
    })
})
```

## Summary

Type-safe tools with schema generation provide:

- **Compile-time type safety**: Catch errors before runtime
- **Automatic schema generation**: No manual JSON schema writing
- **Rich documentation**: Field descriptions from doc comments
- **Flexible validation**: Runtime validation with clear error messages
- **Security by default**: Only expected fields are accepted
- **Better developer experience**: IDE autocomplete and type checking

This approach makes MCP servers more secure, easier to develop, and self-documenting.