# Workbook Render-URI Spec (`workbook://`, Phase 92)

> **The published, versioned `workbook://` render-pointer URI contract (D-16).**
> This is the public SDK contract for the URI that the `render_workbook` tool
> returns and that `resources/read` consumes. It is a sibling of
> `docs/workbook-dialect-spec.md`: the dialect spec governs what a workbook may
> *be*; this spec governs how a rendered workbook is *pointed at and re-fetched*.
>
> Format changes to the `workbook://` scheme are **versioned decisions**, not
> silent edits — see §7. Clients and proxies that store or replay these URIs
> depend on this contract being stable.

## 1. What this URI is

The `render_workbook` tool does **not** return the `.xlsx` bytes. It validates the
caller's inputs, then returns a `workbook://` URI — a self-contained, stateless
POINTER that encodes the (canonical) inputs plus the bundle provenance stamp. The
spreadsheet itself is regenerated **per `resources/read`** by decoding the URI,
re-verifying provenance, re-validating the inputs, re-running the compiled
workbook, and rendering. There is no server-side session and no render cache
(Lambda-safe): every read recomputes the bytes from the URI alone.

This makes the URI a **provenance-bound pointer**: anyone holding it can re-fetch
the exact same spreadsheet from a server serving the exact same bundle, and a URI
minted against one bundle cannot fetch bytes from a different bundle (§5).

## 2. Scheme and layout

```
workbook://render/<payload>
```

| Part        | Value                                                              |
|-------------|-------------------------------------------------------------------|
| scheme      | `workbook`                                                         |
| authority   | `render` (the render-pointer authority)                           |
| path        | `/<payload>` — a single base64url path segment (see §3, §4)       |

The scheme root `workbook://render/` (no payload) is the stable, listable handle
advertised by `resources/list`. The concrete `workbook://render/<payload>` URIs
are minted per call by `render_workbook` and read back via `resources/read`.

## 3. Encoded payload

The `<payload>` segment is the base64url encoding (§4) of a JSON object:

```json
{
  "dto": {
    "inputs":    { "<json_key>": <value>, ... },
    "overrides": { "<param_key>": <value>, ... }
  },
  "provenance": {
    "bundle_id":     "<neutral bundle id>",
    "version":       "<semver>",
    "combined_hash": "<BUNDLE.lock combined hash-of-hashes>"
  }
}
```

- **`dto`** — the canonical wire input shape (the SAME `{ inputs, overrides }`
  shape the `calculate` / `explain` / `render_workbook` tools accept), so it
  re-validates losslessly on read.
- **`provenance`** — the bundle provenance triple. `combined_hash` is the
  `BUNDLE.lock` **combined** hash-of-hashes (it flips when ANY bundle artifact
  changes). It is **never** the source-workbook content hash — the two are
  distinct values and must not be conflated.

## 4. Encoding and the size bound

- **Encoding:** `base64`, **URL-safe, unpadded** alphabet (`base64url` /
  `URL_SAFE_NO_PAD`), so the payload is a clean URI path segment with no `+`,
  `/`, or `=` characters. (Note: the *rendered `.xlsx` bytes* returned on
  `resources/read` use the **standard** base64 alphabet — that is the resource
  content payload, not the URI.)
- **Size bound (`MAX_ENCODED_URI_LEN` = 65536 bytes / 64 KiB):** the decoder
  rejects any URI longer than this **before any base64 decode** (see §5). 64 KiB
  is generous for a typical input map (a handful of scalar inputs plus the small
  provenance triple) while bounding the per-read decode cost. The bound is part
  of this contract — a conforming workbook input set must encode within it.

## 5. Stateless regeneration on read (security properties)

The URI round-trips through the client, so on `resources/read` it is treated as an
**untrusted, attacker-controlled payload**. Every read runs this pipeline, in
order, before rendering a single byte:

1. **Size guard FIRST.** A URI longer than `MAX_ENCODED_URI_LEN` is rejected
   before any base64 work — an oversized payload never reaches the allocating
   decode path (denial-of-service mitigation).
2. **Total, panic-free decode.** A truncated, garbage, non-base64, non-UTF-8, or
   wrong-shaped payload returns an error, never a panic.
3. **Provenance verification.** The decoded `provenance` MUST equal the live
   bundle stamp (`combined_hash` compared). A cross-provenance / forged URI is
   rejected **before** rendering (spoofing guard).
4. **Input re-validation.** The decoded `dto.inputs` / `dto.overrides` are run
   through the SAME fail-closed validation the tools use — an out-of-range,
   out-of-enum, or strict-constant-override input is rejected here (injection
   guard), not rendered.
5. **Re-run + render.** Only then does the server re-run the compiled workbook
   over the validated seeds and render the `.xlsx`.

Document properties are pinned to a fixed creation datetime, so reading the SAME
URI against the SAME bundle yields **byte-identical** bytes (stateless
determinism).

## 6. Privacy warning — the URI encodes the inputs

> **The `workbook://` URI encodes the caller's inputs in its payload.** Decoding
> the base64url segment reveals every input value supplied to `render_workbook`.

Because resource URIs are routinely logged, an MCP **client, proxy, or gateway
that logs resource URIs will therefore log the inputs**. Operators handling
sensitive inputs (incomes, identifiers, financial figures) must treat a
`workbook://` URI as **sensitive data** — equivalent to logging the request body
— and scrub or avoid logging it accordingly.

A future evolution may add an **opaque-handle** alternative (an unguessable
server-minted token that maps to the inputs out of band) for deployments that
cannot tolerate inputs-in-URI. That is a versioned change (§7), not a silent one.

## 7. Render cost and rate-limiting (operational note)

`resources/read` is **not free**: each read decodes the URI, re-validates, re-runs
the compiled workbook executor, and re-renders the `.xlsx` from scratch — there is
no cache. Read cost therefore scales with the workbook's compute + render size,
and a client may legitimately read the same URI many times (each read recomputes).

- The `MAX_ENCODED_URI_LEN` size bound (§4) caps the *input* size, not the render
  cost.
- Deployments expecting high read volume should add **rate-limiting** at the
  transport / gateway layer. This is a documented extension point; the toolkit
  intentionally keeps the render path stateless and unrated so the limiting
  policy lives with the operator.

## 8. Versioning decision note (D-16)

The `workbook://` scheme, its authority/path layout (§2), its payload fields
(§3), its encoding and size bound (§4), and its stateless-regeneration semantics
(§5) are a **published public contract**. Any change to them — a new payload
field, a different encoding, a changed size bound, an opaque-handle variant — is a
**versioned decision**, recorded as such, never a silent edit. Clients, proxies,
and stored URIs depend on this stability the same way the dialect spec (§7 there)
binds the workbook contract.

---

*Phase 92 — `bundlesource-served-tool-toolkit-module`. Codec:
`crates/pmcp-server-toolkit/src/workbook/render_uri.rs`; stateless read handler:
`crates/pmcp-server-toolkit/src/workbook/render_resource.rs`.*
