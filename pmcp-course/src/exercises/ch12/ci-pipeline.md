::: exercise
id: ch12-01-ci-pipeline
difficulty: intermediate
time: 45 minutes
:::

Build a production-ready CI/CD pipeline that runs tests on every push,
deploys to staging and production, and verifies deployments with health checks.

::: objectives
thinking:
  - Why tests must pass before deployment
  - How to handle secrets securely in CI
  - The value of environment protection rules
doing:
  - Create GitHub Actions workflow with build and test
  - Add mcp-tester scenarios to the pipeline
  - Configure deployment with environment protection
  - Add post-deployment health checks
:::

::: discussion
- What would happen if broken code reached production?
- How long should a CI pipeline take before developers get frustrated?
- When should deployment be automatic vs require approval?
:::

## Step 1: Create Basic Workflow

Create `.github/workflows/ci.yml`:

```yaml
name: MCP Server CI/CD

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - uses: dtolnay/rust-toolchain@stable

      - name: Cache cargo
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}

      - name: Run unit tests
        run: cargo test --all-features --test-threads=1
```

## Step 2: Add MCP Integration Tests

Extend the workflow to run mcp-tester:

```yaml
      - name: Build release binary
        run: cargo build --release

      - name: Start server in background
        run: |
          cargo run --release &
          sleep 5  # Wait for server startup

      - name: Run MCP scenarios
        run: |
          cargo pmcp test run \
            --server http://localhost:3000 \
            --format junit \
            --output test-results.xml

      - name: Upload test results
        uses: actions/upload-artifact@v4
        if: always()
        with:
          name: test-results
          path: test-results.xml
```

## Step 3: Add Deployment Job

```yaml
  deploy:
    needs: test
    if: github.ref == 'refs/heads/main'
    runs-on: ubuntu-latest
    environment: production  # Requires approval in GitHub settings

    steps:
      - uses: actions/checkout@v4

      - uses: dtolnay/rust-toolchain@stable

      - name: Install cargo-lambda
        run: cargo install cargo-lambda

      - name: Deploy to Lambda
        run: cargo pmcp deploy
        env:
          AWS_ACCESS_KEY_ID: ${{ secrets.AWS_ACCESS_KEY_ID }}
          AWS_SECRET_ACCESS_KEY: ${{ secrets.AWS_SECRET_ACCESS_KEY }}
          AWS_DEFAULT_REGION: us-east-1
```

## Step 4: Add Health Check

```yaml
      - name: Get deployed endpoint
        id: endpoint
        run: |
          ENDPOINT=$(cargo pmcp deploy outputs --json | jq -r '.endpoint')
          echo "url=$ENDPOINT" >> $GITHUB_OUTPUT

      - name: Health check
        run: |
          curl -f -X POST ${{ steps.endpoint.outputs.url }}/mcp \
            -H "Content-Type: application/json" \
            -d '{"jsonrpc":"2.0","method":"initialize","params":{},"id":1}'
```

## Step 5: Configure Repository Secrets

In GitHub repository settings:
1. Go to Settings > Secrets and variables > Actions
2. Add repository secrets:
   - `AWS_ACCESS_KEY_ID`
   - `AWS_SECRET_ACCESS_KEY`

## Step 6: Configure Environment Protection

In GitHub repository settings:
1. Go to Settings > Environments
2. Create "production" environment
3. Add required reviewers if desired
4. Add deployment branch rules

::: hints
level_1: "Use 'sleep 5' after starting the server to ensure it's ready before tests run."
level_2: "Always use --test-threads=1 for cargo test to avoid race conditions."
level_3: "Consider adding a staging environment that deploys on every PR for testing."
:::

## Success Criteria

- [ ] Workflow triggers on push and pull_request
- [ ] Rust unit tests run and report results
- [ ] mcp-tester scenarios execute against running server
- [ ] Test results uploaded as artifacts
- [ ] Deployment job runs only after tests pass
- [ ] Health check verifies deployed server responds

---

*This connects to [Regression Testing](./ch12-03-regression.md) for maintaining test suites.*
