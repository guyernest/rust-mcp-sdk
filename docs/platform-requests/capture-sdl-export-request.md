# Request: export the package-capture SDL subset for the SDK contract seam

**To:** pmcp.run platform dev team
**From:** SDK / cargo-pmcp side (package-capture contract seam)
**Date:** 2026-07-20

## What we're building and why

We're adding a **versioned contract** that pins `cargo pmcp package capture`'s two
GraphQL operations to your capture API, so the CLI and the platform can't silently
drift again (the Phase-110 class of bug, where the local verb and your remote
service disagreed). The mechanism: a checked-in SDL file (`capture-v1.graphql`) plus
a **blocking test** that fails the SDK build if the CLI's queries/response structs
don't match that SDL.

## What we discovered (why we need you)

Our plan was for the CLI/CI to introspect the **source `amplifyData` AppSync API**
directly and generate/verify the SDL. That path does not work: the source API

```
https://pn5dorma2bdhzcdhascvc4xzka.appsync-api.us-east-1.amazonaws.com/graphql   (dev)
```

rejects **every** client-obtainable token identically:

| token tried | result |
|---|---|
| M2M `client_credentials` access token | `401 UnauthorizedException: "Valid authorization header not provided."` |
| user-pool **access** token (from `cargo pmcp login`) | same |
| user-pool **id** token | same |

Even a bare `{ __typename }` fails, so it's not introspection-specific — the API
simply isn't accepting client-facing JWT auth. This is consistent with the source
API being **IAM-auth'd from your backend resolvers** (SigV4), not client-reachable:
the capture flow reaches it through the merged/proxy API, and no client token —
ours or a user's — is meant to hit it directly.

So the schema is yours to export. This actually fits the ownership model we want
anyway: **the platform owns the capture schema; the SDK vendors it and enforces the
CLI against it.**

## What we need from you

A **GraphQL SDL subset** covering exactly the two capture operations and the types
they reference — generated from the real schema, not hand-written (we want the
authoritative shape, including exact nullability and field names):

- `Mutation.submitPackageCapture(...)` and its return type
- `Query.getPackageCaptureStatus(...)` and its return type
- Any object/scalar types those two transitively reference

From our current client code, the shape we *believe* is (please correct against the
real schema):

```graphql
type Mutation {
  submitPackageCapture(
    rootComponentType: String!
    rootComponentId:   String!
    version:           String!
    bump:              String
  ): CaptureInfo
}

type Query {
  getPackageCaptureStatus(id: ID!): CaptureStatus
}

type CaptureInfo {           # returned by submit — note: captureId + createdAt
  captureId:  String!
  status:     String!
  createdAt:  String!
}

type CaptureStatus {         # returned by status — note: id + updatedAt (asymmetry!)
  id:                  ID!
  status:              String
  message:             String
  errorCode:           String
  divergentComponents: [String!]
  manifestDigest:      String
  updatedAt:           String
}
```

Please return the **true** version of the above (real type names, real
nullability). The details that matter to us:

1. **`status` must be its real type.** Today we believe it's a plain `String`
   (known values `queued|walking|extracting|publishing|completed|failed|not_found`).
   If it's actually a GraphQL **enum**, tell us — we type it truthfully and it
   changes our versioning. Please don't "tidy" it into an enum just for us; give us
   what the schema really has.
2. **Preserve the `captureId` vs `id` / `createdAt` vs `updatedAt` asymmetry**
   between the submit and status return types exactly — that asymmetry is real and
   our contract must capture it.
3. **SDL format**, not JSON/YAML — a plain `.graphql` file (or the two-op subset
   pasted inline is fine; we'll add a provenance header).

Easiest ways to produce it on your side (any one):

```bash
# From the AppSync API (you have the api-id + IAM access):
aws appsync get-introspection-schema \
  --api-id <capture-api-id> --format SDL capture-full.graphql
# then hand us just the submitPackageCapture / getPackageCaptureStatus subset,
# or the whole file and we'll extract the subset.
```

or your Amplify Gen 2 `defineData` schema source for those two operations, or a
codegen introspection — whatever's least effort.

## Two questions so we point the CLI correctly

1. **Which endpoint does the deployed `capture` verb actually call in dev?** The
   merged/proxy GraphQL URL the CLI authenticates to (not `pn5dorma2…`, which we've
   confirmed we can't reach). We need it to configure the client + docs.
2. **The ongoing drift check.** Because the source API isn't client-reachable, our
   planned headless CI job can't introspect it. Preferred fix on your side, either:
   - **(a)** you run a periodic "does the live capture schema still match the
     vendored `capture-v1.graphql`?" check (you have introspection/IAM access), and
     open a PR to update the SDL when it changes; **or**
   - **(b)** you publish the capture SDL as a small read-only artifact/endpoint we
     can diff against in CI.
   Which do you prefer?

## The deal going forward

When you change the capture schema, **you PR the updated `capture-v1.graphql` to the
SDK repo** (`contracts/pmcp-run/`). The SDK's blocking test then forces the CLI's
queries + response structs to be updated in the same change — so the CLI can never
ship out of sync with your API again. You own the contract's contents; we own the
gate that enforces it.

Thanks — this unblocks shipping `cargo pmcp package capture` (+ `show`/`import`/
`approve`) with a drift-proof contract.
