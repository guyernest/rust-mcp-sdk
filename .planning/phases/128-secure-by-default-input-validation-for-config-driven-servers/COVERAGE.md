No external API integration: this phase hardens input validation *inside* the pmcp SDK — core `pmcp`, `pmcp-server-toolkit` and `pmcp-code-mode` — and adopts no new external API, SDK or service. Surfaces enumerated below.

## Surfaces touched (all in-repo)

Core `pmcp`: `src/server/schema_validation.rs`, `src/server/validation.rs`.
`pmcp-server-toolkit`: `config.rs`, `tools.rs`, `http/client.rs`, `code_mode.rs`.
`pmcp-code-mode`: the `HttpExecutor` trait.

None of these is a third-party API, SDK or service being adopted — every one is a module
of this repository.

## Why the detector fired

`api-coverage.cjs` returned `detected: true` on a single signal: the noun `sdk` in the ROADMAP line
`**Source of truth**: the reviewed change request *PMCP SDK change request: secure-by-default input
validation*`, paired with the `(surface)` verb. That is this repository's own name, not a third-party
SDK being integrated. Confirmed by re-reading the phase scope rather than by preference, per the
checkpoint's own instruction.

The phase does touch OpenAPI specs and outbound HTTP — but only to *constrain* requests the toolkit
already knows how to send (D4's placeholder denylist, D4(b)'s spec-`pattern` narrowing). No new
upstream capability surface is being adopted, so there is no capability list to enumerate and no
INTEGRATE/OPT-OUT decision to record. Fabricating matrix rows for capabilities that do not exist
would be worse than this declaration.
