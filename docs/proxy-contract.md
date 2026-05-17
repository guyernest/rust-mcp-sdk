# PMCP Proxy Header Wire Contract

This document specifies the `X-PMCP-*` HTTP-header contract consumed by
`pmcp::server::streamable_http_server::extract_auth_from_proxy_headers`. The
function turns these proxy-forwarded headers into an `AuthContext` for every
incoming MCP request.

The contract is implemented on the platform side by pmcp.run's `mcp-proxy`
and on the SDK side by `src/server/streamable_http_server.rs`. It is
**additive and backwards-compatible**: new header families are introduced
by appending rows to the table below; existing rows do not change semantics.

## Header families

| Header                                  | AuthContext field / claim key                 | Notes                                                                                                          |
| --------------------------------------- | --------------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| `x-pmcp-user-id`                        | `AuthContext.subject`                         | **REQUIRED**. If absent, `extract_auth_from_proxy_headers` returns `None` and no auth context is built.        |
| `x-pmcp-user-email`                     | `claims["email"]`                             | Optional. Stored verbatim as a JSON string.                                                                    |
| `x-pmcp-user-name`                      | `claims["name"]`                              | Optional. Stored verbatim as a JSON string.                                                                    |
| `x-pmcp-user-groups`                    | `claims["groups"]`                            | Optional. Comma-split into a JSON array of strings; empty entries (after `trim`) are dropped.                  |
| `x-pmcp-tenant-id`                      | `AuthContext.tenant_id` (also `claims["tenant_id"]`) | Optional. Used by multi-tenant tool handlers; also mirrored into the claims map for uniform downstream access. |
| `x-pmcp-claim-custom-<kebab-suffix>`    | `claims["custom:<snake_suffix>"]`             | Optional. Forwards Cognito `custom:*` user attributes (see "Kebab-to-snake transform" below).                  |

## Kebab-to-snake transform

HTTP header names are case-insensitive and the canonical wire form uses `-` as
the only word separator (per RFC 7230). Cognito user-pool attribute names, by
contrast, use `_` (snake_case). The SDK therefore rewrites every `-` in the
suffix portion of the header name to `_` to recover the original Cognito
attribute name.

Concretely: the Cognito user-pool attribute `custom:primary_creator` is
serialized as the header `x-pmcp-claim-custom-primary-creator: <value>` by
mcp-proxy and lands in `AuthContext.claims` under the key
`custom:primary_creator`. The `custom:` key prefix is preserved verbatim so
that downstream code that already reads `custom:*` keys from an in-tree JWT
path keeps working without re-mapping.

Defensive guards: a header with an empty suffix (`x-pmcp-claim-custom-`) or
an empty value is dropped — no claim is inserted in either case.

## Trust model

pmcp.run's `mcp-proxy` strips every inbound `x-pmcp-claim-custom-*` header
from client requests **before** injecting its own platform-vetted values.
Any header observed inside `extract_auth_from_proxy_headers` is therefore
trusted and authoritative: SDK consumers do **not** need to re-validate the
value, check signatures, or apply additional authorization rules at this
layer. The 5-tuple of standard headers (`x-pmcp-user-id`, `x-pmcp-user-email`,
`x-pmcp-user-name`, `x-pmcp-user-groups`, `x-pmcp-tenant-id`) is subject to
the same anti-spoof protection.

This trust boundary only holds when the SDK is deployed behind pmcp.run's
mcp-proxy (or another proxy that implements the same strip-rule). Direct
exposure of the streamable-HTTP server without a proxy in front is **not**
supported by this contract — clients could forge any of these headers.

## Reading the values

Use the existing `AuthContext::claim<T>` helper — no new public API is
introduced by the custom-claim family:

```rust
let primary_creator: Option<String> = ctx.claim("custom:primary_creator");
```

For multi-value attributes (groups), the existing helper still applies:

```rust
let groups: Option<Vec<String>> = ctx.claim("groups");
```

The raw claims map remains accessible as `ctx.claims` (`HashMap<String,
serde_json::Value>`) for callers that need to inspect the full set.
