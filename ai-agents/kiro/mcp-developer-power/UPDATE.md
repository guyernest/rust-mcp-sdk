# MCP Developer Power v1.0.1 - Workflow Enhancement

## What Changed

Added **`mcp-workflow.md`** (757 lines) - a critical steering file that explicitly teaches Kiro the cargo-pmcp workflow to prevent manual file creation.

## The Problem

In v1.0.0, Kiro was reading the steering files correctly but implementing tasks by **creating files manually** instead of using **cargo-pmcp commands**. This resulted in:

- ❌ Manual Cargo.toml creation
- ❌ Manual directory structure setup  
- ❌ Missing cargo-pmcp conventions
- ❌ No hot-reload dev server
- ❌ No test scaffolding
- ⏱️ Wasted time on boilerplate

## The Solution

### New Steering File: `mcp-workflow.md`

**Inclusion**: `always` (active in all contexts)

**Key Sections**:

1. **CRITICAL: Never Create Files Manually**
   - Explicit warning against manual file creation
   - Why cargo-pmcp scaffolding is mandatory

2. **The Standard Workflow (ALWAYS Follow This)**
   - Step-by-step cargo-pmcp commands
   - What each command does
   - Expected output structure

3. **Complete Example: Building a Weather Server**
   - ❌ Incorrect Approach (manual files) - what Kiro was doing
   - ✅ Correct Approach (cargo-pmcp) - what Kiro should do

4. **Workflow Decision Tree**
   - Visual flowchart of proper workflow
   - From "New Project?" to "Production!"

5. **When to Use Each Template**
   - `minimal` - Custom servers
   - `calculator` - Learning
   - `complete_calculator` - Full reference
   - `sqlite_explorer` - Database servers

6. **Common Mistakes to Avoid**
   - With explicit ❌/✅ comparisons
   - Shows wrong vs. right approach

7. **Interactive Testing with Kiro**
   - How Kiro can test servers directly via MCP
   - Real-time validation during development

## What Kiro Now Knows

With `mcp-workflow.md` always active, Kiro will:

✅ **Always start with**: `cargo pmcp new <workspace>`
✅ **Always scaffold with**: `cargo pmcp add server <name> --template <type>`
✅ **Only edit**: Generated tool files in `src/tools/*.rs`
✅ **Never create**: Cargo.toml, lib.rs, main.rs, or directory structures manually
✅ **Use dev server**: `cargo pmcp dev --server <name>` for hot-reload
✅ **Generate tests**: `cargo pmcp test --generate-scenarios`
✅ **Validate**: `cargo pmcp test --server <name>`

## Example: Kiro's New Workflow

### User Request
"Create a weather MCP server"

### Kiro's Response (v1.0.1)

```
I'll use the cargo-pmcp workflow to create your weather server.

Step 1: Create workspace
$ cargo pmcp new weather-mcp-workspace
$ cd weather-mcp-workspace

This creates:
- Workspace Cargo.toml with proper dependencies
- server-common crate for HTTP transport
- scenarios/ directory for tests

Step 2: Add weather server with minimal template
$ cargo pmcp add server weather --template minimal

This scaffolds:
- mcp-weather-core/ (library crate)
- weather-server/ (binary crate)
- Complete structure with tools/, resources/, workflows/
- Updated workspace Cargo.toml

Step 3: Now I'll implement the weather tools
[Edits crates/mcp-weather-core/src/tools/get_forecast.rs]
[Implements WeatherInput, WeatherOutput types]
[Implements async handler with API call]
[Edits lib.rs to register tool]

Step 4: Start development server
$ cargo pmcp dev --server weather

Server running on http://0.0.0.0:3000

Step 5: I'll test it directly via MCP...
[Kiro connects to server via MCP protocol]
[Calls get_forecast tool with test data]
[Validates response]

✓ Tool working correctly!

Step 6: Generate test scenarios
$ cargo pmcp test --server weather --generate-scenarios

Created: scenarios/weather/generated.yaml

Step 7: Run automated tests
$ cargo pmcp test --server weather

✓ All scenarios passing!

Your weather server is ready. Quality gates:
- cargo fmt --check ✓
- cargo clippy ✓  
- cargo test ✓

Next steps:
1. Add your weather API key to .env
2. Deploy: cargo build --release
```

## Updated Metrics

### Content
- **Steering files**: 5 (was 4)
- **Steering lines**: 4,487 (was 3,730)
- **Always active**: 2,885 lines (was 2,128)

### Coverage
Now includes:
- ✅ Complete cargo-pmcp workflow
- ✅ Decision trees for scaffolding
- ✅ Template selection guide
- ✅ Testing workflow with Kiro
- ✅ Common mistakes and corrections
- ✅ Environment setup
- ✅ Production deployment

## Migration

If you already installed v1.0.0:

```bash
cd ~/.kiro/powers/mcp-developer
git pull  # If symlinked to repo

# Or re-copy the updated directory
cp -r /path/to/updated/mcp-developer-power ~/.kiro/powers/mcp-developer
```

Restart Kiro to load new steering file.

## Testing

Ask Kiro: "Create a calculator MCP server"

**Expected behavior**:
- Kiro uses `cargo pmcp new` and `cargo pmcp add server`
- Kiro does NOT create Cargo.toml or directories manually
- Kiro edits only the generated tool files
- Kiro uses `cargo pmcp dev` for testing

## Impact

This update is **critical** for proper MCP development workflow. Without it, Kiro generates non-standard server structures that miss cargo-pmcp's proven patterns and tooling.

**Before v1.0.1**: Kiro created files manually (30-60 minutes setup)
**After v1.0.1**: Kiro uses cargo-pmcp (30 seconds setup)

---

**Version**: 1.0.1
**Date**: 2025-11-13
**Type**: Critical workflow enhancement
**Files Changed**: 3
- Added: `steering/mcp-workflow.md` (757 lines)
- Updated: `steering/mcp-product.md` (workflow reference)
- Updated: `power.json` (metadata)
