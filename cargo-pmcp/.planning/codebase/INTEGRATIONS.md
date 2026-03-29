# External Integrations

**Analysis Date:** 2026-02-26

## APIs & External Services

**pmcp.run Platform:**
- Service: pmcp.run managed MCP server hosting
- What it's used for: Deployment target, secret management, test scenario storage
- SDK/Client: Custom GraphQL client implementation
- Auth: OAuth2 with Cognito (custom `CognitoTokenFields`)
- Endpoints:
  - `https://api.pmcp.run` - Base API URL
  - `https://api.pmcp.run/graphql` - GraphQL endpoint
  - `https://auth.pmcp.run` - OAuth authentication domain
- Environment vars: `PMCP_RUN_API_URL`, `PMCP_RUN_GRAPHQL_URL`, `PMCP_ACCESS_TOKEN`
- Related files:
  - `src/deployment/targets/pmcp_run/auth.rs` - OAuth flow and token caching
  - `src/deployment/targets/pmcp_run/graphql.rs` - GraphQL operations
  - `src/deployment/targets/pmcp_run/deploy.rs` - Deployment logic
  - `src/secrets/providers/pmcp_run.rs` - Secret management via pmcp.run

**AWS Lambda:**
- Service: AWS Lambda serverless compute
- What it's used for: Deployment target for MCP servers (via pmcp.run backend)
- SDK/Client: `cargo-lambda` CLI tool (required)
- Tools: AWS CDK for infrastructure (required)
- Related files: `src/deployment/targets/pmcp_run/mod.rs`

**AWS Secrets Manager:**
- Service: AWS cloud secret storage
- What it's used for: Backend storage for secrets when using AWS provider
- SDK/Client: `aws-sdk-secretsmanager` 1.x (optional, feature: `aws-secrets`)
- Configuration: `aws-config` 1.x
- Auth: AWS credential chain (AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY, or AWS_PROFILE)
- Environment vars: `AWS_REGION`, `AWS_DEFAULT_REGION`
- Related files:
  - `src/secrets/providers/aws.rs` - AWS Secrets Manager provider (implementation placeholder)
  - Accessible via feature flag: `--features aws-secrets`

**Google Cloud Run:**
- Service: Google Cloud Run serverless container hosting
- What it's used for: Optional deployment target for MCP servers
- Implementation: `src/deployment/targets/google_cloud_run/mod.rs`
- Uses `reqwest` for status checks

**Cloudflare Workers:**
- Service: Cloudflare edge computing platform
- What it's used for: Optional deployment target for MCP servers
- Implementation: `src/deployment/targets/cloudflare/` (deploy.rs module)

## Data Storage

**Configuration Storage:**
- Location: Home directory under `~/.pmcp/`
- Files:
  - `pmcp-run-config.json` - Cached service discovery configuration
  - `pmcp-run-credentials.json` - Cached OAuth credentials (with 1-hour expiration)
  - `deploy.toml` - Deployment configuration per project
- Client: TOML parsing via `toml` crate, JSON via `serde_json`

**Project Configuration Files:**
- `pmcp-landing.toml` - Landing page configuration (parsed in `src/landing/config.rs`)
  - Parsed by: `LandingConfig::load()`
  - Validated: `LandingConfig::validate()`
- `.pmcp/deployment.toml` - Deployment metadata
- `.pmcp/deploy.toml` - Deployment configuration

**Local File Storage:**
- Zip files: Deployment packages created with `zip` 7.0
  - Path: `deploy/` directory (created during deployment)
- Scenario files: Test scenarios in YAML/JSON format
- Mock data: `mock-data/*.json` for landing page demonstrations

## Caching Strategy

**Configuration Cache:**
- Well-known config discovery cached at `~/.pmcp/pmcp-run-config.json`
- Cache duration: 1 hour (CONFIG_CACHE_DURATION_SECS = 3600)
- Parsed from RFC3339 timestamp in cache file

**OAuth Token Cache:**
- Access token cached at `~/.pmcp/pmcp-run-credentials.json`
- Cache duration: 1 hour (1-hour token expiration)
- Loaded before making GraphQL requests
- Refresh flow: New token fetched if cache expired
- Related file: `src/deployment/targets/pmcp_run/auth.rs`

## Authentication & Identity

**OAuth2 Flow (pmcp.run):**
- Provider: AWS Cognito
- Client ID: Fetched from service discovery
- Flow type: Authorization Code with PKCE
- Callback: Local server on port 8787
- Browser: Launched automatically via `open` crate
- Token exchange: Via Cognito token endpoint
- Implementation: `src/deployment/targets/pmcp_run/auth.rs`
  - Functions:
    - `login()` - Initiate OAuth flow
    - `logout()` - Clear cached credentials
    - `get_credentials()` - Retrieve cached or refreshed credentials
    - `discover_config()` - Fetch OAuth endpoints from well-known URL

**Custom Token Fields:**
- Extended OAuth token response to capture Cognito `id_token`
- Type: `CognitoTokenFields` extends `oauth2::ExtraTokenFields`
- Usage: Alongside standard access_token for API authentication

## Secrets Management

**Provider System:**
- Base trait: `SecretProvider` (async trait with multiple backends)
- Location: `src/secrets/providers/`

**Available Providers:**

1. **Local File Provider**
   - File: `src/secrets/providers/local.rs`
   - Storage: JSON files in `~/.pmcp/secrets/{server_id}/`
   - Use case: Development environment

2. **pmcp.run Provider**
   - File: `src/secrets/providers/pmcp_run.rs`
   - API: GraphQL endpoint at pmcp.run
   - Server-level namespacing: `{server-id}/{SECRET_NAME}`
   - Features: Versioning, tags, descriptions, binary values (64KB max)
   - Auth: OAuth access token (loaded from cache)

3. **AWS Secrets Manager Provider**
   - File: `src/secrets/providers/aws.rs`
   - API: AWS Secrets Manager API
   - Naming: `{prefix}{server_id}/{secret_name}` (configurable prefix)
   - Features: Versioning, tags, descriptions, binary values (64KB max)
   - Status: Implementation placeholder (feature-gated: `aws-secrets`)
   - Auth: AWS credential chain

**Secret Metadata:**
- Name with server namespace: `{server-id}/{SECRET_NAME}`
- Modified timestamp: RFC3339 format via `chrono`
- Validation: Name format validation per provider rules

## Webhooks & Callbacks

**Incoming:**
- OAuth callback receiver: Local HTTP server (port 8787)
  - File: `src/deployment/targets/pmcp_run/auth.rs`
  - Receives authorization code from Cognito
  - Exchanges code for access token
  - Closes after successful authentication

**Outgoing:**
- GraphQL mutations to pmcp.run for:
  - Test scenario uploads
  - Secret management operations
  - Landing page deployment metadata
- File: `src/deployment/targets/pmcp_run/graphql.rs`

## CI/CD & Deployment

**Deployment Targets:**

1. **pmcp.run** (Primary)
   - Type: Managed serverless platform
   - Backend: AWS Lambda
   - Build tool: `cargo-lambda` (required)
   - IaC tool: AWS CDK (required)
   - Configuration: `src/deployment/config.rs`
   - Auth: OAuth via pmcp.run

2. **AWS Lambda** (Direct)
   - Build tool: `cargo-lambda`
   - Package format: ARM64 Linux binary
   - Deployment method: via AWS CDK

3. **Google Cloud Run** (Optional)
   - Container-based serverless
   - Implementation: `src/deployment/targets/google_cloud_run/mod.rs`

4. **Cloudflare Workers** (Optional)
   - Edge computing platform
   - Implementation: `src/deployment/targets/cloudflare/mod.rs`

**Deployment Process:**
- Build artifact: Binary or WASM module
- Optional deployment package: Zip with assets
- Upload to platform-specific endpoint
- Retrieve deployment outputs (URLs, regions, etc.)

## Environment Configuration

**Required Environment Variables (pmcp.run):**
- `PMCP_RUN_API_URL` - Override pmcp.run API endpoint (default: `https://api.pmcp.run`)
- `PMCP_RUN_GRAPHQL_URL` - Override GraphQL endpoint (default: `https://api.pmcp.run/graphql`)
- `PMCP_ACCESS_TOKEN` - OAuth access token (if not cached)
- `PMCP_RUN_API_KEY` - API key for pmcp.run (server registration)

**AWS Environment Variables:**
- `AWS_ACCESS_KEY_ID` - AWS access key
- `AWS_SECRET_ACCESS_KEY` - AWS secret key
- `AWS_PROFILE` - AWS credential profile
- `AWS_REGION` - AWS region
- `AWS_DEFAULT_REGION` - Default AWS region

**Development Environment:**
- `PMCP_VERBOSE` - Enable verbose logging (set by CLI --verbose flag)
- Used in: GraphQL requests, secret operations, auth flows

**Secrets Location:**
- Cached in: `~/.pmcp/` directory
- Never hardcoded: Uses environment variables or credential cache
- Zeroization: `secrecy` and `zeroize` crates for secure memory handling

## Testing & Validation

**Local Testing:**
- Library: `mcp-tester` 0.2.0 (path dependency)
- Features: Server connectivity checks, scenario validation, JSON-RPC compliance
- Transport types: HTTP (SSE streaming), jsonrpc (POST), stdio

**Preview & Development:**
- Library: `mcp-preview` 0.1.0 (path dependency)
- Features: Browser-based widget preview for MCP Apps
- HTML generation with mock bridge: `src/publishing/landing.rs`
- Mock data: Loads from `mock-data/*.json` directory

**Scenario Management:**
- Upload: To pmcp.run via GraphQL
- Download: From pmcp.run for local execution
- Storage: Local YAML/JSON files in `tests/scenarios/`
- Generation: From server capabilities

## Key Integration Points

**Landing Page Deployment:**
- File: `src/commands/landing/deploy.rs`
- Flow:
  1. Load `pmcp-landing.toml` configuration
  2. Generate HTML with widget preview
  3. Create zip deployment package
  4. Upload to pmcp.run via GraphQL
  5. Update project metadata

**Schema Discovery:**
- Command: `cargo pmcp schema`
- Process: Connect to MCP server, extract tool schemas
- Output: Generate typed Rust client code
- Support: Both HTTP and Lambda invocation patterns

**Secret Provisioning:**
- Commands: `cargo pmcp secret list|get|set|delete`
- Provider selection: Auto-detect or explicit
- Server namespace: Automatic via server ID
- Validation: Per-provider naming rules

---

*Integration audit: 2026-02-26*
