# Deployment Metadata & Resource Abstraction Design

> **Status**: Design Draft
> **Version**: 0.1.0
> **Date**: January 2025
> **Related**: [TEMPLATE_REGISTRY_DESIGN.md](./TEMPLATE_REGISTRY_DESIGN.md)

## Executive Summary

This document specifies the design for deployment metadata extraction and resource abstraction in cargo-pmcp. The system enables:

1. **Template-declared resources** - Templates declare what secrets, parameters, and permissions they need
2. **Standardized metadata** - Deployment metadata using the `mcp:` namespace for cross-platform interoperability
3. **Platform extensions** - Vendor-specific namespaces for value-add features (e.g., `pmcp-run:*`)
4. **Simplified cargo-pmcp** - cargo-pmcp extracts and injects metadata; platforms handle provisioning

**Core Principle**: cargo-pmcp's responsibility is metadata extraction and injection into deployment artifacts. Platforms like pmcp.run handle the actual resource provisioning, allowing for cost-effective implementations (e.g., org-level secret bundling).

---

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Resource Declaration in Templates](#resource-declaration-in-templates)
3. [Server Manifest Specification](#server-manifest-specification)
4. [Metadata Schema](#metadata-schema)
5. [Deployment Flow](#deployment-flow)
6. [Platform Extension Model](#platform-extension-model)
7. [Runtime Resource Abstraction](#runtime-resource-abstraction)
8. [Deployment Target Contributions](#deployment-target-contributions)
9. [Implementation Guide](#implementation-guide)

---

## Architecture Overview

### The Three-Dimensional Challenge

Template-based MCP server deployment involves three dimensions:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    THREE-DIMENSIONAL DEPLOYMENT                              │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  GENERATION DIMENSION                                                       │
│  ════════════════════                                                       │
│  Create MCP servers from schemas (REST/GraphQL/DB)                          │
│  • Parse schema files                                                       │
│  • Generate secure Rust types                                               │
│  • Create tool scaffolds with client code                                   │
│  • Declare required resources                                               │
│                                                                             │
│  DEPLOYMENT DIMENSION                                                       │
│  ═════════════════════                                                      │
│  Deploy to multiple targets (AWS, GCP, CloudFlare, Azure)                   │
│  • Each target has different resource implementations                       │
│  • Secrets: AWS Secrets Manager vs GCP Secret Manager vs CF Secrets         │
│  • Parameters: SSM vs GCP Config vs CF Vars                                 │
│  • Permissions: IAM vs IAM vs Workers bindings                              │
│                                                                             │
│  PLATFORM DIMENSION                                                         │
│  ══════════════════                                                         │
│  Platforms add value beyond basic deployment                                │
│  • pmcp.run: org-level secrets, SSO, composition                            │
│  • Future platforms: different value propositions                           │
│  • Open source: neutral core, vendor extensions                             │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Separation of Concerns

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    RESPONSIBILITY BOUNDARIES                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  cargo-pmcp (Open Source)                                                   │
│  ════════════════════════                                                   │
│  ✓ Extract metadata from templates and manifests                            │
│  ✓ Inject metadata into deployment artifacts (CDK, Terraform, etc.)         │
│  ✓ Generate standardized mcp:* metadata                                     │
│  ✓ Pass context to IaC tools                                                │
│  ✗ Does NOT provision resources                                             │
│  ✗ Does NOT add IAM policies (platform responsibility)                      │
│  ✗ Does NOT manage secrets storage                                          │
│                                                                             │
│  Platform (pmcp.run, etc.)                                                  │
│  ═════════════════════════                                                  │
│  ✓ Read mcp:* metadata from deployment artifacts                            │
│  ✓ Provision resources based on metadata                                    │
│  ✓ Add IAM policies for secret/parameter access                             │
│  ✓ Implement cost-effective patterns (org-level bundling)                   │
│  ✓ Extend with vendor-specific features (pmcp-run:*)                        │
│  ✓ Handle onboarding UI, obtain URLs, etc.                                  │
│                                                                             │
│  Vanilla Deployment (AWS CDK direct)                                        │
│  ═══════════════════════════════════                                        │
│  • CloudFormation ignores unknown metadata                                  │
│  • Developer manually provisions secrets                                    │
│  • Manual IAM policy configuration                                          │
│  • Works but without platform conveniences                                  │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Resource Declaration in Templates

### Template Manifest Resource Section

Templates declare abstract resources they require. These declarations are schema-agnostic and target-agnostic.

```toml
# templates/types/graphql/manifest.toml

[template]
name = "graphql-api"
version = "1.0.0"
category = "types"
description = "Generate MCP server from GraphQL schema"

# ... existing template fields ...

# ============================================================================
# RESOURCE DECLARATIONS
# ============================================================================

[template.resources]
# Documentation shown to developers during generation
description = """
This template generates an MCP server that connects to an external GraphQL API.
The server requires API credentials and endpoint configuration.
"""

# Secret declarations - sensitive values
[[template.resources.secrets]]
id = "api_key"
description = "API key or bearer token for the GraphQL endpoint"
required = true
env_var = "GRAPHQL_API_KEY"
# Optional: help developers find where to get the secret
obtain_url_template = "Check your GraphQL API provider's dashboard"

[[template.resources.secrets]]
id = "api_secret"
description = "API secret (if using OAuth client credentials)"
required = false
env_var = "GRAPHQL_API_SECRET"

# Parameter declarations - non-sensitive configuration
[[template.resources.parameters]]
id = "endpoint_url"
description = "Base URL for the GraphQL API"
required = true
env_var = "GRAPHQL_ENDPOINT_URL"
# Default can be overridden during server generation
default_template = "https://api.example.com/graphql"
validation = "url"

[[template.resources.parameters]]
id = "timeout_seconds"
description = "Request timeout in seconds"
required = false
env_var = "GRAPHQL_TIMEOUT"
default = "30"
validation = "integer"

# Permission declarations - access requirements
[[template.resources.permissions]]
id = "network_egress"
type = "outbound_https"
description = "HTTPS access to external GraphQL API"
# Used by platforms to configure security groups, VPC, etc.
```

### Resource Types

| Type | Description | Examples |
|------|-------------|----------|
| `secret` | Sensitive values that must be encrypted at rest | API keys, tokens, passwords, certificates |
| `parameter` | Non-sensitive configuration values | URLs, feature flags, timeouts, region settings |
| `permission` | Access requirements for the server | Network egress, file system, other AWS services |

### Resource Declaration Fields

```toml
[[template.resources.secrets]]
id = "unique_id"              # Required: unique identifier within template
description = "Human desc"     # Required: shown in UI and documentation
required = true|false          # Required: whether server fails without it
env_var = "ENV_VAR_NAME"       # Required: environment variable name in server code
obtain_url_template = "..."    # Optional: help text for obtaining the secret
default = "..."                # Optional: default value (parameters only, not secrets)
validation = "url|integer|..." # Optional: validation type
```

---

## Server Manifest Specification

When `cargo pmcp server add --template graphql --schema schema.graphql` runs, it creates a **builtin-manifest.toml** that captures the instantiated resources for this specific server.

### Manifest Location

```
servers/{server-name}/
├── builtin-manifest.toml    # Server manifest with instantiated resources
├── src/
│   ├── lib.rs
│   ├── types/               # Generated from schema
│   └── tools/               # Tool scaffolds
└── Cargo.toml
```

### Manifest Schema

```toml
# servers/state-policies/builtin-manifest.toml

[server]
# Server identification
name = "state-policies"
type = "graphql-api"

# Template provenance
template_id = "types/graphql"
template_version = "1.0.0"
generated_at = "2025-01-08T12:00:00Z"

# Schema source (for documentation/regeneration)
[server.source]
type = "graphql"
path = "schema.graphql"
# Or URL: url = "https://api.example.com/graphql/schema"

# ============================================================================
# INSTANTIATED RESOURCES
# ============================================================================

# Secrets - instantiated from template with server-specific names
[[secrets.definitions]]
id = "api_key"                                    # From template
name = "STATE_POLICIES_API_KEY"                   # Instantiated name
description = "AWS AppSync API Key for the State Policies GraphQL API"
required = true
env_var = "GRAPHQL_API_KEY"
obtain_url = "https://console.aws.amazon.com/appsync"

# Parameters - instantiated with actual values
[[parameters.definitions]]
id = "endpoint_url"
name = "STATE_POLICIES_ENDPOINT"
value = "https://xxx.appsync-api.us-east-1.amazonaws.com/graphql"
env_var = "GRAPHQL_ENDPOINT_URL"

[[parameters.definitions]]
id = "timeout_seconds"
name = "STATE_POLICIES_TIMEOUT"
value = "30"
env_var = "GRAPHQL_TIMEOUT"

# Permissions - instantiated with specific targets
[[permissions]]
id = "network_egress"
type = "outbound_https"
description = "HTTPS access to AWS AppSync"
targets = ["*.appsync-api.*.amazonaws.com:443"]

# ============================================================================
# SERVER CAPABILITIES (auto-detected from code)
# ============================================================================

[capabilities]
tools = ["query-policies", "get-policy", "search-policies"]
resources = false
prompts = ["policy-lookup-workflow"]
composition = true
```

### Manifest Generation Flow

```
Template Manifest (abstract)     User Input (specific)
        │                               │
        │   ┌───────────────────────────┘
        │   │
        ▼   ▼
   cargo pmcp server add --template graphql \
       --schema schema.graphql \
       --name state-policies \
       --endpoint https://xxx.appsync-api.us-east-1.amazonaws.com/graphql
        │
        ▼
   builtin-manifest.toml (instantiated)
   • template.resources.secrets[api_key] → secrets.definitions[STATE_POLICIES_API_KEY]
   • template.resources.parameters[endpoint_url] → parameters.definitions[value=https://...]
```

---

## Metadata Schema

### Namespace Strategy

```
mcp:*        - Core MCP metadata (governed by cargo-pmcp project)
             - Standardized, stable, cross-platform compatible
             - Any platform can read and act on this metadata

pmcp-run:*   - pmcp.run platform extensions
cloudflare:* - CloudFlare-specific configurations
google:*     - GCP-specific metadata
azure:*      - Azure-specific metadata
custom:*     - User-defined extensions
```

### Core Metadata Schema (mcp:*)

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://schemas.pmcp.run/mcp-metadata/v1.0.json",
  "title": "MCP Server Deployment Metadata v1.0",
  "description": "Standardized metadata for MCP server deployments",

  "type": "object",
  "properties": {
    "mcp:version": {
      "type": "string",
      "const": "1.0",
      "description": "Metadata schema version"
    },

    "mcp:serverType": {
      "type": "string",
      "enum": ["graphql-api", "openapi-api", "database", "custom"],
      "description": "Type of MCP server (from template category)"
    },

    "mcp:serverId": {
      "type": "string",
      "pattern": "^[a-z][a-z0-9-]*$",
      "description": "Unique server identifier"
    },

    "mcp:templateId": {
      "type": "string",
      "description": "Template used to generate this server (e.g., 'types/graphql')"
    },

    "mcp:templateVersion": {
      "type": "string",
      "pattern": "^\\d+\\.\\d+\\.\\d+$",
      "description": "Semantic version of the template"
    },

    "mcp:resources": {
      "$ref": "#/$defs/ResourceRequirements"
    },

    "mcp:capabilities": {
      "$ref": "#/$defs/ServerCapabilities"
    }
  },

  "patternProperties": {
    "^[a-z][a-z0-9-]*:.*$": {
      "description": "Vendor-specific metadata extensions"
    }
  },

  "$defs": {
    "ResourceRequirements": {
      "type": "object",
      "properties": {
        "secrets": {
          "type": "array",
          "items": { "$ref": "#/$defs/SecretRequirement" }
        },
        "parameters": {
          "type": "array",
          "items": { "$ref": "#/$defs/ParameterRequirement" }
        },
        "permissions": {
          "type": "array",
          "items": { "$ref": "#/$defs/PermissionRequirement" }
        }
      }
    },

    "SecretRequirement": {
      "type": "object",
      "required": ["name", "required", "envVar"],
      "properties": {
        "name": {
          "type": "string",
          "description": "Secret name (server-specific, e.g., STATE_POLICIES_API_KEY)"
        },
        "description": {
          "type": "string",
          "description": "Human-readable description"
        },
        "required": {
          "type": "boolean",
          "description": "Whether the server requires this secret to function"
        },
        "envVar": {
          "type": "string",
          "description": "Environment variable name the server code expects"
        },
        "obtainUrl": {
          "type": "string",
          "format": "uri",
          "description": "URL where developers can obtain this secret"
        }
      }
    },

    "ParameterRequirement": {
      "type": "object",
      "required": ["name", "envVar"],
      "properties": {
        "name": {
          "type": "string",
          "description": "Parameter name"
        },
        "value": {
          "type": "string",
          "description": "Configured value"
        },
        "envVar": {
          "type": "string",
          "description": "Environment variable name"
        },
        "description": {
          "type": "string"
        }
      }
    },

    "PermissionRequirement": {
      "type": "object",
      "required": ["id", "type"],
      "properties": {
        "id": {
          "type": "string"
        },
        "type": {
          "type": "string",
          "enum": ["outbound_https", "outbound_http", "s3_read", "s3_write", "dynamodb", "custom"]
        },
        "targets": {
          "type": "array",
          "items": { "type": "string" },
          "description": "Specific targets (URLs, ARN patterns, etc.)"
        },
        "description": {
          "type": "string"
        }
      }
    },

    "ServerCapabilities": {
      "type": "object",
      "properties": {
        "tools": {
          "type": "array",
          "items": { "type": "string" },
          "description": "List of tool names this server provides"
        },
        "resources": {
          "type": "boolean",
          "description": "Whether server provides MCP resources"
        },
        "prompts": {
          "type": "array",
          "items": { "type": "string" },
          "description": "List of prompt names"
        },
        "composition": {
          "type": "boolean",
          "description": "Whether server supports server-to-server composition"
        }
      }
    }
  }
}
```

### CloudFormation Metadata Example

```json
{
  "AWSTemplateFormatVersion": "2010-09-09",
  "Metadata": {
    "mcp:version": "1.0",
    "mcp:serverType": "graphql-api",
    "mcp:serverId": "state-policies",
    "mcp:templateId": "types/graphql",
    "mcp:templateVersion": "1.0.0",
    "mcp:resources": {
      "secrets": [
        {
          "name": "STATE_POLICIES_API_KEY",
          "description": "AWS AppSync API Key for the State Policies GraphQL API",
          "required": true,
          "envVar": "GRAPHQL_API_KEY",
          "obtainUrl": "https://console.aws.amazon.com/appsync"
        }
      ],
      "parameters": [
        {
          "name": "STATE_POLICIES_ENDPOINT",
          "value": "https://xxx.appsync-api.us-east-1.amazonaws.com/graphql",
          "envVar": "GRAPHQL_ENDPOINT_URL"
        }
      ],
      "permissions": [
        {
          "id": "network_egress",
          "type": "outbound_https",
          "targets": ["*.appsync-api.*.amazonaws.com:443"]
        }
      ]
    },
    "mcp:capabilities": {
      "tools": ["query-policies", "get-policy", "search-policies"],
      "resources": false,
      "prompts": ["policy-lookup-workflow"],
      "composition": true
    }
  },
  "Resources": {
    "...": "..."
  }
}
```

---

## Deployment Flow

### Complete Flow Diagram

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           GENERATION PHASE                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Schema File (GraphQL/OpenAPI/DB)                                           │
│         │                                                                   │
│         ▼                                                                   │
│  cargo pmcp server add --template graphql \                                 │
│      --schema schema.graphql \                                              │
│      --name state-policies \                                                │
│      --endpoint https://xxx.appsync-api.us-east-1.amazonaws.com/graphql     │
│         │                                                                   │
│         ├──► Generated Rust Code                                            │
│         │    ├── src/types/          (from GraphQL schema)                  │
│         │    ├── src/client.rs       (GraphQL client with reqwest)          │
│         │    ├── src/tools/          (tool scaffolds)                       │
│         │    └── src/lib.rs          (server entry point)                   │
│         │                                                                   │
│         └──► builtin-manifest.toml                                          │
│              ├── [server] type, template info                               │
│              ├── [[secrets.definitions]] instantiated secrets               │
│              ├── [[parameters.definitions]] instantiated params             │
│              └── [capabilities] detected from code                          │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                           DEPLOYMENT PHASE                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  cargo pmcp deploy --target pmcp-run                                        │
│         │                                                                   │
│         ├──► MetadataExtractor                                              │
│         │    Reads: builtin-manifest.toml, .pmcp/deploy.toml, Cargo.toml    │
│         │    Outputs: McpMetadata struct                                    │
│         │                                                                   │
│         ├──► Binary Build                                                   │
│         │    cargo-lambda build → bootstrap binary                          │
│         │                                                                   │
│         ├──► CDK Synthesis                                                  │
│         │    npx cdk synth \                                                │
│         │      -c "mcp:version=1.0" \                                       │
│         │      -c "mcp:serverType=graphql-api" \                            │
│         │      -c "mcp:serverId=state-policies" \                           │
│         │      -c 'mcp:resources={...}'                                     │
│         │                                                                   │
│         └──► cdk.out/template.json                                          │
│              Contains: CloudFormation with mcp:* metadata                   │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                           PLATFORM PHASE                                     │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  pmcp.run receives:                                                         │
│  • template.json (CloudFormation with metadata)                             │
│  • bootstrap binary                                                         │
│         │                                                                   │
│         ├──► Read mcp:* metadata                                            │
│         │    • mcp:resources.secrets → know what secrets needed             │
│         │    • mcp:resources.permissions → know access requirements         │
│         │                                                                   │
│         ├──► Platform Logic (NOT in cargo-pmcp)                             │
│         │    • Bundle secrets at org level (cost-effective)                 │
│         │    • Add IAM policies to Lambda role                              │
│         │    • Configure secret injection                                   │
│         │    • Set up VPC/security groups if needed                         │
│         │                                                                   │
│         ├──► Read pmcp-run:* extensions (if present)                        │
│         │    • Org secret mappings                                          │
│         │    • SSO configuration                                            │
│         │    • Composition settings                                         │
│         │                                                                   │
│         └──► Deploy enriched CloudFormation                                 │
│              • Lambda with correct IAM                                      │
│              • Secrets injected at runtime                                  │
│              • Monitoring configured                                        │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Vanilla AWS Deployment (Without Platform)

When deploying directly to AWS (not through pmcp.run):

```
cargo pmcp deploy --target aws-lambda
        │
        ├──► Same metadata extraction
        ├──► Same CDK synthesis with mcp:* metadata
        │
        └──► Developer responsibilities:
             • Manually create secrets in Secrets Manager
             • Manually add IAM policies
             • CloudFormation ignores mcp:* metadata
             • Works, but without platform conveniences
```

---

## Platform Extension Model

### pmcp.run Extensions

pmcp.run adds value through its `pmcp-run:*` namespace:

```json
{
  "Metadata": {
    "mcp:version": "1.0",
    "mcp:serverType": "graphql-api",
    "mcp:serverId": "state-policies",
    "mcp:resources": {
      "secrets": [{
        "name": "STATE_POLICIES_API_KEY",
        "required": true,
        "envVar": "GRAPHQL_API_KEY",
        "obtainUrl": "https://console.aws.amazon.com/appsync"
      }]
    },

    "pmcp-run:orgSecrets": {
      "enabled": true,
      "bundleId": "org-secrets-bundle",
      "mapping": {
        "STATE_POLICIES_API_KEY": "appsync.state-policies.api-key"
      }
    },

    "pmcp-run:provisioning": {
      "autoCreate": true,
      "onboardingFlow": "guided"
    },

    "pmcp-run:composition": {
      "tier": "domain",
      "dependencies": ["foundation/auth-server"],
      "exposeToGateway": true,
      "internalOnly": false
    },

    "pmcp-run:sso": {
      "enabled": true,
      "sharedPoolId": "us-east-1_XXX"
    }
  }
}
```

### Platform Implementation: Org-Level Secret Bundling

pmcp.run's cost-effective secret management:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│              pmcp.run Org-Level Secret Bundling                              │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  Traditional Approach (per-server secrets):                                 │
│  ═══════════════════════════════════════════                                │
│                                                                             │
│  Server A → Secret: pmcp/server-a/API_KEY     ($0.40/month)                 │
│  Server B → Secret: pmcp/server-b/API_KEY     ($0.40/month)                 │
│  Server C → Secret: pmcp/server-c/API_KEY     ($0.40/month)                 │
│  ...                                                                        │
│  100 servers → 100 secrets → $40/month                                      │
│                                                                             │
│  pmcp.run Approach (org-level bundling):                                    │
│  ═══════════════════════════════════════                                    │
│                                                                             │
│  Single Secret: pmcp/org-123/secrets-bundle                                 │
│  {                                                                          │
│    "server-a": { "API_KEY": "..." },                                        │
│    "server-b": { "API_KEY": "..." },                                        │
│    "server-c": { "API_KEY": "..." },                                        │
│    ...                                                                      │
│  }                                                                          │
│  100 servers → 1 secret → $0.40/month                                       │
│                                                                             │
│  Platform Logic:                                                            │
│  1. Read mcp:resources.secrets from metadata                                │
│  2. Store in org bundle under server namespace                              │
│  3. Inject Lambda environment with bundle ARN                               │
│  4. Runtime: parse bundle, extract server's secrets                         │
│  5. Add single IAM policy for bundle access                                 │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Adding Platform Extensions to Metadata

Platforms can add their extensions during `cargo pmcp deploy`:

```toml
# .pmcp/deploy.toml

[target]
type = "pmcp-run"

# Platform-specific settings become pmcp-run:* metadata
[pmcp-run]
org_secrets_enabled = true
auto_provision = true
composition_tier = "domain"
sso_enabled = true
```

cargo-pmcp injects these as `pmcp-run:*` metadata in the CloudFormation template.

### Future Platform Contributions

Other platforms can define their namespaces:

```json
{
  "cloudflare:bindings": {
    "secrets": ["API_KEY"],
    "vars": ["ENDPOINT_URL"],
    "kvNamespaces": ["cache"]
  },

  "google:cloudRun": {
    "secretManager": {
      "project": "my-project",
      "secrets": ["API_KEY"]
    },
    "vpcConnector": "projects/my-project/locations/us-central1/connectors/vpc"
  },

  "azure:functions": {
    "keyVault": {
      "name": "my-keyvault",
      "secrets": ["API_KEY"]
    }
  }
}
```

---

## Runtime Resource Abstraction

### Resource Provider Interface

Generated server code uses an abstraction layer for portable resource access:

```rust
// pmcp-resource crate

use async_trait::async_trait;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ResourceError {
    #[error("Secret not found: {0}")]
    SecretNotFound(String),
    #[error("Parameter not found: {0}")]
    ParameterNotFound(String),
    #[error("Provider error: {0}")]
    ProviderError(String),
}

#[async_trait]
pub trait ResourceProvider: Send + Sync {
    /// Get a secret value by name
    async fn get_secret(&self, name: &str) -> Result<String, ResourceError>;

    /// Get a parameter value by name
    async fn get_parameter(&self, name: &str) -> Result<String, ResourceError>;

    /// Check if a secret exists
    async fn has_secret(&self, name: &str) -> bool;

    /// Provider identifier for logging/debugging
    fn provider_id(&self) -> &str;
}
```

### Provider Implementations

```rust
// AWS Secrets Manager Provider (for vanilla AWS deployments)
pub struct AwsSecretsManagerProvider {
    client: aws_sdk_secretsmanager::Client,
    prefix: String,
}

#[async_trait]
impl ResourceProvider for AwsSecretsManagerProvider {
    async fn get_secret(&self, name: &str) -> Result<String, ResourceError> {
        let secret_id = format!("{}/{}", self.prefix, name);
        let result = self.client
            .get_secret_value()
            .secret_id(&secret_id)
            .send()
            .await
            .map_err(|e| ResourceError::ProviderError(e.to_string()))?;

        result.secret_string()
            .map(|s| s.to_string())
            .ok_or_else(|| ResourceError::SecretNotFound(name.to_string()))
    }

    // ...
}

// pmcp.run Bundled Secrets Provider
pub struct PmcpRunBundledProvider {
    bundle: HashMap<String, String>,
}

impl PmcpRunBundledProvider {
    pub async fn from_env() -> Result<Self, ResourceError> {
        // Read bundle ARN from environment
        let bundle_arn = std::env::var("PMCP_SECRETS_BUNDLE_ARN")
            .map_err(|_| ResourceError::ProviderError("PMCP_SECRETS_BUNDLE_ARN not set".into()))?;

        let server_id = std::env::var("PMCP_SERVER_ID")
            .map_err(|_| ResourceError::ProviderError("PMCP_SERVER_ID not set".into()))?;

        // Fetch and parse bundle
        let client = aws_sdk_secretsmanager::Client::new(&aws_config::load_from_env().await);
        let result = client.get_secret_value().secret_id(&bundle_arn).send().await
            .map_err(|e| ResourceError::ProviderError(e.to_string()))?;

        let bundle_json: HashMap<String, HashMap<String, String>> =
            serde_json::from_str(result.secret_string().unwrap_or("{}"))
                .map_err(|e| ResourceError::ProviderError(e.to_string()))?;

        let server_secrets = bundle_json.get(&server_id)
            .cloned()
            .unwrap_or_default();

        Ok(Self { bundle: server_secrets })
    }
}

#[async_trait]
impl ResourceProvider for PmcpRunBundledProvider {
    async fn get_secret(&self, name: &str) -> Result<String, ResourceError> {
        self.bundle.get(name)
            .cloned()
            .ok_or_else(|| ResourceError::SecretNotFound(name.to_string()))
    }

    // ...
}

// Environment Variable Provider (for local development)
pub struct EnvResourceProvider;

#[async_trait]
impl ResourceProvider for EnvResourceProvider {
    async fn get_secret(&self, name: &str) -> Result<String, ResourceError> {
        std::env::var(name)
            .map_err(|_| ResourceError::SecretNotFound(name.to_string()))
    }

    async fn get_parameter(&self, name: &str) -> Result<String, ResourceError> {
        std::env::var(name)
            .map_err(|_| ResourceError::ParameterNotFound(name.to_string()))
    }

    fn provider_id(&self) -> &str {
        "env"
    }
}
```

### Auto-Detection

```rust
pub struct ServerResources {
    provider: Box<dyn ResourceProvider>,
}

impl ServerResources {
    pub async fn auto_detect() -> Result<Self, ResourceError> {
        // Check for pmcp.run bundled secrets first
        if std::env::var("PMCP_SECRETS_BUNDLE_ARN").is_ok() {
            return Ok(Self {
                provider: Box::new(PmcpRunBundledProvider::from_env().await?),
            });
        }

        // Check for AWS Lambda environment
        if std::env::var("AWS_LAMBDA_FUNCTION_NAME").is_ok() {
            return Ok(Self {
                provider: Box::new(AwsSecretsManagerProvider::from_env().await?),
            });
        }

        // Check for CloudFlare Workers
        if std::env::var("CF_WORKER").is_ok() {
            return Ok(Self {
                provider: Box::new(CloudflareResourceProvider::new()?),
            });
        }

        // Fall back to environment variables (local dev)
        Ok(Self {
            provider: Box::new(EnvResourceProvider),
        })
    }

    pub async fn get_secret(&self, name: &str) -> Result<String, ResourceError> {
        self.provider.get_secret(name).await
    }
}
```

### Usage in Generated Code

```rust
// Generated in src/client.rs

use pmcp_resource::ServerResources;

pub struct GraphQLClient {
    client: reqwest::Client,
    endpoint: String,
    api_key: String,
}

impl GraphQLClient {
    pub async fn from_resources(resources: &ServerResources) -> Result<Self, Error> {
        let api_key = resources
            .get_secret("GRAPHQL_API_KEY")
            .await
            .map_err(|e| Error::config(format!("Failed to get API key: {}", e)))?;

        let endpoint = resources
            .get_parameter("GRAPHQL_ENDPOINT_URL")
            .await
            .map_err(|e| Error::config(format!("Failed to get endpoint: {}", e)))?;

        Ok(Self {
            client: reqwest::Client::new(),
            endpoint,
            api_key,
        })
    }
}

// In server initialization
pub async fn create_server() -> Result<McpServer, Error> {
    let resources = ServerResources::auto_detect().await?;
    let client = GraphQLClient::from_resources(&resources).await?;

    // Build server with client...
}
```

---

## Deployment Target Contributions

### Contribution Model

Cloud providers and community can contribute deployment targets:

```
pmcp-templates/
└── deployment/
    ├── aws-lambda/              # AWS team or community
    │   ├── manifest.toml        # Target metadata
    │   ├── resource-mapping.toml# How resources map
    │   ├── cdk-template/        # CDK scaffolding
    │   ├── runtime/             # Resource provider impl
    │   └── README.md
    │
    ├── cloudflare-workers/      # CloudFlare contribution
    │   ├── manifest.toml
    │   ├── resource-mapping.toml
    │   ├── wrangler-template/
    │   └── runtime/
    │
    ├── google-cloud-run/        # GCP contribution
    │   ├── manifest.toml
    │   ├── resource-mapping.toml
    │   ├── terraform-template/
    │   └── runtime/
    │
    └── azure-functions/         # Azure contribution
        └── ...
```

### Resource Mapping Specification

```toml
# deployment/aws-lambda/resource-mapping.toml

[target]
id = "aws-lambda"
name = "AWS Lambda"
description = "Deploy MCP server as AWS Lambda function"

[requirements]
tools = ["cargo-lambda", "aws-cdk"]
credentials = ["AWS_ACCESS_KEY_ID", "AWS_SECRET_ACCESS_KEY"]

# How abstract resources map to AWS services
[mapping.secret]
service = "secretsmanager"
arn_pattern = "arn:aws:secretsmanager:${region}:${account}:secret:${prefix}/${server_id}/${name}"
iam_actions = ["secretsmanager:GetSecretValue"]
cost_per_secret = 0.40  # USD/month (for documentation)

[mapping.secret.alternatives.ssm]
service = "ssm"
parameter_type = "SecureString"
arn_pattern = "arn:aws:ssm:${region}:${account}:parameter/${prefix}/${server_id}/${name}"
iam_actions = ["ssm:GetParameter", "ssm:GetParametersByPath"]

[mapping.parameter]
service = "ssm"
parameter_type = "String"
arn_pattern = "arn:aws:ssm:${region}:${account}:parameter/${prefix}/${server_id}/${name}"
iam_actions = ["ssm:GetParameter"]

[mapping.permission.outbound_https]
# Lambda has internet access by default (no VPC)
vpc_required = false
security_group_egress = "0.0.0.0/0:443"

[mapping.permission.s3_read]
iam_actions = ["s3:GetObject", "s3:ListBucket"]
resource_pattern = "arn:aws:s3:::${bucket}/*"
```

```toml
# deployment/cloudflare-workers/resource-mapping.toml

[target]
id = "cloudflare-workers"
name = "CloudFlare Workers"

[mapping.secret]
binding_type = "secret_text"
injection = "direct"  # Injected as environment variable
# No IAM - CloudFlare handles access control

[mapping.parameter]
binding_type = "vars"
injection = "direct"

[mapping.permission.outbound_https]
# Workers have internet access by default
no_configuration_needed = true
```

### Contribution Requirements

New deployment targets must provide:

1. **manifest.toml** - Target metadata, requirements, capabilities
2. **resource-mapping.toml** - How abstract resources map to target services
3. **IaC templates** - CDK, Terraform, Pulumi, or native configs
4. **Runtime provider** - ResourceProvider implementation for the target
5. **Documentation** - Setup guide, prerequisites, limitations
6. **Tests** - Deployment and resource access tests

---

## Implementation Guide

### Phase 1: Metadata Extraction (cargo-pmcp)

```rust
// src/deployment/metadata_extractor.rs

use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
pub struct McpMetadata {
    pub version: String,
    pub server_type: String,
    pub server_id: String,
    pub template_id: Option<String>,
    pub template_version: Option<String>,
    pub resources: ResourceRequirements,
    pub capabilities: ServerCapabilities,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResourceRequirements {
    pub secrets: Vec<SecretRequirement>,
    pub parameters: Vec<ParameterRequirement>,
    pub permissions: Vec<PermissionRequirement>,
}

impl McpMetadata {
    /// Extract metadata from project, checking multiple sources
    pub fn extract(project_root: &Path) -> Result<Self, MetadataError> {
        // Priority order:
        // 1. builtin-manifest.toml (template-generated servers)
        // 2. .pmcp/template-info.toml (legacy)
        // 3. pmcp.toml (custom servers)

        let manifest_path = project_root.join("builtin-manifest.toml");
        if manifest_path.exists() {
            return Self::from_builtin_manifest(&manifest_path);
        }

        let template_info = project_root.join(".pmcp/template-info.toml");
        if template_info.exists() {
            return Self::from_template_info(&template_info);
        }

        let pmcp_toml = project_root.join("pmcp.toml");
        if pmcp_toml.exists() {
            return Self::from_pmcp_toml(&pmcp_toml);
        }

        // Default for custom servers
        Self::default_custom(project_root)
    }

    /// Convert to CDK context arguments
    pub fn to_cdk_context(&self) -> Vec<String> {
        vec![
            format!("-c mcp:version={}", self.version),
            format!("-c mcp:serverType={}", self.server_type),
            format!("-c mcp:serverId={}", self.server_id),
            format!("-c mcp:resources={}",
                serde_json::to_string(&self.resources).unwrap()),
            format!("-c mcp:capabilities={}",
                serde_json::to_string(&self.capabilities).unwrap()),
        ]
    }

    /// Convert to CloudFormation metadata object
    pub fn to_cloudformation_metadata(&self) -> serde_json::Value {
        serde_json::json!({
            "mcp:version": self.version,
            "mcp:serverType": self.server_type,
            "mcp:serverId": self.server_id,
            "mcp:templateId": self.template_id,
            "mcp:templateVersion": self.template_version,
            "mcp:resources": self.resources,
            "mcp:capabilities": self.capabilities,
        })
    }
}
```

### Phase 2: CDK Template Update

```typescript
// deploy/lib/stack.ts (generated template)

import * as cdk from 'aws-cdk-lib';
import * as lambda from 'aws-cdk-lib/aws-lambda';
import { Construct } from 'constructs';

export class McpServerStack extends cdk.Stack {
  constructor(scope: Construct, id: string, props?: cdk.StackProps) {
    super(scope, id, props);

    // ========================================================================
    // READ MCP METADATA FROM CONTEXT
    // ========================================================================

    const mcpVersion = this.node.tryGetContext('mcp:version') || '1.0';
    const mcpServerType = this.node.tryGetContext('mcp:serverType') || 'custom';
    const mcpServerId = this.node.tryGetContext('mcp:serverId') || this.stackName;
    const mcpResources = JSON.parse(
      this.node.tryGetContext('mcp:resources') || '{}'
    );
    const mcpCapabilities = JSON.parse(
      this.node.tryGetContext('mcp:capabilities') || '{}'
    );

    // ========================================================================
    // SET CLOUDFORMATION METADATA
    // Platforms read this; vanilla CloudFormation ignores it
    // ========================================================================

    this.templateOptions.metadata = {
      'mcp:version': mcpVersion,
      'mcp:serverType': mcpServerType,
      'mcp:serverId': mcpServerId,
      'mcp:resources': mcpResources,
      'mcp:capabilities': mcpCapabilities,
      // Platform-specific extensions added by deploy.toml parsing
      ...this.getPlatformMetadata(),
    };

    // ========================================================================
    // CREATE LAMBDA FUNCTION
    // Note: IAM policies for secrets are NOT added here
    // Platform (pmcp.run) handles that based on metadata
    // ========================================================================

    const fn = new lambda.Function(this, 'McpServer', {
      runtime: lambda.Runtime.PROVIDED_AL2023,
      handler: 'bootstrap',
      code: lambda.Code.fromAsset('../.build'),
      architecture: lambda.Architecture.ARM_64,
      memorySize: 512,
      timeout: cdk.Duration.seconds(30),
      environment: {
        RUST_LOG: 'info',
        MCP_SERVER_ID: mcpServerId,
        // Note: Secrets are NOT passed here
        // Platform injects them based on metadata
      },
    });

    // ... rest of stack (Lambda URL, etc.)
  }

  private getPlatformMetadata(): Record<string, unknown> {
    // Read platform-specific settings from context
    // These become pmcp-run:*, cloudflare:*, etc. namespaces
    const platformContext = this.node.tryGetContext('platform') || {};
    const result: Record<string, unknown> = {};

    for (const [key, value] of Object.entries(platformContext)) {
      result[key] = value;
    }

    return result;
  }
}
```

### Phase 3: Deploy Command Integration

```rust
// src/commands/deploy/mod.rs (updated)

impl DeployCommand {
    async fn execute_deploy(&self, ctx: &DeployContext) -> Result<DeploymentOutputs> {
        let project_root = ctx.project_root();
        let config = ctx.deploy_config();
        let target = ctx.target();

        // ====================================================================
        // METADATA EXTRACTION
        // ====================================================================

        let metadata = McpMetadata::extract(&project_root)
            .context("Failed to extract MCP metadata")?;

        info!("Extracted metadata for server: {}", metadata.server_id);
        info!("Server type: {}", metadata.server_type);
        info!("Required secrets: {}", metadata.resources.secrets.len());

        // ====================================================================
        // BUILD BINARY
        // ====================================================================

        let artifact = target.build(&config, &project_root).await?;

        // ====================================================================
        // CDK SYNTHESIS WITH METADATA
        // ====================================================================

        let cdk_context = metadata.to_cdk_context();

        // Add platform-specific context if configured
        let platform_context = self.build_platform_context(&config)?;

        // Run CDK synth
        let synth_result = self.run_cdk_synth(
            &project_root,
            &cdk_context,
            &platform_context,
        ).await?;

        // ====================================================================
        // DEPLOY TO TARGET
        // ====================================================================

        // For pmcp-run: upload template + binary, platform handles the rest
        // For aws-lambda: run cdk deploy directly
        let outputs = target.deploy(&config, &artifact, &synth_result).await?;

        Ok(outputs)
    }

    fn build_platform_context(&self, config: &DeployConfig) -> Result<Vec<String>> {
        let mut context = vec![];

        // Add pmcp-run specific context
        if let Some(pmcp_run) = &config.pmcp_run {
            if pmcp_run.org_secrets_enabled {
                context.push("-c platform:pmcp-run:orgSecrets.enabled=true".into());
            }
            if pmcp_run.auto_provision {
                context.push("-c platform:pmcp-run:provisioning.autoCreate=true".into());
            }
            // ... other pmcp-run settings
        }

        Ok(context)
    }
}
```

---

## Summary

### What cargo-pmcp Does

1. **Extracts metadata** from builtin-manifest.toml, template-info.toml, or pmcp.toml
2. **Injects metadata** into CDK context and CloudFormation template
3. **Synthesizes IaC** with standardized `mcp:*` metadata
4. **Uploads artifacts** (for platforms like pmcp.run)

### What cargo-pmcp Does NOT Do

1. **Does NOT provision secrets** - Platform responsibility
2. **Does NOT add IAM policies** - Platform responsibility
3. **Does NOT manage secret storage** - Platform responsibility
4. **Does NOT implement platform-specific logic** - Platforms own their namespaces

### Benefits

| Stakeholder | Benefit |
|-------------|---------|
| **Developers** | Single workflow for any target, portable servers |
| **Template Authors** | Declare resources abstractly, works everywhere |
| **Platforms (pmcp.run)** | Full control over resource provisioning, cost optimization |
| **Cloud Providers** | Contribute deployment targets, grow ecosystem |
| **Open Source** | Neutral core, vendor extensions via namespaces |

---

## References

- [Template Registry Design](./TEMPLATE_REGISTRY_DESIGN.md)
- [OAuth Design](./oauth-design.md)
- [pmcp.run Integration](./PMCP_RUN_INTEGRATION_UPDATE.md)
- [MCP Specification](https://spec.modelcontextprotocol.io)
