# pmcp-package

The **AI-Package format** for portable MCP packages. `pmcp-package` defines the
typed manifest schemas, config-slot aggregation, local OCI pack/unpack, and
canonical-digest computation that let an MCP agent, server, team, or workflow be
described once and moved between tools without ambiguity.

This crate is the dual-consumer format contract: `cargo-pmcp` packs packages and
the pmcp.run platform unpacks them. It is **format only** — no agent runtime
semantics, no network or cloud calls, and no resolved secret *values* (config
slots declare secrets by name, never by value).

## What's in the format

Four package kinds, each with a typed manifest schema:

| Kind | Describes |
|------|-----------|
| `agent` | A single agent: instructions, LLM settings, connectors, tool selection, I/O schemas |
| `server` | An MCP server: binary reference, tools, config slots, deploy descriptor, policies |
| `team` | A team of members composed from agent/server packages |
| `workflow` | A multi-step workflow over agents/servers |

Supporting pieces:

- **Config slots** — declarative inputs (including secrets *by name*) with
  aggregation and deviation detection across a package tree.
- **Local OCI pack/unpack** — write and read packages as an OCI Image Layout on
  the local filesystem, using standard `oci-spec` types (`Descriptor`, `Digest`,
  `MediaType`) for zero-translation interop with OCI clients.
- **Canonical digest** — a stable content identity for a manifest.

## Canonical digest: canonicalize-then-hash

Manifest identity is computed by **canonicalizing then hashing**, never by
hashing a struct's default serialization. A typed manifest is serialized through
an [OLPC canonical JSON](https://crates.io/crates/olpc-cjson) formatter
(Docker/Notary/TUF-compatible) and the resulting canonical bytes are hashed with
SHA-256, yielding a `sha256:<64-hex>` string. Because canonicalization is
order-independent, two structurally-equal manifests always produce the same
digest regardless of map/field ordering.

The digest string is wrapped in a construct-only-by-validation newtype: a value
that is not a well-formed `sha256:<64-hex>` cannot enter a typed manifest.

## Wire-Freeze Policy

`pmcp-package` follows a strict serialization-stability policy so that packages
produced by one tool version can always be read by another:

- **`0.1.x` is digest- and serialization-stable.** The serialized shape of every
  package kind is frozen across all `0.1.x` releases.
- This freeze is **mechanically enforced** by golden-fixture tests that pin the
  expected canonical digest of representative packages (see
  `tests/digest_stability.rs`). Any change that would alter a serialized shape —
  a renamed/added/removed field, a changed default, a different canonical byte
  sequence — breaks these tests.
- **Any serialized-shape change bumps the minor version to `0.2.0`.** A
  wire-breaking change is never shipped as a `0.1.x` patch. Consumers can
  therefore depend on `pmcp-package = "0.1"` (caret) and trust that reads and
  digests stay stable for the life of the `0.1` line.

The human-readable policy above is exactly what the pinned-digest golden-fixture
tests enforce in code.

## License

Licensed under either of

- MIT license ([LICENSE-MIT](LICENSE-MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))

at your option. See [NOTICE](NOTICE) for attribution.
