# Connecting MCP Clients

After deploying your MCP server to AWS Lambda, you need to connect clients to it. This lesson covers connecting Claude Desktop, Claude.ai, and custom applications to your remote MCP server.

## Connection Overview

Remote MCP servers use HTTP transport instead of stdio:

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    MCP CLIENT CONNECTION FLOW                           │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  LOCAL SERVER (stdio)              REMOTE SERVER (HTTP)                 │
│                                                                         │
│  ┌─────────────┐                   ┌─────────────┐                      │
│  │ MCP Client  │                   │ MCP Client  │                      │
│  │             │                   │             │                      │
│  └──────┬──────┘                   └──────┬──────┘                      │
│         │                                 │                             │
│         │ stdin/stdout                    │ HTTPS                       │
│         │                                 │                             │
│         ▼                                 ▼                             │
│  ┌─────────────┐                   ┌─────────────┐                      │
│  │ Local       │                   │ API Gateway │                      │
│  │ Process     │                   │ + Lambda    │                      │
│  └─────────────┘                   └─────────────┘                      │
│                                                                         │
│  Config:                           Config:                              │
│  {                                 {                                    │
│    "command": "my-server"            "url": "https://...",              │
│  }                                   "transport": "streamable-http"     │
│                                    }                                    │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

## Getting Your Server URL

After deployment, get your MCP endpoint:

```bash
cargo pmcp deploy outputs

# Output:
# ┌────────────────────────────────────────────────────────────────────┐
# │                     Deployment Outputs                             │
# ├────────────────────────────────────────────────────────────────────┤
# │ ApiEndpoint:  https://abc123.execute-api.us-east-1.amazonaws.com   │
# │ McpEndpoint:  https://abc123.execute-api.us-east-1.amazonaws.com/mcp│
# │ OAuthUrl:     https://auth.abc123.amazoncognito.com                │
# │ ClientId:     1234567890abcdef                                     │
# └────────────────────────────────────────────────────────────────────┘
```

## Connecting Claude Desktop

### Without Authentication

For internal servers without OAuth (not recommended for production):

Edit `~/.config/claude/claude_desktop_config.json` (macOS/Linux) or `%APPDATA%\Claude\claude_desktop_config.json` (Windows):

```json
{
  "mcpServers": {
    "my-remote-server": {
      "transport": "streamable-http",
      "url": "https://abc123.execute-api.us-east-1.amazonaws.com/mcp"
    }
  }
}
```

### With OAuth Authentication

For production servers with Cognito authentication:

```json
{
  "mcpServers": {
    "my-remote-server": {
      "transport": "streamable-http",
      "url": "https://abc123.execute-api.us-east-1.amazonaws.com/mcp",
      "oauth": {
        "client_id": "1234567890abcdef",
        "authorization_url": "https://auth.abc123.amazoncognito.com/oauth2/authorize",
        "token_url": "https://auth.abc123.amazoncognito.com/oauth2/token",
        "scopes": ["openid", "mcp:read", "mcp:write"]
      }
    }
  }
}
```

When you start Claude Desktop:
1. It detects the OAuth configuration
2. Opens your browser to the Cognito login page
3. You authenticate (username/password or SSO)
4. Browser redirects back with authorization code
5. Claude Desktop exchanges code for access token
6. All MCP requests include the access token

### With API Key Authentication

For simpler authentication using API keys:

```json
{
  "mcpServers": {
    "my-remote-server": {
      "transport": "streamable-http",
      "url": "https://abc123.execute-api.us-east-1.amazonaws.com/mcp",
      "headers": {
        "Authorization": "Bearer your-api-key-here"
      }
    }
  }
}
```

**Security note**: Store API keys securely. Consider using environment variables:

```json
{
  "mcpServers": {
    "my-remote-server": {
      "transport": "streamable-http",
      "url": "https://abc123.execute-api.us-east-1.amazonaws.com/mcp",
      "headers": {
        "Authorization": "Bearer ${MCP_API_KEY}"
      }
    }
  }
}
```

Then set the environment variable before starting Claude Desktop:
```bash
export MCP_API_KEY="your-api-key-here"
open -a "Claude"
```

## Connecting Claude.ai (Web)

Claude.ai supports connecting to remote MCP servers through the Integrations settings.

### Step 1: Register Your Server

In Claude.ai settings, navigate to **Integrations** → **Add MCP Server**:

```
Server Name:  My Data Server
Server URL:   https://abc123.execute-api.us-east-1.amazonaws.com/mcp
Auth Type:    OAuth 2.0

OAuth Settings:
  Client ID:         1234567890abcdef
  Authorization URL: https://auth.abc123.amazoncognito.com/oauth2/authorize
  Token URL:         https://auth.abc123.amazoncognito.com/oauth2/token
  Scopes:            openid mcp:read mcp:write
```

### Step 2: Authorize

Click **Connect** to initiate the OAuth flow:
1. Redirects to your Cognito login page
2. Enter credentials or use SSO
3. Grant permission to Claude.ai
4. Redirected back to Claude.ai with connection established

### Step 3: Verify Connection

Start a new conversation and verify the server is connected:

```
You: What tools do you have available from my data server?

Claude: I have access to the following tools from "My Data Server":
- query_users: Search for users by name or email
- get_user_details: Get detailed information about a specific user
- list_departments: List all departments in the organization
```

## OAuth Flow Details

Understanding the OAuth flow helps debug connection issues:

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         OAUTH 2.0 FLOW                                  │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  1. USER INITIATES CONNECTION                                           │
│     Claude Desktop/Claude.ai detects OAuth config                       │
│                                                                         │
│  2. AUTHORIZATION REQUEST                                               │
│     Browser opens:                                                      │
│     https://auth.abc123.amazoncognito.com/oauth2/authorize              │
│       ?client_id=1234567890abcdef                                       │
│       &response_type=code                                               │
│       &redirect_uri=http://localhost:8765/callback                      │
│       &scope=openid%20mcp:read%20mcp:write                              │
│       &state=random_state_value                                         │
│                                                                         │
│  3. USER AUTHENTICATES                                                  │
│     - Username/password                                                 │
│     - Or federated SSO (Google, SAML, etc.)                             │
│                                                                         │
│  4. AUTHORIZATION CODE RETURNED                                         │
│     Browser redirects to:                                               │
│     http://localhost:8765/callback?code=AUTH_CODE&state=random_state    │
│                                                                         │
│  5. TOKEN EXCHANGE                                                      │
│     Client POSTs to token endpoint:                                     │
│     POST https://auth.abc123.amazoncognito.com/oauth2/token             │
│       grant_type=authorization_code                                     │
│       &code=AUTH_CODE                                                   │
│       &client_id=1234567890abcdef                                       │
│       &redirect_uri=http://localhost:8765/callback                      │
│                                                                         │
│     Response:                                                           │
│     {                                                                   │
│       "access_token": "eyJhbGciOi...",                                  │
│       "refresh_token": "eyJjdHki...",                                   │
│       "expires_in": 3600                                                │
│     }                                                                   │
│                                                                         │
│  6. MCP REQUESTS WITH TOKEN                                             │
│     POST https://abc123.execute-api.us-east-1.amazonaws.com/mcp         │
│     Authorization: Bearer eyJhbGciOi...                                 │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### Token Refresh

Access tokens expire (typically after 1 hour). Clients automatically refresh:

```
┌─────────────────────────────────────────────────────────────────────────┐
│                        TOKEN REFRESH FLOW                               │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  1. Access token expires (401 Unauthorized)                             │
│                                                                         │
│  2. Client uses refresh token:                                          │
│     POST https://auth.abc123.amazoncognito.com/oauth2/token             │
│       grant_type=refresh_token                                          │
│       &refresh_token=eyJjdHki...                                        │
│       &client_id=1234567890abcdef                                       │
│                                                                         │
│  3. New tokens returned                                                 │
│                                                                         │
│  4. Retry original request with new access token                        │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

## Cognito User Management

### Creating Users

Create users in the Cognito console or via CLI:

```bash
# Create a user
aws cognito-idp admin-create-user \
  --user-pool-id us-east-1_ABC123 \
  --username alice@company.com \
  --user-attributes Name=email,Value=alice@company.com \
  --temporary-password "TempPass123!"

# Set permanent password (skip temporary)
aws cognito-idp admin-set-user-password \
  --user-pool-id us-east-1_ABC123 \
  --username alice@company.com \
  --password "SecurePass456!" \
  --permanent
```

### Configuring Scopes

Define custom scopes in Cognito for fine-grained access:

```bash
# Create resource server with scopes
aws cognito-idp create-resource-server \
  --user-pool-id us-east-1_ABC123 \
  --identifier "mcp" \
  --name "MCP API" \
  --scopes ScopeName=read,ScopeDescription="Read access" \
          ScopeName=write,ScopeDescription="Write access" \
          ScopeName=admin,ScopeDescription="Admin access"
```

Update your app client to include scopes:

```bash
aws cognito-idp update-user-pool-client \
  --user-pool-id us-east-1_ABC123 \
  --client-id 1234567890abcdef \
  --allowed-oauth-scopes openid mcp/read mcp/write
```

### Federated Identity (SSO)

Connect Cognito to your identity provider:

```typescript
// In CDK stack
const userPool = new cognito.UserPool(this, 'McpUserPool', {
  // ...
});

// Add Google SSO
const googleProvider = new cognito.UserPoolIdentityProviderGoogle(
  this, 'Google',
  {
    userPool,
    clientId: 'google-client-id',
    clientSecretValue: SecretValue.secretsManager('google-client-secret'),
    scopes: ['email', 'profile'],
    attributeMapping: {
      email: cognito.ProviderAttribute.GOOGLE_EMAIL,
      fullname: cognito.ProviderAttribute.GOOGLE_NAME,
    },
  }
);

// Add SAML provider for enterprise SSO
const samlProvider = new cognito.UserPoolIdentityProviderSaml(
  this, 'Okta',
  {
    userPool,
    metadata: cognito.UserPoolIdentityProviderSamlMetadata.url(
      'https://company.okta.com/app/metadata'
    ),
    attributeMapping: {
      email: cognito.ProviderAttribute.other('email'),
    },
  }
);
```

## Custom MCP Clients

Build your own application that connects to the remote MCP server:

### Rust Client

```rust
use pmcp::client::{Client, HttpTransport};
use pmcp::types::CallToolParams;

#[tokio::main]
async fn main() -> Result<()> {
    // Create HTTP transport with OAuth token
    let transport = HttpTransport::new("https://abc123.execute-api.us-east-1.amazonaws.com/mcp")
        .with_bearer_token("eyJhbGciOi...")
        .build()?;

    // Connect to server
    let client = Client::connect(transport).await?;

    // Initialize
    let server_info = client.initialize().await?;
    println!("Connected to: {}", server_info.name);

    // List available tools
    let tools = client.list_tools().await?;
    for tool in &tools {
        println!("Tool: {} - {}", tool.name, tool.description.as_deref().unwrap_or(""));
    }

    // Call a tool
    let result = client.call_tool(CallToolParams {
        name: "query_users".to_string(),
        arguments: serde_json::json!({
            "department": "Engineering"
        }),
    }).await?;

    println!("Result: {}", serde_json::to_string_pretty(&result)?);

    Ok(())
}
```

### TypeScript/JavaScript Client

```typescript
import { Client, HttpTransport } from '@anthropic/mcp-sdk';

async function main() {
  // Create transport with authentication
  const transport = new HttpTransport({
    url: 'https://abc123.execute-api.us-east-1.amazonaws.com/mcp',
    headers: {
      'Authorization': `Bearer ${process.env.MCP_TOKEN}`,
    },
  });

  // Connect
  const client = new Client({ transport });
  await client.connect();

  // Initialize
  const serverInfo = await client.initialize({
    protocolVersion: '2024-11-05',
    capabilities: {},
    clientInfo: { name: 'my-app', version: '1.0.0' },
  });

  console.log(`Connected to: ${serverInfo.serverInfo.name}`);

  // List tools
  const tools = await client.listTools();
  console.log('Available tools:', tools.tools.map(t => t.name));

  // Call a tool
  const result = await client.callTool({
    name: 'query_users',
    arguments: { department: 'Engineering' },
  });

  console.log('Result:', result);
}

main().catch(console.error);
```

### Python Client

```python
import asyncio
from mcp import Client, HttpTransport

async def main():
    # Create transport with authentication
    transport = HttpTransport(
        url="https://abc123.execute-api.us-east-1.amazonaws.com/mcp",
        headers={"Authorization": f"Bearer {os.environ['MCP_TOKEN']}"}
    )

    # Connect
    async with Client(transport) as client:
        # Initialize
        server_info = await client.initialize()
        print(f"Connected to: {server_info.name}")

        # List tools
        tools = await client.list_tools()
        print(f"Available tools: {[t.name for t in tools]}")

        # Call a tool
        result = await client.call_tool(
            name="query_users",
            arguments={"department": "Engineering"}
        )
        print(f"Result: {result}")

asyncio.run(main())
```

## Testing the Connection

### Using curl

Test your endpoint directly:

```bash
# Initialize
curl -X POST https://abc123.execute-api.us-east-1.amazonaws.com/mcp \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -d '{
    "jsonrpc": "2.0",
    "method": "initialize",
    "params": {
      "protocolVersion": "2024-11-05",
      "capabilities": {},
      "clientInfo": {"name": "curl", "version": "1.0"}
    },
    "id": 1
  }'

# List tools
curl -X POST https://abc123.execute-api.us-east-1.amazonaws.com/mcp \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -d '{
    "jsonrpc": "2.0",
    "method": "tools/list",
    "params": {},
    "id": 2
  }'

# Call a tool
curl -X POST https://abc123.execute-api.us-east-1.amazonaws.com/mcp \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -d '{
    "jsonrpc": "2.0",
    "method": "tools/call",
    "params": {
      "name": "query_users",
      "arguments": {"department": "Engineering"}
    },
    "id": 3
  }'
```

### Using cargo pmcp deploy test

PMCP provides a built-in test command:

```bash
# Run integration tests against deployed server
cargo pmcp deploy test

# Output:
# Testing connection to https://abc123.execute-api.us-east-1.amazonaws.com/mcp
# ✓ Initialize: 45ms
# ✓ List Tools: 23ms (found 5 tools)
# ✓ Call 'query_users': 156ms
# ✓ Call 'get_user_details': 89ms
#
# All tests passed!
```

## Troubleshooting

### "401 Unauthorized"

**Cause**: Invalid or expired token.

**Solution**:
1. Check token is included in Authorization header
2. Verify token hasn't expired
3. Re-authenticate to get fresh token

### "403 Forbidden"

**Cause**: Token valid but missing required scopes.

**Solution**:
1. Check Cognito app client has required scopes
2. Ensure user has permission for requested scopes
3. Re-authorize with correct scope request

### "CORS Error" (Browser)

**Cause**: API Gateway CORS not configured for your origin.

**Solution**: Update CDK to allow your origin:
```typescript
corsPreflight: {
  allowOrigins: ['https://your-app.com'],
  // ...
}
```

### "Connection Timeout"

**Cause**: Lambda in VPC without NAT gateway, or cold start too slow.

**Solution**:
1. Ensure VPC has NAT gateway for outbound traffic
2. Check Lambda timeout is sufficient
3. Consider provisioned concurrency

### "Invalid Redirect URI"

**Cause**: Callback URL doesn't match Cognito configuration.

**Solution**: Add the redirect URI to Cognito app client:
```bash
aws cognito-idp update-user-pool-client \
  --user-pool-id us-east-1_ABC123 \
  --client-id 1234567890abcdef \
  --callback-urls "http://localhost:8765/callback" "https://claude.ai/callback"
```

## Summary

Connecting clients to your remote MCP server:

1. **Get your endpoint URL**: `cargo pmcp deploy outputs`
2. **Configure authentication**: OAuth (recommended) or API keys
3. **Set up client configuration**: Claude Desktop config or Claude.ai integration
4. **Test the connection**: curl, built-in test, or your application

Key configuration patterns:

```json
// Claude Desktop with OAuth
{
  "mcpServers": {
    "my-server": {
      "transport": "streamable-http",
      "url": "https://abc123.execute-api.us-east-1.amazonaws.com/mcp",
      "oauth": {
        "client_id": "...",
        "authorization_url": "...",
        "token_url": "...",
        "scopes": ["openid", "mcp:read"]
      }
    }
  }
}
```

Your MCP server is now accessible to anyone with proper credentials, from anywhere in the world.
