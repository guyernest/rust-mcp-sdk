# Changelog

All notable changes to `pmcp-package` are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.0] - 2026-08-27

Phase 122 (attestation carriage). Five source-breaking changes; `PackageError` is
**not** `#[non_exhaustive]`, so the two new variants break exhaustive downstream
matches as well. Ships at tag `v2.19.1`.

> The 0.2.0 line below was never published — crates.io went 0.1.1 -> 0.3.0. Its
> entry is retained because the wire-shape break it records is real and 0.3.0
> inherits it.

### Changed (BREAKING)

- `pack_server` takes a sixth positional parameter,
  `attestation: Option<AttestationFile<'_>>` (`src/oci/pack.rs`).
- `pack_team` takes the same as its second parameter, and can now REFUSE input
  that previously packed: an attestation over a team whose components are not
  fully resolved is rejected up front (Gate A, D-09).
- `unpack_team` returns `Result<UnpackedTeam>` instead of `Result<TeamPackage>`
  (`src/oci/unpack.rs`), so the carriage state is reported as data rather than
  discarded.
- `PinnedRef` gains a fifth public field, `resolved_from: Option<semver::VersionReq>`,
  breaking every struct literal. It is serde-optional, so the WIRE format is
  unchanged — only Rust construction sites break.
- `PackageError` gains `AttestationSubjectMismatch` and
  `AttestationAnnotationInvalid`. The enum is not `#[non_exhaustive]`, so
  downstream exhaustive `match` arms must be extended.

### Added

- Attestation carriage as a single OCI layer, with all three states distinguishable
  on read: attested-and-matching, attested-but-mismatched, and unattested.
- A subject-digest mismatch is refused before any write, and reported as data on
  the unpack side rather than raised as an error.

### Notes

- The crate's no-crypto boundary is unchanged and still machine-enforced by the
  crate-local `[bans].allow` allowlist in `deny.toml`: carriage is opaque
  transport, and this crate verifies no signatures.
- Also in this line, without a version move of their own: the normative
  artifact-tar framing rule (`src/oci/mod.rs`, docs only) and the golden fixture
  corpus at `tests/golden_fixtures/artifact_tar_v1/`, whose bytes are checked in
  and never regenerated from the writer under test.

## [0.2.0] - Unreleased

Phase 120 (config-server packaging). The first declared break of the wire
freeze, exactly per the 0.1.0 policy below: the serialized `ServerPackage`
shape changed, so the minor version moved. `0.2.x` starts a new frozen line
(`EXPECTED_SERVER_DIGEST` re-pinned; `ARTIFACT_TYPE_SERVER` stays `.v1` — no
second version axis). There are no consumers of 0.1.x packages, so there is
no migration path by design: a 0.1.x server envelope is refused at unpack
with a shape error instead of deserializing with fields silently dropped.

### Changed (BREAKING)

- `ServerPackage.binary_ref` is dropped (D-08). `BinaryMode` —
  `Embedded(&[u8])` / `Referenced { digest, media_type }` — is the single
  source of the binary's identity: `pack_server` takes it, `unpack_server`
  returns the same two-arm shape (D-06), and a caller structurally cannot
  mistake a referenced package for one that carries bytes.

### Added

- Server packages can carry a **config layer**, with `ConfigSlot.config_key`
  binding each value slot to the dotted TOML key it fills. Pack gates enforce:
  slot-declared value keys hold environment references (never resolved
  literals), slots and config declarations agree, the auth-mode key's baked
  literal equals the slot's declared `tested_value`, and a package whose slots
  carry `config_key`s cannot pack without its config file.
- env-ref grammar v2: a `${...}` reference names exactly ONE variable
  (`[A-Za-z0-9_]+`); multi-placeholder compositions like `${SCHEME}://${HOST}`
  are refused at pack with an error naming the defect. The grammar stays in
  lock-step with `pmcp-server-toolkit` via the shared
  `env_ref_grammar_v1.tsv` parity table.
- `aggregate()` refuses two same-identity slots whose `config_key`s differ —
  the only order-independent outcome, preserving permutation/digest stability.

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
