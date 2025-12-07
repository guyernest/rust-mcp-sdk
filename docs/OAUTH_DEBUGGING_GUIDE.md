# OAuth Debugging Guide for pmcp.run MCP Servers

This guide explains how to use the `mcp-tester` tool to debug OAuth authentication issues with MCP servers deployed to pmcp.run.

## Prerequisites

1. **Build the mcp-tester tool:**
   ```bash
   cd ~/Development/mcp/sdk/rust-mcp-sdk
   cargo build --release -p mcp-server-tester
   ```

2. **Ensure you have an OAuth client ID** from your Cognito User Pool (created during `cargo pmcp deploy --oauth`)

## Quick Start

### Test OAuth-Protected Server (Auto-Discovery)

The mcp-tester can automatically discover OAuth endpoints from your server:

```bash
# The tool auto-discovers OAuth configuration from the server
./target/release/mcp-tester quick https://api.pmcp.run/chess/mcp \
  --oauth-client-id YOUR_CLIENT_ID
```

This will:
1. Discover OAuth endpoints from `https://api.pmcp.run/chess/.well-known/openid-configuration`
2. Open a browser for Cognito login
3. Cache the token for future requests
4. Test the server connection

### Test with Explicit OAuth Issuer

If auto-discovery fails, specify the issuer explicitly:

```bash
./target/release/mcp-tester quick https://api.pmcp.run/chess/mcp \
  --oauth-client-id YOUR_CLIENT_ID \
  --oauth-issuer https://cognito-idp.us-east-1.amazonaws.com/YOUR_USER_POOL_ID
```

## Authentication Flows

### 1. Authorization Code Flow with PKCE (Default)

The tool uses the Authorization Code Flow with PKCE, which:
- Opens a browser for user login
- Starts a local callback server on `http://localhost:8080/callback`
- Exchanges the authorization code for tokens

```bash
# Use a different callback port if 8080 is in use
./target/release/mcp-tester quick https://api.pmcp.run/chess/mcp \
  --oauth-client-id YOUR_CLIENT_ID \
  --oauth-redirect-port 3000
```

**Important**: The redirect URI (`http://localhost:8080/callback`) must be registered in your Cognito User Pool App Client settings.

### 2. Device Code Flow (Fallback)

If the authorization code flow fails, the tool automatically falls back to device code flow (if supported by the server).

## Common Commands

### Quick Connectivity Test
```bash
./target/release/mcp-tester quick https://api.pmcp.run/chess/mcp \
  --oauth-client-id YOUR_CLIENT_ID
```

### Full Test Suite
```bash
./target/release/mcp-tester test https://api.pmcp.run/chess/mcp \
  --oauth-client-id YOUR_CLIENT_ID \
  --with-tools
```

### List Available Tools
```bash
./target/release/mcp-tester tools https://api.pmcp.run/chess/mcp \
  --oauth-client-id YOUR_CLIENT_ID
```

### Test a Specific Tool
```bash
./target/release/mcp-tester test https://api.pmcp.run/chess/mcp \
  --oauth-client-id YOUR_CLIENT_ID \
  --tool make_move \
  --args '{"move": "e2e4"}'
```

### Health Check
```bash
./target/release/mcp-tester health https://api.pmcp.run/chess/mcp \
  --oauth-client-id YOUR_CLIENT_ID
```

### Connection Diagnostics
```bash
./target/release/mcp-tester diagnose https://api.pmcp.run/chess/mcp \
  --network
```

## Token Caching

By default, tokens are cached at `~/.mcp-tester/tokens.json`. To disable caching:

```bash
./target/release/mcp-tester quick https://api.pmcp.run/chess/mcp \
  --oauth-client-id YOUR_CLIENT_ID \
  --oauth-no-cache
```

To clear the cache manually:
```bash
rm ~/.mcp-tester/tokens.json
```

## Environment Variables

You can set OAuth configuration via environment variables:

```bash
export MCP_OAUTH_CLIENT_ID="your-client-id"
export MCP_OAUTH_ISSUER="https://cognito-idp.us-east-1.amazonaws.com/us-east-1_xxxxx"
export MCP_OAUTH_SCOPES="openid,profile,email"
export MCP_OAUTH_REDIRECT_PORT=8080

# Then run without flags
./target/release/mcp-tester quick https://api.pmcp.run/chess/mcp
```

## OAuth Scopes

Default scope is `openid`. Add additional scopes if needed:

```bash
./target/release/mcp-tester quick https://api.pmcp.run/chess/mcp \
  --oauth-client-id YOUR_CLIENT_ID \
  --oauth-scopes openid,mcp:read,mcp:write
```

## Debugging Tips

### 1. Check OAuth Discovery

First verify the OAuth discovery endpoint is working:

```bash
curl https://api.pmcp.run/chess/.well-known/openid-configuration | jq
```

Expected response:
```json
{
  "issuer": "https://cognito-idp.us-east-1.amazonaws.com/us-east-1_xxxxx",
  "authorization_endpoint": "https://xxx.auth.us-east-1.amazoncognito.com/oauth2/authorize",
  "token_endpoint": "https://xxx.auth.us-east-1.amazoncognito.com/oauth2/token",
  ...
}
```

### 2. Verbose Mode

Enable verbose logging to see detailed OAuth flow:

```bash
./target/release/mcp-tester quick https://api.pmcp.run/chess/mcp \
  --oauth-client-id YOUR_CLIENT_ID \
  -v    # verbosity level 1
```

Or with maximum verbosity:
```bash
RUST_LOG=debug ./target/release/mcp-tester quick https://api.pmcp.run/chess/mcp \
  --oauth-client-id YOUR_CLIENT_ID
```

### 3. Check Token Manually

After authentication, inspect the cached token:

```bash
cat ~/.mcp-tester/tokens.json | jq
```

### 4. Test Token Validity

Make a manual request with the token:

```bash
TOKEN=$(cat ~/.mcp-tester/tokens.json | jq -r '.access_token')
curl -H "Authorization: Bearer $TOKEN" https://api.pmcp.run/chess/mcp
```

### 5. Common Errors

| Error | Cause | Solution |
|-------|-------|----------|
| `Failed to bind to localhost:8080` | Port in use | Use `--oauth-redirect-port 3000` |
| `redirect_uri_mismatch` | Callback not registered | Add `http://localhost:8080/callback` to Cognito App Client |
| `invalid_client` | Wrong client ID | Check Client ID in Cognito console |
| `OAuth discovery failed` | Server not OAuth-enabled | Run `cargo pmcp oauth enable <server-id>` |

## Output Formats

### Pretty (Default)
```bash
./target/release/mcp-tester quick https://api.pmcp.run/chess/mcp \
  --oauth-client-id YOUR_CLIENT_ID \
  --format pretty
```

### JSON (for CI/CD)
```bash
./target/release/mcp-tester quick https://api.pmcp.run/chess/mcp \
  --oauth-client-id YOUR_CLIENT_ID \
  --format json
```

### Minimal
```bash
./target/release/mcp-tester quick https://api.pmcp.run/chess/mcp \
  --oauth-client-id YOUR_CLIENT_ID \
  --format minimal
```

## Finding Your OAuth Client ID

After deploying with OAuth, the client ID is stored in the deployment outputs:

```bash
# From deployment output
cargo pmcp deploy --target pmcp-run --oauth

# Or check the Cognito console:
# AWS Console > Cognito > User Pools > [Your Pool] > App clients
```

## Example: Full Debugging Session

```bash
# 1. Build the tester
cd ~/Development/mcp/sdk/rust-mcp-sdk
cargo build --release -p mcp-server-tester

# 2. Check if server is deployed
curl -I https://api.pmcp.run/chess/health

# 3. Check OAuth discovery
curl https://api.pmcp.run/chess/.well-known/openid-configuration | jq

# 4. Run quick test (this will prompt for login)
./target/release/mcp-tester quick https://api.pmcp.run/chess/mcp \
  --oauth-client-id 1abc2def3ghi4jkl5mno6pqr7s

# 5. If successful, run full test suite
./target/release/mcp-tester test https://api.pmcp.run/chess/mcp \
  --oauth-client-id 1abc2def3ghi4jkl5mno6pqr7s \
  --with-tools

# 6. Test specific tool
./target/release/mcp-tester test https://api.pmcp.run/chess/mcp \
  --oauth-client-id 1abc2def3ghi4jkl5mno6pqr7s \
  --tool get_board
```

## See Also

- [pmcp.run OAuth Documentation](../pmcp-run/docs/OAUTH_SDK_INTEGRATION_ANSWERS.md)
- [cargo-pmcp OAuth Commands](../cargo-pmcp/docs/COMMANDS.md)
