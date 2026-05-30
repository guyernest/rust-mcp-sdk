# OpenAPI over Microsoft Graph (Contoso M365)

The [Config-Driven OpenAPI Servers](./openapi-built-in-server.md) chapter built an
HTTP MCP server from config with an `api_key` backend. This chapter is the
advanced, security-forward sibling: **Contoso**, a fictional company whose finance
team keeps customers and orders in an Excel workbook on Microsoft 365 and wants to
query it through AI — without moving the data, copying credentials, or writing
Rust. The backend is Microsoft Graph, and every read must happen *as the signed-in
user*. The headline is the outgoing auth model: `oauth_passthrough`.

## What You'll Learn

- How to point a config-driven OpenAPI server at Microsoft Graph to read an
  existing Excel workbook — no data migration
- The `oauth_passthrough` **double lock**: admin-consent ceiling + per-user
  forwarded token, with no standing credential on the server
- Two read-only `customer_id`-keyed script tools (`get_customer`,
  `get_customer_orders`) and why the address math lives in the script
- The headline Code Mode query and why its result is deterministic and
  time-stable

## The Problem (Keep Your Excel Files, Connect Them to AI)

Contoso already has the data. `Customers.xlsx` lives on the "Sales" SharePoint
site: a **Customers** sheet (`customer_id`, `name`, `segment`, `region`) and an
**Orders** sheet (`order_id`, `customer_id`, `order_date`, `amount`), with
fictional customers like Northwind Traders, Fabrikam, and Tailspin Toys. The team
wants to keep the files where they are and connect them to AI through MCP. Deciding
*which* slice of the enormous Graph API to expose is a job for a business analyst,
not a Rust programmer — and that slice is curated in `config.toml`:

```toml
[server]
name = "contoso-m365"
version = "1.0.0"
description = "Contoso M365: per-user delegated OAuth (passthrough) reading a Customers/Orders Excel workbook over Microsoft Graph."

[backend]
base_url = "https://graph.microsoft.com/v1.0"
```

## Authenticate to the Backend — The Double Lock

This is the headline. The workbook holds sensitive data, so the server must never
read more than the asker is allowed to, and it must hold **no standing credential**
to steal. `oauth_passthrough` gives both with one config block, quoted verbatim
from the shipped `crates/pmcp-openapi-server/examples/contoso-m365.toml`:

```toml
[backend.auth]
type = "oauth_passthrough"
target_header = "Authorization"
required = true
```

Two locks, both engaging on every request:

1. **The admin sets the ceiling.** A Microsoft 365 admin consents *once* to a
   bounded scope (e.g. delegated `Files.Read`). That admin consent is the
   **ceiling** — the most the server could ever request on a user's behalf, and it
   cannot be exceeded.
2. **The user's forwarded token further restricts.** At request time the server
   captures the signed-in user's inbound `Authorization: Bearer <token>` and
   forwards it to Graph as the `target_header`. Because that per-user token only
   carries that user's effective access, it **further restricts** every read to
   the files the user can already see. The admin ceiling never widens an
   individual user's reach.

The key property: the server **holds no standing credentials** — no client secret,
no cached service token, no API key — so the only bearer reaching Graph is the
caller's own, and the server can only ever **act as the calling user**. Least
privilege by default.

> The static `oauth2_client_credentials` variant would instead have the server hold
> its own credential and act as itself — a different trust model Contoso
> deliberately avoids, since it would expose every user's files. Passthrough is the
> right choice when the backend's per-user authorization should govern access.

## Two Kinds of Tools (`get_customer` / `get_customer_orders`)

Contoso curates two read-only **script** tools, each keyed by `customer_id`. A
script tool runs a tiny engine-accurate JS body that templates the Graph
worksheet-range URL and issues one `api.get` (the connector does literal string
substitution, no arithmetic, so the `customer_id → range-address` mapping lives in
the script):

```toml
[[tools]]
name = "get_customer"
description = "Fetch one customer row (customer_id, name, segment, region) from the Contoso Customers sheet."
script = """
let addr = "";
if (args.customer_id === "C001") { addr = "A2:D2"; }
else if (args.customer_id === "C002") { addr = "A3:D3"; }
else if (args.customer_id === "C003") { addr = "A4:D4"; }
else if (args.customer_id === "C004") { addr = "A5:D5"; }
else if (args.customer_id === "C005") { addr = "A6:D6"; }
else if (args.customer_id === "C006") { addr = "A7:D7"; }
const r = await api.get(`/drives/CONTOSO_DRIVE/items/CUSTOMERS_ITEM/workbook/worksheets/Customers/range(address='${addr}')?$select=values`);
return r.values;
"""

[[tools]]
name = "get_customer_orders"
description = "Fetch the contiguous Orders block (order_id, customer_id, order_date, amount) for one customer from the Contoso Orders sheet."
script = """
let ordersAddr = "";
if (args.customer_id === "C001") { ordersAddr = "A2:D3"; }
else if (args.customer_id === "C002") { ordersAddr = "A4:D4"; }
else if (args.customer_id === "C003") { ordersAddr = "A5:D5"; }
else if (args.customer_id === "C004") { ordersAddr = "A6:D6"; }
else if (args.customer_id === "C005") { ordersAddr = "A7:D7"; }
const r = await api.get(`/drives/CONTOSO_DRIVE/items/ORDERS_ITEM/workbook/worksheets/Orders/range(address='${ordersAddr}')?$select=values`);
return r.values;
"""
```

`get_customer` maps `C00N` to its Customers row (`C001 → A2:D2`);
`get_customer_orders` maps it to that customer's contiguous Orders block
(`C001 → A2:D3`). The mapping is a literal `if`/`else if` chain on purpose: the
engine-accurate JS subset renders arithmetic as floats (`A${idx+1}` →
`A2.0:D2.0`) and has no object string-indexing, so literal addresses are the
engine-safe form. Anything richer is left to Code Mode.

## Resources & Prompts (Code Mode Context)

The server ships three inert static-markdown resources plus a `start_code_mode`
prompt bundling them. These URIs are the **exact** strings from the shipped
`examples/contoso-m365.toml` — a typo would break prompt assembly and rot the docs:

```toml
[[resources]]
uri = "docs://contoso-m365/schema"
name = "Contoso Workbook Schema"

[[resources]]
uri = "docs://contoso-m365/examples"
name = "Contoso Example Scripts"

[[resources]]
uri = "code-mode://learnings"
name = "Contoso Code Mode Learnings"

[[prompts]]
name = "start_code_mode"
description = "Load all context needed for Code Mode script generation against the Contoso workbook"
include_resources = [
    "docs://contoso-m365/schema",
    "docs://contoso-m365/examples",
    "code-mode://learnings",
]
```

`docs://contoso-m365/schema`, `docs://contoso-m365/examples`, and
`code-mode://learnings` describe the columns, the Graph range-read shape, and the
engine gotchas (notably: the embedded JS engine has **no `Date` builtin**, so date
logic uses a pinned reference date). Every `include_resources` URI must match a
`[[resources]] uri` exactly.

## The Headline Query

The curated tools answer simple questions; the interesting one is uncurated:
**"customers who bought more than 100 in the last 3 months."** That headline Code
Mode query reads the whole Customers and Orders blocks over the *same* Graph
range-read API the tools expose (still as the calling user, still forwarding their
bearer), joins orders to customers, filters to the trailing-3-month window, sums
each customer's in-window `amount`, and keeps totals above 100.

Two properties make it trustworthy:

- **Deterministic.** Against the shipped workbook it returns exactly
  `["C001", "C003", "C005"]` (C001 = 140, C003 = 150, C005 = 200). C002 (40) is
  below the threshold; C004's only order falls one day *outside* the window (so the
  date filter, not the amount, excludes it); C006 has no orders.
- **Time-stable.** With no `Date` builtin, the "last 3 months" window is computed
  from a **pinned reference date** (`2026-05-28` → inclusive window
  `[2026-02-28, 2026-05-28]`), never a wall-clock `now`. C003 is the boundary case
  — its order is dated exactly the window start and is classified *in*. An offline
  Code Mode test (Plan 03) asserts the returned set equals the canonical expected
  set, so it cannot rot as the calendar advances.

## Exercise: Add a Segment Filter

**Goal:** extend the Contoso surface with an analyst-curated read.

1. Point the config at the Graph base URL with `[backend.auth] type =
   "oauth_passthrough"` and confirm no standing credential appears in the file.
2. Add a third script tool `get_customers_by_segment` that reads the all-customers
   block (`A2:D7`) and returns rows whose `segment` matches an `args.segment`
   parameter. Keep all logic in the script (literal address, no arithmetic).
3. **Stretch:** write a Code Mode script that answers "Enterprise customers who
   bought more than 100 in the last 3 months" by joining the segment filter to the
   headline windowed-sum, using a pinned reference date.

**Success criteria:** the segment tool returns only matching rows; the stretch
query is deterministic against the shipped workbook; the config holds no standing
credential and the server reads only as the calling user.

## Key Takeaways

- A config-driven OpenAPI server can read an existing Microsoft 365 Excel workbook
  over Graph with **no data migration** — the business analyst curates the slice in
  `config.toml`.
- `oauth_passthrough` is a **double lock**: admin-consent **ceiling** *and*
  per-user **forwarded token** that further restricts, with **no standing
  credential** — the server can only act as the calling user.
- Two read-only `customer_id`-keyed script tools (`get_customer`,
  `get_customer_orders`) keep the address math in engine-safe literal JS; the long
  tail goes to Code Mode.
- The headline query ("bought more than 100 in the last 3 months") returns a
  deterministic `["C001", "C003", "C005"]`, stable across calendar time because the
  reference date is pinned (the engine has no `Date` builtin).
