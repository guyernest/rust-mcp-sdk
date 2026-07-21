# Ticket: `capture-v2` (optional) — promote GraphQL `status` from `String` to a real enum

**Status:** open / not yet filed as a GitHub issue (stub — file against `paiml/rust-mcp-sdk`)
**Spun off from:** package-capture contract seam (design spec §9, 2026-07-20)
**Priority:** optional cleanup — zero dependency on the v1 critical path

## Context

`contracts/pmcp-run/capture-v1.graphql` types the capture `status` field as a plain
`String!` in **both** return types (`SubmitPackageCaptureReturnType`,
`GetPackageCaptureStatusReturnType`), matching the live platform schema. Known
runtime values are documented as an SDL comment, not schema-enforced:

```
queued | walking | extracting | publishing | completed | failed | not_found
```

The CLI poller already tolerates unknown statuses (treats any unrecognized value as
in-progress, warns once), so additive platform statuses never break an older CLI.

## Why v1 keeps `String`

Typing `status` as a GraphQL `enum` in the vendored contract while the live schema
still returns `String` would make the platform-owned online schema-vs-schema diff
report **permanent false drift** (`enum` in the contract vs `String` live). v1 must
match today's schema exactly.

## The optional change

If the **platform** promotes `status` to a server-side GraphQL `enum` (a clean schema
improvement on their side), then a future **`capture-v2.graphql`** contract can assert
the enum truthfully, and the CLI can type the status as an enum end-to-end.

Sequencing (must be in this order to avoid false drift):
1. Platform changes the live schema `status` → enum.
2. Platform exports and PRs `capture-v2.graphql` with the enum (per the §6 ownership
   model — platform owns the contract contents).
3. SDK bumps the CLI ops/structs + the offline contract test to `capture-v2`.

## Out of scope

Not required for v1. No SDK work until/unless the platform does step 1.
