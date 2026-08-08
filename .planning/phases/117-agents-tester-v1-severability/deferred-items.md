# Phase 117 — Deferred Items

Out-of-scope discoveries logged during execution. NOT fixed in the plan that found them.

## D-117-01-A — `[package.metadata.docs.rs]` covers `v1-compat` only IMPLICITLY

**Found during:** 117-01 Task 3
**Status:** No defect today. Forward hazard for 117-02 / 117-06.

`Cargo.toml:698-717` pins an explicit 16-entry docs.rs feature list and does **not** set
`no-default-features = true`. Because `v1-compat` is now a member of `default`, docs.rs builds
will still document `v1-compat`-gated modules once those land — the coverage is correct, but it
is inherited from `default` rather than stated.

If anyone later adds `no-default-features = true` to that metadata block, every
`v1-compat`-gated module silently disappears from docs.rs with no error and no warning. This is
the same class of hazard that the `Makefile` `doc-check` edit in 117-01 Task 3 closed explicitly.

**Remedy when the modules actually get gated (117-02 / 117-06):** add `"v1-compat"` to the
docs.rs `features` list so the coverage is explicit rather than implicit. Not done in 117-01
because `v1-compat` gates zero modules there, so there is nothing yet to lose.
