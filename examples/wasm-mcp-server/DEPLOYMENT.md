# Production Deployment Guide

**Target**: Cloudflare Workers
**Architecture**: Layered Security with Separation of Concerns

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────┐
│                  Internet/Clients                    │
└──────────────────────┬──────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────┐
│            Cloudflare Edge (CDN/WAF)                 │
│  - CORS Policy                                       │
│  - Request Size Limits                               │
│  - Rate Limiting                                     │
│  - DDoS Protection                                   │
│  - WAF Rules                                         │
└──────────────────────┬──────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────┐
│        Optional: API Gateway / Auth Layer            │
│  - Token Validation (JWT, OAuth)                    │
│  - Authentication                                    │
│  - Authorization                                     │
│  - Request Logging                                   │
└──────────────────────┬──────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────┐
│              MCP Server (This Example)               │
│  - Input Validation (TypedTool)                     │
│  - Business Logic                                    │
│  - Type Safety                                       │
│  - Clear Error Messages                              │
└─────────────────────────────────────────────────────┘
```

## Design Philosophy: Separation of Concerns

### Infrastructure Layer (Cloudflare/API Gateway)
**Responsibility**: Network-level security and resource protection

- CORS policy
- Request size limits
- Rate limiting
- DDoS protection
- Authentication tokens
- WAF rules

**Why here**: These are cross-cutting concerns that apply to ALL requests before they reach your application.

### Application Layer (MCP Server)
**Responsibility**: Business logic and data validation

- Input validation via TypedTool
- Business rule enforcement
- Type safety with Rust structs
- Clear, helpful error messages for MCP clients

**Why here**: The MCP server understands your domain logic, required parameters, and valid ranges for your specific tools.

---

## Production Deployment Checklist

### 1. Cloudflare Configuration (Infrastructure)

#### CORS Configuration
Configure in Cloudflare Dashboard → Workers → Settings:

**Development**:
```
Access-Control-Allow-Origin: *
```

**Production**:
```
Access-Control-Allow-Origin: https://your-app.com, https://app.company.com
Access-Control-Allow-Methods: GET, POST, OPTIONS
Access-Control-Allow-Headers: Content-Type
Access-Control-Max-Age: 86400
```

**Alternative**: Use Cloudflare Workers KV or Durable Objects to store allowed origins dynamically.

#### Request Size Limits
Configure in Cloudflare Dashboard → Workers → Settings → Limits:

- **Maximum Request Size**: 1-5 MB (default for Workers)
- **Memory**: 128 MB (default)
- **CPU Time**: 50ms for free tier, 50-30000ms for paid

**Recommendation**: 1MB is sufficient for MCP JSON-RPC requests.

#### Rate Limiting
Configure in Cloudflare Dashboard → Security → Rate Limiting:

**Example Rule**:
```
Name: MCP Server Rate Limit
Path: /* (or /mcp/*)
Rate: 100 requests per minute
Period: 1 minute
Action: Block
Match by: IP Address
```

**Recommended Limits**:
- **Per IP**: 100-500 requests/minute
- **Global**: Monitor and adjust based on usage
- **Burst**: Allow 10-20 requests/second for normal clients

#### WAF (Web Application Firewall)
Enable Cloudflare WAF → Managed Rules:

- ✅ OWASP Core Ruleset
- ✅ Cloudflare Managed Ruleset
- ✅ Block common attacks (SQL injection, XSS, etc.)

**Note**: MCP servers use JSON-RPC, so most injection attacks won't apply, but WAF provides defense in depth.

---

### 2. MCP Server Configuration (Application)

#### ✅ Input Validation (Already Implemented)

The example already demonstrates proper MCP server validation:

```rust
// ✅ TypedTool with proper validation
.tool(
    "calculator",
    SimpleTool::new(
        "calculator",
        "Perform arithmetic calculations",
        |args: Value| {
            // Validate required parameters
            let operation = args.get("operation")
                .and_then(|v| v.as_str())
                .ok_or_else(|| pmcp::Error::protocol(
                    pmcp::ErrorCode::INVALID_PARAMS,
                    "operation is required"  // ✅ Clear message
                ))?;

            let a = args.get("a")
                .and_then(|v| v.as_f64())
                .ok_or_else(|| pmcp::Error::protocol(
                    pmcp::ErrorCode::INVALID_PARAMS,
                    "parameter 'a' is required"  // ✅ Helpful
                ))?;

            // Business logic validation
            if operation == "divide" && b == 0.0 {
                return Err(pmcp::Error::protocol(
                    pmcp::ErrorCode::INVALID_PARAMS,
                    "Division by zero"  // ✅ Specific error
                ));
            }

            // ... rest of implementation
        }
    )
)
```

**Key Points**:
- ✅ **Clear error messages**: Help MCP clients understand what went wrong
- ✅ **Type validation**: Rust's type system ensures correctness
- ✅ **Business logic**: Domain-specific rules (e.g., no division by zero)
- ✅ **Protocol compliance**: Uses standard MCP error codes

#### Using Structured Validation (Recommended)

For complex tools, use Rust structs with validation:

```rust
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Deserialize, Validate)]
struct CalculatorInput {
    #[validate(length(min = 1))]
    operation: String,

    #[validate(range(min = -1000000.0, max = 1000000.0))]
    a: f64,

    #[validate(range(min = -1000000.0, max = 1000000.0))]
    b: f64,
}

// In tool handler:
let input: CalculatorInput = serde_json::from_value(args)
    .map_err(|e| pmcp::Error::validation(
        format!("Invalid input: {}", e)  // ✅ Still helpful!
    ))?;

input.validate()
    .map_err(|e| pmcp::Error::validation(
        format!("Validation failed: {}", e)  // ✅ Shows what failed
    ))?;
```

**Benefits**:
- Declarative validation
- Reusable across tools
- Clear error messages maintained
- Type safety

---

### 3. Optional: Authentication Layer

**When to Add Authentication**:
- Handling sensitive data
- Executing privileged operations
- Cost control (prevent abuse)
- Compliance requirements

**Where to Implement**: API Gateway, NOT in MCP server

#### Option A: Cloudflare Access (Recommended)

Use Cloudflare Access for authentication:

1. **Configure** in Cloudflare Dashboard → Zero Trust → Access
2. **Create Policy**: Require email, OAuth, or SAML
3. **Apply to Worker**: Protect the Worker route

**Benefit**: No code changes needed in MCP server.

#### Option B: API Gateway with Token Validation

Use AWS API Gateway, Google Cloud Endpoints, or similar:

```
Client → API Gateway (validates JWT) → Cloudflare Worker → MCP Server
```

**API Gateway handles**:
- Token validation (JWT, OAuth2)
- Rate limiting per user
- Usage plans and quotas
- Request logging

**MCP Server receives**: Validated requests only

#### Option C: Custom Token Validation (If Needed)

If you must validate tokens in the Worker:

```rust
// In Worker, BEFORE calling MCP server
async fn validate_token(req: &Request, env: &Env) -> Result<bool> {
    let auth_header = req.headers().get("Authorization")?;

    match auth_header {
        Some(h) if h.starts_with("Bearer ") => {
            let token = &h[7..];

            // Validate against environment variable or KV store
            let valid_tokens = env.var("VALID_TOKENS")?;
            Ok(valid_tokens.to_string().split(',').any(|t| t == token))
        }
        _ => Ok(false),
    }
}

// In main handler:
if !validate_token(&req, &env).await? {
    return Response::error("Unauthorized", 401);
}

// Now process with MCP server
let server = create_mcp_server();
// ...
```

**Important**: Keep auth logic in the Worker wrapper, NOT in the MCP server business logic.

---

## Deployment Steps

### Step 1: Development Testing

```bash
# Install wrangler
npm install -g wrangler

# Login to Cloudflare
wrangler login

# Run locally
wrangler dev

# Test the endpoints
curl http://localhost:8787/
curl -X POST http://localhost:8787/ -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0.0"}}}'
```

### Step 2: Cloudflare Configuration

1. **Create Worker** in Cloudflare Dashboard
2. **Configure CORS**: Add allowed origins
3. **Set Rate Limits**: 100-500 req/min per IP
4. **Enable WAF**: Turn on managed rulesets
5. **Set Environment Variables** (if using token auth):
   ```bash
   wrangler secret put VALID_TOKENS
   ```

### Step 3: Deploy

```bash
# Deploy to Cloudflare
wrangler publish

# Get Worker URL
# Example: https://mcp-server.your-account.workers.dev
```

### Step 4: Test Production

```bash
# Test from allowed origin (use browser with CORS or curl)
curl -H "Origin: https://your-app.com" \
     -X POST \
     https://mcp-server.your-account.workers.dev/ \
     -d '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}'

# Should return tool list

# Test from unknown origin (should fail if CORS configured)
curl -H "Origin: https://evil-site.com" \
     -X POST \
     https://mcp-server.your-account.workers.dev/ \
     -d '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}'

# Should return 403 Forbidden
```

### Step 5: Monitor

**Cloudflare Dashboard → Analytics**:
- Request volume
- Error rates
- Latency (p50, p95, p99)
- Rate limit hits

**Set up Alerts**:
- High error rate (>5%)
- Excessive rate limit hits
- Unusual traffic patterns
- CPU time approaching limits

---

## Common Deployment Patterns

### Pattern 1: Public MCP Server (Demo/OSS)

**Use Case**: Public APIs, documentation, open-source tools

**Configuration**:
- CORS: `*` (allow all origins)
- Rate Limiting: 100 req/min per IP
- Auth: None (public)
- WAF: Enabled

**Example**: Calculator demo, weather tool, public utilities

### Pattern 2: Internal MCP Server (Company Tools)

**Use Case**: Internal tools, company integrations

**Configuration**:
- CORS: `https://app.company.com` (specific origins)
- Rate Limiting: 500 req/min per IP
- Auth: Cloudflare Access with company SSO
- WAF: Enabled

**Example**: Internal data tools, workflow automation

### Pattern 3: Customer-Facing MCP Server (SaaS)

**Use Case**: Multi-tenant SaaS applications

**Configuration**:
- CORS: Dynamic per customer (KV store)
- Rate Limiting: 1000 req/min per customer
- Auth: JWT tokens validated by API Gateway
- WAF: Enabled with custom rules

**Example**: SaaS platform features exposed as MCP tools

---

## Error Message Strategy

### ✅ DO: Provide Clear, Helpful Errors

```rust
// ✅ Good - Helps client fix the issue
return Err(pmcp::Error::protocol(
    pmcp::ErrorCode::INVALID_PARAMS,
    "parameter 'a' must be a number between -1,000,000 and 1,000,000"
));

// ✅ Good - Explains what went wrong
return Err(pmcp::Error::protocol(
    pmcp::ErrorCode::INVALID_PARAMS,
    &format!("Unknown operation '{}'. Valid operations: add, subtract, multiply, divide", operation)
));
```

### ❌ DON'T: Expose Internal Implementation

```rust
// ❌ Bad - Shows internal structure
return Err(pmcp::Error::internal(
    "Failed to connect to database at postgres://internal-db:5432"
));

// ✅ Better - Generic but actionable
return Err(pmcp::Error::internal(
    "Service temporarily unavailable. Please try again."
));
```

### Balance: Security vs Usability

**Rule of Thumb**:
- **Validation errors**: Be specific (helps clients fix requests)
- **Internal errors**: Be generic (don't expose infrastructure)
- **Business logic**: Be clear (helps clients understand your domain)

**Example**:
```rust
match some_operation() {
    // ✅ Validation: Specific and helpful
    Err(ValidationError::OutOfRange) =>
        Err(pmcp::Error::protocol(
            ErrorCode::INVALID_PARAMS,
            "Value must be between 0 and 100"
        )),

    // ✅ Internal: Generic, no details
    Err(InternalError::DatabaseDown) =>
        Err(pmcp::Error::internal(
            "Service temporarily unavailable"
        )),

    // ✅ Business logic: Clear domain explanation
    Err(BusinessError::InsufficientFunds) =>
        Err(pmcp::Error::protocol(
            ErrorCode::INVALID_PARAMS,
            "Account balance insufficient for this operation"
        )),
}
```

---

## Advanced: Multi-Region Deployment

For high availability and low latency:

### Global Deployment with Cloudflare Workers

```bash
# Workers automatically deploy globally
wrangler publish
# ✅ Deployed to 300+ Cloudflare edge locations worldwide
```

**Benefits**:
- Low latency (< 50ms globally)
- High availability (99.99%+)
- Automatic failover
- No regional configuration needed

### Monitoring Multiple Regions

Use Cloudflare Analytics to monitor:
- Traffic distribution by country
- Latency by region
- Error rates by location

---

## Cost Optimization

### Cloudflare Workers Pricing (2024)

**Free Tier**:
- 100,000 requests/day
- 10ms CPU time per request
- Sufficient for development and small deployments

**Paid Tier** ($5/month):
- 10M requests/month included
- $0.50 per additional million
- 50ms CPU time per request

**Cost Examples**:
- **1M requests/month**: $5/month (included in paid tier)
- **10M requests/month**: $5/month (included)
- **100M requests/month**: $50/month ($5 base + $45 for 90M extra)

**Tips**:
- Cache GET requests when possible
- Optimize tool execution time
- Use KV for frequently accessed data
- Monitor CPU time usage

---

## Troubleshooting

### Issue: CORS Errors

**Symptom**: Browser shows "CORS policy blocked"

**Solution**:
1. Check Cloudflare CORS configuration
2. Verify `Access-Control-Allow-Origin` matches exactly
3. Ensure OPTIONS requests return 200
4. Check `Access-Control-Allow-Headers` includes `Content-Type`

### Issue: Rate Limit Hit

**Symptom**: 429 Too Many Requests

**Solution**:
1. Review rate limit configuration in Cloudflare
2. Implement exponential backoff in client
3. Consider increasing limits for production
4. Use per-user rate limiting if using auth

### Issue: Request Too Large

**Symptom**: 413 Request Entity Too Large

**Solution**:
1. Check request size (should be < 100KB for typical MCP requests)
2. Increase Cloudflare Worker size limit if justified
3. Consider pagination for large data operations
4. Optimize JSON payloads

### Issue: Worker CPU Time Exceeded

**Symptom**: Worker timeout errors

**Solution**:
1. Optimize tool implementation (avoid heavy computation)
2. Use async operations where possible
3. Consider upgrading to paid tier (50ms → 30s CPU time)
4. Move heavy processing to external services

---

## Best Practices Summary

### Infrastructure (Cloudflare)
- ✅ Configure CORS for your origins
- ✅ Set appropriate rate limits
- ✅ Enable WAF protection
- ✅ Monitor analytics and set alerts
- ✅ Use Cloudflare Access for auth when needed

### Application (MCP Server)
- ✅ Use TypedTool for input validation
- ✅ Provide clear, helpful error messages
- ✅ Validate business logic rules
- ✅ Keep MCP server focused on domain logic
- ✅ Don't implement infrastructure concerns in app code

### Development
- ✅ Test locally with `wrangler dev`
- ✅ Use staging environment before production
- ✅ Monitor errors and latency
- ✅ Implement logging for debugging
- ✅ Document your tools with good schemas

---

## Additional Resources

- [Cloudflare Workers Documentation](https://developers.cloudflare.com/workers/)
- [MCP Protocol Specification](https://modelcontextprotocol.io/)
- [Wrangler CLI Documentation](https://developers.cloudflare.com/workers/wrangler/)
- [Cloudflare Rate Limiting](https://developers.cloudflare.com/waf/rate-limiting-rules/)
- [Cloudflare Access](https://developers.cloudflare.com/cloudflare-one/applications/)

---

**Last Updated**: 2025-11-21
**Feedback**: Issues or suggestions? File an issue in the PMCP SDK repository.
