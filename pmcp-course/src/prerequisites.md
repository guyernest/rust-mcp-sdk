# Prerequisites

This chapter covers everything you need to set up before starting the course.

## Required Software

### Rust Toolchain

Install Rust using rustup:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Verify installation:

```bash
rustc --version  # Should be 1.82.0 or later
cargo --version
```

### cargo-pmcp

Install the PMCP development toolkit:

```bash
cargo install cargo-pmcp
```

Verify installation:

```bash
cargo pmcp --version
```

### MCP Inspector

For manual testing and debugging (no installation needed):

```bash
npx @modelcontextprotocol/inspector
```

## MCP Clients

You'll need at least one MCP client. We recommend:

### Claude Desktop (Recommended)

Download from [claude.ai](https://claude.ai/download). Claude Desktop has native MCP support and works well for testing.

### VS Code with Continue

If you prefer an IDE-based workflow:

1. Install [Continue extension](https://marketplace.visualstudio.com/items?itemName=Continue.continue)
2. Configure MCP servers in Continue settings

### Cursor

[Cursor](https://cursor.sh) has built-in MCP support.

## Cloud Accounts

For the deployment chapters, you'll need access to at least one:

### AWS (Recommended for Enterprise)

- Create an AWS account or use your organization's account
- Install AWS CLI: `brew install awscli` or [download](https://aws.amazon.com/cli/)
- Configure credentials: `aws configure`
- For Lambda deployment, ensure you have permissions for:
  - Lambda
  - API Gateway
  - IAM (for role creation)
  - CloudWatch (for logs)

### Cloudflare (For Edge Deployment)

- Create a Cloudflare account
- Install Wrangler: `npm install -g wrangler`
- Authenticate: `wrangler login`

### Google Cloud (For Container Deployment)

- Create a GCP account
- Install gcloud CLI: `brew install google-cloud-sdk`
- Authenticate: `gcloud auth login`
- Set project: `gcloud config set project YOUR_PROJECT_ID`

## Identity Provider (For OAuth Chapters)

For the security chapters, you'll need an identity provider. Choose one:

### AWS Cognito (If Using AWS)

We'll create a Cognito User Pool during the course.

### Auth0 (Cross-Platform)

Create a free account at [auth0.com](https://auth0.com).

### Microsoft Entra ID (For Microsoft Shops)

If your organization uses Microsoft 365, you can use your existing Entra ID tenant.

## Development Tools

### Recommended IDE

- **VS Code** with rust-analyzer extension
- **RustRover** (JetBrains)
- **Cursor** with Rust extensions

### Database Tools (For db-explorer Chapter)

- SQLite CLI: `brew install sqlite` (usually pre-installed on macOS)
- Optional: [DBeaver](https://dbeaver.io/) for visual database management

### HTTP Tools

- **curl** (pre-installed on most systems)
- Optional: [httpie](https://httpie.io/) for friendlier HTTP testing

## Claude Code (Optional but Recommended)

For the AI-assisted development chapters:

```bash
npm install -g @anthropic-ai/claude-code
```

Or use Claude Code within VS Code/Cursor.

## Verify Your Setup

Run this checklist to ensure everything is ready:

```bash
# Rust toolchain
rustc --version
cargo --version

# cargo-pmcp
cargo pmcp --version

# Cloud CLI (at least one)
aws --version
wrangler --version
gcloud --version

# Node.js (for MCP Inspector)
node --version
npx --version
```

## Troubleshooting

### Rust Installation Issues

If rustup fails, try:
```bash
# Remove existing installations
rm -rf ~/.rustup ~/.cargo

# Reinstall
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

### cargo-pmcp Installation Issues

If cargo install fails:
```bash
# Update Rust
rustup update stable

# Try with verbose output
cargo install cargo-pmcp --verbose
```

### Permission Issues on macOS

If you get permission errors:
```bash
# For Homebrew
sudo chown -R $(whoami) /usr/local/bin

# For cargo binaries
chmod +x ~/.cargo/bin/*
```

## Ready?

Once you've verified all prerequisites, you're ready to start building.

---

*Continue to [Part I: Foundations](./part1-foundations/ch01-enterprise-case.md) →*
