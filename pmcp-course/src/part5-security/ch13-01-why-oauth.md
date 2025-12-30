# Why OAuth, Not API Keys

Many developers reach for API keys as the first authentication mechanism. They're simple, familiar, and work immediately. But for enterprise MCP servers, API keys create serious security and operational problems that OAuth 2.0 solves elegantly.

## The API Key Trap

### How API Keys Typically Work

```bash
# Developer creates an API key in a dashboard
# Key: sk_live_abc123def456...

# Client includes it in every request
curl -H "X-API-Key: sk_live_abc123def456" \
  https://mcp-server.example.com/mcp
```

This seems simple and effective. What could go wrong?

### Problem 1: No User Identity

```
┌─────────────────────────────────────────────────────────────────────┐
│                    API Key Authentication                            │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  Request 1:                                                         │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │  X-API-Key: sk_live_abc123                                   │   │
│  │  Tool: delete_customer                                       │   │
│  │  Args: { "id": "cust_789" }                                  │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  Who made this request?                                             │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │  ❓ Could be Alice from accounting                           │   │
│  │  ❓ Could be Bob from engineering                            │   │
│  │  ❓ Could be an attacker who found the key                   │   │
│  │  ❓ Could be an automated system                             │   │
│  │                                                              │   │
│  │  Answer: We have no idea                                     │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  Audit log:                                                         │
│  "Customer cust_789 deleted by... someone with API key abc123"      │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

When something goes wrong, you can't answer "who did it?" The API key identifies the application, not the person.

### Problem 2: No Granular Permissions

```rust
// With API keys, you typically have two options:

// Option 1: Full access
if request.api_key == valid_key {
    // User can do EVERYTHING
    allow_all_operations();
}

// Option 2: Separate keys per feature (unmanageable)
let read_key = "sk_read_abc123";
let write_key = "sk_write_def456";
let admin_key = "sk_admin_ghi789";
// Now you need to manage 3x the keys...
// And what about per-resource permissions?
```

Real enterprise scenarios require:
- User A can read customer data but not modify it
- User B can modify their own team's data
- User C has admin access but only during business hours
- User D can access everything except financial records

API keys can't express these nuances.

### Problem 3: Key Rotation is Painful

```
┌─────────────────────────────────────────────────────────────────────┐
│                    API Key Rotation Nightmare                        │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  Day 0: Key potentially compromised                                 │
│                                                                     │
│  Day 1-7: Security team investigates                                │
│                                                                     │
│  Day 8: Decision to rotate key                                      │
│                                                                     │
│  Day 9-14: Find all places using the key                            │
│    • Production server configs                                      │
│    • CI/CD pipelines                                                │
│    • Developer machines                                             │
│    • Third-party integrations                                       │
│    • Mobile apps (oh no, need app store update)                     │
│    • Partner systems (need to coordinate)                           │
│                                                                     │
│  Day 15-30: Coordinate the change                                   │
│    • Update all systems simultaneously                              │
│    • Some systems break anyway                                      │
│    • Rollback, fix, retry                                           │
│                                                                     │
│  Day 31: Finally rotated                                            │
│    • Attacker had access for a full month                           │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Problem 4: Keys Leak Easily

```bash
# Leakage vectors for API keys:

# 1. Git history (most common)
git log --all -p | grep "sk_live_"

# 2. Error logs
[ERROR] Failed to connect: auth failed with key sk_live_abc123

# 3. Browser developer tools
fetch('/api/data', { headers: { 'X-API-Key': 'sk_live_abc123' }})

# 4. Shared documentation
curl -H "X-API-Key: sk_live_abc123" https://...  # "Replace with your key"

# 5. Environment variable dumps
env | grep API  # Often logged during debugging

# 6. Configuration backups
cat /backup/2024/config.json | grep key
```

GitHub continuously scans for leaked API keys. They find millions every year.

### Problem 5: No Federation

```
┌─────────────────────────────────────────────────────────────────────┐
│               Enterprise Identity Architecture                       │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  What enterprises have:                                             │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │  Active Directory / Entra ID / Okta / etc.                   │   │
│  │  • Single source of truth for users                         │   │
│  │  • Group memberships                                         │   │
│  │  • Role assignments                                          │   │
│  │  • Automatic deprovisioning when employees leave             │   │
│  │  • Compliance and audit requirements                         │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  What API keys need:                                                │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │  Separate key management                                     │   │
│  │  • Manual provisioning                                       │   │
│  │  • Manual deprovisioning (often forgotten!)                  │   │
│  │  • No connection to corporate identity                       │   │
│  │  • Separate audit trail                                      │   │
│  │  • Yet another credential to manage                          │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  Result: Former employees still have valid API keys                │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

## OAuth 2.0: The Enterprise Solution

OAuth 2.0 addresses every API key problem:

### User Identity

```json
// JWT token payload
{
  "sub": "auth0|user123",
  "email": "alice@company.com",
  "name": "Alice Smith",
  "groups": ["engineering", "data-team"],
  "roles": ["developer", "data-analyst"],
  "iat": 1699996399,
  "exp": 1700000000
}
```

Every request is tied to a specific user. Audit logs show exactly who did what.

### Granular Permissions (Scopes)

```json
{
  "scope": "read:customers write:own-data admin:reports"
}
```

Scopes define exactly what operations a user can perform. Different users get different scopes based on their role.

### Automatic Token Rotation

```
┌─────────────────────────────────────────────────────────────────────┐
│                    OAuth Token Lifecycle                             │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  Access Token                                                       │
│  ├─ Lifetime: 1 hour (typical)                                     │
│  ├─ Used for API requests                                          │
│  └─ Automatically expires                                          │
│                                                                     │
│  Refresh Token                                                      │
│  ├─ Lifetime: 30 days (typical)                                    │
│  ├─ Used to get new access tokens                                  │
│  └─ Can be revoked immediately                                     │
│                                                                     │
│  Key rotation happens automatically:                                │
│  • Signing keys rotate on the IdP                                  │
│  • Clients get new tokens transparently                            │
│  • No coordinated deployment needed                                │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### Harder to Leak (and Easier to Recover)

```
Why OAuth tokens are safer:

1. Short-lived
   - Access tokens expire in ~1 hour
   - Even if leaked, damage is limited

2. Bound to specific client
   - Tokens include client_id
   - Can't be used from other applications

3. Revocable
   - Revoke user's refresh token
   - All their sessions end immediately

4. Not stored in code
   - Tokens are obtained at runtime
   - Never committed to git

5. Automatic refresh
   - No reason to store long-lived credentials
```

### Full Federation

```
┌─────────────────────────────────────────────────────────────────────┐
│                    Federated Identity Flow                           │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  Corporate IdP (Entra ID)                                           │
│       │                                                             │
│       │ SAML/OIDC Federation                                        │
│       ▼                                                             │
│  OAuth Provider (Auth0/Cognito)                                     │
│       │                                                             │
│       │ JWT with corporate identity                                 │
│       ▼                                                             │
│  MCP Server                                                         │
│       │                                                             │
│       │ User identity preserved                                     │
│       ▼                                                             │
│  Audit Log:                                                         │
│  "alice@company.com (Engineering) called delete_customer"           │
│                                                                     │
│  When Alice leaves the company:                                     │
│  1. IT disables her in Entra ID                                    │
│  2. Her OAuth tokens stop working immediately                       │
│  3. No manual key revocation needed                                 │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

## Comparison Summary

| Aspect | API Keys | OAuth 2.0 |
|--------|----------|-----------|
| User identity | Application only | Full user info |
| Permissions | All or nothing | Granular scopes |
| Rotation | Manual, painful | Automatic |
| Leak impact | Long-term access | 1 hour max |
| Revocation | Find and delete | Instant, central |
| Enterprise IdP | No integration | Full federation |
| Compliance | Difficult | Built-in audit trail |
| Standard | Proprietary | Industry standard |

## When API Keys Are Still Okay

API keys aren't always wrong. They're acceptable for:

- **Internal development/testing** - Not facing the internet
- **Server-to-server with no user context** - Background jobs
- **Simple public APIs** - Where abuse is limited
- **Rate limiting identifier** - Combined with other auth

But for MCP servers that:
- Handle sensitive enterprise data
- Need user-level audit trails
- Must integrate with corporate identity
- Require granular permissions
- Face compliance requirements

OAuth 2.0 is the right choice.

## Summary

API keys are a tempting shortcut that creates long-term security debt:

1. **No identity** - Can't audit who did what
2. **No permissions** - Full access or no access
3. **Hard to rotate** - Changes break everything
4. **Easy to leak** - End up in logs and git
5. **No federation** - Separate from corporate identity

OAuth 2.0 solves all of these with:

1. **JWT tokens** - Full user identity in every request
2. **Scopes** - Fine-grained, role-based permissions
3. **Auto-rotation** - Short-lived tokens, seamless refresh
4. **Limited exposure** - Tokens expire, revocation is instant
5. **Federation** - Works with existing enterprise IdP

The next section covers OAuth 2.0 fundamentals for MCP servers.

---

*Continue to [OAuth 2.0 Fundamentals](./ch13-02-oauth-basics.md) →*
