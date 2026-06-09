# Chapter 16: Deployment Strategies

`cargo pmcp deploy` ships several first-class deployment targets — AWS Lambda, Google
Cloud Run, **Azure Container Apps**, Cloudflare Workers, and pmcp.run. Every target follows
the same CLI-first loop: scaffold a server, run `deploy init --target <name>`, authenticate
with the platform CLI, then `deploy`. This chapter leads with Azure Container Apps because
it best illustrates the container path with the least ceremony.

## Azure Container Apps

Azure Container Apps (ACA) runs your MCP server as a managed container behind auto-HTTPS
ingress. Its standout property versus the Cloud Run target: **no local Docker is required**.
The deploy primitive is `az containerapp up --source`, which ships your build context to
**Azure Container Registry (ACR) cloud-build**. The only prerequisite is the `az` CLI.

### CLI workflow (start here)

```bash
# 1. Scaffold a server — the template ships the two ingress-safe defaults (see below)
cargo pmcp new my-mcp-server
cd my-mcp-server
cargo pmcp add server calculator --template complete

# 2. Generate the Azure deploy artifacts (Dockerfile + .dockerignore + deploy.toml stub)
cargo pmcp deploy init --target azure-container-apps

# 3. Authenticate (management-plane calls need a fresh interactive token)
az login

# 4. Deploy — ACR cloud-builds the Dockerfile and provisions the Container App
cargo pmcp deploy --target-type azure-container-apps
```

`init` writes a proven multi-stage `Dockerfile` (`rust:1-slim-bookworm` builder →
`debian:bookworm-slim` runtime, non-root, `ENV PORT=8080`, a plain `cargo build --release`
— never `--locked` without a shipped `Cargo.lock`), a `.dockerignore`, and a
`.pmcp/deploy.toml` stub. The deployed MCP endpoint is served at the root:
`https://<app>.<env-suffix>.<region>.azurecontainerapps.io/`.

### `[azure]` configuration

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

Precedence is **`ENV (AZURE_*) > [azure] section > built-in defaults`**. Numeric env
overrides (`AZURE_TARGET_PORT`, `AZURE_MIN_REPLICAS`) are validated before they reach `az`.

### Two server requirements the scaffold ships

A server behind ACA ingress MUST do two things — `cargo pmcp new` bakes both into the
generated template, but a hand-rolled server must set them itself:

1. **`allowed_origins: Some(AllowedOrigins::any())`** — otherwise pmcp's DNS-rebinding
   guard returns `403 Forbidden: Host header not in allowed origins` for every request
   behind ingress (`None` = localhost-only).
2. **Bind `0.0.0.0:$PORT`** (not `127.0.0.1`), honoring `$PORT`/8080, so ingress can reach
   the container.

`cargo pmcp deploy init` prints a warning surfacing both.

### Operational notes

- Provider registration for `Microsoft.App`, `Microsoft.OperationalInsights`, and
  `Microsoft.ContainerRegistry` is **awaited** (`--wait`) before environment creation — a
  one-time 1–5 minute cost on a cold subscription.
- `az` long-polls drop intermittently (`RemoteDisconnected`); the deploy runner re-checks
  `provisioningState` and only treats `Failed` as a real failure.
- No `/health` route is needed — pmcp mounts MCP at `/` and ACA's default TCP probe
  suffices.

### Security: the broad-origin default is a starting point, not a destination

The scaffold default `allowed_origins: Some(AllowedOrigins::any())` and the ingress CORS
`*` that `deploy` enables are **proxy-safe but broad** — they accept browser requests from
any origin. That is the right *default* (a server that 403s behind every reverse proxy is
the worse failure mode), but production deployments should harden once the FQDN is known:

1. Restrict `allowed_origins` to the deployment FQDN (not `any()`) and tighten the ingress
   CORS allowed-origins to the same host.
2. Enable authentication — public ingress with broad CORS and no auth means anyone with the
   URL can call your tools. Wire pmcp's auth layer.

Azure Entra ID integration is a planned future phase and is **out of scope** today; this
target gives you a reachable, TLS-terminated MCP server, and locking it to an audience is
the explicit next step.

> Full reference: [`cargo-pmcp/DEPLOYMENT.md` § Azure Container Apps](https://github.com/paiml/rust-mcp-sdk/blob/main/cargo-pmcp/DEPLOYMENT.md).

## Other targets

- **AWS Lambda** — serverless, OAuth/Cognito built in, CDK-generated infra.
  `cargo pmcp deploy init --target aws-lambda --oauth cognito`.
- **Google Cloud Run** — container target with local `docker buildx` → `gcloud run deploy`.
- **Cloudflare Workers** — WASM build for edge/global low-latency deployments.
- **pmcp.run** — managed hosting for quick sharing.

Each shares the `cargo pmcp deploy` CLI surface (`init`, `deploy`, `logs`, `outputs`,
`rollback`, `destroy`); the per-target config lives under its section in `deploy.toml`.
