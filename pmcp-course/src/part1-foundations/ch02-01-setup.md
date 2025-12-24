# Development Environment Setup

<div class="learning-objectives">
<h3>Learning Objectives</h3>

After this lesson, you will be able to:
- Install and configure the Rust toolchain
- Set up cargo-pmcp for MCP development
- Connect to an MCP server using MCP Inspector
- Configure Claude Desktop for local testing

</div>

## Video: Setting Up Your Environment

<div class="video-container">
<iframe
  src="https://www.youtube.com/embed/PLACEHOLDER_VIDEO_ID"
  title="PMCP Development Environment Setup"
  allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture"
  allowfullscreen>
</iframe>
</div>
<p class="video-caption">Watch: Complete environment setup walkthrough (12 min)</p>

## Installing Rust

If you don't have Rust installed:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

<div class="pro-tip">
On macOS, you may need to install Xcode command line tools first: <code>xcode-select --install</code>
</div>

Verify your installation:

```bash
rustc --version
# Should output: rustc 1.82.0 or later

cargo --version
# Should output: cargo 1.82.0 or later
```

## Installing cargo-pmcp

Install the PMCP development toolkit:

```bash
cargo install cargo-pmcp
```

This provides:
- `cargo pmcp new` - Create workspaces
- `cargo pmcp add` - Add servers and tools
- `cargo pmcp test` - Run MCP tests
- `cargo pmcp deploy` - Deploy to cloud

Verify installation:

```bash
cargo pmcp --version
```

## Installing MCP Inspector

MCP Inspector is essential for debugging:

```bash
npm install -g @anthropic-ai/mcp-inspector
```

Or use npx (no installation needed):

```bash
npx @anthropic-ai/mcp-inspector http://localhost:3000
```

## Configuring Your IDE

### VS Code (Recommended)

Install these extensions:
1. **rust-analyzer** - Rust language support
2. **Even Better TOML** - TOML syntax highlighting
3. **CodeLLDB** - Debugging support

### RustRover

JetBrains RustRover works out of the box with Rust projects.

### Cursor

Cursor with rust-analyzer provides AI-assisted Rust development.

---

## Knowledge Check

Test your understanding of the setup process:

{{#quiz ../quizzes/ch02-01-setup.toml}}

---

<div class="enterprise-note">
In enterprise environments, you may need to configure cargo to use an internal registry or mirror. See your IT department's Rust setup guide.
</div>

## Next Steps

Now that your environment is ready, let's create your first MCP workspace.

---

*Continue to [Creating a Workspace](./ch02-02-workspace.md) →*
