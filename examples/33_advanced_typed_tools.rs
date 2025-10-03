//! Advanced typed tools example showing field descriptions, validation, and constraints
//!
//! This example demonstrates:
//! - Field descriptions using doc comments
//! - Optional fields with Option<T>
//! - Default values
//! - Validation with regex, ranges, and custom validators
//! - Enum string matching
//! - Complex nested structures

use anyhow::Result;
use pmcp::{ServerBuilder, ServerCapabilities, TypedTool};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::info;

// Basic example with field descriptions and optional fields
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct UserRegistration {
    /// The user's email address (required)
    /// Must be a valid email format
    email: String,

    /// The user's full name
    /// Minimum 2 characters, maximum 100 characters
    name: String,

    /// The user's age (optional)
    /// Must be between 18 and 120
    age: Option<u32>,

    /// User's phone number (optional)
    /// Format: +XX-XXX-XXX-XXXX
    #[serde(default)]
    phone: Option<String>,

    /// Whether to subscribe to newsletter
    /// Defaults to false if not provided
    #[serde(default)]
    subscribe_newsletter: bool,

    /// User's country (optional with default)
    /// Defaults to "US" if not provided
    #[serde(default = "default_country")]
    country: String,

    /// Account type for the user
    /// Defaults to Free if not provided
    #[serde(default = "default_account_type")]
    account_type: AccountType,
}

fn default_country() -> String {
    "US".to_string()
}

fn default_account_type() -> AccountType {
    AccountType::Free
}

// Example with string enums that automatically match
#[derive(Debug, Deserialize, Serialize, JsonSchema, Clone)]
#[serde(rename_all = "lowercase")]
enum AccountType {
    /// Free tier account
    Free,
    /// Professional account with advanced features
    Professional,
    /// Enterprise account with full access
    Enterprise,
}

// Example with validation done in handler
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct DatabaseQuery {
    /// The SQL query to execute
    /// Must start with SELECT (read-only queries)
    query: String,

    /// Maximum number of results to return
    /// Must be between 1 and 1000
    #[serde(default = "default_limit")]
    limit: u32,

    /// Database name to query
    /// Must be one of: users, products, orders, analytics
    database: String,

    /// Query timeout in seconds
    /// Defaults to 30 seconds, max 300 seconds
    #[serde(default = "default_timeout")]
    timeout_seconds: u32,

    /// Include metadata in response
    #[serde(default)]
    include_metadata: bool,
}

fn default_limit() -> u32 {
    100
}

fn default_timeout() -> u32 {
    30
}

// Example with nested structures
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct FileOperation {
    /// The operation to perform
    operation: FileOpType,

    /// File path (must be within /tmp for safety)
    /// Path must start with /tmp/
    path: String,

    /// File permissions (Unix octal notation)
    /// Must be between 0o000 and 0o777
    #[serde(default = "default_permissions")]
    permissions: u32,

    /// Additional options for the operation
    options: FileOptions,
}

fn default_permissions() -> u32 {
    0o644 // rw-r--r--
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum FileOpType {
    /// Read file contents
    Read,
    /// Write data to file
    Write {
        /// Content to write (required for write operations, max 10MB)
        content: String,
    },
    /// Copy file to another location
    Copy {
        /// Destination path (must also be in /tmp)
        destination: String,
    },
    /// Delete file (use with caution)
    Delete,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct FileOptions {
    /// Create parent directories if they don't exist
    #[serde(default)]
    create_parents: bool,

    /// Overwrite existing file
    #[serde(default)]
    overwrite: bool,

    /// Backup existing file before operation
    #[serde(default)]
    backup: bool,

    /// Encoding for text files (utf8, utf16, or ascii)
    #[serde(default = "default_encoding")]
    encoding: String,
}

fn default_encoding() -> String {
    "utf8".to_string()
}

// Example with runtime validation
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct PaymentRequest {
    /// Amount in cents (minimum $1.00, max $10,000)
    amount_cents: u32,

    /// Currency code (ISO 4217) - Examples: USD, EUR, GBP
    currency: String,

    /// Payment method
    payment_method: PaymentMethod,

    /// Customer email for receipt
    customer_email: String,

    /// Optional description for the payment (max 500 chars)
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,

    /// Metadata as key-value pairs (max 10 entries)
    #[serde(default)]
    metadata: Vec<MetadataEntry>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "type")]
enum PaymentMethod {
    /// Credit card payment
    CreditCard {
        /// Last 4 digits of card (must be exactly 4 digits)
        last_four: String,

        /// Cardholder name
        holder_name: String,
    },
    /// Bank transfer
    BankTransfer {
        /// IBAN or account number (15-34 characters)
        account: String,
    },
    /// Digital wallet
    Wallet {
        /// Wallet provider (paypal, apple_pay, or google_pay)
        provider: String,
    },
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct MetadataEntry {
    /// Metadata key (alphanumeric and underscores only, max 50 chars)
    key: String,

    /// Metadata value (max 200 chars)
    value: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("Starting advanced typed tools example");

    let server = ServerBuilder::new()
        .name("advanced-typed-tools")
        .version("1.0.0")
        .capabilities(ServerCapabilities {
            tools: Some(pmcp::types::ToolCapabilities::default()),
            ..Default::default()
        })
        // User registration with validation
        .tool(
            "register_user",
            TypedTool::new("register_user", |args: UserRegistration, _extra| {
                Box::pin(async move {
                    // Additional runtime validation if needed
                    if args.email.contains("spam") {
                        return Err(pmcp::Error::Validation(
                            "Email domain not allowed".to_string(),
                        ));
                    }

                    // Check age if provided
                    if let Some(age) = args.age {
                        if age < 18 {
                            return Err(pmcp::Error::Validation(
                                "User must be 18 or older".to_string(),
                            ));
                        }
                    }

                    // Set user quota based on account type
                    let quota = match args.account_type {
                        AccountType::Free => 100,
                        AccountType::Professional => 1000,
                        AccountType::Enterprise => 10000,
                    };

                    Ok(json!({
                        "success": true,
                        "email": args.email,
                        "name": args.name,
                        "country": args.country,
                        "account_type": format!("{:?}", args.account_type).to_lowercase(),
                        "quota": quota,
                        "newsletter": args.subscribe_newsletter,
                        "has_phone": args.phone.is_some()
                    }))
                })
            })
            .with_description("Register a new user with validation"),
        )
        // Database query with runtime validation
        .tool(
            "query_database",
            TypedTool::new("query_database", |args: DatabaseQuery, _extra| {
                Box::pin(async move {
                    // Validate query starts with SELECT
                    if !args.query.to_uppercase().trim_start().starts_with("SELECT") {
                        return Err(pmcp::Error::Validation(
                            "Only SELECT queries are allowed".to_string(),
                        ));
                    }

                    // Validate database name
                    let allowed_databases = ["users", "products", "orders", "analytics"];
                    if !allowed_databases.contains(&args.database.as_str()) {
                        return Err(pmcp::Error::Validation(format!(
                            "Invalid database '{}'. Must be one of: {:?}",
                            args.database, allowed_databases
                        )));
                    }

                    // Validate limit range
                    if args.limit == 0 || args.limit > 1000 {
                        return Err(pmcp::Error::Validation(
                            "Limit must be between 1 and 1000".to_string(),
                        ));
                    }

                    // Validate timeout
                    if args.timeout_seconds == 0 || args.timeout_seconds > 300 {
                        return Err(pmcp::Error::Validation(
                            "Timeout must be between 1 and 300 seconds".to_string(),
                        ));
                    }

                    Ok(json!({
                        "database": args.database,
                        "query": args.query,
                        "limit": args.limit,
                        "timeout": args.timeout_seconds,
                        "rows": [
                            {"id": 1, "name": "Example"},
                            {"id": 2, "name": "Data"}
                        ],
                        "metadata_included": args.include_metadata
                    }))
                })
            })
            .with_description("Execute read-only database queries"),
        )
        // File operations with safety constraints
        .tool(
            "file_operation",
            TypedTool::new("file_operation", |args: FileOperation, _extra| {
                Box::pin(async move {
                    // Validate path is within /tmp
                    if !args.path.starts_with("/tmp/") {
                        return Err(pmcp::Error::Validation(
                            "Path must be within /tmp/ directory".to_string(),
                        ));
                    }

                    // Additional safety checks
                    if args.path.contains("..") {
                        return Err(pmcp::Error::Validation(
                            "Path traversal not allowed".to_string(),
                        ));
                    }

                    // Validate permissions
                    if args.permissions > 0o777 {
                        return Err(pmcp::Error::Validation(
                            "Invalid permissions (must be 0o000-0o777)".to_string(),
                        ));
                    }

                    // Validate encoding
                    let valid_encodings = ["utf8", "utf16", "ascii"];
                    if !valid_encodings.contains(&args.options.encoding.as_str()) {
                        return Err(pmcp::Error::Validation(format!(
                            "Invalid encoding. Must be one of: {:?}",
                            valid_encodings
                        )));
                    }

                    let result = match args.operation {
                        FileOpType::Read => {
                            json!({
                                "operation": "read",
                                "path": args.path,
                                "content": "File contents here...",
                                "encoding": args.options.encoding
                            })
                        }
                        FileOpType::Write { content } => {
                            // Validate content size (max 10MB)
                            if content.len() > 10_485_760 {
                                return Err(pmcp::Error::Validation(
                                    "Content exceeds maximum size of 10MB".to_string(),
                                ));
                            }

                            json!({
                                "operation": "write",
                                "path": args.path,
                                "bytes_written": content.len(),
                                "overwrite": args.options.overwrite,
                                "backup_created": args.options.backup
                            })
                        }
                        FileOpType::Copy { destination } => {
                            // Validate destination is also in /tmp
                            if !destination.starts_with("/tmp/") {
                                return Err(pmcp::Error::Validation(
                                    "Destination must be within /tmp/".to_string(),
                                ));
                            }
                            if destination.contains("..") {
                                return Err(pmcp::Error::Validation(
                                    "Path traversal not allowed in destination".to_string(),
                                ));
                            }

                            json!({
                                "operation": "copy",
                                "source": args.path,
                                "destination": destination,
                                "success": true
                            })
                        }
                        FileOpType::Delete => {
                            json!({
                                "operation": "delete",
                                "path": args.path,
                                "backup_created": args.options.backup,
                                "success": true
                            })
                        }
                    };

                    Ok(result)
                })
            })
            .with_description("Perform safe file operations within /tmp"),
        )
        // Payment processing with complex validation
        .tool(
            "process_payment",
            TypedTool::new("process_payment", |args: PaymentRequest, _extra| {
                Box::pin(async move {
                    // Validate amount (100 cents = $1.00 minimum, 1000000 cents = $10,000 max)
                    if args.amount_cents < 100 || args.amount_cents > 1_000_000 {
                        return Err(pmcp::Error::Validation(
                            "Amount must be between $1.00 and $10,000.00".to_string(),
                        ));
                    }

                    // Validate currency format (3 uppercase letters)
                    if args.currency.len() != 3 || !args.currency.chars().all(|c| c.is_uppercase()) {
                        return Err(pmcp::Error::Validation(
                            "Currency must be a 3-letter ISO code (e.g., USD)".to_string(),
                        ));
                    }

                    // Validate currency is supported
                    let supported_currencies = ["USD", "EUR", "GBP", "JPY"];
                    if !supported_currencies.contains(&args.currency.as_str()) {
                        return Err(pmcp::Error::Validation(format!(
                            "Currency {} not supported",
                            args.currency
                        )));
                    }

                    // Validate email format (basic check)
                    if !args.customer_email.contains('@') || !args.customer_email.contains('.') {
                        return Err(pmcp::Error::Validation(
                            "Invalid email address format".to_string(),
                        ));
                    }

                    // Validate description length if provided
                    if let Some(ref desc) = args.description {
                        if desc.len() > 500 {
                            return Err(pmcp::Error::Validation(
                                "Description cannot exceed 500 characters".to_string(),
                            ));
                        }
                    }

                    // Validate metadata
                    if args.metadata.len() > 10 {
                        return Err(pmcp::Error::Validation(
                            "Maximum 10 metadata entries allowed".to_string(),
                        ));
                    }

                    for entry in &args.metadata {
                        if entry.key.len() > 50 {
                            return Err(pmcp::Error::Validation(
                                "Metadata key cannot exceed 50 characters".to_string(),
                            ));
                        }
                        if !entry.key.chars().all(|c| c.is_alphanumeric() || c == '_') {
                            return Err(pmcp::Error::Validation(
                                "Metadata key must be alphanumeric with underscores only".to_string(),
                            ));
                        }
                        if entry.value.len() > 200 {
                            return Err(pmcp::Error::Validation(
                                "Metadata value cannot exceed 200 characters".to_string(),
                            ));
                        }
                    }

                    // Validate and process based on payment method
                    let method_details = match &args.payment_method {
                        PaymentMethod::CreditCard { last_four, holder_name } => {
                            // Validate last_four is exactly 4 digits
                            if last_four.len() != 4 || !last_four.chars().all(|c| c.is_ascii_digit()) {
                                return Err(pmcp::Error::Validation(
                                    "Card last four must be exactly 4 digits".to_string(),
                                ));
                            }
                            // Validate holder name
                            if holder_name.len() < 2 || holder_name.len() > 100 {
                                return Err(pmcp::Error::Validation(
                                    "Cardholder name must be 2-100 characters".to_string(),
                                ));
                            }

                            json!({
                                "type": "credit_card",
                                "last_four": last_four,
                                "holder": holder_name
                            })
                        }
                        PaymentMethod::BankTransfer { account } => {
                            // Validate account length (IBAN or account number)
                            if account.len() < 15 || account.len() > 34 {
                                return Err(pmcp::Error::Validation(
                                    "Account must be 15-34 characters (IBAN or account number)".to_string(),
                                ));
                            }

                            json!({
                                "type": "bank_transfer",
                                "account": format!("{}****", &account[..4.min(account.len())])
                            })
                        }
                        PaymentMethod::Wallet { provider } => {
                            // Validate wallet provider
                            let valid_providers = ["paypal", "apple_pay", "google_pay"];
                            if !valid_providers.contains(&provider.as_str()) {
                                return Err(pmcp::Error::Validation(format!(
                                    "Invalid wallet provider. Must be one of: {:?}",
                                    valid_providers
                                )));
                            }

                            json!({
                                "type": "wallet",
                                "provider": provider
                            })
                        }
                    };

                    Ok(json!({
                        "transaction_id": "txn_abc123",
                        "amount_cents": args.amount_cents,
                        "currency": args.currency,
                        "method": method_details,
                        "customer_email": args.customer_email,
                        "description": args.description,
                        "metadata_count": args.metadata.len(),
                        "status": "success"
                    }))
                })
            })
            .with_description("Process payments with validation"),
        )
        .build()?;

    info!("Server initialized with advanced typed tools");
    info!("All tools have comprehensive validation and documentation");

    server.run_stdio().await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_generation_with_descriptions() {
        // Test that schemas are generated with field descriptions
        let schema = schemars::schema_for!(UserRegistration);
        let schema_json = serde_json::to_value(&schema).unwrap();

        // Check that properties are present
        assert!(schema_json["properties"]["email"].is_object());
        assert!(schema_json["properties"]["name"].is_object());
        assert!(schema_json["properties"]["age"].is_object());
        assert!(schema_json["properties"]["phone"].is_object());
        assert!(schema_json["properties"]["subscribe_newsletter"].is_object());
        assert!(schema_json["properties"]["country"].is_object());
    }

    #[test]
    fn test_enum_serialization() {
        // Test that enums serialize correctly
        let account = AccountType::Professional;
        let json = serde_json::to_value(&account).unwrap();
        assert_eq!(json, "professional");

        // Test deserialization
        let parsed: AccountType = serde_json::from_value(json!("enterprise")).unwrap();
        assert!(matches!(parsed, AccountType::Enterprise));
    }

    #[test]
    fn test_optional_fields() {
        // Test with minimal required fields
        let user = json!({
            "email": "test@example.com",
            "name": "Test User"
        });

        let registration: UserRegistration = serde_json::from_value(user).unwrap();
        assert_eq!(registration.email, "test@example.com");
        assert_eq!(registration.name, "Test User");
        assert_eq!(registration.age, None);
        assert_eq!(registration.phone, None);
        assert_eq!(registration.subscribe_newsletter, false); // Default
        assert_eq!(registration.country, "US"); // Default
    }
}
