# CI/CD Integration

Integrating MCP server testing into CI/CD pipelines ensures every change is tested before reaching production. This chapter covers patterns for GitHub Actions, GitLab CI, and other CI systems.

## Pipeline Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                    MCP Server CI/CD Pipeline                         │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌────────────────────────────────────────────────────────────────┐ │
│  │                         COMMIT                                  │ │
│  └─────────────────────────────┬──────────────────────────────────┘ │
│                                │                                    │
│                                ▼                                    │
│  ┌────────────────────────────────────────────────────────────────┐ │
│  │  STAGE 1: Build & Unit Tests                                   │ │
│  │  • cargo build --release                                       │ │
│  │  • cargo test --all-features                                   │ │
│  │  • cargo clippy                                                │ │
│  │  ⏱ ~3-5 minutes                                                │ │
│  └─────────────────────────────┬──────────────────────────────────┘ │
│                                │                                    │
│                                ▼                                    │
│  ┌────────────────────────────────────────────────────────────────┐ │
│  │  STAGE 2: Integration Tests                                    │ │
│  │  • Start local server with test database                       │ │
│  │  • cargo pmcp test run (full suite)                           │ │
│  │  • Generate coverage report                                    │ │
│  │  ⏱ ~5-10 minutes                                               │ │
│  └─────────────────────────────┬──────────────────────────────────┘ │
│                                │                                    │
│                                ▼                                    │
│  ┌────────────────────────────────────────────────────────────────┐ │
│  │  STAGE 3: Deploy to Staging                                    │ │
│  │  • Build container/package                                     │ │
│  │  • Deploy to staging environment                               │ │
│  │  • Wait for deployment to stabilize                            │ │
│  │  ⏱ ~5-10 minutes                                               │ │
│  └─────────────────────────────┬──────────────────────────────────┘ │
│                                │                                    │
│                                ▼                                    │
│  ┌────────────────────────────────────────────────────────────────┐ │
│  │  STAGE 4: Staging Tests                                        │ │
│  │  • Smoke tests (critical paths)                                │ │
│  │  • Full integration suite                                      │ │
│  │  • Performance validation                                      │ │
│  │  ⏱ ~5-15 minutes                                               │ │
│  └─────────────────────────────┬──────────────────────────────────┘ │
│                                │                                    │
│                                ▼                                    │
│  ┌────────────────────────────────────────────────────────────────┐ │
│  │  STAGE 5: Production Deployment                                │ │
│  │  • Canary deployment (10%)                                     │ │
│  │  • Smoke tests on canary                                       │ │
│  │  • Gradual rollout (25%, 50%, 100%)                           │ │
│  │  • Monitor for errors                                          │ │
│  │  ⏱ ~15-30 minutes                                              │ │
│  └────────────────────────────────────────────────────────────────┘ │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

## GitHub Actions Configuration

### Complete Workflow

```yaml
# .github/workflows/ci.yml
name: CI/CD Pipeline

on:
  push:
    branches: [main, develop]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always
  RUST_BACKTRACE: 1

jobs:
  # ============================================
  # Stage 1: Build and Lint
  # ============================================
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-action@stable
        with:
          components: rustfmt, clippy

      - name: Cache dependencies
        uses: Swatinem/rust-cache@v2

      - name: Check formatting
        run: cargo fmt --check

      - name: Clippy
        run: cargo clippy --all-features -- -D warnings

      - name: Build
        run: cargo build --release

      - name: Upload binary
        uses: actions/upload-artifact@v4
        with:
          name: mcp-server
          path: target/release/mcp-server

  # ============================================
  # Stage 1b: Unit Tests (parallel with build)
  # ============================================
  unit-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-action@stable

      - name: Cache dependencies
        uses: Swatinem/rust-cache@v2

      - name: Run unit tests
        run: cargo test --all-features --lib

      - name: Generate coverage
        run: |
          cargo install cargo-tarpaulin
          cargo tarpaulin --out Xml --output-dir coverage/

      - name: Upload coverage
        uses: codecov/codecov-action@v3
        with:
          files: coverage/cobertura.xml

  # ============================================
  # Stage 2: Integration Tests
  # ============================================
  integration-tests:
    needs: [build, unit-tests]
    runs-on: ubuntu-latest

    services:
      postgres:
        image: postgres:15
        env:
          POSTGRES_PASSWORD: test_password
          POSTGRES_DB: mcp_test
        ports:
          - 5432:5432
        options: >-
          --health-cmd pg_isready
          --health-interval 10s
          --health-timeout 5s
          --health-retries 5

    steps:
      - uses: actions/checkout@v4

      - name: Download binary
        uses: actions/download-artifact@v4
        with:
          name: mcp-server
          path: ./bin

      - name: Make executable
        run: chmod +x ./bin/mcp-server

      - name: Setup database
        run: |
          PGPASSWORD=test_password psql -h localhost -U postgres -d mcp_test \
            -f tests/fixtures/schema.sql

      - name: Start MCP server
        run: |
          ./bin/mcp-server &
          echo $! > /tmp/server.pid
          sleep 5
        env:
          DATABASE_URL: postgres://postgres:test_password@localhost:5432/mcp_test
          PORT: 3000

      - name: Install pmcp
        run: cargo install cargo-pmcp

      - name: Run mcp-tester
        run: |
          cargo pmcp test run \
            --server http://localhost:3000/mcp \
            --scenario tests/scenarios/ \
            --format junit \
            --output test-results/integration.xml

      - name: Stop server
        if: always()
        run: kill $(cat /tmp/server.pid) || true

      - name: Upload test results
        uses: actions/upload-artifact@v4
        if: always()
        with:
          name: integration-results
          path: test-results/

      - name: Publish test report
        uses: dorny/test-reporter@v1
        if: always()
        with:
          name: Integration Tests
          path: test-results/*.xml
          reporter: java-junit

  # ============================================
  # Stage 3: Deploy to Staging
  # ============================================
  deploy-staging:
    needs: integration-tests
    if: github.ref == 'refs/heads/main'
    runs-on: ubuntu-latest
    environment: staging
    outputs:
      deployment_url: ${{ steps.deploy.outputs.url }}

    steps:
      - uses: actions/checkout@v4

      - name: Download binary
        uses: actions/download-artifact@v4
        with:
          name: mcp-server

      - name: Deploy to staging
        id: deploy
        run: |
          # Example: Deploy to Cloud Run
          gcloud run deploy mcp-server-staging \
            --source . \
            --region us-central1 \
            --set-env-vars "ENV=staging" \
            --format "value(status.url)" > /tmp/url.txt
          echo "url=$(cat /tmp/url.txt)" >> $GITHUB_OUTPUT

      - name: Wait for deployment
        run: |
          # Wait for service to be ready
          for i in {1..30}; do
            if curl -sf "${{ steps.deploy.outputs.url }}/health"; then
              echo "Service is healthy"
              exit 0
            fi
            echo "Waiting for service... ($i/30)"
            sleep 10
          done
          echo "Service failed to become healthy"
          exit 1

  # ============================================
  # Stage 4: Staging Tests
  # ============================================
  test-staging:
    needs: deploy-staging
    runs-on: ubuntu-latest
    environment: staging

    steps:
      - uses: actions/checkout@v4

      - name: Install pmcp
        run: cargo install cargo-pmcp

      - name: Smoke tests
        run: |
          cargo pmcp test run \
            --server "${{ needs.deploy-staging.outputs.deployment_url }}/mcp" \
            --header "Authorization: Bearer ${{ secrets.STAGING_API_KEY }}" \
            --scenario tests/scenarios/smoke/ \
            --fail-fast \
            --format junit \
            --output test-results/staging-smoke.xml

      - name: Full integration tests
        run: |
          cargo pmcp test run \
            --server "${{ needs.deploy-staging.outputs.deployment_url }}/mcp" \
            --header "Authorization: Bearer ${{ secrets.STAGING_API_KEY }}" \
            --scenario tests/scenarios/integration/ \
            --format junit \
            --output test-results/staging-full.xml

      - name: Upload results
        uses: actions/upload-artifact@v4
        if: always()
        with:
          name: staging-results
          path: test-results/

  # ============================================
  # Stage 5: Production Deployment
  # ============================================
  deploy-production:
    needs: test-staging
    if: github.ref == 'refs/heads/main'
    runs-on: ubuntu-latest
    environment: production

    steps:
      - uses: actions/checkout@v4

      - name: Deploy canary (10%)
        run: |
          gcloud run services update-traffic mcp-server \
            --to-revisions LATEST=10

      - name: Test canary
        run: |
          cargo pmcp test run \
            --server "https://mcp.example.com/mcp" \
            --header "Authorization: Bearer ${{ secrets.PROD_API_KEY }}" \
            --scenario tests/scenarios/smoke/ \
            --fail-fast

      - name: Promote to 50%
        run: |
          gcloud run services update-traffic mcp-server \
            --to-revisions LATEST=50

      - name: Monitor for 5 minutes
        run: |
          # Check error rates
          for i in {1..10}; do
            cargo pmcp test run \
              --server "https://mcp.example.com/mcp" \
              --header "Authorization: Bearer ${{ secrets.PROD_API_KEY }}" \
              --scenario tests/scenarios/smoke/ \
              --quiet
            sleep 30
          done

      - name: Full rollout
        run: |
          gcloud run services update-traffic mcp-server \
            --to-revisions LATEST=100

      - name: Rollback on failure
        if: failure()
        run: |
          gcloud run services update-traffic mcp-server \
            --to-revisions PREVIOUS=100
```

### Reusable Workflow

```yaml
# .github/workflows/mcp-test.yml
name: MCP Test Workflow

on:
  workflow_call:
    inputs:
      server_url:
        required: true
        type: string
      scenarios:
        required: false
        type: string
        default: "tests/scenarios/"
      fail_fast:
        required: false
        type: boolean
        default: false
    secrets:
      api_key:
        required: false

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install pmcp
        run: cargo install cargo-pmcp

      - name: Run tests
        run: |
          AUTH_HEADER=""
          if [ -n "${{ secrets.api_key }}" ]; then
            AUTH_HEADER="--header \"Authorization: Bearer ${{ secrets.api_key }}\""
          fi

          FAIL_FAST=""
          if [ "${{ inputs.fail_fast }}" == "true" ]; then
            FAIL_FAST="--fail-fast"
          fi

          cargo pmcp test run \
            --server "${{ inputs.server_url }}" \
            $AUTH_HEADER \
            --scenario "${{ inputs.scenarios }}" \
            $FAIL_FAST \
            --format junit \
            --output test-results/results.xml

      - name: Upload results
        uses: actions/upload-artifact@v4
        if: always()
        with:
          name: test-results
          path: test-results/
```

Using the reusable workflow:

```yaml
# .github/workflows/test-all-environments.yml
name: Test All Environments

on:
  schedule:
    - cron: '0 */6 * * *'  # Every 6 hours

jobs:
  test-staging:
    uses: ./.github/workflows/mcp-test.yml
    with:
      server_url: https://staging.mcp.example.com/mcp
      scenarios: tests/scenarios/
    secrets:
      api_key: ${{ secrets.STAGING_API_KEY }}

  test-production:
    uses: ./.github/workflows/mcp-test.yml
    with:
      server_url: https://mcp.example.com/mcp
      scenarios: tests/scenarios/smoke/
      fail_fast: true
    secrets:
      api_key: ${{ secrets.PROD_API_KEY }}
```

## Test Result Reporting

### JUnit Format for CI Systems

```bash
# Generate JUnit XML for CI parsing
cargo pmcp test run \
  --server http://localhost:3000/mcp \
  --format junit \
  --output test-results/results.xml
```

The output looks like:
```xml
<?xml version="1.0" encoding="UTF-8"?>
<testsuites name="mcp-tests" tests="15" failures="1" time="5.234">
  <testsuite name="smoke/health_check.yaml" tests="3" failures="0" time="1.234">
    <testcase name="Server responds" time="0.456"/>
    <testcase name="Execute simple query" time="0.789"/>
    <testcase name="Sample rows work" time="0.234"/>
  </testsuite>
  <testsuite name="integration/crud.yaml" tests="5" failures="1" time="2.567">
    <testcase name="Create record" time="0.234"/>
    <testcase name="Read record" time="0.123"/>
    <testcase name="Update record" time="0.345">
      <failure message="Assertion failed: content.contains('updated')">
Expected content to contain 'updated', got: '{"status":"unchanged"}'
      </failure>
    </testcase>
    <testcase name="Delete record" time="0.234"/>
    <testcase name="Verify deletion" time="0.123"/>
  </testsuite>
</testsuites>
```

### GitHub Annotations

```yaml
- name: Annotate failures
  if: failure()
  run: |
    # Parse JUnit and create annotations
    python3 << 'EOF'
    import xml.etree.ElementTree as ET

    tree = ET.parse('test-results/results.xml')
    for testsuite in tree.findall('.//testsuite'):
        for testcase in testsuite.findall('testcase'):
            failure = testcase.find('failure')
            if failure is not None:
                name = testcase.get('name')
                message = failure.get('message')
                print(f"::error title=Test Failed: {name}::{message}")
    EOF
```

### Slack Notifications

```yaml
- name: Notify on failure
  if: failure()
  uses: slackapi/slack-github-action@v1
  with:
    payload: |
      {
        "text": "MCP Tests Failed",
        "blocks": [
          {
            "type": "section",
            "text": {
              "type": "mrkdwn",
              "text": "*MCP Server Tests Failed* :x:\n\n*Branch:* ${{ github.ref_name }}\n*Commit:* ${{ github.sha }}\n*Author:* ${{ github.actor }}"
            }
          },
          {
            "type": "actions",
            "elements": [
              {
                "type": "button",
                "text": {"type": "plain_text", "text": "View Run"},
                "url": "${{ github.server_url }}/${{ github.repository }}/actions/runs/${{ github.run_id }}"
              }
            ]
          }
        ]
      }
  env:
    SLACK_WEBHOOK_URL: ${{ secrets.SLACK_WEBHOOK }}
```

## Parallel Test Execution

### Matrix Strategy

```yaml
jobs:
  test:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        scenario-dir:
          - tests/scenarios/smoke
          - tests/scenarios/integration
          - tests/scenarios/performance
          - tests/scenarios/security
      fail-fast: false

    steps:
      - uses: actions/checkout@v4

      - name: Run tests
        run: |
          cargo pmcp test run \
            --server http://localhost:3000/mcp \
            --scenario ${{ matrix.scenario-dir }}/ \
            --format junit \
            --output test-results/${{ matrix.scenario-dir }}.xml

  aggregate:
    needs: test
    runs-on: ubuntu-latest
    if: always()
    steps:
      - name: Download all results
        uses: actions/download-artifact@v4
        with:
          path: all-results

      - name: Merge results
        run: |
          # Combine all JUnit files
          npx junit-merge -d all-results -o final-results.xml
```

### Parallel Within mcp-tester

```bash
# Run scenarios in parallel
cargo pmcp test run \
  --server http://localhost:3000/mcp \
  --scenario tests/scenarios/ \
  --parallel 4  # Run 4 scenarios concurrently
```

## Caching Strategies

### Rust Build Cache

```yaml
- name: Cache Rust
  uses: Swatinem/rust-cache@v2
  with:
    shared-key: "mcp-server"
    cache-targets: true
```

### Docker Layer Cache

```yaml
- name: Set up Docker Buildx
  uses: docker/setup-buildx-action@v3

- name: Build and push
  uses: docker/build-push-action@v5
  with:
    context: .
    push: true
    tags: ghcr.io/${{ github.repository }}:${{ github.sha }}
    cache-from: type=gha
    cache-to: type=gha,mode=max
```

## Summary

Effective CI/CD integration requires:

1. **Staged pipeline** - Build → Test → Deploy → Verify
2. **Parallel execution** - Run independent jobs concurrently
3. **Proper reporting** - JUnit format for CI parsing
4. **Notifications** - Alert on failures
5. **Caching** - Speed up builds with proper caching
6. **Rollback strategy** - Auto-rollback on test failures

## Exercises

1. **Set up GitHub Actions** - Create a complete CI pipeline for an MCP server
2. **Add test reporting** - Configure JUnit reporting and GitHub annotations
3. **Implement canary deployment** - Add gradual rollout with testing gates
4. **Add Slack notifications** - Alert the team on test failures

---

*Continue to [Regression Testing](./ch12-03-regression.md) →*
