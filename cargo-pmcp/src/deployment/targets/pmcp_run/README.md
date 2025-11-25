# pmcp.run Deployment Target

Deploy your Rust MCP servers to [pmcp.run](https://pmcp.run) - a fully managed, serverless MCP hosting platform.

## Overview

pmcp.run provides:
- ✅ **Serverless deployment** - AWS Lambda + API Gateway
- ✅ **Global CDN** - Low latency worldwide
- ✅ **Auto-scaling** - Scales from zero to millions of requests
- ✅ **OAuth 2.0 authentication** - Secure AWS Cognito integration
- ✅ **GraphQL API** - Programmatic deployment management
- ✅ **Zero configuration** - Works out of the box

## Quick Start

### 1. Login (First Time Only)

```bash
cargo pmcp deploy login --target pmcp-run
```

This will:
1. Start a local callback server on port 8787
2. Open your browser for OAuth authentication
3. Save credentials to `~/.pmcp/credentials.toml`

### 2. Deploy Your Server

```bash
cargo pmcp deploy --target pmcp-run
```

Your MCP server will be deployed and you'll receive a public URL like:
```
https://your-server.pmcp.run
```

## Authentication

### Login Process

The login command uses OAuth 2.0 with PKCE (Proof Key for Code Exchange) for secure authentication:

```bash
cargo pmcp deploy login --target pmcp-run
```

**What happens:**
1. Generates a PKCE challenge for enhanced security
2. Opens browser to AWS Cognito authorization page
3. After you authenticate, redirects to `http://localhost:8787` with auth code
4. Exchanges auth code for access token, refresh token, and ID token
5. Saves credentials to `~/.pmcp/credentials.toml`

**Credentials include:**
- `access_token` - Used for API requests (expires in 1 hour)
- `refresh_token` - Used to get new access tokens (long-lived)
- `id_token` - OpenID Connect identity token
- `expires_at` - ISO 8601 timestamp when access token expires

### Token Refresh

Access tokens expire after 1 hour. The CLI automatically refreshes them using the refresh token when needed.

If refresh fails, simply login again:

```bash
cargo pmcp deploy login --target pmcp-run
```

### Logout

Remove stored credentials:

```bash
cargo pmcp deploy logout --target pmcp-run
```

This removes the `pmcp-run` section from `~/.pmcp/credentials.toml`.

## Environment Variables

All environment variables are **optional** - defaults are configured for production pmcp.run.

### Authentication

#### `PMCP_RUN_COGNITO_DOMAIN`
- **Purpose**: AWS Cognito domain for OAuth authentication
- **Default**: `4f40d547593aca2fc5dd.auth.us-west-2.amazoncognito.com`
- **When to set**: Testing against staging/dev environment

```bash
export PMCP_RUN_COGNITO_DOMAIN="your-cognito-domain.auth.region.amazoncognito.com"
```

#### `PMCP_RUN_COGNITO_CLIENT_ID`
- **Purpose**: AWS Cognito client ID for OAuth
- **Default**: `3nbmeos20h8o3vsj0demc191et`
- **When to set**: Testing against staging/dev environment

```bash
export PMCP_RUN_COGNITO_CLIENT_ID="your-client-id"
```

### GraphQL API

#### `PMCP_RUN_GRAPHQL_URL`
- **Purpose**: GraphQL API endpoint for deployment operations
- **Default**: `https://noet4bfxcfdptmhw6tmirhtycm.appsync-api.us-west-2.amazonaws.com/graphql`
- **When to set**: Testing against staging/dev environment

```bash
export PMCP_RUN_GRAPHQL_URL="https://your-api.appsync-api.region.amazonaws.com/graphql"
```

## Deployment Commands

### Initialize Deployment Configuration

```bash
cargo pmcp deploy init --target pmcp-run
```

Creates deployment configuration files in `deploy/` directory.

### Deploy

```bash
cargo pmcp deploy --target pmcp-run
```

**What happens:**
1. ✅ Builds your MCP server for AWS Lambda (Linux ARM64)
2. ✅ Uploads binary to S3 via presigned URL
3. ✅ Triggers CDK deployment via GraphQL mutation
4. ✅ Returns deployment outputs (URL, ARN, etc.)

### View Deployment Outputs

```bash
cargo pmcp deploy outputs --target pmcp-run
```

Shows:
- Function URL
- Lambda ARN
- CloudWatch log group
- Deployment timestamp

### Destroy Deployment

```bash
cargo pmcp deploy destroy --target pmcp-run
```

Removes all AWS resources for your deployment.

### View Logs (Coming Soon)

```bash
cargo pmcp deploy logs --target pmcp-run
```

Currently redirects to https://pmcp.run/dashboard

## Architecture

pmcp.run deploys your MCP server using:

1. **AWS Lambda** - Runs your Rust binary
   - Runtime: Custom runtime (provided.al2023)
   - Architecture: ARM64 (Graviton2)
   - Memory: 256MB (configurable)
   - Timeout: 30s (configurable)

2. **Lambda Function URL** - Public HTTPS endpoint
   - CORS enabled
   - Streaming responses supported
   - No API Gateway needed

3. **AWS CDK** - Infrastructure as Code
   - Deployed via pmcp.run backend
   - Managed CloudFormation stacks
   - Automatic rollback on failure

4. **S3** - Binary storage
   - Presigned URL upload
   - Temporary storage during deployment
   - Automatic cleanup

## Troubleshooting

### Authentication Failed

**Symptom**: `Not authenticated with pmcp.run`

**Solution**: Login first
```bash
cargo pmcp deploy login --target pmcp-run
```

### Token Refresh Failed

**Symptom**: `Failed to refresh token - Server returned error response`

**Cause**: Refresh token expired or invalid

**Solution**: Login again
```bash
cargo pmcp deploy login --target pmcp-run
```

### Port 8787 Already in Use

**Symptom**: Callback server fails to start

**Solution**: Kill process using port 8787
```bash
lsof -ti:8787 | xargs kill -9
```

Or wait a few seconds and try again.

### Deployment Timeout

**Symptom**: Deployment takes too long

**Solution**: Check AWS CloudFormation console for stack status. Contact pmcp.run support if stack is stuck.

### Binary Too Large

**Symptom**: Upload fails or Lambda deployment fails

**Solution**:
1. Build in release mode: `cargo build --release`
2. Strip debug symbols: `strip target/release/your-server`
3. Check Lambda size limits (250MB unzipped, 50MB zipped)

## Security

### OAuth 2.0 with PKCE

pmcp.run uses OAuth 2.0 Authorization Code flow with PKCE:
- ✅ No client secret (public client)
- ✅ PKCE prevents authorization code interception
- ✅ Scopes: `openid`, `email`, `profile`
- ✅ Tokens stored locally in `~/.pmcp/credentials.toml`

### Credentials Storage

Credentials are stored in:
```
~/.pmcp/credentials.toml
```

**Format:**
```toml
[pmcp-run]
access_token = "..."
refresh_token = "..."
id_token = "..."
expires_at = "2024-11-24T12:00:00+00:00"
```

**Permissions**:
- File is created with user-only read/write permissions
- Never commit credentials to git
- Add `~/.pmcp/` to `.gitignore`

### GraphQL API

All GraphQL requests include:
- `Authorization: Bearer <access_token>` header
- Cognito validates JWT signature and claims
- API rejects expired or invalid tokens

## Examples

### Deploy with Custom Name

```bash
cargo pmcp deploy --target pmcp-run
```

By default, uses the server name from `Cargo.toml`.

### Check Prerequisites

```bash
cargo pmcp deploy init --target pmcp-run
```

Verifies:
- ✅ cargo-lambda installed
- ✅ aws-cdk installed
- ✅ pmcp.run authentication

### View Current Credentials

```bash
cat ~/.pmcp/credentials.toml
```

### Logout from All Targets

```bash
rm ~/.pmcp/credentials.toml
```

## Development

### Testing Against Staging

Set environment variables to point to staging environment:

```bash
export PMCP_RUN_COGNITO_DOMAIN="staging-cognito.auth.us-west-2.amazoncognito.com"
export PMCP_RUN_COGNITO_CLIENT_ID="staging-client-id"
export PMCP_RUN_GRAPHQL_URL="https://staging-api.appsync-api.us-west-2.amazonaws.com/graphql"

cargo pmcp deploy login --target pmcp-run
cargo pmcp deploy --target pmcp-run
```

### Running Your Own Instance

To run your own pmcp.run backend:

1. Deploy the pmcp.run infrastructure (AWS CDK stacks)
2. Configure Cognito user pool and app client
3. Deploy GraphQL API (AWS AppSync)
4. Set environment variables to your endpoints

See the pmcp.run repository for backend deployment instructions.

## Support

- **Documentation**: https://pmcp.run/docs
- **Dashboard**: https://pmcp.run/dashboard
- **Issues**: https://github.com/anthropics/pmcp/issues
- **Discord**: https://discord.gg/pmcp

## License

The pmcp.run deployment target is part of the cargo-pmcp tool, licensed under Apache 2.0 or MIT.
