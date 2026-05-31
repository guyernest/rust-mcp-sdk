---
spike: 001
name: skills-as-resources-mapping
type: standard
validates: "Given SEP-2640 maps Skills onto existing MCP Resources, when a PMCP server publishes a skill via resources/list + resources/read per the SEP's conventions, then a client can discover + load the skill payload with no protocol-extension code on either side."
verdict: VALIDATED (with caveats)
related: []
tags: [skills, sep-2640, resources, wire-protocol, capabilities]
---

# Spike 001: Skills as Resources Mapping (SEP-2640)

## What This Validates

**Given** SEP-2640 ("Skills Extension", Extensions Track) defines Agent Skills
purely as a `skill://` resource convention layered on existing MCP `resources/*`
primitives with no new RPC methods,

**when** a PMCP server publishes a sample skill via `resources/list` +
`resources/read` per the SEP's conventions,

**then** a representative client can discover and load the skill payload using
only existing PMCP types, and the JSON wire form matches the SEP-2640 spec
examples for §2 (resource representation), §4 (read response shapes), §6
(capability declaration), and §9 (discovery index).

## Research

### SEP-2640 wire format (from the PR text)

- **URI scheme:** `skill://<skill-path>/<file-path>`. Final segment of
  `<skill-path>` must equal the skill's `name` from its `SKILL.md` frontmatter.
  Examples: `skill://git-workflow/SKILL.md`, `skill://acme/billing/refunds/SKILL.md`.
- **Discovery:** Three mechanisms — direct read, optional `skill://index.json`
  well-known resource, server `instructions` field. No dedicated listing RPC.
- **Capability:** `capabilities.extensions["io.modelcontextprotocol/skills"] = {}`
  in the server `initialize` response. Identifier: `io.modelcontextprotocol/skills`.
- **No new methods/notifications.** Skills are pure-resources convention.
- **Resource representation:** `uri`, `name` (= frontmatter `name`),
  `mimeType` (= `text/markdown` for SKILL.md), `description` (= frontmatter
  `description`), `_meta` may expose frontmatter fields under
  `io.modelcontextprotocol.skills/` reverse-domain prefix.
- **Archive distribution (optional):** `{ "mimeType": "application/gzip",
  "contents": "<base64>" }`. Suffix stripping: `skill://x.tar.gz` unpacks to
  `skill://x/`. `SKILL.md` must be at archive root.
- **Discovery index (`skill://index.json`):**
  `{"$schema": ..., "skills": [{"name", "type": "skill-md"|"archive"|"mcp-resource-template", "description", "url"}]}`.

### Approach comparison

| Approach | Notes | Status |
|---|---|---|
| Implement on top of `ResourceHandler` | PMCP's existing trait fits SEP-2640 1:1 — `list()` + `read()` are exactly the two methods needed. | **Chosen.** |
| Add a dedicated `SkillHandler` trait | Would duplicate `ResourceHandler` surface for no protocol benefit, given SEP-2640 has no new methods. | Rejected — premature abstraction. |
| Use `ServerCapabilities.experimental` for capability | Wire-incompatible: SEP uses `extensions`, not `experimental`. | Workaround only; surfaces as GAP #1. |

### Chosen approach
Implement `ResourceHandler` with `skill://...` URIs. Serialize `ResourceInfo` /
`ReadResourceResult` directly via `serde_json::to_value` and assert the
resulting JSON matches SEP-2640 examples field-for-field.

## How to Run

```bash
cargo run --manifest-path .planning/spikes/001-skills-as-resources-mapping/Cargo.toml
```

The binary prints a labeled transcript of each spec section being exercised
plus the wire-format JSON output of each operation.

## What to Expect

Five labelled steps printed to stdout:

1. **Server capability declaration** — shows the SEP-2640 required JSON and
   PMCP's current serialized shape side by side. Surfaces GAP #1.
2. **`resources/list`** — calls `SkillsHandler::list()` and prints the
   `ListResourcesResult` wire form. Three resources: two SKILL.md entries and
   the discovery index.
3. **`resources/read("skill://hello-world/SKILL.md")`** — prints the
   markdown payload with YAML frontmatter intact.
4. **`resources/read("skill://index.json")`** — prints the discovery index
   matching SEP-2640 §9 (two `skill-md` entries with `$schema` set).
5. **Archive distribution probe** — constructs a `Content::Resource` with an
   `application/gzip` mime type and shows what PMCP emits versus what SEP-2640
   §4 requires. Surfaces GAP #2.

All runtime assertions inside the binary use `assert_eq!`; the spike fails loud
if anything regresses.

## Investigation Trail

### Iteration 1 — happy path
Built `SkillsHandler` with two SKILL.md files and the discovery index. Verified
that `ResourceInfo::new(uri, name).with_description(...).with_mime_type(...)`
serializes to exactly the SEP-2640 §2 wire shape (uri / name / description /
mimeType). No surprises.

### Iteration 2 — capability declaration
Tried to construct `ServerCapabilities { extensions: ... }`. **Discovery:**
PMCP `ServerCapabilities` has no `extensions` field. It has `experimental`
(an `Option<HashMap<String, Value>>`) but SEP-2640 specifically requires the
`extensions` key. The two are wire-incompatible — different JSON paths, not
synonyms. Recorded as **GAP #1**.

### Iteration 3 — depth probe on archive distribution
SEP-2640 §4 includes an optional archive distribution mode using
`application/gzip` + base64 blob. Probed whether PMCP can emit this wire form.
**Discovery:** `Content::Resource` in `src/types/content.rs:60-92` has only a
`text` field, no `blob`. The MCP spec's `ResourceContents` is
`uri + (text | blob) + mimeType?`. The custom serializer at
`src/types/content.rs:325` does not emit a `blob` key for any variant. Wire-
correct binary archive serving via `ReadResourceResult` is not possible today.
Recorded as **GAP #2**. SEP-2640 §4 explicitly marks archive distribution as
optional, so this does not block text-mode skill serving — but it bounds the
scope of what PMCP can offer until the gap is closed.

### Iteration 4 — verdict review
Re-read the spike's Given/When/Then. The text-mode happy path is fully
validated; archive distribution is partially validated (the SEP's optional
path is unsupported in PMCP's current types). Settled on
**VALIDATED with caveats** rather than VALIDATED or PARTIAL, because the
spike's core claim ("a client can discover + load the skill payload with no
protocol-extension code") is true for the universally-required SKILL.md flow
and only fails for the spec-optional archive flow.

## Results

**Verdict: VALIDATED with caveats.**

### What works (no PMCP changes required)
- `skill://` URI convention round-trips through `ResourceHandler` cleanly.
- `ResourceInfo` builder produces a wire form that matches SEP-2640 §2
  byte-for-byte (uri / name / description / mimeType).
- `ReadResourceResult` with `Content::text(...)` produces SEP-2640 §4
  text-mode wire form correctly (no `type` discriminator leakage; the
  custom serializer at `src/types/content.rs:330` strips it for Text).
- Discovery index (`skill://index.json`) serves as text/json content; the
  inner JSON matches SEP-2640 §9 schema.

### Gaps that need follow-on PMCP changes

| # | Gap | Spec ref | Severity | Suggested fix |
|---|---|---|---|---|
| 1 | `ServerCapabilities` has no `extensions` field; PMCP only exposes `experimental`. SEP-2640 mandates the `extensions` key. | §6 | **Required** for any SEP-2640-compliant server. | Add `pub extensions: Option<HashMap<String, Value>>` to `ServerCapabilities` in `src/types/capabilities.rs`. Additive, mirrors `experimental` exactly. |
| 2 | `Content::Resource` has no `blob` field. Custom `resource_contents_serde::serialize` does not emit one. | §4 (archive mode) | **Optional** path in the SEP; text-mode is fully usable without this. | Add `blob: Option<String>` to `Content::Resource` and emit it from `resource_contents_serde::serialize` (and parse it from deserialize). |

### Surprises
- The custom resource_contents serializer at `src/types/content.rs:330` strips
  `type` for `Resource`/`Text` variants but **passes `Image`/`Audio`/`ResourceLink`
  through tagged**. This is fine for skills (text-only path) but is a latent
  inconsistency worth noting if someone tries to embed an image in a skill
  resource — they'd get a `{"type": "image", ...}` shape inside `contents`
  which the spec doesn't define.
- `RequestHandlerExtra` is `#[non_exhaustive]` and has both
  `RequestHandlerExtra::new(id, token)` and `RequestHandlerExtra::default()`.
  The Default is sufficient for in-process spike testing.

### What this means for spike 002

Spike 002 (`skill-ergonomics-pragmatic`) can proceed regardless of whether
the two gaps above are closed:

- The DX layer (`register_skill(...)` or `#[pmcp::skill]`) only needs to wrap
  the existing `ResourceHandler` trait — already validated here.
- The `extensions` capability gap is independent of the DX surface; the
  ergonomic helper would set whichever capability field PMCP exposes. If GAP
  #1 is closed before 002 ships, the helper sets `extensions`. Otherwise it
  documents the temporary `experimental` workaround.
- Archive distribution (GAP #2) is optional in the SEP; 002 can ignore it
  cleanly and only support text-mode skills + the discovery index.

The pragmatic recommendation for a v1 "batteries-included" PMCP Skills API:

1. Close GAP #1 in the same patch series as the DX layer (it's a one-line
   addition).
2. Ship text-mode skills with the DX layer.
3. File GAP #2 as a follow-on; archive support unlocks it but is optional.
