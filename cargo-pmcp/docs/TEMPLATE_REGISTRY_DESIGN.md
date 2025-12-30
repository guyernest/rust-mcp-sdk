# Template Registry Design Specification

> **Status**: Design Draft
> **Version**: 0.1.0
> **Date**: December 2024

## Executive Summary

This document specifies the design for an extensible template registry system for cargo-pmcp. The system enables dynamic templates served via MCP server while maintaining compatibility with the existing embedded template approach.

**Core Philosophy**: Templates should help developers **design focused MCP servers**, not automatically mirror source schemas. A good MCP server exposes a curated set of tools that solve specific problems—not a 1:1 mapping of every API endpoint.

## Table of Contents

1. [Design Principles](#design-principles)
2. [Template Categories](#template-categories)
3. [Manifest Specification](#manifest-specification)
4. [Generation Modes](#generation-modes)
5. [Types-First Workflow](#types-first-workflow)
6. [Template File Structure](#template-file-structure)
7. [Integration with cargo-pmcp](#integration-with-cargo-pmcp)
8. [MCP Server Interface](#mcp-server-interface)
9. [Community Contribution Model](#community-contribution-model)
10. [Migration Path](#migration-path)

---

## Design Principles

### 1. Design-First, Not Schema-First

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    Design-First Philosophy                              │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  ❌ ANTI-PATTERN: Automatic Schema Mirroring                           │
│  ════════════════════════════════════════════                          │
│                                                                         │
│  Swagger API (50 endpoints)  ──automatic──▶  MCP Server (50 tools)     │
│                                                                         │
│  Problems:                                                              │
│  • Overwhelming for AI clients (too many choices)                      │
│  • No cohesion or purpose                                              │
│  • Exposes internal API structure                                      │
│  • Maintenance nightmare                                               │
│  • Poor user experience                                                │
│                                                                         │
│  ✅ PATTERN: Designed Application                                       │
│  ════════════════════════════════                                       │
│                                                                         │
│  Swagger API (50 endpoints)                                            │
│         │                                                              │
│         ▼                                                              │
│  Developer picks 5-10 operations                                       │
│         │                                                              │
│         ▼                                                              │
│  Designs MCP server with:                                              │
│  • 5 focused tools (user-centric naming)                               │
│  • 2 workflow prompts (common use cases)                               │
│  • 1 resource (documentation/context)                                  │
│                                                                         │
│  Benefits:                                                              │
│  • Clear purpose and cohesion                                          │
│  • AI can understand and use effectively                               │
│  • Maintainable and testable                                           │
│  • Good user experience                                                │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### 2. Types and Tools Are Separate Concerns

Templates should support generating:

1. **Types only** - Rust structs from schemas (no business logic)
2. **Tool scaffolds** - Empty handlers using generated types
3. **Complete implementations** - Only for simple, well-defined cases

This separation allows developers to:
- Generate types once, use them in multiple tools
- Compose types from different sources
- Implement business logic manually with type safety

### 3. Progressive Disclosure

Templates should support different experience levels:

| Level | User | Template Provides |
|-------|------|-------------------|
| **Beginner** | Learning MCP | Complete working examples (calculator) |
| **Intermediate** | Building real servers | Type generation + scaffolds |
| **Advanced** | Custom architectures | Types only, manual composition |

### 4. Compatibility with Existing System

New templates must work alongside the current embedded templates:

```rust
// Current (continues to work)
cargo pmcp add server calc --template calculator

// New (template registry)
cargo pmcp add server petstore --template swagger:petstore-api

// New (types only)
cargo pmcp generate types --from swagger --source ./api.yaml --output ./src/types/
```

---

## Template Categories

### Core Categories

```
templates/
├── educational/           # Learning MCP (existing templates)
│   ├── calculator/
│   ├── complete-calculator/
│   └── sqlite-explorer/
│
├── types/                 # Type generation only (no tools)
│   ├── swagger/          # Generate Rust types from OpenAPI
│   ├── graphql/          # Generate Rust types from GraphQL
│   ├── database/         # Generate Rust types from DB schema
│   ├── protobuf/         # Generate Rust types from .proto
│   └── json-schema/      # Generate Rust types from JSON Schema
│
├── scaffolds/            # Tool scaffolds using types
│   ├── crud/             # CRUD operations scaffold
│   ├── search/           # Search/filter scaffold
│   ├── workflow/         # Multi-step workflow scaffold
│   └── aggregator/       # Multi-source aggregation scaffold
│
├── deployment/           # Deployment configurations
│   ├── aws-lambda/
│   ├── cloudflare-workers/
│   ├── google-cloud-run/
│   ├── fly-io/
│   ├── railway/
│   └── docker/
│
├── auth/                 # Authentication providers
│   ├── cognito/
│   ├── auth0/
│   ├── okta/
│   ├── keycloak/
│   └── entra-id/
│
└── composition/          # Server composition patterns
    ├── gateway/          # API gateway pattern
    ├── aggregator/       # Multi-server aggregation
    └── transform/        # Response transformation
```

### Category Metadata

Each category has a `category.toml`:

```toml
[category]
id = "types"
name = "Type Generators"
description = "Generate Rust types from external schemas without tool implementations"
icon = "📦"

[category.guidance]
when_to_use = """
Use type generators when:
- You want to start with type safety before designing tools
- You're composing types from multiple sources
- You want to manually implement tool logic
"""

when_not_to_use = """
Don't use if:
- You're learning MCP (use educational templates instead)
- You want a complete working server immediately
"""

[category.workflow]
typical_steps = [
    "1. Generate types from your schema",
    "2. Review generated types, customize if needed",
    "3. Design your tools (pick 5-10 operations)",
    "4. Use scaffold template for tool structure",
    "5. Implement business logic"
]
```

---

## Manifest Specification

### Manifest Schema (v1.0)

```toml
# manifest.toml - Template manifest specification

[template]
# Required metadata
name = "swagger-types"
version = "1.0.0"
category = "types"
description = "Generate Rust types from Swagger/OpenAPI specifications"

# Extended metadata
long_description = """
Generates type-safe Rust structs from Swagger/OpenAPI schemas.
Does NOT generate tool implementations - this template focuses on
creating a solid type foundation for your MCP server design.
"""
author = "PMCP Team"
license = "MIT"
repository = "https://github.com/paiml/pmcp-templates"
keywords = ["swagger", "openapi", "types", "codegen"]

# What this template produces
[template.output]
type = "types"  # "types" | "scaffold" | "server" | "config"
description = "Rust type definitions only"

# Compatibility requirements
[template.compatibility]
pmcp_min_version = "0.5.0"
rust_edition = "2021"
cargo_pmcp_min_version = "0.3.0"

# ============================================================================
# INPUT SPECIFICATION
# ============================================================================

[inputs]

# Schema source - the external schema to process
[inputs.source]
type = "string"
required = true
description = "URL or file path to Swagger/OpenAPI JSON/YAML"
examples = [
    "https://petstore.swagger.io/v2/swagger.json",
    "./api/openapi.yaml"
]
validation = "url_or_file"

# Output module name
[inputs.module_name]
type = "string"
required = true
description = "Name for the generated Rust module"
pattern = "^[a-z][a-z0-9_]*$"
default = "api_types"
examples = ["petstore_types", "github_types"]

# Schema selection - CRITICAL for avoiding schema explosion
[inputs.schemas]
type = "array"
items = "string"
required = false
description = """
Specific schema names to include. If empty, generates ALL schemas.
RECOMMENDED: Explicitly list the schemas you need to avoid bloat.
"""
default = []
examples = [
    ["Pet", "Category", "Tag"],
    ["User", "Repository", "Issue"]
]

# Operation selection - for extracting types from operations
[inputs.operations]
type = "array"
items = "string"
required = false
description = """
Generate input/output types for specific operation IDs only.
Alternative to schema selection - useful when you know which
API calls you'll use but not the underlying schema names.
"""
default = []
examples = [
    ["getPetById", "addPet", "findPetsByStatus"],
    ["getUser", "listRepositories"]
]

# Customization options
[inputs.options]
type = "object"
required = false
description = "Generation options"

[inputs.options.fields.derive_traits]
type = "array"
items = "string"
default = ["Debug", "Clone", "Serialize", "Deserialize", "JsonSchema"]
description = "Traits to derive on generated types"

[inputs.options.fields.serde_rename]
type = "enum"
values = ["camelCase", "snake_case", "PascalCase", "none"]
default = "camelCase"
description = "Serde rename strategy for fields"

[inputs.options.fields.optional_nullable]
type = "boolean"
default = true
description = "Treat nullable fields as Option<T>"

[inputs.options.fields.validation]
type = "boolean"
default = true
description = "Generate validator attributes for constraints"

# ============================================================================
# OUTPUT SPECIFICATION
# ============================================================================

[outputs]
description = "Generated type definitions"

[[outputs.files]]
path = "src/{module_name}/mod.rs"
description = "Module root with re-exports"

[[outputs.files]]
path = "src/{module_name}/types.rs"
description = "Generated type definitions"

[[outputs.files]]
path = "src/{module_name}/enums.rs"
condition = "has_enums"
description = "Enum definitions (if schema contains enums)"

# ============================================================================
# GENERATION CONFIGURATION
# ============================================================================

[generation]
# How this template generates code
type = "rust"  # "static" | "tera" | "rust"

# For rust generators
entry = "src/lib.rs"
function = "generate"

# Pre-generation hooks
[generation.hooks]
pre_generate = "validate_schema"
post_generate = "format_output"

# ============================================================================
# DESIGN GUIDANCE (shown to user)
# ============================================================================

[guidance]
# Shown before generation
pre_generation = """
## Before You Generate

This template generates Rust types from your Swagger/OpenAPI schema.
It does NOT generate MCP tools - that's intentional.

### Recommended Workflow

1. **Analyze your API**: What operations will your MCP server expose?
2. **Select schemas**: Only generate types you'll actually use
3. **Generate types**: Run this template with your selections
4. **Design tools**: Decide on tool names, groupings, prompts
5. **Implement**: Use scaffold templates or manual implementation

### Avoid This Anti-Pattern

❌ Generating types for ALL 50 schemas then creating 50 tools
✅ Selecting 5-10 schemas for 5-10 focused tools
"""

# Shown after generation
post_generation = """
## Next Steps

Types generated successfully! Now design your MCP server:

1. **Review generated types** in `src/{module_name}/`
2. **Design your tools** - which operations solve user problems?
3. **Create tool scaffolds**: `cargo pmcp add tool <name> --types {module_name}`
4. **Implement logic**: Fill in the handler functions
5. **Add prompts**: Create workflow prompts for common use cases

### Design Questions to Consider

- What problems will users solve with this server?
- Which 5-10 operations are most valuable?
- Can some operations be combined into higher-level tools?
- What prompts would guide users through common workflows?
"""

# ============================================================================
# EXAMPLES
# ============================================================================

[examples]

[[examples.focused]]
name = "Focused Type Generation"
description = "Generate only the types you need"
inputs = {
    source = "https://petstore.swagger.io/v2/swagger.json",
    module_name = "petstore",
    schemas = ["Pet", "Category", "Tag"],
    operations = []
}

[[examples.operation_based]]
name = "Operation-Based Selection"
description = "Generate types for specific operations"
inputs = {
    source = "https://api.github.com/swagger.json",
    module_name = "github",
    schemas = [],
    operations = ["getRepository", "listIssues", "createIssue"]
}
```

---

## Generation Modes

### Mode 1: Types Only

Generates Rust struct definitions without any tool logic.

```bash
cargo pmcp generate types \
    --from swagger \
    --source https://petstore.swagger.io/v2/swagger.json \
    --module petstore \
    --schemas Pet,Category,Tag
```

**Output**: `src/petstore/types.rs`

```rust
//! Generated from Swagger: Petstore API v1.0.0
//! Selected schemas: Pet, Category, Tag
//!
//! IMPORTANT: These are type definitions only. You must implement
//! tool handlers separately. See: cargo pmcp add tool --help

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use validator::Validate;

/// A pet in the store
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct Pet {
    /// Unique identifier
    #[schemars(description = "Unique identifier for the pet")]
    pub id: Option<i64>,

    /// Pet's name
    #[validate(length(min = 1, max = 100))]
    #[schemars(description = "The pet's name")]
    pub name: String,

    /// Category this pet belongs to
    pub category: Option<Category>,

    /// Tags associated with this pet
    #[serde(default)]
    pub tags: Vec<Tag>,

    /// Pet's availability status
    #[schemars(description = "Pet status in the store")]
    pub status: Option<PetStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum PetStatus {
    Available,
    Pending,
    Sold,
}

// ... Category, Tag definitions
```

### Mode 2: Tool Scaffold with Client Code

Generates tool handlers with **working client code** for the target system. The developer focuses on designing which operations to expose and adding business logic—the plumbing is already done.

```bash
cargo pmcp add tool get-pet \
    --server petstore \
    --from swagger \
    --operation getPetById \
    --output-type petstore::Pet
```

#### REST API Scaffold (reqwest)

**Output**: `src/tools/get_pet.rs`

```rust
//! Tool: get-pet
//! Generated from: Petstore API - getPetById operation
//!
//! The HTTP client code is generated. Customize the tool interface
//! and add any business logic transformations you need.

use crate::types::petstore::Pet;
use crate::client::PetstoreClient;
use pmcp::{Error, RequestHandlerExtra, Result, TypedToolWithOutput};
use schemars::JsonSchema;
use serde::Deserialize;

/// Input for get-pet tool
/// CUSTOMIZE: Adjust fields to match your desired tool interface
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct GetPetInput {
    /// The ID of the pet to retrieve
    #[schemars(description = "Unique identifier of the pet")]
    pub pet_id: i64,
}

async fn handler(
    input: GetPetInput,
    _extra: RequestHandlerExtra,
) -> Result<Pet> {
    // Client code is generated - calls the REST API
    let client = PetstoreClient::from_env()?;

    let pet = client
        .get_pet_by_id(input.pet_id)
        .await
        .map_err(|e| match e {
            ClientError::NotFound => Error::validation(format!("Pet {} not found", input.pet_id)),
            ClientError::Unauthorized => Error::internal("API authentication failed"),
            ClientError::RateLimited => Error::internal("API rate limit exceeded, try again later"),
            e => Error::internal(format!("Failed to fetch pet: {}", e)),
        })?;

    // ADD YOUR BUSINESS LOGIC HERE
    // Example: filter sensitive fields, transform data, combine with other sources

    Ok(pet)
}

pub fn build_tool() -> TypedToolWithOutput<GetPetInput, Pet> {
    TypedToolWithOutput::new("get-pet", |input, extra| {
        Box::pin(handler(input, extra))
    })
    .with_description("Retrieve a pet by its ID")
}
```

**Also generated**: `src/client/mod.rs`

```rust
//! Generated REST client for Petstore API
//! Base URL configured via PETSTORE_API_URL environment variable
//! API key via PETSTORE_API_KEY (if required)

use reqwest::{Client, StatusCode};
use crate::types::petstore::*;

pub struct PetstoreClient {
    client: Client,
    base_url: String,
    api_key: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("Resource not found")]
    NotFound,
    #[error("Authentication failed")]
    Unauthorized,
    #[error("Rate limit exceeded")]
    RateLimited,
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Invalid response: {0}")]
    InvalidResponse(String),
}

impl PetstoreClient {
    pub fn from_env() -> Result<Self, ClientError> {
        let base_url = std::env::var("PETSTORE_API_URL")
            .unwrap_or_else(|_| "https://petstore.swagger.io/v2".to_string());
        let api_key = std::env::var("PETSTORE_API_KEY").ok();

        Ok(Self {
            client: Client::new(),
            base_url,
            api_key,
        })
    }

    /// GET /pet/{petId} - Find pet by ID
    pub async fn get_pet_by_id(&self, pet_id: i64) -> Result<Pet, ClientError> {
        let url = format!("{}/pet/{}", self.base_url, pet_id);

        let mut request = self.client.get(&url);
        if let Some(ref key) = self.api_key {
            request = request.header("api_key", key);
        }

        let response = request.send().await?;

        match response.status() {
            StatusCode::OK => Ok(response.json().await?),
            StatusCode::NOT_FOUND => Err(ClientError::NotFound),
            StatusCode::UNAUTHORIZED => Err(ClientError::Unauthorized),
            StatusCode::TOO_MANY_REQUESTS => Err(ClientError::RateLimited),
            status => Err(ClientError::InvalidResponse(
                format!("Unexpected status: {}", status)
            )),
        }
    }

    // Other operations you selected are generated here...
}
```

#### SQL Database Scaffold (sqlx)

```bash
cargo pmcp add tool list-users \
    --server users-db \
    --from db-schema \
    --table users \
    --operation select
```

**Output**: `src/tools/list_users.rs`

```rust
//! Tool: list-users
//! Generated from: users table schema

use crate::types::users::User;
use crate::db::DbPool;
use pmcp::{Error, RequestHandlerExtra, Result, TypedToolWithOutput};
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct ListUsersInput {
    /// Filter by status (optional)
    #[schemars(description = "Filter users by status")]
    pub status: Option<String>,

    /// Maximum results to return
    #[schemars(description = "Limit results (1-100)", default = 20)]
    pub limit: Option<i32>,
}

async fn handler(
    input: ListUsersInput,
    _extra: RequestHandlerExtra,
) -> Result<Vec<User>> {
    let pool = DbPool::from_env()
        .map_err(|e| Error::internal(format!("Database connection failed: {}", e)))?;

    let limit = input.limit.unwrap_or(20).min(100);

    let users = match input.status {
        Some(status) => {
            sqlx::query_as!(
                User,
                r#"SELECT id, email, name, status, created_at
                   FROM users
                   WHERE status = $1
                   ORDER BY created_at DESC
                   LIMIT $2"#,
                status,
                limit as i64
            )
            .fetch_all(&pool)
            .await
        }
        None => {
            sqlx::query_as!(
                User,
                r#"SELECT id, email, name, status, created_at
                   FROM users
                   ORDER BY created_at DESC
                   LIMIT $1"#,
                limit as i64
            )
            .fetch_all(&pool)
            .await
        }
    }
    .map_err(|e| Error::internal(format!("Query failed: {}", e)))?;

    // ADD YOUR BUSINESS LOGIC HERE
    // Example: filter sensitive fields, apply access control

    Ok(users)
}
```

**Also generated**: `src/db/mod.rs`

```rust
//! Database connection pool
//! Configure via DATABASE_URL environment variable

use sqlx::postgres::PgPoolOptions;
pub type DbPool = sqlx::PgPool;

pub async fn create_pool() -> Result<DbPool, sqlx::Error> {
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");

    PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
}
```

#### GraphQL Scaffold (graphql-client)

```bash
cargo pmcp add tool get-repository \
    --server github \
    --from graphql \
    --operation GetRepository
```

**Output**: `src/tools/get_repository.rs`

```rust
//! Tool: get-repository
//! Generated from: GitHub GraphQL API - repository query

use crate::graphql::{GitHubClient, get_repository};
use pmcp::{Error, RequestHandlerExtra, Result, TypedToolWithOutput};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct GetRepositoryInput {
    /// Repository owner (user or organization)
    pub owner: String,
    /// Repository name
    pub name: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct RepositoryOutput {
    pub name: String,
    pub description: Option<String>,
    pub star_count: i32,
    pub fork_count: i32,
    pub is_private: bool,
    pub default_branch: String,
}

async fn handler(
    input: GetRepositoryInput,
    _extra: RequestHandlerExtra,
) -> Result<RepositoryOutput> {
    let client = GitHubClient::from_env()?;

    let variables = get_repository::Variables {
        owner: input.owner.clone(),
        name: input.name.clone(),
    };

    let response = client
        .query::<get_repository::GetRepository>(variables)
        .await
        .map_err(|e| Error::internal(format!("GraphQL query failed: {}", e)))?;

    let repo = response.repository
        .ok_or_else(|| Error::validation(format!(
            "Repository {}/{} not found", input.owner, input.name
        )))?;

    // Transform GraphQL response to our output type
    Ok(RepositoryOutput {
        name: repo.name,
        description: repo.description,
        star_count: repo.stargazer_count,
        fork_count: repo.fork_count,
        is_private: repo.is_private,
        default_branch: repo.default_branch_ref
            .map(|b| b.name)
            .unwrap_or_else(|| "main".to_string()),
    })
}
```

#### Scaffold Summary

The scaffold mode generates:

| Component | REST (reqwest) | SQL (sqlx) | GraphQL |
|-----------|---------------|------------|---------|
| **Types** | From OpenAPI schemas | From table schema | From GraphQL schema |
| **Client** | HTTP client with auth | Connection pool | GraphQL client |
| **Error mapping** | HTTP status → MCP errors | DB errors → MCP errors | GraphQL errors → MCP |
| **Tool handler** | Working implementation | Working queries | Working queries |
| **What you add** | Business logic, field filtering | Access control, transforms | Response shaping |

**Developer focuses on**:
1. Which operations to expose as tools
2. How to name and describe them for AI clients
3. Business logic transformations
4. Access control and validation beyond schema

### Mode 3: Design Assistant

Interactive mode that helps developers design their MCP server:

```bash
cargo pmcp design --from swagger --source ./api.yaml
```

**Interactive Flow**:

```
╔══════════════════════════════════════════════════════════════════════════╗
║                    MCP Server Design Assistant                           ║
╠══════════════════════════════════════════════════════════════════════════╣
║                                                                          ║
║  Analyzing: ./api.yaml                                                   ║
║  Found: 47 operations, 23 schemas                                        ║
║                                                                          ║
║  ⚠️  RECOMMENDATION: Don't expose all 47 operations as tools.            ║
║     A focused MCP server with 5-10 tools is more usable.                ║
║                                                                          ║
╠══════════════════════════════════════════════════════════════════════════╣
║                                                                          ║
║  What problem will this MCP server solve?                                ║
║  > Help developers manage GitHub issues and PRs                          ║
║                                                                          ║
║  Based on your goal, these operations seem most relevant:                ║
║                                                                          ║
║  Issues:                              Pull Requests:                     ║
║  [x] listIssues                       [x] listPullRequests              ║
║  [x] getIssue                         [x] getPullRequest                ║
║  [x] createIssue                      [ ] createPullRequest             ║
║  [x] updateIssue                      [ ] mergePullRequest              ║
║  [ ] deleteIssue                      [ ] listPRReviews                 ║
║                                                                          ║
║  Selected: 6 operations (recommended range: 5-10)                       ║
║                                                                          ║
║  Would you like to:                                                      ║
║  1. Generate types for selected operations                               ║
║  2. Modify selection                                                     ║
║  3. See suggested tool names and descriptions                            ║
║  4. Generate complete server scaffold                                    ║
║                                                                          ║
╚══════════════════════════════════════════════════════════════════════════╝
```

---

## Types-First Workflow

### Recommended Development Flow

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    Types-First Development Workflow                     │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  Step 1: ANALYZE                                                        │
│  ════════════════                                                       │
│                                                                         │
│  $ cargo pmcp analyze swagger ./api.yaml                                │
│                                                                         │
│  Output:                                                                │
│  • 47 operations found                                                  │
│  • 23 schema definitions                                                │
│  • Suggested groupings: Users (5), Orders (8), Products (12)...        │
│                                                                         │
│  Step 2: DESIGN                                                         │
│  ══════════════                                                         │
│                                                                         │
│  Developer decides:                                                     │
│  • "I need a product search MCP server"                                │
│  • "Users only need: search, get details, check availability"          │
│  • "I'll combine 'list' + 'filter' into one 'search' tool"            │
│                                                                         │
│  Step 3: GENERATE TYPES                                                 │
│  ══════════════════════                                                 │
│                                                                         │
│  $ cargo pmcp generate types \                                          │
│      --from swagger \                                                   │
│      --source ./api.yaml \                                              │
│      --module products \                                                │
│      --schemas Product,Category,Inventory                               │
│                                                                         │
│  Created: src/types/products/                                           │
│           ├── mod.rs                                                    │
│           ├── types.rs      (Product, Category, Inventory)              │
│           └── enums.rs      (ProductStatus, ...)                        │
│                                                                         │
│  Step 4: CREATE TOOL SCAFFOLDS                                          │
│  ═════════════════════════════                                          │
│                                                                         │
│  $ cargo pmcp add tool search-products --server products                │
│  $ cargo pmcp add tool get-product --server products                    │
│  $ cargo pmcp add tool check-availability --server products             │
│                                                                         │
│  Step 5: IMPLEMENT HANDLERS                                             │
│  ══════════════════════════                                             │
│                                                                         │
│  Developer implements business logic in each tool handler,              │
│  using the generated types for input/output.                            │
│                                                                         │
│  Step 6: ADD PROMPTS                                                    │
│  ════════════════════                                                   │
│                                                                         │
│  $ cargo pmcp add prompt find-product-workflow --server products        │
│                                                                         │
│  Creates a prompt that guides users through:                            │
│  1. Search for products                                                 │
│  2. Get details on interesting ones                                     │
│  3. Check availability                                                  │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### Types Crate Structure

For larger projects, types can be a separate crate:

```
workspace/
├── crates/
│   ├── types-petstore/        # Generated types (can be shared)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── pet.rs
│   │       ├── order.rs
│   │       └── user.rs
│   │
│   ├── mcp-petstore-core/     # MCP server using types
│   │   ├── Cargo.toml         # depends on types-petstore
│   │   └── src/
│   │       ├── lib.rs
│   │       └── tools/
│   │           ├── get_pet.rs    # uses types_petstore::Pet
│   │           └── search.rs
│   │
│   └── petstore-server/
│       └── ...
```

This allows:
- Types regenerated without affecting tool logic
- Multiple servers sharing same types
- Clear separation of concerns

---

## Template File Structure

### Directory Layout

```
pmcp-templates/                    # Template repository
├── README.md
├── CONTRIBUTING.md
├── manifest-schema.json           # JSON Schema for manifest validation
│
├── categories/
│   ├── types.toml
│   ├── scaffolds.toml
│   ├── deployment.toml
│   └── auth.toml
│
├── templates/
│   ├── types/
│   │   ├── swagger/
│   │   │   ├── manifest.toml      # Template metadata
│   │   │   ├── README.md          # Documentation
│   │   │   ├── DESIGN.md          # Design decisions
│   │   │   ├── examples/
│   │   │   │   ├── petstore/
│   │   │   │   │   ├── input.toml
│   │   │   │   │   └── expected/
│   │   │   │   └── github/
│   │   │   ├── files/             # Tera templates
│   │   │   │   ├── mod.rs.tera
│   │   │   │   ├── types.rs.tera
│   │   │   │   └── enums.rs.tera
│   │   │   └── src/               # Rust generator (for complex logic)
│   │   │       ├── lib.rs
│   │   │       ├── parser.rs
│   │   │       └── codegen.rs
│   │   │
│   │   ├── graphql/
│   │   └── database/
│   │
│   ├── scaffolds/
│   │   ├── crud/
│   │   ├── search/
│   │   └── workflow/
│   │
│   └── deployment/
│       ├── aws-lambda/
│       └── fly-io/
│
└── steering/                      # AI assistant guidance
    ├── mcp-developer.md           # Full developer guidance
    ├── workflow.md                # cargo-pmcp workflow
    └── patterns/
        ├── typed-tool.md
        └── error-handling.md
```

### Template Types

#### Static Templates (Tera)

For simple variable substitution:

```
files/
├── Cargo.toml.tera
├── lib.rs.tera
└── types.rs.tera
```

`types.rs.tera`:
```rust
//! Generated types for {{ module_name }}
//! Source: {{ source }}
//! Generated: {{ timestamp }}

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
{% if options.validation %}
use validator::Validate;
{% endif %}

{% for type in types %}
/// {{ type.description }}
#[derive({{ options.derive_traits | join(", ") }})]
{% if options.serde_rename != "none" %}
#[serde(rename_all = "{{ options.serde_rename }}")]
{% endif %}
{% if options.validation %}
#[derive(Validate)]
{% endif %}
pub struct {{ type.name }} {
{% for field in type.fields %}
    {% if field.description %}
    /// {{ field.description }}
    {% endif %}
    {% if field.validation %}
    #[validate({{ field.validation }})]
    {% endif %}
    pub {{ field.name }}: {{ field.type }},
{% endfor %}
}

{% endfor %}
```

#### Rust Generators

For complex transformations (parsing Swagger, GraphQL, etc.):

```rust
// src/lib.rs
use pmcp_template_sdk::{GeneratorContext, GeneratorResult};

pub fn generate(ctx: &GeneratorContext) -> GeneratorResult {
    // Parse source schema
    let spec = parse_openapi(&ctx.input_string("source")?)?;

    // Filter to selected schemas/operations
    let selected = filter_selections(&spec, &ctx)?;

    // Generate Rust types
    let types = generate_rust_types(&selected, &ctx.input_object("options")?)?;

    // Return generated files
    Ok(GeneratorResult {
        files: vec![
            GeneratedFile::new("src/types.rs", types),
            GeneratedFile::new("src/mod.rs", generate_mod_rs(&selected)?),
        ],
        next_steps: vec![
            "Review generated types".into(),
            "Create tool scaffolds: cargo pmcp add tool <name>".into(),
        ],
        warnings: collect_warnings(&spec, &selected),
    })
}
```

---

## Integration with cargo-pmcp

### New Commands

```bash
# Analyze a schema source
cargo pmcp analyze swagger ./api.yaml
cargo pmcp analyze graphql https://api.example.com/graphql
cargo pmcp analyze database postgres://localhost/mydb

# Generate types only
cargo pmcp generate types --from swagger --source ./api.yaml [options]

# Interactive design assistant
cargo pmcp design --from swagger --source ./api.yaml

# List available templates
cargo pmcp templates list
cargo pmcp templates list --category types
cargo pmcp templates search swagger

# Get template info
cargo pmcp templates info swagger-types

# Use template from registry
cargo pmcp add server myapi --template registry:swagger-types --source ./api.yaml
```

### Configuration

`.pmcp/templates.toml`:

```toml
[registry]
# Primary template registry
url = "https://templates.pmcp.run"

# Fallback registries
fallback = [
    "https://github.com/paiml/pmcp-templates/releases/latest"
]

# Cache settings
cache_dir = ".pmcp/template-cache"
cache_ttl = "24h"

[defaults]
# Default options for type generation
[defaults.types]
derive_traits = ["Debug", "Clone", "Serialize", "Deserialize", "JsonSchema"]
validation = true
serde_rename = "camelCase"

[defaults.scaffolds]
include_tests = true
error_handling = "pmcp"
```

### Backward Compatibility

Existing commands continue to work:

```bash
# These still work exactly as before
cargo pmcp new my-workspace
cargo pmcp add server calc --template calculator
cargo pmcp add server db --template sqlite-explorer
```

The embedded templates (`calculator`, `minimal`, `complete-calculator`, `sqlite-explorer`) remain available and are not affected by the registry system.

---

## MCP Server Interface

### Template Registry as MCP Server

The template registry can be exposed as an MCP server for AI-assisted development:

```
┌─────────────────────────────────────────────────────────────────────────┐
│              Template Registry MCP Server                               │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  DISCOVERY TOOLS                                                        │
│                                                                         │
│  list_template_categories                                              │
│  list_templates(category?, search?)                                    │
│  get_template_info(name, version?)                                     │
│                                                                         │
│  ANALYSIS TOOLS                                                         │
│                                                                         │
│  analyze_swagger(source)                                               │
│    → Returns: operations, schemas, suggested_groupings                 │
│    → Includes design recommendations                                   │
│                                                                         │
│  analyze_graphql(source)                                               │
│  analyze_database(ddl | connection_url)                                │
│                                                                         │
│  DESIGN TOOLS                                                           │
│                                                                         │
│  suggest_tool_design(source, goal_description)                         │
│    → Returns: recommended operations, tool names, groupings            │
│    → Warns against anti-patterns (too many tools, etc.)               │
│                                                                         │
│  validate_design(operations, tool_names)                               │
│    → Returns: validation result, suggestions                           │
│                                                                         │
│  GENERATION TOOLS                                                       │
│                                                                         │
│  preview_generation(template, inputs)                                  │
│    → Returns: list of files that would be generated                    │
│                                                                         │
│  generate_types(source, selections, options)                           │
│    → Returns: generated Rust type definitions                          │
│    → Does NOT generate tools (by design)                               │
│                                                                         │
│  generate_scaffold(tool_name, input_type, output_type)                 │
│    → Returns: tool scaffold with empty handler                         │
│                                                                         │
│  RESOURCES                                                              │
│                                                                         │
│  resource://templates/{name}/readme                                     │
│  resource://templates/{name}/examples/{example}                         │
│  resource://pmcp/steering/mcp-developer                                 │
│  resource://pmcp/patterns/{pattern}                                     │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### Design-Centric Tools

The MCP server emphasizes design over automation:

```json
// suggest_tool_design input
{
  "source": "https://petstore.swagger.io/v2/swagger.json",
  "goal": "Help users manage their pet store inventory"
}

// Response includes design guidance
{
  "recommended_tools": [
    {
      "name": "search-pets",
      "description": "Search pets by status, category, or tags",
      "combines_operations": ["findPetsByStatus", "findPetsByTags"],
      "rationale": "Single search tool is more intuitive than multiple filter tools"
    },
    {
      "name": "get-pet",
      "description": "Get detailed information about a specific pet",
      "uses_operation": "getPetById"
    },
    {
      "name": "update-inventory",
      "description": "Update pet status (available/pending/sold)",
      "uses_operation": "updatePet",
      "note": "Consider limiting to status updates only for safety"
    }
  ],
  "suggested_prompts": [
    {
      "name": "inventory-check",
      "description": "Guide user through checking and updating inventory",
      "workflow": ["search-pets(status: available)", "review results", "update-inventory if needed"]
    }
  ],
  "warnings": [
    "Excluding 'deletePet' - destructive operations need careful consideration",
    "Excluding 'uploadImage' - file uploads add complexity, consider for v2"
  ],
  "anti_patterns_avoided": [
    "Not generating all 20 operations as separate tools",
    "Combining related filter operations into single search"
  ]
}
```

---

## Community Contribution Model

### Contributing New Templates

1. **Fork** the pmcp-templates repository
2. **Create** template in appropriate category
3. **Include**:
   - `manifest.toml` with full metadata
   - `README.md` with usage documentation
   - `examples/` with at least one example
   - Tests that verify generation
4. **Submit** PR with description of use case

### Template Quality Requirements

- [ ] Manifest validates against schema
- [ ] At least one working example
- [ ] Generation produces valid Rust code
- [ ] Generated code passes `cargo fmt` and `cargo clippy`
- [ ] Documentation explains when to use (and when not to)
- [ ] No automatic generation of >10 tools without explicit selection
- [ ] Includes design guidance for users

### Versioning

Templates follow semver:
- **Major**: Breaking changes to manifest or output format
- **Minor**: New features, new optional inputs
- **Patch**: Bug fixes, documentation updates

---

## Migration Path

### Phase 1: Foundation (Current)

- Existing embedded templates continue to work
- Document template manifest specification
- Build template SDK for contributors

### Phase 2: Registry Infrastructure

- Deploy template registry MCP server
- Add `cargo pmcp templates` commands
- Port existing templates to manifest format (as examples)

### Phase 3: Type Generators

- Implement swagger-types template
- Implement graphql-types template
- Add `cargo pmcp generate types` command

### Phase 4: Design Tools

- Implement `cargo pmcp analyze` command
- Implement `cargo pmcp design` interactive mode
- Add design-centric MCP tools

### Phase 5: Community

- Open template contributions
- Add deployment templates (fly-io, railway, etc.)
- Add auth templates (okta, keycloak, etc.)

---

## Appendix: Anti-Patterns to Prevent

### Anti-Pattern 1: Schema Explosion

```bash
# ❌ DON'T
cargo pmcp generate --from swagger --source api.yaml --all

# ✅ DO
cargo pmcp generate types --from swagger --source api.yaml --schemas Pet,Order
```

### Anti-Pattern 2: Operation Mirroring

```
❌ API has 50 endpoints → Generate 50 tools
✅ API has 50 endpoints → Design 5-10 focused tools
```

### Anti-Pattern 3: Skipping Design

```bash
# ❌ DON'T
cargo pmcp generate server --from swagger --source api.yaml

# ✅ DO
cargo pmcp analyze swagger api.yaml          # Understand the API
cargo pmcp design --from swagger api.yaml     # Plan your tools
cargo pmcp generate types ...                 # Generate types
cargo pmcp add tool ...                       # Add designed tools
```

### Anti-Pattern 4: No Prompts

```
❌ 10 tools with no guidance
✅ 10 tools + 2-3 workflow prompts that guide usage
```

---

## References

- [cargo-pmcp README](../README.md)
- [PMCP SDK Documentation](https://docs.rs/pmcp)
- [MCP Specification](https://spec.modelcontextprotocol.io)
- [Existing Templates](../src/templates/)
