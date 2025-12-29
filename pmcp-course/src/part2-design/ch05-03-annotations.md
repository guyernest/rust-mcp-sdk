# Type-Safe Tool Annotations

MCP tool annotations provide metadata beyond schemas—hints about behavior, safety, and usage that help AI clients make better decisions. Combined with Rust's type system, annotations create a powerful safety net.

## What Are Tool Annotations?

Annotations are structured metadata attached to tools that describe characteristics the AI should consider:

```rust
Tool::new("db_modify")
    .description("Insert, update, or delete database records")
    .input_schema(/* ... */)
    .annotations(json!({
        "audience": ["developer", "admin"],
        "readOnlyHint": false,
        "destructiveHint": true,
        "idempotentHint": false,
        "openWorldHint": false
    }))
```

These annotations tell the AI:
- This tool is for developers and admins, not end users
- It modifies data (not read-only)
- It can be destructive (data loss possible)
- It's not idempotent (calling twice has different effects)
- It operates on a closed world (internal database)

## Standard MCP Annotations

The MCP specification defines several standard annotation hints:

### `readOnlyHint`

Indicates whether the tool only reads data or can modify state:

```rust
// Read-only tool - safe to call speculatively
Tool::new("sales_query")
    .annotations(json!({
        "readOnlyHint": true
    }))

// Modifying tool - AI should confirm before calling
Tool::new("order_update")
    .annotations(json!({
        "readOnlyHint": false
    }))
```

AI clients may call read-only tools more freely, while being cautious with modifying tools.

### `destructiveHint`

Indicates whether the operation can cause irreversible changes:

```rust
// Non-destructive: data can be recovered
Tool::new("archive_order")
    .description("Move order to archive (can be restored)")
    .annotations(json!({
        "destructiveHint": false
    }))

// Destructive: data is permanently lost
Tool::new("delete_customer")
    .description("Permanently delete customer and all associated data")
    .annotations(json!({
        "destructiveHint": true
    }))
```

Some AI clients will refuse to call destructive tools without explicit user confirmation.

### `idempotentHint`

Indicates whether calling the tool multiple times has the same effect as calling once:

```rust
// Idempotent: safe to retry
Tool::new("set_customer_status")
    .description("Set customer status to specified value")
    .annotations(json!({
        "idempotentHint": true
    }))

// Not idempotent: each call has cumulative effect
Tool::new("add_order_item")
    .description("Add item to order (quantity increases each call)")
    .annotations(json!({
        "idempotentHint": false
    }))
```

AI clients can safely retry idempotent operations on failure.

### `openWorldHint`

Indicates whether the tool interacts with external systems:

```rust
// Closed world: internal database only
Tool::new("internal_query")
    .annotations(json!({
        "openWorldHint": false
    }))

// Open world: calls external APIs
Tool::new("fetch_stock_price")
    .description("Fetch current stock price from market data API")
    .annotations(json!({
        "openWorldHint": true
    }))
```

Open world tools may have rate limits, costs, or unpredictable latency.

## Custom Annotations

Beyond standard hints, define custom annotations for your domain:

### Audience Annotations

```rust
Tool::new("admin_reset_password")
    .annotations(json!({
        "audience": ["admin"],
        "requiresRole": "security_admin",
        "auditLog": true
    }))

Tool::new("customer_view_orders")
    .annotations(json!({
        "audience": ["customer", "support"],
        "selfServiceAllowed": true
    }))
```

### Cost Annotations

```rust
Tool::new("generate_report")
    .annotations(json!({
        "computeCost": "high",
        "estimatedDuration": "10-30 seconds",
        "billingImpact": true
    }))

Tool::new("simple_lookup")
    .annotations(json!({
        "computeCost": "low",
        "estimatedDuration": "<100ms",
        "billingImpact": false
    }))
```

### Rate Limit Annotations

```rust
Tool::new("external_api_call")
    .annotations(json!({
        "rateLimit": {
            "requestsPerMinute": 60,
            "requestsPerHour": 1000
        },
        "retryable": true,
        "backoffStrategy": "exponential"
    }))
```

## Type-Safe Annotations in Rust

Use Rust's type system to ensure annotation consistency:

### Annotation Structs

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolAnnotations {
    /// Who should use this tool
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audience: Option<Vec<Audience>>,

    /// MCP standard hints
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_only_hint: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub destructive_hint: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub idempotent_hint: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub open_world_hint: Option<bool>,

    /// Custom annotations
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requires_role: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub compute_cost: Option<ComputeCost>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Audience {
    Developer,
    Admin,
    Support,
    Customer,
    System,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ComputeCost {
    Low,
    Medium,
    High,
}
```

### Annotation Builders

```rust
impl ToolAnnotations {
    pub fn new() -> Self {
        Self {
            audience: None,
            read_only_hint: None,
            destructive_hint: None,
            idempotent_hint: None,
            open_world_hint: None,
            requires_role: None,
            compute_cost: None,
        }
    }

    pub fn read_only(mut self) -> Self {
        self.read_only_hint = Some(true);
        self.destructive_hint = Some(false);
        self
    }

    pub fn modifying(mut self) -> Self {
        self.read_only_hint = Some(false);
        self
    }

    pub fn destructive(mut self) -> Self {
        self.read_only_hint = Some(false);
        self.destructive_hint = Some(true);
        self
    }

    pub fn idempotent(mut self) -> Self {
        self.idempotent_hint = Some(true);
        self
    }

    pub fn for_audience(mut self, audience: Vec<Audience>) -> Self {
        self.audience = Some(audience);
        self
    }

    pub fn requires_role(mut self, role: &str) -> Self {
        self.requires_role = Some(role.to_string());
        self
    }

    pub fn to_value(&self) -> serde_json::Value {
        serde_json::to_value(self).expect("Valid annotations")
    }
}

// Usage
let annotations = ToolAnnotations::new()
    .destructive()
    .for_audience(vec![Audience::Admin])
    .requires_role("security_admin")
    .to_value();
```

### Derive Macros for Tools

Create derive macros to generate schemas and annotations from Rust types:

```rust
use pmcp_sdk::derive::McpTool;

#[derive(McpTool)]
#[mcp(
    name = "customer_delete",
    description = "Permanently delete a customer",
    destructive,
    audience = "admin"
)]
pub struct DeleteCustomerParams {
    /// Customer ID to delete
    #[mcp(required)]
    pub customer_id: String,

    /// Reason for deletion (required for audit)
    #[mcp(required)]
    pub reason: String,

    /// Skip confirmation (dangerous)
    #[mcp(default = false)]
    pub force: bool,
}
```

This generates:
- Input schema from struct fields
- Annotations from `#[mcp(...)]` attributes
- Validation functions
- Tool registration code

## Runtime Validation with Annotations

Use annotations to drive runtime behavior:

```rust
pub struct ToolExecutor {
    tools: HashMap<String, RegisteredTool>,
}

impl ToolExecutor {
    pub async fn execute(
        &self,
        tool_name: &str,
        params: Value,
        context: &ExecutionContext,
    ) -> Result<Value> {
        let tool = self.tools.get(tool_name)
            .ok_or_else(|| Error::ToolNotFound(tool_name.to_string()))?;

        let annotations = &tool.annotations;

        // Check audience
        if let Some(audiences) = &annotations.audience {
            if !audiences.contains(&context.user_audience) {
                return Err(Error::AccessDenied(format!(
                    "Tool '{}' not available for audience '{:?}'",
                    tool_name, context.user_audience
                )));
            }
        }

        // Check role requirements
        if let Some(required_role) = &annotations.requires_role {
            if !context.user_roles.contains(required_role) {
                return Err(Error::AccessDenied(format!(
                    "Tool '{}' requires role '{}'",
                    tool_name, required_role
                )));
            }
        }

        // Confirm destructive operations
        if annotations.destructive_hint == Some(true) {
            if !context.destructive_confirmed {
                return Err(Error::ConfirmationRequired(format!(
                    "Tool '{}' is destructive. Set destructive_confirmed to proceed.",
                    tool_name
                )));
            }
        }

        // Execute the tool
        (tool.handler)(params, context).await
    }
}
```

## Annotation-Driven Documentation

Generate documentation from annotations:

```rust
pub fn generate_tool_docs(tools: &[RegisteredTool]) -> String {
    let mut doc = String::new();

    for tool in tools {
        doc.push_str(&format!("## {}\n\n", tool.name));
        doc.push_str(&format!("{}\n\n", tool.description));

        // Safety information from annotations
        if let Some(annotations) = &tool.annotations {
            doc.push_str("### Safety\n\n");

            if annotations.read_only_hint == Some(true) {
                doc.push_str("- ✅ Read-only (safe to call)\n");
            } else {
                doc.push_str("- ⚠️ Modifies data\n");
            }

            if annotations.destructive_hint == Some(true) {
                doc.push_str("- ❌ Destructive (irreversible)\n");
            }

            if annotations.idempotent_hint == Some(true) {
                doc.push_str("- 🔄 Idempotent (safe to retry)\n");
            }

            if let Some(audiences) = &annotations.audience {
                doc.push_str(&format!("- 👤 Audience: {:?}\n", audiences));
            }

            doc.push_str("\n");
        }
    }

    doc
}
```

## Combining Annotations and Schemas

Annotations complement schemas—schemas define structure, annotations define behavior:

```rust
Tool::new("bulk_update")
    .description("Update multiple records in a single operation")
    // Schema: what parameters are accepted
    .input_schema(json!({
        "type": "object",
        "required": ["table", "updates"],
        "properties": {
            "table": { "type": "string", "enum": ["customers", "orders"] },
            "updates": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "string" },
                        "changes": { "type": "object" }
                    }
                },
                "maxItems": 1000
            },
            "dry_run": { "type": "boolean", "default": false }
        }
    }))
    // Annotations: how the tool behaves
    .annotations(json!({
        "readOnlyHint": false,
        "destructiveHint": false,  // Updates are recoverable
        "idempotentHint": true,    // Same updates = same result
        "audience": ["developer", "admin"],
        "computeCost": "medium",
        "estimatedDuration": "1-10 seconds",
        "batchOperation": true,
        "maxBatchSize": 1000
    }))
```

## Summary

Tool annotations provide behavioral metadata that:

| Annotation | Purpose | AI Behavior |
|------------|---------|-------------|
| `readOnlyHint` | Read vs write | Controls speculation |
| `destructiveHint` | Irreversible changes | Requires confirmation |
| `idempotentHint` | Safe to retry | Retry on failure |
| `openWorldHint` | External systems | Expects latency/limits |
| `audience` | Who can use | Access control |
| Custom | Domain-specific | Your logic |

Combined with Rust's type system:
- Structs ensure annotation consistency
- Builders provide ergonomic construction
- Derive macros generate boilerplate
- Runtime checks enforce policies

Annotations transform tools from opaque functions into self-describing components that AI clients can reason about safely.
