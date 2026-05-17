# Skills — Wire Protocol (SEP-2640)

How a PMCP server publishes Agent Skills via the existing `resources/*`
primitives, and the two small protocol-types additions PMCP needs to be
wire-correct.

## Requirements

These are non-negotiable for any real implementation:

- **No new traits.** Skills are served via the existing `ResourceHandler`
  trait. Do NOT introduce a parallel `SkillHandler` trait — spike 001
  validated that `ResourceHandler` is sufficient and adding a parallel trait
  would duplicate surface area for zero protocol benefit.
- **`ServerCapabilities` must gain an `extensions` field.** SEP-2640 §6
  mandates `capabilities.extensions["io.modelcontextprotocol/skills"] = {}`.
  PMCP today only has `experimental`, which is wire-incompatible (different
  JSON path, different intent). This is GAP #1.
- **Archive distribution is out of scope for v1.** SEP-2640 §4 archive mode
  (`application/gzip` + base64 blob) cannot be served wire-correctly because
  `Content::Resource` has no `blob` field today (GAP #2). The SEP marks
  archive mode as optional, so text-mode skills work fully without it.

## How to Build It

### Step 1 — Close GAP #1: add `extensions` to `ServerCapabilities`

In `src/types/capabilities.rs`, add a field parallel to `experimental`:

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "camelCase")]
pub struct ServerCapabilities {
    // ... existing fields unchanged ...

    /// Experimental capabilities (pre-SEP namespace, kept for compatibility).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experimental: Option<HashMap<String, serde_json::Value>>,

    /// Extension capabilities — reverse-domain-keyed protocol extensions.
    /// Required by SEP-2640 (Skills) and the Extensions Track in general.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Option<HashMap<String, serde_json::Value>>,
}
```

This is additive — no breaking change. Same shape as `experimental`. Mirror
on `ClientCapabilities` if needed for future extensions; not required for
SEP-2640 (server-declared capability, client acknowledgement is implicit).

### Step 2 — Implement a `ResourceHandler` for skills

Per SEP-2640 the wire pattern is:

| Method | URI | Response |
|---|---|---|
| `resources/read` | `skill://<path>/SKILL.md` | `{contents: [{text: "<markdown body>"}]}` mimeType `text/markdown` |
| `resources/read` | `skill://<path>/<file>` | `{contents: [{text: "<body>"}]}` mimeType from file extension |
| `resources/read` | `skill://index.json` | `{contents: [{text: "<discovery index JSON>"}]}` mimeType `application/json` |
| `resources/list` | — | Returns `ResourceInfo` for each SKILL.md + the index. **Supporting files are NOT enumerated** (per §9). |

Construct each `ResourceInfo` with:
- `uri` = `skill://<path>/SKILL.md`
- `name` = the frontmatter `name` field (must equal the final `<path>` segment)
- `description` = the frontmatter `description` field
- `mimeType` = `text/markdown`

The custom serializer in `src/types/content.rs:325` (`resource_contents_serde`)
strips the `type` discriminator for `Content::Text` and `Content::Resource`,
which produces the exact wire shape SEP-2640 expects. Verified in spike 001.

### Step 3 — Advertise the capability

After GAP #1 lands:

```rust
let mut caps = ServerCapabilities::default();
caps.resources = Some(ResourceCapabilities::default());
let mut ext = HashMap::new();
ext.insert("io.modelcontextprotocol/skills".to_string(), serde_json::json!({}));
caps.extensions = Some(ext);
```

Until GAP #1 lands, use `caps.experimental` as a temporary stand-in. Migrate
to `caps.extensions` in one place when the field is available.

### Step 4 — Synthesize the discovery index

SEP-2640 §9 — the optional well-known resource at `skill://index.json`:

```json
{
  "$schema": "https://schemas.agentskills.io/discovery/0.2.0/schema.json",
  "skills": [
    {
      "name": "<name>",
      "type": "skill-md",
      "description": "<description>",
      "url": "skill://<path>/SKILL.md"
    }
  ]
}
```

Generate this from the registered skills at server-init time, not per-request.
Entry types: `skill-md` (file), `archive` (defer to v2), `mcp-resource-template`
(URI template for parameterized skills — useful for `skill://docs/{product}/`
patterns).

## What to Avoid

- **Using `experimental` as the permanent home for the skills capability.**
  Wire-incompatible with SEP-2640 hosts. Hosts that look at
  `capabilities.extensions["io.modelcontextprotocol/skills"]` will not see
  it under `experimental`. Use `experimental` only as a temporary workaround
  until the `extensions` field lands.
- **Enumerating supporting files in `resources/list`.** SEP-2640 §9 explicitly
  defines discovery via SKILL.md URIs + the index. Supporting files are
  addressable via direct read but not enumerated. Including them inflates
  the list response and confuses agents about what's a skill vs a supporting
  file.
- **Stuffing base64 archive content into `Content::Resource.text`.** Spec
  uses `blob`, not `text`. Wire-incompatible. Either wait for GAP #2 to land
  or simply don't ship archive distribution in v1 (it's optional per SEP).
- **Server-prefixing skill names instinctively.** A skill named `git-workflow`
  served by one server is fine. But on a multi-server host, multiple servers
  shipping `skill://code-mode/...` will collide in the agent's mental model.
  Either let host-side disambiguation handle it (current spec stance) or
  scope deliberately (`skill://my-server/code-mode/SKILL.md`). Decide before
  publishing.

## Constraints

- **MCP spec version:** SEP-2640 is currently in-review (Extensions Track).
  Targeting the SEP as-of commit `ade2a58` (April 2026). Wire format may
  shift before merge; the `io.modelcontextprotocol/skills` identifier is
  stable.
- **No new RPC methods.** Skills add zero new methods or notifications.
  All discovery and reading happens via existing `resources/list`,
  `resources/read`, optionally `resources/subscribe`.
- **Frontmatter is YAML, body is markdown.** `name` and `description` are
  required in frontmatter; the final URI path segment MUST equal `name`.
- **Skills do not grant authority.** A skill is pure context-loading.
  Authorization, validation, and execution remain the responsibility of
  whatever tools the skill references (e.g. `validate_code` / `execute_code`
  for code-mode skills). Document this explicitly in the SKILL.md so
  reviewers don't conflate the bootstrap layer with the security layer.

## Origin

Synthesized from spike: 001 (skills-as-resources-mapping, VALIDATED with caveats).
Source files available in: `sources/001-skills-as-resources-mapping/`.

The spike's `src/main.rs` is a runnable in-process demo that prints labelled
wire-format JSON for every endpoint and asserts the shape with `assert_eq!`.
Use it as the reference for what the synthesized wire output should look
like — copy the assertions into the real implementation's integration test.
