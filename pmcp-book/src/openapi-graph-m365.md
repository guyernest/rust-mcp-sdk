# OpenAPI over Microsoft Graph (Contoso M365)

The previous chapter built a config-driven OpenAPI MCP server in the abstract: a
backend, a few curated tools, a Code Mode policy, six outgoing-auth variants. This
chapter makes it concrete with an advanced, security-forward example — **Contoso**,
a fictional company whose finance team keeps its customers and orders in an Excel
workbook on Microsoft 365, and who wants to ask questions of that data through an
AI assistant *without* moving the data, copying credentials, or writing a line of
Rust.

It is the `oauth_passthrough` sibling of the [Config-Driven OpenAPI Servers](openapi-built-in-server.md)
chapter's `london-tube` (api_key) walkthrough: same scaffold-and-config model,
same two-kinds-of-tools split, same Code Mode long tail — but the headline is the
**outgoing auth model**, because here the backend is Microsoft Graph and every read
must happen *as the signed-in user*.

## The Problem (Keep Your Excel Files, Connect Them to AI)

Contoso already has the data it needs. `Customers.xlsx` lives on the company's
"Sales" SharePoint site: a **Customers** sheet (`customer_id`, `name`, `segment`,
`region`) and an **Orders** sheet (`order_id`, `customer_id`, `order_date`,
`amount`). The customers are real-feeling but fictional — Northwind Traders,
Fabrikam, Tailspin Toys. The team does not want to migrate this workbook into a
database or build a bespoke reporting service. They want to *keep the files where
they are* and connect them to AI through MCP.

The work of deciding **which** slice of the giant Microsoft Graph API to expose
belongs to a business analyst, not a Rust programmer. Graph has thousands of
operations; Contoso needs exactly one read shape (a worksheet range read) and two
curated entry points. The analyst curates that slice in `config.toml`: declare the
Graph base URL, the auth model, two read-only tools, and the Code Mode context.
Nothing about this server is hand-coded per operation.

```toml
[server]
name = "contoso-m365"
version = "1.0.0"
description = "Contoso M365: per-user delegated OAuth (passthrough) reading a Customers/Orders Excel workbook over Microsoft Graph."

[backend]
base_url = "https://graph.microsoft.com/v1.0"
```

## Outgoing Authentication — The Double Lock

This is the headline of the chapter. Contoso's workbook contains commercially
sensitive data; the server must **never** be able to read more than the person
asking is allowed to read, and it must hold **no standing credential** that an
attacker could steal and replay. The `oauth_passthrough` auth model gives both
guarantees with a single config block, quoted here verbatim from the shipped
`crates/pmcp-openapi-server/examples/contoso-m365.toml`:

```toml
[backend.auth]
type = "oauth_passthrough"
target_header = "Authorization"
required = true
```

Read this as a **double lock**, and both locks must engage on every request:

1. **The admin sets the ceiling.** A Microsoft 365 administrator consents *once*
   to a bounded scope for the Contoso application (for example, delegated
   `Files.Read`). That admin consent is the **ceiling** — the maximum set of
   permissions the server could ever request on a user's behalf. It is granted a
   single time, out of band, and the server cannot exceed it.

2. **The user's forwarded token further restricts.** At request time the server
   captures the signed-in user's inbound `Authorization: Bearer <token>` and
   forwards it to Graph as the `target_header`. Because that per-user token only
   carries *that user's* effective access, it **further restricts** every read to
   the files and rows that user can already see in their own M365 session. A user
   who cannot open `Customers.xlsx` in Excel cannot read it through this server
   either — the admin ceiling does not widen what any individual user can reach.

The crucial property: the server **holds no standing credentials**. There is no
client secret, no cached service token, no API key in the config — the only
bearer that ever reaches Graph is the one the calling user brought with them, so
the server can only ever **act as the calling user**. Least privilege is the
default, not an add-on.

> Contrast: the static `oauth2_client_credentials` variant *would* have the server
> hold its own credential and act as itself — a different trust model that Contoso
> deliberately does **not** use, because it would let the server (and anyone who
> compromised it) read every user's files. Passthrough is the right tool when the
> backend's own per-user authorization should govern access.

## Two Kinds of Tools (`get_customer` / `get_customer_orders`)

Contoso curates exactly two read-only **script** tools, each keyed by
`customer_id`. A script tool runs a tiny engine-accurate JS body: it reads a
sheet's data block over the Graph worksheet-range API with a single `api.get`, then
returns the rows it wants. The natural, readable shape is **load the records, then
filter by the id column** — exactly what you'd write against any tabular source:

```toml
[[tools]]
name = "get_customer"
description = "Fetch one customer row (customer_id, name, segment, region) from the Contoso Customers sheet."
script = """
const resp = await api.get("/drives/CONTOSO_DRIVE/items/CUSTOMERS_ITEM/workbook/worksheets/Customers/range(address='A2:D7')?$select=values");
const rows = resp.values;
const matches = rows.filter(row => row[0] === args.customer_id);
return matches;
"""

[[tools]]
name = "get_customer_orders"
description = "Fetch the Orders rows (order_id, customer_id, order_date, amount) for one customer from the Contoso Orders sheet."
script = """
const resp = await api.get("/drives/CONTOSO_DRIVE/items/ORDERS_ITEM/workbook/worksheets/Orders/range(address='A2:D7')?$select=values");
const rows = resp.values;
const matches = rows.filter(row => row[1] === args.customer_id);
return matches;
"""
```

Both tools read the same whole-sheet data block (`A2:D7`) and differ only in which
column they filter on: `customer_id` is **column 0** in the Customers sheet and
**column 1** in the Orders sheet. Reading the block and calling `.filter()` keeps
the logic obvious and side-steps the engine's gotchas — the accurate JS subset has
no `Date` builtin and renders numeric arithmetic as floats, so *computing* a per-id
cell address (`A${idx+1}` would build `A2.0:D2.0`) would be both clumsy and wrong,
whereas filtering an already-read block just works. A customer with no orders falls
out naturally as an empty result — no special case needed. Everything richer than
these two reads is left to Code Mode.

## Resources & Prompts (Code Mode Context)

The server ships three inert static-markdown resources as Code Mode context, plus
a `start_code_mode` prompt that bundles them. These URIs are the **exact** strings
defined in the shipped `examples/contoso-m365.toml` (a typo here would rot the
docs and break prompt assembly):

```toml
[[resources]]
uri = "docs://contoso-m365/schema"
name = "Contoso Workbook Schema"
# ...sheet columns, the Graph range-read shape, the id->row addressing convention

[[resources]]
uri = "docs://contoso-m365/examples"
name = "Contoso Example Scripts"
# ...example Code Mode scripts for common Customers/Orders queries

[[resources]]
uri = "code-mode://learnings"
name = "Contoso Code Mode Learnings"
# ...tips: bind api.get to a const; no Date builtin; pin a reference date

[[prompts]]
name = "start_code_mode"
description = "Load all context needed for Code Mode script generation against the Contoso workbook"
include_resources = [
    "docs://contoso-m365/schema",
    "docs://contoso-m365/examples",
    "code-mode://learnings",
]
```

The three resource URIs — `docs://contoso-m365/schema`,
`docs://contoso-m365/examples`, and `code-mode://learnings` — describe the
workbook's columns, the Graph range-read shape, and the engine gotchas an agent
needs (the embedded JS engine has **no `Date` builtin**, so date logic uses a
pinned reference date). Every URI in `include_resources` must match a
`[[resources]] uri` exactly, or the prompt loads against a missing resource.

## The OpenAPI Spec Is Optional (D-03)

As in the base chapter, `--spec` is optional. Contoso ships a curated Graph
range-read spec so Code Mode can author scripts against the real contract, but the
two script tools dispatch from their `[[tools]]` bodies and need no spec to run.
If Code Mode is enabled with no spec supplied, the server warns and proceeds —
Code Mode simply runs without the contract resource rather than failing.

## What You Built — The Headline Query

The two curated tools answer the simple questions. The interesting question is the
one nobody curated: **"customers who bought more than 100 in the last 3 months."**
That is the headline Code Mode query, and it shows why the long tail belongs to
Code Mode rather than a hand-written tool. The agent writes a script that reads the
whole Customers and Orders blocks over the *same* Graph range-read API the tools
expose (still as the calling user, still forwarding their bearer), joins orders to
customers, filters orders to the trailing-3-month window, sums each customer's
in-window `amount`, and keeps those whose total exceeds 100.

Two properties make this trustworthy:

- **It is deterministic.** Against the shipped workbook, the query returns exactly
  `["C001", "C003", "C005"]`. C001 totals 140 (two in-window orders), C003 totals
  150, and C005 totals 200 — all above 100. C002 (40) is below the threshold;
  C004's only order falls one day *outside* the window (so despite a large amount
  it is excluded — proving the date filter, not the amount, drives the result);
  C006 has no orders at all.

- **It is stable across calendar time.** The embedded JS engine has no `Date`
  builtin, so the "last 3 months" window is computed from a **pinned reference
  date** (`2026-05-28`, giving the inclusive window `[2026-02-28, 2026-05-28]`)
  rather than a wall-clock `now`. Pinning the reference date is what makes the
  result set stable — the answer cannot silently drift as the calendar advances.
  C003 is the boundary case: its single order is dated exactly the window start
  and is classified *in*. This result is proven by an offline Code Mode test
  (Plan 03) that asserts the returned set equals the canonical expected set.

You now have an advanced, security-forward OpenAPI MCP server that:

- reads an existing Excel workbook on Microsoft 365 over Graph, with **no data
  migration**,
- enforces the **double lock** — admin-consent ceiling *and* per-user forwarded
  token — while holding **no standing credential**, so it can only act as the
  calling user,
- curates two read-only `customer_id`-keyed tools and leaves the long tail to a
  deterministic Code Mode headline query, and
- is changed by editing config — not by recompiling.

For the base config-driven walkthrough and the six outgoing-auth variants, see
[Config-Driven OpenAPI Servers](openapi-built-in-server.md); for the Code Mode
internals that make the long-tail path safe, revisit
[Chapter 12.9](ch12-9-code-mode.md).
