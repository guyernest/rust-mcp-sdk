::: exercise
id: ch02-00-environment-setup
difficulty: beginner
time: 15 minutes
:::

Before building your first MCP server, let's ensure your development environment is properly configured. This setup exercise will verify all required tools are installed and working.

::: objectives
- Install and verify the Rust toolchain
- Install cargo-pmcp development toolkit
- Set up an MCP client for testing
- Verify your complete development environment
:::

::: starter
```bash
# Run these commands to check your current setup
# Each should return a version number

# 1. Check Rust installation
rustc --version
cargo --version

# 2. Check cargo-pmcp (may not be installed yet)
cargo pmcp --version

# 3. Check Node.js (for MCP Inspector)
node --version
npx --version
```
:::

::: hint level=1 title="Installing Rust"
If Rust is not installed, run:
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```
Verify with `rustc --version` - should be 1.82.0 or later.
:::

::: hint level=2 title="Installing cargo-pmcp"
Install the PMCP development toolkit:
```bash
cargo install cargo-pmcp
```
If installation fails, first update Rust: `rustup update stable`
:::

::: hint level=3 title="MCP Inspector"
The MCP Inspector is a web-based tool for testing MCP servers:
```bash
npx @modelcontextprotocol/inspector
```
No installation needed - it runs via npx.
:::

::: hint level=4 title="Setting up Claude Desktop"
Download Claude Desktop from [claude.ai](https://claude.ai/download).

Configure MCP servers in `~/Library/Application Support/Claude/claude_desktop_config.json` (macOS) or `%APPDATA%\Claude\claude_desktop_config.json` (Windows).
:::

::: solution
Your environment is ready when all these commands succeed:

```bash
# Complete verification script
echo "=== Rust Toolchain ==="
rustc --version && cargo --version

echo -e "\n=== cargo-pmcp ==="
cargo pmcp --version

echo -e "\n=== Node.js (for MCP Inspector) ==="
node --version && npx --version

echo -e "\n=== Environment Ready! ==="
```

Expected output:
```
=== Rust Toolchain ===
rustc 1.82.0 (or later)
cargo 1.82.0 (or later)

=== cargo-pmcp ===
cargo-pmcp 0.x.x

=== Node.js (for MCP Inspector) ===
v20.x.x (or later)
10.x.x

=== Environment Ready! ===
```
:::

::: tests
mode: local
```bash
# Test 1: Rust is installed
rustc --version | grep -q "rustc 1\." && echo "PASS: Rust installed" || echo "FAIL: Rust not found"

# Test 2: Cargo is available
cargo --version | grep -q "cargo 1\." && echo "PASS: Cargo installed" || echo "FAIL: Cargo not found"

# Test 3: cargo-pmcp is installed
cargo pmcp --version 2>/dev/null && echo "PASS: cargo-pmcp installed" || echo "FAIL: cargo-pmcp not found - run: cargo install cargo-pmcp"

# Test 4: Node.js is available (for MCP Inspector)
node --version | grep -q "v" && echo "PASS: Node.js installed" || echo "WARN: Node.js not found - needed for MCP Inspector"
```
:::

::: reflection
- Did you encounter any installation issues? Note them for troubleshooting.
- Which MCP client will you use? (Claude Desktop, Cursor, VS Code + Continue)
- Are you planning to deploy to cloud? If so, ensure you have the relevant CLI installed (aws, wrangler, or gcloud).
:::
