# The Anti-Pattern: 50 Confusing Tools

The most common mistake when building MCP servers is treating them like REST APIs. "We have 47 endpoints, so we'll create 47 tools." This approach fails spectacularly in the MCP environment.

## The API Conversion Trap

Consider a typical e-commerce API:

```
POST   /api/products              # Create product
GET    /api/products              # List products
GET    /api/products/{id}         # Get product
PUT    /api/products/{id}         # Update product
DELETE /api/products/{id}         # Delete product
POST   /api/products/{id}/images  # Add image
DELETE /api/products/{id}/images/{img_id}  # Remove image
GET    /api/products/{id}/reviews # Get reviews
POST   /api/products/{id}/reviews # Add review
PUT    /api/products/{id}/inventory # Update inventory
GET    /api/categories            # List categories
POST   /api/categories            # Create category
# ... 35 more endpoints
```

The naive approach converts each endpoint to a tool:

```rust
// DON'T DO THIS
let tools = vec![
    Tool::new("create_product"),
    Tool::new("list_products"),
    Tool::new("get_product"),
    Tool::new("update_product"),
    Tool::new("delete_product"),
    Tool::new("add_product_image"),
    Tool::new("remove_product_image"),
    Tool::new("get_product_reviews"),
    Tool::new("add_product_review"),
    Tool::new("update_inventory"),
    Tool::new("list_categories"),
    Tool::new("create_category"),
    // ... 35 more tools
];
```

This creates a nightmare for AI clients.

## Why This Fails

### Problem 1: Tool Selection Overload

When an AI sees 47 tools, it must evaluate each one against the user's request. The cognitive load increases non-linearly:

```
User: "Add a new laptop to the store"

AI must consider:
- create_product? (probably)
- add_product_image? (maybe needed after?)
- update_inventory? (should set initial stock?)
- list_categories? (need to find Electronics category first?)
- create_category? (if Electronics doesn't exist?)

With 47 tools, the AI might:
- Choose the wrong tool
- Call tools in a suboptimal order
- Miss required steps
- Get confused and ask for clarification
```

### Problem 2: Name Collisions

Your 47 tools don't exist in isolation. Other MCP servers connected to the same client may have similar names:

```
Your server:                  Asana server:            Google Drive server:
- create_product             - create_task            - create_document
- update_product             - update_task            - update_document
- delete_product             - delete_task            - delete_document
- list_products              - list_tasks             - list_documents
- get_product                - get_task               - get_document
```

A business user might have your e-commerce server connected alongside their project management (Asana, Notion) and document storage (Google Drive, SharePoint). The AI sees a sea of `create_*`, `update_*`, `delete_*`, `list_*`, `get_*` tools. Without excellent descriptions, it will make mistakes.

### Problem 3: Implicit Workflows Hidden

APIs encode workflows implicitly through endpoint sequences. MCP tools are independent—there's no built-in way to say "call A, then B, then C":

```
REST workflow (implicit in client code):
1. POST /api/products → get product_id
2. POST /api/products/{id}/images → attach image
3. PUT /api/products/{id}/inventory → set stock

MCP reality:
- AI sees 3 independent tools
- No indication they should be called together
- User must know to request all three steps
- Or AI must infer the workflow (unreliable)
```

### Problem 4: Description Burden

Each of your 47 tools needs a description good enough for an AI to understand when to use it. Most API endpoints don't have descriptions written for this purpose:

```rust
// Typical API-converted tool (inadequate)
Tool::new("update_inventory")
    .description("Updates inventory")  // Useless for AI decision-making

// What the AI actually needs
Tool::new("update_product_stock_level")
    .description(
        "Set the available quantity for a product in the inventory system. \
        Use this after creating a new product or when restocking. \
        Requires product_id and quantity. Quantity must be non-negative. \
        Returns the updated inventory record with last_modified timestamp."
    )
```

Writing 47 descriptions of this quality is significant work—and maintaining them as the API evolves is even harder.

## Real-World Consequences

### Case Study: The 73-Tool Disaster

A team converted their entire internal API to MCP tools: 73 tools covering user management, billing, reporting, and admin functions. Results:

- **AI accuracy dropped to 34%** for multi-step tasks
- **Response latency increased 5x** as the AI evaluated all 73 tools
- **Support tickets tripled** as users got unexpected results
- **Rollback within 2 weeks** to a 12-tool focused design

### Case Study: The Naming Collision

A database tool server used `query` as a tool name. When connected alongside a logging server (which also had `query`), the AI would randomly choose between them based on subtle description differences. Users reported "sometimes it queries the database, sometimes it searches logs, I can't predict which."

## The Better Approach: Purposeful Design

Instead of converting APIs to tools 1:1, design for how AI clients actually work:

### 1. Focus on User Tasks, Not API Operations

```rust
// Instead of 7 product CRUD tools, one task-focused tool:
Tool::new("manage_product_catalog")
    .description(
        "Create, update, or manage products in the catalog. \
        Handles product details, images, categories, and initial inventory. \
        Provide the operation type and relevant product data."
    )
    .input_schema(json!({
        "type": "object",
        "properties": {
            "operation": {
                "type": "string",
                "enum": ["create", "update", "add_image", "set_category", "discontinue"]
            },
            "product": {
                "type": "object",
                "properties": {
                    "id": { "type": "string" },
                    "name": { "type": "string" },
                    "description": { "type": "string" },
                    "price": { "type": "number" },
                    "category": { "type": "string" },
                    "initial_stock": { "type": "integer" }
                }
            }
        }
    }))
```

### 2. Use Prompts for Workflows

Instead of hoping the AI calls tools in the right order, define workflows as prompts:

```rust
Prompt::new("add-new-product")
    .description("Complete workflow to add a new product with images and inventory")
    .template(
        "I'll help you add a new product to the catalog. \
        This will:\n\
        1. Create the product with basic details\n\
        2. Upload any product images\n\
        3. Set initial inventory levels\n\
        4. Assign to appropriate categories\n\n\
        Please provide the product details..."
    )
```

### 3. Use Resources for Reference Data

Instead of `list_categories` and `get_category` tools, expose categories as resources:

```rust
Resource::new("catalog://categories")
    .description("All product categories with IDs and hierarchy")
    .mime_type("application/json")
```

The AI can read this resource to understand available categories without making a tool call.

## Summary: From API to MCP

| API Thinking | MCP Thinking |
|--------------|--------------|
| One endpoint = one tool | One user task = one tool |
| CRUD operations | High-level actions |
| Client controls workflow | Prompts guide workflow |
| Endpoints are independent | Tools designed for multi-server environment |
| Minimal descriptions | AI-decision-quality descriptions |
| 47 endpoints → 47 tools | 47 endpoints → 8-12 focused tools + prompts + resources |

The next section covers how to design tool sets that are cohesive—tools that work together naturally and are easily distinguished by AI clients.
