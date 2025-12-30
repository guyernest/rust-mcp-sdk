# Testing Deployed Servers

This chapter covers the detailed configuration and strategies for testing MCP servers running in production or staging environments.

## Connection Configuration

### Basic Remote Connection

```bash
# Simple remote test
cargo pmcp test run --server https://mcp.example.com/mcp

# With HTTPS verification disabled (for self-signed certs in staging)
cargo pmcp test run \
  --server https://staging.mcp.example.com/mcp \
  --insecure
```

### Authentication Headers

Most production servers require authentication:

```bash
# Bearer token authentication
cargo pmcp test run \
  --server https://mcp.example.com/mcp \
  --header "Authorization: Bearer eyJhbGciOiJIUzI1NiIs..."

# API key authentication
cargo pmcp test run \
  --server https://mcp.example.com/mcp \
  --header "X-API-Key: your-api-key-here"

# Multiple headers
cargo pmcp test run \
  --server https://mcp.example.com/mcp \
  --header "Authorization: Bearer ${TOKEN}" \
  --header "X-Request-ID: test-run-$(date +%s)" \
  --header "X-Environment: staging"
```

### Environment Variables

Use environment variables for secure configuration:

```bash
# Set credentials
export MCP_SERVER_URL="https://mcp.example.com/mcp"
export MCP_API_KEY="your-secret-key"

# Run tests (configuration file references env vars)
cargo pmcp test run --config tests/config/remote.yaml
```

```yaml
# tests/config/remote.yaml
server:
  url: "${MCP_SERVER_URL}"
  headers:
    Authorization: "Bearer ${MCP_API_KEY}"
```

## Timeout and Retry Configuration

### Handling Cold Starts

Cloud deployments often have cold start latency:

```yaml
# tests/config/lambda.yaml
server:
  url: https://abc123.lambda-url.us-east-1.on.aws/mcp
  timeout_ms: 30000    # 30 seconds for cold start
  retry_count: 3       # Retry on timeout
  retry_delay_ms: 1000 # Wait 1s between retries

# First request allows extra time
first_request:
  timeout_ms: 60000    # 60 seconds for initial cold start
```

```bash
# CLI equivalent
cargo pmcp test run \
  --server https://abc123.lambda-url.us-east-1.on.aws/mcp \
  --timeout 30000 \
  --retry 3 \
  --retry-delay 1000
```

### Platform-Specific Timeouts

Different platforms have different characteristics:

| Platform | First Request | Subsequent | Notes |
|----------|--------------|------------|-------|
| Lambda | 30-60s | 1-5s | Cold starts |
| Cloud Run | 15-30s | 1-3s | Cold starts with min-instances=0 |
| Workers | <1s | <100ms | No cold starts |
| ECS/Kubernetes | 1-5s | 100-500ms | Always warm |

```yaml
# tests/config/cloudflare-workers.yaml
server:
  url: https://mcp-server.yourname.workers.dev
  timeout_ms: 5000     # Workers are fast
  retry_count: 1       # Rarely need retries

# tests/config/aws-lambda.yaml
server:
  url: https://abc123.lambda-url.us-east-1.on.aws/mcp
  timeout_ms: 45000    # Allow for cold starts
  retry_count: 3
```

## Response Time Assertions

Validate performance meets SLAs:

```yaml
# tests/scenarios/performance/latency_requirements.yaml
name: "Performance - Latency SLA"
description: "Verify response times meet production SLAs"
tags:
  - performance
  - sla

steps:
  - name: "Health check under 1s"
    tool: list_tables
    input: {}
    expect:
      success: true
      response_time_ms:
        less_than: 1000

  - name: "Simple query under 2s"
    tool: execute_query
    input:
      sql: "SELECT 1"
    expect:
      success: true
      response_time_ms:
        less_than: 2000

  - name: "Complex query under 5s"
    tool: execute_query
    input:
      sql: "SELECT COUNT(*) FROM large_table"
    expect:
      success: true
      response_time_ms:
        less_than: 5000
        greater_than: 0  # Ensure it's not cached
```

## Load Testing Scenarios

### Concurrent Request Testing

```yaml
# tests/scenarios/load/concurrent_requests.yaml
name: "Load - Concurrent requests"
description: "Test server handles concurrent connections"
tags:
  - load
  - performance

config:
  parallel: 10    # Run 10 concurrent tests
  iterations: 5   # Each runs 5 times

steps:
  - name: "Concurrent queries"
    tool: execute_query
    input:
      sql: "SELECT * FROM users LIMIT 10"
    expect:
      success: true
      response_time_ms:
        less_than: 3000  # Even under load
```

### Burst Traffic Simulation

```bash
# Simulate burst traffic
for i in {1..100}; do
  cargo pmcp test run \
    --server https://mcp.example.com/mcp \
    --scenario tests/scenarios/smoke/ \
    --quiet &
done
wait

# Check results
grep -r "FAIL" test-results/
```

## Testing Different Environments

### Environment Configuration Files

```
tests/
├── config/
│   ├── local.yaml       # Local development
│   ├── staging.yaml     # Staging environment
│   ├── production.yaml  # Production (smoke only)
│   └── preview.yaml     # PR preview environments
└── scenarios/
    ├── smoke/           # Quick validation
    ├── integration/     # Full integration tests
    └── performance/     # Performance tests
```

```yaml
# tests/config/local.yaml
server:
  url: http://localhost:3000/mcp
  timeout_ms: 5000
scenarios:
  - tests/scenarios/

# tests/config/staging.yaml
server:
  url: https://staging.mcp.example.com/mcp
  headers:
    Authorization: "Bearer ${STAGING_TOKEN}"
  timeout_ms: 30000
scenarios:
  - tests/scenarios/smoke/
  - tests/scenarios/integration/

# tests/config/production.yaml
server:
  url: https://mcp.example.com/mcp
  headers:
    Authorization: "Bearer ${PROD_TOKEN}"
  timeout_ms: 10000
scenarios:
  - tests/scenarios/smoke/  # Only smoke tests in prod
options:
  fail_fast: true
  parallel: 2  # Light load on production
```

### PR Preview Environments

For platforms that deploy PR previews:

```yaml
# .github/workflows/pr-preview.yml
on:
  pull_request:
    types: [opened, synchronize]

jobs:
  deploy-preview:
    runs-on: ubuntu-latest
    outputs:
      preview_url: ${{ steps.deploy.outputs.url }}
    steps:
      - uses: actions/checkout@v4
      - id: deploy
        run: |
          # Deploy to preview environment
          URL=$(./deploy.sh preview --pr ${{ github.event.pull_request.number }})
          echo "url=$URL" >> $GITHUB_OUTPUT

  test-preview:
    needs: deploy-preview
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Test preview environment
        run: |
          cargo pmcp test run \
            --server "${{ needs.deploy-preview.outputs.preview_url }}/mcp" \
            --scenario tests/scenarios/smoke/ \
            --format junit \
            --output preview-results.xml
```

## Debugging Remote Test Failures

### Verbose Output Mode

```bash
# Maximum verbosity for debugging
cargo pmcp test run \
  --server https://mcp.example.com/mcp \
  --scenario tests/scenarios/failing_test.yaml \
  --verbose \
  --show-requests \
  --show-responses
```

Output includes:
```
[10:23:45.123] Connecting to https://mcp.example.com/mcp
[10:23:45.234] → Request: tools/call
{
  "name": "execute_query",
  "arguments": {
    "sql": "SELECT * FROM users"
  }
}
[10:23:45.567] ← Response (333ms):
{
  "content": [...],
  "isError": false
}
[10:23:45.568] ✓ Assertion passed: success = true
[10:23:45.568] ✗ Assertion failed: response_time_ms < 200
              Actual: 333ms, Expected: < 200ms
```

### Saving Request/Response Logs

```bash
# Save all requests and responses
cargo pmcp test run \
  --server https://mcp.example.com/mcp \
  --scenario tests/scenarios/ \
  --log-requests test-logs/requests.json \
  --log-responses test-logs/responses.json

# Analyze failures
jq '.[] | select(.status == "error")' test-logs/responses.json
```

### Network Debugging

```bash
# Test with curl first
curl -X POST https://mcp.example.com/mcp \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $TOKEN" \
  -d '{
    "jsonrpc": "2.0",
    "method": "tools/list",
    "params": {},
    "id": 1
  }' \
  -v  # Verbose output shows headers, timing

# Check DNS resolution
nslookup mcp.example.com

# Check SSL certificate
openssl s_client -connect mcp.example.com:443 -servername mcp.example.com

# Check connectivity
nc -zv mcp.example.com 443
```

## Health Check Integration

### Pre-Test Health Verification

```yaml
# tests/scenarios/health/pre_check.yaml
name: "Health - Pre-test verification"
description: "Verify server is healthy before running full suite"
tags:
  - health
  - prerequisite

steps:
  - name: "Server responds"
    tool: list_tables
    input: {}
    expect:
      success: true
      response_time_ms:
        less_than: 10000

  - name: "Database connected"
    tool: execute_query
    input:
      sql: "SELECT 1 as health"
    expect:
      success: true
```

```bash
# Run health check first, then full suite
cargo pmcp test run --scenario tests/scenarios/health/ --fail-fast && \
cargo pmcp test run --scenario tests/scenarios/
```

### Continuous Health Monitoring

```bash
#!/bin/bash
# health_monitor.sh - Run periodic health checks

while true; do
  if ! cargo pmcp test run \
    --server https://mcp.example.com/mcp \
    --scenario tests/scenarios/health/ \
    --quiet; then

    # Alert on failure
    curl -X POST https://hooks.slack.com/services/xxx \
      -d '{"text":"MCP Server health check failed!"}'
  fi

  sleep 60  # Check every minute
done
```

## Summary

Testing deployed servers requires:

1. **Proper authentication** - Headers, tokens, API keys
2. **Timeout configuration** - Account for cold starts
3. **Environment-specific settings** - Different configs per environment
4. **Performance assertions** - Verify SLAs are met
5. **Debugging tools** - Verbose logs for troubleshooting

## Exercises

1. **Configure staging tests** - Set up authentication and timeouts for a staging server
2. **Add latency assertions** - Create performance tests with response time requirements
3. **Test cold starts** - Configure tests that handle Lambda cold start times
4. **Debug a failure** - Use verbose mode to diagnose a failing remote test

---

*Continue to [CI/CD Integration](./ch12-02-cicd.md) →*
