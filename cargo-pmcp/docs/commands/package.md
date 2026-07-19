# cargo pmcp package

Inspect and capture portable AI-Package bundles.

## Usage

```
cargo pmcp package <SUBCOMMAND>
```

## Description

An AI-Package is a portable, OCI image-layout bundle of an agent, team, server, or
workflow (produced by the [`pmcp-package`](../../../crates/pmcp-package/README.md)
format). `cargo pmcp package` inspects one locally (`show`) and uploads one to a
platform (`capture`):

- `package show` opens a local `.pmcp` package, detects its kind, verifies digests
  while unpacking, and prints the key fields — **fully offline**.
- `package capture` uploads a local `.pmcp` package to a platform target configured
  with `cargo pmcp configure` / `cargo pmcp auth`. It is a thin authenticated client
  (Bearer token, request timeout) — it invents no new config.

## Subcommands

| Subcommand | Description |
|------------|-------------|
| `show` | Show the kind and key fields of a local AI-Package, fully offline |
| `capture` | Capture (upload) a local AI-Package to a configured platform target |

---

## package show

Open a local OCI image-layout `.pmcp` package, resolve its kind
(`agent` / `team` / `server` / `workflow`), and render its key fields. Digest
verification runs during unpack and surfaces verbatim — it is never bypassed.

```
cargo pmcp package show <PATH>
```

### Options

| Option | Description |
|--------|-------------|
| `<PATH>` | Path to the AI-Package (OCI image-layout directory) to inspect (positional, required) |

### Example

```
$ cargo pmcp package show ./my-agent.pmcp

Package
  Kind:          agent
  Name:          demo-agent
  Version:       1.0.0
  Instructions:  You are a concise, helpful assistant. Use tools when helpful.
  Max tokens:    100000
  Max iterations: 5
  Connectors:    0
```

---

## package capture

Upload a local `.pmcp` package to a configured platform target. The target selects
the platform API URL and the cached token (from `cargo pmcp configure` /
`cargo pmcp auth`); an expired or near-expiry token is refused with guidance rather
than uploaded. The token is sent as a `Bearer` header and never printed.

```
cargo pmcp package capture <PATH> [--target <NAME>]
```

### Options

| Option | Description |
|--------|-------------|
| `<PATH>` | Path to the AI-Package (OCI image-layout directory) to capture (positional, required) |
| `--target <NAME>` | Platform target to capture to (a `cargo pmcp configure` target name; falls back to `PMCP_TARGET` and the active target) |

### Example

```
# Configure and authenticate a target once, then capture:
cargo pmcp configure add prod --api-url https://platform.example.com
cargo pmcp auth login https://platform.example.com
cargo pmcp package capture ./my-agent.pmcp --target prod
```

## See also

- [`pmcp-package`](../../../crates/pmcp-package/README.md) — the AI-Package format crate
- [`cargo pmcp configure`](../../README.md) / `cargo pmcp auth` — set up and authenticate a platform target
- [`cargo pmcp agent`](agent.md) · [`cargo pmcp team`](team.md) — produce the packages `show`/`capture` operate on
