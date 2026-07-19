# Changelog

All notable changes to `pmcp-package` are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0]

Initial release. `pmcp-package` is adopted into `rust-mcp-sdk` as its canonical
home and published as the AI-Package format contract shared by `cargo-pmcp`
(packing) and the pmcp.run platform (unpacking).

### Added

- Typed manifest schemas for the four package kinds: `agent`, `server`, `team`,
  and `workflow`.
- Config-slot model with aggregation and deviation detection across a package
  tree (secrets are declared by name, never by value).
- Local OCI Image Layout pack/unpack built on standard `oci-spec` types, with a
  path-traversal guard on unpack.
- Canonical-digest computation: canonicalize-then-hash (OLPC canonical JSON +
  SHA-256) producing a `sha256:<64-hex>` identity, wrapped in a
  construct-only-by-validation newtype.

### Wire-Freeze Policy

- `0.1.x` is digest- and serialization-stable; the serialized shape of every
  package kind is frozen across the `0.1` line.
- The freeze is enforced by golden-fixture tests with pinned digests.
- Any serialized-shape change bumps the minor version to `0.2.0` — a
  wire-breaking change is never shipped as a `0.1.x` patch.
