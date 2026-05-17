# Code-Mode Policy

The toolkit ships `validate_code` + `execute_code` tools that let the
LLM author ad-hoc code (SQL strings / JS scripts depending on backend)
for the long-tail operations not covered by the curated `[[tools]]`.
The `[code_mode]` config block controls what's allowed.

## SQL backends

```toml
[code_mode]
enabled         = true
allow_writes    = false   # Block INSERT / UPDATE / DELETE / MERGE
allow_deletes   = false   # Even if allow_writes=true, DELETE is separately gated
allow_ddl       = false   # Block CREATE / DROP / ALTER
require_limit   = true    # SELECT must have LIMIT clause
max_limit       = 10000   # Cap on LIMIT value

# Table / column blocklists
blocked_tables    = ["user_passwords", "api_keys"]
blocked_columns   = []
sensitive_columns = ["customers.email", "customers.phone"]

[code_mode.limits]
max_tables_per_query  = 5
max_join_depth        = 4
max_subquery_depth    = 2
```

## OpenAPI backends

```toml
[code_mode]
enabled              = true
allow_unsafe_methods = false   # Block POST / PUT / PATCH / DELETE
write_mode           = "deny"  # Even with allow_unsafe_methods, this gates writes
delete_mode          = "deny"

# Operation / path filtering
allowed_methods   = ["GET", "HEAD"]
blocked_methods   = ["DELETE"]
blocked_paths     = ["/admin/*", "/internal/*"]

# Field exposure
internal_blocked_fields = ["password_hash", "ssn", "api_key"]
output_blocked_fields   = ["customer.internal_notes"]
```

## Approval tokens

When the LLM submits code via `validate_code`, the toolkit returns an
HMAC-signed approval token bound to the validated code, the user
identity, and the current policy hash. `execute_code` requires this
token and re-validates that nothing changed between approval and
execution. Token TTL is configurable:

```toml
[code_mode]
token_secret      = "${CODE_MODE_SECRET}"   # From Secrets Manager
token_ttl_seconds = 300
```
