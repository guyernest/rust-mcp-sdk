# OpenAPI — Pareto Tools

For OpenAPI backends, each tool's `path` + `method` selects an
operation from the OpenAPI document. The toolkit handles path
templating (`{id}` → from `path` params) and HTTP verb dispatch.

## Tool design heuristics

1. **One tool = one OpenAPI operationId,** in most cases. Don't try to
   fan multiple operations into one tool — agents handle chained tool
   calls fine.
2. **Honor the verb.** GET tools should set `read_only_hint = true`.
   DELETE tools should set `destructive_hint = true`. POST/PUT/PATCH
   depend on whether the operation is idempotent.
3. **`/admin/*` paths default to BLOCKED.** Mark them
   explicitly-allowed only when the agent genuinely needs admin
   surface and you've audited the policy.
4. **Filter sensitive response fields.** Use `outputs.exclude` or
   `outputs.fields` to whitelist what the agent sees. This is your
   sole defense against the LLM exfiltrating PII through the response.

## Auth modes

| Mode | When to use | Config |
|---|---|---|
| `bearer` | Static API token (machine identity) | `[backend.auth] type = "bearer"`, `token = "${ENV_VAR}"` |
| `api_key` | Header or query-param key | `type = "api_key"`, target shape |
| `oauth2_client_credentials` | Server-to-server with token issuer | `type = "oauth2_client_credentials"`, `token_url`, `client_id`, `client_secret` |
| `oauth_passthrough` | Per-end-user identity multiplexing | `type = "oauth_passthrough"`, `target_header = "Authorization"` |

Passthrough is the load-bearing mode for multi-tenant deployments — the
MCP client's per-request user token flows through to the backend so the
backend's own auth model applies.

## TOML shape

```toml
[[tools]]
name        = "list_customer_orders"
description = "List recent orders for a customer."
path        = "/customers/{customer_id}/orders"
method      = "GET"

[[tools.parameters]]
name        = "customer_id"
type        = "integer"
description = "Customer id (path parameter)."
required    = true

[tools.outputs]
path        = "$.orders[*]"
fields      = ["id", "total", "status", "created_at"]
```
