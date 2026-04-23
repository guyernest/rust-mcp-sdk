# Declarative IAM for MCP servers

**Status:** Shipped in `cargo-pmcp` 0.10.0 (April 2026) · **Audience:** Server authors deploying to AWS Lambda via `cargo pmcp deploy`.

Declare the AWS permissions your MCP server needs in `.pmcp/deploy.toml`. `cargo pmcp deploy` translates them into `mcpFunction.addToRolePolicy(...)` calls in the generated CDK stack — no hand-written bolt-on stacks, no guessing the CFN-generated role name.

This guide is the **task-oriented how-to**. For schema reference, action-translation tables, and every validation rule, see [DEPLOYMENT.md § IAM Declarations](../DEPLOYMENT.md#iam-declarations-iam-section).

---

## TL;DR

```toml
# .pmcp/deploy.toml
[[iam.tables]]
name = "cost-coach-tenants"
actions = ["readwrite"]
include_indexes = true

[[iam.buckets]]
name = "cost-coach-snapshots"
actions = ["readwrite"]

[[iam.statements]]
effect = "Allow"
actions = ["secretsmanager:GetSecretValue"]
resources = ["arn:aws:secretsmanager:us-west-2:*:secret:cost-coach/*"]
```

```bash
cargo pmcp validate deploy     # dry-run: catches footguns before any AWS call
cargo pmcp deploy              # regenerates deploy/lib/stack.ts from config, then `cdk deploy`
```

That's it. Skip to [Recipes](#recipes) for copy-paste starting points.

---

## The three sections

The `[iam]` block has three repeated tables — use whichever match your access pattern.

| Section | When to use | What you get |
|---|---|---|
| `[[iam.tables]]` | DynamoDB tables (incl. GSI/LSI access) | 4 or 8 `dynamodb:*` actions, `read`/`write`/`readwrite` sugar |
| `[[iam.buckets]]` | S3 object-level access | 1–3 `s3:*` actions on `arn:aws:s3:::NAME/*` |
| `[[iam.statements]]` | Everything else (SecretsManager, KMS, Lambda invoke, custom) | Raw `PolicyStatement` passthrough after validation |

All three are optional; an omitted section = no permissions of that kind.

---

## Workflow

### 1. Scaffold the deploy project (one-time)

```bash
cargo pmcp deploy init --target aws-lambda
```

Creates `.pmcp/deploy.toml` and the `deploy/` CDK project. The initial config has **no `[iam]` section** — the Lambda gets the platform-composition permissions only (DynamoDB `McpServer` table read, cross-Lambda invoke).

### 2. Declare your IAM needs

Edit `.pmcp/deploy.toml` and add an `[iam]` block. See [Recipes](#recipes) below for starting points.

### 3. Dry-run validate

```bash
cargo pmcp validate deploy
```

Fails fast on misconfiguration **before touching AWS**. Validates:

- Sugar keywords (`read` / `write` / `readwrite`) are spelled correctly
- `[[iam.statements]]` `effect` is `"Allow"` or `"Deny"`
- Action strings match `^[a-z0-9-]+:[A-Za-z0-9*]+$`
- No wildcard escalation (`Allow` + `actions=["*"]` + `resources=["*"]`)
- `name`, `actions`, `resources` are non-empty where required
- Warns on unknown service prefixes and cross-account ARN pins

Hard errors return non-zero; warnings print to stderr but don't block.

### 4. Deploy

```bash
cargo pmcp deploy
```

`cargo pmcp deploy` now:

1. Loads `.pmcp/deploy.toml`.
2. Runs `iam::validate(&config.iam)` (the same gate as `validate deploy`). Fail-closed — aborts before any AWS call if validation fails.
3. Builds your Lambda binary.
4. **Regenerates `deploy/lib/stack.ts`** from the loaded config, splicing your `[iam]` declarations in at a single seam. Re-run `cargo pmcp deploy` whenever you change `.pmcp/deploy.toml` — the stack file is sourced from `.pmcp/deploy.toml`, not hand-edited.
5. Runs `cdk deploy`.

### 5. Inspect what gets emitted

To see the exact TypeScript your declarations translate to **without deploying**, run the reference example:

```bash
cargo run -p cargo-pmcp --example deploy_with_iam
```

For a config like the [TL;DR](#tldr), it prints:

```typescript
    mcpFunction.addToRolePolicy(new iam.PolicyStatement({
      effect: iam.Effect.ALLOW,
      actions: ['dynamodb:GetItem', 'dynamodb:Query', 'dynamodb:Scan', 'dynamodb:BatchGetItem',
                'dynamodb:PutItem', 'dynamodb:UpdateItem', 'dynamodb:DeleteItem', 'dynamodb:BatchWriteItem'],
      resources: [
        `arn:aws:dynamodb:${this.region}:${this.account}:table/cost-coach-tenants`,
        `arn:aws:dynamodb:${this.region}:${this.account}:table/cost-coach-tenants/index/*`,
      ],
    }));
    mcpFunction.addToRolePolicy(new iam.PolicyStatement({
      effect: iam.Effect.ALLOW,
      actions: ['s3:GetObject', 's3:PutObject', 's3:DeleteObject'],
      resources: [`arn:aws:s3:::cost-coach-snapshots/*`],
    }));
    mcpFunction.addToRolePolicy(new iam.PolicyStatement({
      effect: iam.Effect.ALLOW,
      actions: ['secretsmanager:GetSecretValue'],
      resources: ['arn:aws:secretsmanager:us-west-2:*:secret:cost-coach/*'],
    }));
```

---

## Recipes

### DynamoDB table + GSI/LSI

```toml
[[iam.tables]]
name = "cost-coach-tenants"
actions = ["readwrite"]
include_indexes = true    # grants table/NAME/index/* for GSI/LSI queries
```

**Action mapping:** `read` → `GetItem`, `Query`, `Scan`, `BatchGetItem` (4). `write` → `PutItem`, `UpdateItem`, `DeleteItem`, `BatchWriteItem` (4). `readwrite` → union (8).

**Tip:** separate tables get separate entries — repeat `[[iam.tables]]` as many times as needed.

### DynamoDB, read-only

```toml
[[iam.tables]]
name = "cost-coach-catalog"
actions = ["read"]
# include_indexes defaults to false — add `include_indexes = true` if you query GSIs
```

### S3 bucket, object-level RW

```toml
[[iam.buckets]]
name = "cost-coach-snapshots"
actions = ["readwrite"]
```

**Action mapping:** `read` → `s3:GetObject`. `write` → `s3:PutObject`, `s3:DeleteObject`. `readwrite` → union (3). Resources are always `arn:aws:s3:::NAME/*` (object-level).

**Bucket-level operations** (`s3:ListBucket`, `s3:GetBucketLocation`) are intentionally not in the sugar — declare them via `[[iam.statements]]`:

```toml
[[iam.statements]]
effect = "Allow"
actions = ["s3:ListBucket"]
resources = ["arn:aws:s3:::cost-coach-snapshots"]
```

### SecretsManager read under a narrow prefix

```toml
[[iam.statements]]
effect = "Allow"
actions = ["secretsmanager:GetSecretValue"]
resources = ["arn:aws:secretsmanager:us-west-2:*:secret:cost-coach/*"]
```

The trailing `/*` in the resource ARN is critical — without it you'd grant access to every secret in the account.

### KMS decrypt for a specific CMK

```toml
[[iam.statements]]
effect = "Allow"
actions = ["kms:Decrypt", "kms:GenerateDataKey"]
resources = ["arn:aws:kms:us-west-2:123456789012:key/abcd-..."]
```

A 12-digit account ID in a resource ARN triggers a cross-account warning at validate time — verify it matches your deploy target.

### Invoke another Lambda (foundation-server composition)

The platform already grants `lambda:InvokeFunction` on `function:*` for domain-server composition. Only declare this if you need to **restrict** to specific functions:

```toml
[[iam.statements]]
effect = "Allow"
actions = ["lambda:InvokeFunction"]
resources = [
  "arn:aws:lambda:us-west-2:*:function:cost-coach-pricing-*",
  "arn:aws:lambda:us-west-2:*:function:cost-coach-billing-*",
]
```

### Multiple tables + buckets + raw statements in one server

```toml
[[iam.tables]]
name = "cost-coach-tenants"
actions = ["readwrite"]
include_indexes = true

[[iam.tables]]
name = "cost-coach-audit-log"
actions = ["write"]         # append-only

[[iam.buckets]]
name = "cost-coach-snapshots"
actions = ["readwrite"]

[[iam.buckets]]
name = "cost-coach-reports"
actions = ["read"]          # dashboard reads only

[[iam.statements]]
effect = "Allow"
actions = ["secretsmanager:GetSecretValue"]
resources = ["arn:aws:secretsmanager:us-west-2:*:secret:cost-coach/*"]

[[iam.statements]]
effect = "Allow"
actions = ["ses:SendEmail"]
resources = ["arn:aws:ses:us-west-2:*:identity/*@costcoach.example.com"]
```

---

## Migrating from a hand-written bolt-on stack

Before phase 76, teams hand-wrote a separate CDK stack that looked up the Lambda role by name and attached policies. That pattern is brittle — the role name changes on redeploy — and the recommended path is to migrate to declarative IAM.

### Step 1 — translate your bolt-on to `.pmcp/deploy.toml`

Map each `addToRolePolicy` call in your bolt-on to an entry:

| Your bolt-on pattern | Replace with |
|---|---|
| `table.grantReadWriteData(role)` | `[[iam.tables]]` with `actions = ["readwrite"]` |
| `bucket.grantReadWrite(role)` | `[[iam.buckets]]` with `actions = ["readwrite"]` |
| A custom `PolicyStatement({ effect, actions, resources })` | `[[iam.statements]]` with the same three fields |

### Step 2 — validate and dry-run

```bash
cargo pmcp validate deploy
cargo run -p cargo-pmcp --example deploy_with_iam    # (not strictly necessary — confirms the rendered TS)
```

### Step 3 — tear down the bolt-on stack, redeploy with declarations

```bash
# After confirming your declarations produce the TS you expect:
cdk destroy my-bolt-on-stack          # remove the old hand-written stack
cargo pmcp deploy                     # redeploy with declarative IAM
```

---

## External stacks that still need the role — `McpRoleArn`

If another CDK stack in your account (e.g. a separate data-layer stack that `grantReadWriteData`s to the MCP Lambda) still needs a reference to the role, **don't** look it up by name. Both generated stacks now emit a stable CFN export:

```
exportName: pmcp-${serverName}-McpRoleArn
```

Consume it with `Fn::ImportValue`:

```typescript
// In your external stack:
const mcpRole = iam.Role.fromRoleArn(
  this,
  'McpRole',
  cdk.Fn.importValue(`pmcp-${serverName}-McpRoleArn`),
);

// Then grant whatever:
myTable.grantReadWriteData(mcpRole);
```

This is stable across redeploys and survives role-name changes.

---

## Troubleshooting

| Message | What it means | Fix |
|---|---|---|
| `[iam.statements][0]: Allow + actions=["*"] + resources=["*"] is a wildcard escalation footgun` | You tried to grant `*:*` on `*` — blocked by hard-error validation (T-76-02). | Tighten either actions or resources. If you genuinely need broad access, enumerate the service prefixes (`["s3:*", "dynamodb:*"]`) and keep `resources = ["*"]` — still broad, but no longer the footgun shape. |
| `[iam.tables][0] 'foo': unknown sugar keyword 'readonly'` | Sugar keywords are `read` / `write` / `readwrite` only. | Use `actions = ["read"]` (not `"readonly"`). |
| `[iam.statements][0]: effect must be 'Allow' or 'Deny', got 'allow'` | `effect` is case-sensitive. | Use exactly `"Allow"` or `"Deny"`. |
| `[iam.statements][0]: action 'dynamodb:get_item' does not match ^[a-z0-9-]+:[A-Za-z0-9*]+$` | Action names use AWS's conventional mixed case. | Use `"dynamodb:GetItem"`, not `"dynamodb:get_item"`. |
| `[iam.statements][0]: actions must not be empty` | An empty `actions` array. | Add at least one entry or remove the `[[iam.statements]]` block. |
| ⚠ `unknown service prefix 'foo' in action 'foo:Bar'` | The prefix isn't in the curated 40-service list. **Warning only** — not blocking. | Verify the prefix is real. Typos (`"dynaodb:"`, `"s3i:"`) almost always land here. |
| ⚠ `resource '...' pins a specific AWS account '123456789012'` | A resource ARN has a 12-digit account segment. **Warning only**. | Verify it matches your deploy target, or use `*` / omit the account segment for account-agnostic ARNs. |
| `IAM validation failed — fix .pmcp/deploy.toml before deploying` | Any of the above hard-error rules fired at `cargo pmcp deploy` time. | Run `cargo pmcp validate deploy` to see the specific rule that tripped. |

### `cargo pmcp deploy` succeeded but my Lambda doesn't have the permissions I declared

If you upgraded from an earlier version, check that `deploy/lib/stack.ts` was regenerated. Version 0.10.0+ regenerates it automatically every deploy. If you're pinned to an older version, upgrade:

```bash
cargo install cargo-pmcp --force
```

---

## Backward compatibility

Servers with **no `[iam]` section** continue to emit byte-identical `deploy/lib/stack.ts` (modulo the additive `McpRoleArn` CFN output). No action required for existing deployments until you want to declare IAM.

---

## See also

- [DEPLOYMENT.md § IAM Declarations](../DEPLOYMENT.md#iam-declarations-iam-section) — schema reference, full translation tables, every validation rule
- [`cargo-pmcp/examples/deploy_with_iam.rs`](../examples/deploy_with_iam.rs) — runnable parse → validate → render walkthrough
- [`cargo-pmcp/examples/fixtures/cost-coach.deploy.toml`](../examples/fixtures/cost-coach.deploy.toml) — the reference fixture this guide is built around
- [CHANGELOG § 0.10.0](../CHANGELOG.md) — full release notes (additions, changed behaviour, security mitigations)
- [`cargo pmcp validate deploy`](commands/validate.md#validate-deploy) — command reference
- [`cargo pmcp deploy`](commands/deploy.md) — command reference
