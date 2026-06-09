# Azure Container Apps Deployment

Azure Container Apps (ACA) runs your MCP server as a managed container behind automatic
HTTPS ingress, with scale-to-zero, revision-based rollouts, and a Container Apps
environment that gives you Log Analytics and Dapr if you want them. For MCP servers it is
the closest Azure analog to Google Cloud Run — but with one standout property: the
`cargo pmcp` deploy path needs **no local Docker**. Azure Container Registry (ACR)
cloud-builds your image from source.

## Learning Objectives

By the end of this chapter, you will:
- Deploy an MCP server to Azure Container Apps with a single `cargo pmcp deploy` command
- Understand the `az containerapp up --source` cloud-build primitive (no local Docker)
- Configure the `[azure]` section of `deploy.toml` and the `AZURE_*` env overrides
- Recognize the two ingress requirements every container MCP server must satisfy
- Harden a deployment: restrict origins to the FQDN and add authentication
- Know when to choose Azure Container Apps over Cloud Run or Lambda

## Why Azure Container Apps for MCP?

### The cloud-build advantage

```
┌─────────────────────────────────────────────────────────────────────┐
│              Container Deploy Path Comparison                        │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  Google Cloud Run                 Azure Container Apps              │
│  ────────────────                 ─────────────────────            │
│  docker buildx (LOCAL)            az containerapp up --source       │
│  → push to gcr.io                 → ACR cloud-build (NO local Docker)│
│  gcloud run deploy                managed environment + ingress     │
│  needs Docker installed           needs only the `az` CLI           │
│                                                                     │
│  Best for:                        Best for:                         │
│  GCP ecosystem                    Azure ecosystem                   │
│  local image control              zero-Docker dev machines / CI     │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

Because the build happens in ACR, `cargo pmcp`'s Azure target only requires the `az` CLI
to be installed and authenticated — its `is_available()` check does not look for Docker.
On a developer laptop or a minimal CI runner with no Docker daemon, this is a real
ergonomic win over the Cloud Run target.

### When to choose Azure Container Apps

| Requirement | Why Azure Container Apps |
|-------------|--------------------------|
| No local Docker | ACR cloud-builds from `--source` |
| Azure ecosystem | Native to the subscription, Log Analytics, managed identity |
| Container flexibility | Full Linux image, any dependency |
| Auto HTTPS ingress | TLS-terminated FQDN issued on deploy |
| Scale to zero | `min-replicas 0` (we set `1` by default to avoid cold starts) |
| Revision rollback | `ingress traffic set --revision-weight` |

## The CLI Workflow (Start Here)

The entire loop is four commands. This is the framing you should teach and reach for first.

```bash
# 1. Scaffold a server — the template ships the two ingress-safe defaults (see below)
cargo pmcp new my-mcp-server
cd my-mcp-server
cargo pmcp add server calculator --template complete

# 2. Generate the Azure deploy artifacts
cargo pmcp deploy init --target azure-container-apps
#   writes: Dockerfile, .dockerignore, .pmcp/deploy.toml (type = "azure-container-apps")
#   skips:  AWS credentials + CDK entirely

# 3. Authenticate — management-plane calls need a fresh interactive token
az login            # pick the target subscription if prompted

# 4. Deploy — ACR cloud-builds the Dockerfile and provisions the Container App
cargo pmcp deploy --target-type azure-container-apps
```

After a successful deploy your MCP endpoint is served at the **root** of the issued FQDN:

```
https://<app>.<env-suffix>.<region>.azurecontainerapps.io/
```

### What `init` generates

`cargo pmcp deploy init --target azure-container-apps` produces a proven multi-stage
Dockerfile:

```dockerfile
# Builder
FROM rust:1-slim-bookworm AS builder
RUN apt-get update && apt-get install -y build-essential pkg-config && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY . .
RUN cargo build --release           # NOT --locked: no Cargo.lock is shipped in the context

# Runtime
FROM debian:bookworm-slim
RUN useradd -m -u 1000 appuser
COPY --from=builder /app/target/release/<binary> /usr/local/bin/server
USER appuser
ENV PORT=8080
CMD ["server"]
```

> **Why no `--locked`?** Spike 007 hit a build failure: `.dockerignore` excludes
> `Cargo.lock`, so `cargo build --locked` had nothing to lock against. The generated
> Dockerfile uses a plain `cargo build --release`. If you want reproducible builds, ship
> the lockfile and adjust `.dockerignore`.

## Configuration: the `[azure]` section

`init` leaves `[azure]` at defaults. Add or override keys in `.pmcp/deploy.toml`:

```toml
[target]
type = "azure-container-apps"

[server]
name = "calculator-server"

[azure]
resource_group = "calculator-server-rg"   # default: <server-name>-rg
location        = "eastus"                 # default: eastus
environment     = "calculator-server-env"  # default: <server-name>-env
target_port     = 8080                     # default: 8080
min_replicas    = 1                        # default: 1
```

### Precedence: ENV > section > defaults

Every setting resolves in this order:

| Setting | Env override | `[azure]` key | Default |
|---------|--------------|---------------|---------|
| Resource group | `AZURE_RESOURCE_GROUP` | `resource_group` | `<server-name>-rg` |
| Environment | `AZURE_CONTAINERAPP_ENV` | `environment` | `<server-name>-env` |
| Location | `AZURE_LOCATION` | `location` | `eastus` |
| Target port | `AZURE_TARGET_PORT` (u16) | `target_port` | `8080` |
| Min replicas | `AZURE_MIN_REPLICAS` (u32) | `min_replicas` | `1` |

The numeric env overrides are **validated** — an unparseable `AZURE_TARGET_PORT` is a
clear named error, never a raw string handed to `az`. An absent/empty `[azure]` section
serialises byte-identically to a pre-Azure `deploy.toml`, so adding the target never
disturbs an existing AWS or Cloud Run config.

## The Two Ingress Requirements (Don't Skip This)

Spike 007 deployed a hand-built server and got `403` on **every** call. Two settings are
mandatory for any container MCP server behind ingress. `cargo pmcp new` now bakes both into
the generated template (a global default across all targets), but if you hand-roll your
server you must set them yourself:

1. **`allowed_origins: Some(AllowedOrigins::any())`** — `None` means localhost-only, so
   pmcp's DNS-rebinding guard rejects the public FQDN Host header with
   `403 Forbidden: Host header not in allowed origins`.

   ```rust
   use pmcp::{AllowedOrigins, StreamableHttpServerConfig};

   let config = StreamableHttpServerConfig {
       allowed_origins: Some(AllowedOrigins::any()), // proxy-safe default
       // ...
   };
   ```

2. **Bind `0.0.0.0:$PORT`** (`Ipv4Addr::UNSPECIFIED`), honoring `$PORT` (default 8080) —
   binding `127.0.0.1` means ingress cannot reach the container.

`cargo pmcp deploy init` prints a warning surfacing both requirements, so you find out
before you spend the cloud-build time, not after a `403`.

## Comparison with Google Cloud Run

| Aspect | Google Cloud Run | Azure Container Apps |
|--------|------------------|----------------------|
| Build | local `docker buildx` → push | `az containerapp up --source` (ACR cloud-build) |
| Local Docker required | Yes | **No** |
| CORS | application-level | ingress-level (`az containerapp ingress cors enable`) |
| Provider setup | enable APIs (`gcloud services enable`) | register providers **with `--wait`** (cold-sub one-time) |
| Poll reliability | stable | `az` long-polls drop (`RemoteDisconnected`) — re-check state |
| Endpoint | `*.run.app/mcp` | `*.azurecontainerapps.io/` (MCP at root) |

Two Azure-specific gotchas the `cargo pmcp` target already handles for you:

- **Provider registration is awaited.** `Microsoft.App`, `Microsoft.OperationalInsights`,
  and `Microsoft.ContainerRegistry` are registered with `--wait` before the environment is
  created. A cold subscription that skips this races and fails `env create`. It is a
  one-time 1–5 minute cost.
- **Dropped long-polls are not failures.** `az` intermittently drops a long-poll connection
  (`RemoteDisconnected` / read-timeout / connection-reset). The deploy runner re-checks
  `properties.provisioningState` and continues unless the state is actually `Failed`.

## Lifecycle Commands

All share the `cargo pmcp deploy --target-type azure-container-apps` surface:

```bash
# Stream logs
cargo pmcp deploy --target-type azure-container-apps logs --follow --tail 100

# Show outputs (probes the FQDN)
cargo pmcp deploy --target-type azure-container-apps outputs

# Roll back to a prior revision (ingress traffic set --revision-weight <rev>=100)
cargo pmcp deploy --target-type azure-container-apps rollback --version <revision>

# Tear down (group delete --no-wait; clean also removes Dockerfile/.dockerignore)
cargo pmcp deploy --target-type azure-container-apps destroy
```

## Securing the Deployment

A freshly deployed server is reachable — and **broadly open**. The scaffold default
`allowed_origins: Some(AllowedOrigins::any())` together with the ingress CORS `*` that
`deploy` enables are **proxy-safe but broad**: they accept browser requests from any origin.

That breadth is the correct *default*. A server that 403s behind every reverse proxy is the
worse failure mode, and at deploy time you do not yet know your FQDN. But it is the starting
point, not the destination. Once the FQDN is issued, harden:

### 1. Restrict origins to the FQDN

Replace `any()` with the actual deployment host, and tighten ingress CORS to match:

```rust
let config = StreamableHttpServerConfig {
    allowed_origins: Some(AllowedOrigins::new([
        "https://my-mcp-server.delightfulpebble.eastus.azurecontainerapps.io",
    ])),
    // ...
};
```

```bash
az containerapp ingress cors enable \
  --name my-mcp-server --resource-group my-mcp-server-rg \
  --allowed-origins 'https://my-mcp-server.delightfulpebble.eastus.azurecontainerapps.io' \
  --allowed-methods 'GET,POST,DELETE,OPTIONS' --allowed-headers '*'
```

### 2. Add authentication

Public ingress with broad CORS and no auth means anyone who learns the URL can call your
tools. Wire pmcp's auth layer (OAuth / token validation) — see the authentication chapters
and the pmcp auth docs for the patterns. Authentication is what turns "reachable" into
"reachable by the right callers."

> **Azure Entra ID** integration (managed identity / OAuth passthrough native to Container
> Apps) is a **planned future phase and is out of scope** for this target today. The target
> ships you a reachable, TLS-terminated MCP server; restricting origins and adding auth are
> the production hardening steps you own.

## Summary

Azure Container Apps gives MCP servers a managed, auto-HTTPS container runtime with a
zero-local-Docker deploy path:

- `cargo pmcp deploy init --target azure-container-apps` → `az login` →
  `cargo pmcp deploy --target-type azure-container-apps` is the whole loop.
- `az containerapp up --source` cloud-builds in ACR — no Docker on your machine.
- Configure via `[azure]` with `ENV > section > defaults` precedence.
- The scaffold ships the two ingress requirements (`allowed_origins::any()` + `0.0.0.0`);
  hand-rolled servers must set them or every request 403s.
- Provider registration is awaited and dropped long-polls are tolerated for you.
- The broad-origin default is proxy-safe but a *starting point* — restrict to the FQDN and
  add auth for production.

## Knowledge Check

- Why does the Azure target not require local Docker, and what builds the image instead?
- What two settings must every container MCP server have to survive ingress, and what
  symptom appears if `allowed_origins` is wrong?
- In what order do `AZURE_TARGET_PORT`, the `[azure]` `target_port` key, and the default
  resolve?
- What are the two production hardening steps once the FQDN is known, and which one is
  explicitly out of scope for this target?

---

*Continue to [Part IV: Testing Strategies](../part4-testing/ch11-local-testing.md) →*
