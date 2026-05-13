# Code-Mode Policies

- Read-only: `query` operations only. `mutation` is rejected at validation
  time (returns no approval token).
- Pagination required: `users(limit: ...)` is enforced; queries without a
  `limit` argument are rejected.
- Per-query node budget: max 200 leaf nodes; queries exceeding this are
  rejected with `risk_level: high`.
- Approval-token TTL: 5 minutes from `validate_code` to `execute_code`.
