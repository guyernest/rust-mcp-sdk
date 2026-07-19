# cargo pmcp package

Inspect portable AI-Package bundles locally.

## Usage

```
cargo pmcp package <SUBCOMMAND>
```

## Description

An AI-Package is a portable, OCI image-layout bundle of an agent, team, server, or
workflow (the [`pmcp-package`](../../../crates/pmcp-package/README.md) format).
`cargo pmcp package inspect` opens one **locally and offline** and prints its kind
and key fields.

> **Scope note.** This CLI ships only the local `inspect` verb. The verbs `show`
> and `capture` are reserved for the pmcp.run platform's **remote** capture service
> (remote manifest fetch / dependency-graph capture) and will land as a coordinated
> thin client against the platform contract — they are deliberately *not* defined
> here, to avoid one word meaning two opposite things.

## Subcommands

| Subcommand | Description |
|------------|-------------|
| `inspect` | Show the kind and key fields of a local AI-Package, fully offline |

---

## package inspect

Open a local OCI image-layout `.pmcp` package, resolve its kind
(`agent` / `team` / `server` / `workflow`), and render its key fields. Digest
verification runs during unpack and surfaces verbatim — it is never bypassed.

```
cargo pmcp package inspect <PATH>
```

### Options

| Option | Description |
|--------|-------------|
| `<PATH>` | Path to the AI-Package (OCI image-layout directory) to inspect (positional, required) |

### Example

```
$ cargo pmcp package inspect ./some-agent.pmcp

Package
  Kind:          agent
  Name:          demo-agent
  Version:       1.0.0
  Instructions:  You are a concise, helpful assistant. Use tools when helpful.
  Max tokens:    100000
  Max iterations: 5
  Connectors:    0
```

### Where do `.pmcp` layouts come from?

`inspect` consumes an **OCI image-layout** directory (one with an `index.json`).
Note that `agent new` and `team dev` emit plain JSON manifests
(`agent.package.json`, `team.package.json`), **not** an OCI layout — packing a
manifest tree into an OCI `.pmcp` layout is done by the `pmcp-package` `pack_*` API
(and, on the platform side, by the capture service). A local `package pack` verb is
a candidate follow-on; today, produce a layout via the `pmcp-package` library.

## See also

- [`pmcp-package`](../../../crates/pmcp-package/README.md) — the AI-Package format crate (the `pack_*` API that produces layouts)
- [`cargo pmcp agent`](agent.md) · [`cargo pmcp team`](team.md) — define the agents/teams a package describes
