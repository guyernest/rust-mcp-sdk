# Phase 26: Add OAuth Support to Load-Testing - Context

**Gathered:** 2026-02-28
**Status:** Ready for planning

<domain>
## Phase Boundary

Add authentication support to the loadtest engine so VUs can target OAuth-protected MCP servers. Three auth types: OAuth client_credentials flow, static bearer tokens, and API key headers. Auth is configured via a new `[auth]` section in the loadtest TOML config.

</domain>

<decisions>
## Implementation Decisions

### Auth config shape
- New top-level `[auth]` section in loadtest TOML alongside `[settings]` and `[[scenario]]`
- `type` field discriminates between auth types: `"oauth"`, `"bearer"`, `"api_key"`
- Omitting `[auth]` entirely means no auth (backward compatible)
- Secrets via env vars only — env var NAMES specified in TOML, not secret values
- OAuth config explicitly names env vars: `client_id_env`, `client_secret_env`

### Auth types
- **OAuth (`type = "oauth"`)**: client_credentials flow with `token_url`, `client_id_env`, `client_secret_env`, `scopes`
- **Bearer (`type = "bearer"`)**: static token from env var via `token_env`
- **API Key (`type = "api_key"`)**: configurable header name via `header` field, key from env var via `key_env`

### TOML examples
```toml
# OAuth client_credentials
[auth]
type = "oauth"
token_url = "https://auth.example.com/token"
client_id_env = "MY_CLIENT_ID"
client_secret_env = "MY_CLIENT_SECRET"
scopes = ["mcp:read", "mcp:write"]

# Bearer token
[auth]
type = "bearer"
token_env = "MY_BEARER_TOKEN"

# API key
[auth]
type = "api_key"
header = "X-API-Key"
key_env = "MY_API_KEY"
```

### Token lifecycle
- Shared token across all VUs (one fetch, all VUs use it)
- Pre-fetch before test starts (during setup, before VU spawn) — fail fast on misconfigured auth
- Token fetch time NOT counted in test metrics
- Auto-refresh during long-running tests if token expires (check expiry or react to 401)
- Detailed error guidance on token fetch failure: which env var is missing/empty, what the token URL returned, suggested fixes

### Header injection
- New `auth_header: Option<(String, String)>` field on `McpClient` struct
- `send_request()` adds auth header if present (alongside existing Content-Type and session-id)
- For OAuth: `("Authorization", "Bearer {token}")`
- For Bearer: `("Authorization", "Bearer {token}")`
- For API Key: `("{header_name}", "{key_value}")`

### Display and metrics
- Auth type shown in test summary ASCII header (e.g., "Auth: OAuth (client_credentials)")
- New `auth` error category alongside existing timeout/jsonrpc/http/connection — 401 responses classified as auth errors
- Manual config only — no auto-discovery from server well-known endpoints

### Claude's Discretion
- Token refresh strategy details (proactive expiry check vs reactive 401 handling)
- Auth validation ordering within config parse
- Exact error message wording
- Whether to cache token with Arc<RwLock> or simpler approach

</decisions>

<specifics>
## Specific Ideas

- OAuth config should mirror the env-var-reference pattern: TOML names the env var, runtime reads the value. This prevents secrets from being committed.
- The `[auth]` section is optional — existing TOML configs without it continue to work unchanged.
- Token fetch failure should be actionable: "Environment variable MY_CLIENT_ID is not set. Set it or update client_id_env in your loadtest config."

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `McpClient` (client.rs): Already has optional header pattern (`session_id`). Adding `auth_header` follows the same approach.
- `LoadTestConfig` (config.rs): serde-based TOML parsing with `#[serde(default)]` for optional sections. `[auth]` can use the same pattern.
- `McpError` enum (error.rs): Classifies errors into categories. Adding `Auth` variant follows existing pattern.
- `reqwest::Client`: Shared across VUs via `Clone`. Auth header can be set per-request in `send_request()`.

### Established Patterns
- serde internally-tagged enums: `ScenarioStep` uses `#[serde(tag = "type")]` — same pattern works for auth type discrimination
- Optional config sections: `stage` field uses `#[serde(default)]` for optional `[[stage]]` blocks
- Error classification: `McpError::classify_reqwest()` maps reqwest errors to categories
- Test summary display: `display.rs` renders the ASCII header box with test parameters

### Integration Points
- `LoadTestConfig` struct in config.rs — add `auth: Option<AuthConfig>` field
- `McpClient::new()` — add auth_header parameter or setter
- `McpClient::send_request()` — inject auth header into request builder
- `display.rs` or `summary.rs` — add auth type line to test header
- `McpError` enum — add `Auth` variant for 401 classification
- Engine setup (engine.rs) — pre-fetch token before spawning VUs

</code_context>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 26-add-oauth-support-to-load-testing*
*Context gathered: 2026-02-28*
