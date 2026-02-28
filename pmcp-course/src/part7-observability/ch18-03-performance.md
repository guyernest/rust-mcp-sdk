# Performance Optimization

In this chapter, you'll learn to load test your MCP servers using `cargo pmcp loadtest` -- a k6-inspired engine purpose-built for the MCP protocol. You'll start by running a test in under a minute, then progressively learn to configure scenarios, shape load profiles, interpret HdrHistogram percentiles, find breaking points, and plan capacity for production deployments. Every concept is introduced through a hands-on step you perform yourself.

## Learning Objectives

By the end of this chapter, you will be able to:

- Run a load test against an MCP server using `cargo pmcp loadtest`
- Write TOML configuration files defining test scenarios
- Use schema discovery to auto-generate configs from running servers
- Configure staged load profiles for capacity planning
- Interpret HdrHistogram percentiles and understand coordinated omission
- Find your server's breaking point under load
- Plan capacity for production deployments

## Why Load Test MCP Servers?

You've built your server, written unit tests, and integration tests are passing. So why add another layer of testing? Because **load testing answers questions that functional tests can't**: How many concurrent clients can your server handle? At what point do response times start degrading? Will the server survive a Monday morning traffic spike?

MCP has a unique characteristic compared to traditional web APIs: the ecosystem has **many more servers than clients** (similar to how there are many more websites than web browsers). Your server might be discovered by dozens of AI clients simultaneously, each sending rapid tool calls. Understanding how it performs under concurrent load is critical before deployment.

```
                  Testing Pyramid
          ┌───────────────────────────┐
          │       Load Testing        │  <-- You are here
          │   (capacity & limits)     │
          ├───────────────────────────┤
          │    Integration Testing    │
          │   (end-to-end flows)      │
          ├───────────────────────────┤
          │      Unit Testing         │
          │   (individual functions)  │
          └───────────────────────────┘

  Load testing sits at the top of the pyramid.
  It tests what happens when many clients
  hit your server at the same time.
```

Unlike generic HTTP load testing tools like `wrk` or `ab`, `cargo pmcp loadtest` understands MCP protocol semantics. It performs proper `initialize` handshakes, sends typed `tools/call`, `resources/read`, and `prompts/get` requests, and tracks per-operation metrics with HdrHistogram percentile accuracy. You don't need to manually construct JSON-RPC payloads -- the tool does it for you.

> For the complete reference documentation covering every flag, field, and algorithm in detail, see [Chapter 14 of the PMCP Book](../../pmcp-book/src/ch14-performance.md). This tutorial teaches the same material hands-on.

## Your First Load Test

Let's run your first load test in three steps. By the end of this section, you'll have performance numbers for an MCP server running on your machine.

### Step 1: Start Your Server

You'll need a running MCP server to test against. Let's use the calculator server from Chapter 2 -- it's simple, fast, and you already know how it works.

Open a terminal and start the calculator server:

```bash
# Build and run in release mode for accurate performance numbers
cargo run --release --bin calculator
```

Make sure it's running on `http://localhost:3000/mcp`. You should see a log line like:

```text
INFO  Starting calculator server
```

Leave this terminal open -- the server needs to keep running while we test it.

### Step 2: Generate a Config

Open a **second terminal** in the same project directory. The loadtest tool can connect to your running server, discover what tools it offers, and generate a TOML configuration file automatically. This is called **schema discovery**.

```bash
# Connect to the server and generate a config
cargo pmcp loadtest init http://localhost:3000/mcp
```

You'll see output like:

```text
Discovering server schema at http://localhost:3000/mcp...
Created .pmcp/loadtest.toml
Edit the file to customize your load test scenario.
```

Open `.pmcp/loadtest.toml` and notice how it discovered your calculator's tools automatically:

```toml
# Generated from server: http://localhost:3000/mcp

[settings]
# Number of concurrent virtual users
virtual_users = 10

# Test duration in seconds
duration_secs = 60

# Per-request timeout in milliseconds
timeout_ms = 5000

# Scenario steps discovered from server capabilities.
# Adjust weights to control the mix of operations.

[[scenario]]
type = "tools/call"
weight = 25
tool = "add"
# arguments = {}

[[scenario]]
type = "tools/call"
weight = 25
tool = "subtract"
# arguments = {}

[[scenario]]
type = "tools/call"
weight = 25
tool = "multiply"
# arguments = {}

[[scenario]]
type = "tools/call"
weight = 25
tool = "divide"
# arguments = {}
```

The `init` command connected to your server, called `tools/list` to discover the available tools, and generated a balanced config with each tool getting an equal weight. It also set up sensible defaults for VU count, duration, and timeout.

Let's add some arguments so the tool calls actually compute something. Edit the file to fill in the `arguments` lines:

```toml
[[scenario]]
type = "tools/call"
weight = 25
tool = "add"
arguments = { a = 42, b = 17 }

[[scenario]]
type = "tools/call"
weight = 25
tool = "subtract"
arguments = { a = 100, b = 37 }

[[scenario]]
type = "tools/call"
weight = 25
tool = "multiply"
arguments = { a = 6, b = 7 }

[[scenario]]
type = "tools/call"
weight = 25
tool = "divide"
arguments = { dividend = 100, divisor = 4 }
```

### Step 3: Run the Test

Now run the load test:

```bash
cargo pmcp loadtest run http://localhost:3000/mcp
```

You'll see a live progress display, followed by a k6-style summary when the test completes:

```text
          /\      |  cargo-pmcp loadtest
         /  \     |
    /\  /    \    |  target:    http://localhost:3000/mcp
   /  \/      \   |  vus:       10
  /    \       \  |  duration:  60s
 /      \       \ |  scenarios: 4 steps

  mcp_req_duration............: p50=12ms  p95=45ms  p99=120ms
  mcp_req_success_count.......: 4820
  mcp_req_error_count.........: 0
  mcp_req_error_rate..........: 0.0%
  mcp_req_throughput..........: 80.3 req/s
  mcp_req_total...............: 4820
  mcp_req_elapsed.............: 60.0s
```

Let's read the key metrics:

| Metric | Meaning |
|--------|---------|
| `p50=12ms` | Half your requests completed in under 12ms (the "typical" experience) |
| `p95=45ms` | 95% of requests finished in under 45ms (most users' worst case) |
| `p99=120ms` | 99% of requests finished in under 120ms (the unlucky 1%) |
| `mcp_req_throughput` | Your server handled ~80 requests per second |
| `mcp_req_error_rate` | No errors -- the calculator is rock solid |

Congratulations -- you've run your first MCP load test! A JSON report was also saved to `.pmcp/reports/` for programmatic consumption.

> **Your numbers will differ.** Performance depends on your hardware, OS, and what else is running. The important thing is that you have a baseline to compare against.

## Understanding the Config File

Now let's understand what that generated config actually means. The TOML configuration file has two main sections: settings and scenarios.

### The [settings] Block

The `[settings]` block controls how the load test runs:

```toml
[settings]
virtual_users = 10          # Simulated clients hitting your server simultaneously
duration_secs = 60          # How long the test runs (in seconds)
timeout_ms = 5000           # When to give up on a single request (5 seconds)
# expected_interval_ms = 100  # For coordinated omission correction (we'll cover this later)
```

Here's what each field does:

| Field | Purpose | Think of it as... |
|-------|---------|-------------------|
| `virtual_users` | Number of concurrent simulated clients | "10 AI clients using my server at the same time" |
| `duration_secs` | Total test duration | "Hammer the server for 60 seconds" |
| `timeout_ms` | Per-request timeout | "If a request takes more than 5 seconds, count it as failed" |
| `expected_interval_ms` | Coordinated omission correction interval | "How fast my server normally responds" (covered in the metrics section) |

**Why the URL isn't in the config:** Notice the server URL is passed on the command line, not in the config file. This is intentional -- the same config can be used against `localhost` in development, a staging server, and production without editing the file.

### The [[scenario]] Blocks

Each `[[scenario]]` block defines an MCP operation the load tester will execute. The `weight` field controls how often each operation is selected relative to the others.

**Weight-based proportional scheduling:** Weights are relative, not absolute percentages. If you have three scenarios with weights 60, 30, and 10, roughly 60% of requests will use the first scenario, 30% the second, and 10% the third. Weights do not need to sum to 100.

The tool supports three MCP operation types:

```toml
# Call a tool (like tools/call in the MCP protocol)
[[scenario]]
type = "tools/call"
weight = 60
tool = "calculate"
arguments = { expression = "2+2" }

# Read a resource (like resources/read)
[[scenario]]
type = "resources/read"
weight = 30
uri = "file:///data/config.json"

# Get a prompt (like prompts/get)
[[scenario]]
type = "prompts/get"
weight = 10
prompt = "summarize"
arguments = { text = "Hello world" }
```

**Try this:** Go back to your `.pmcp/loadtest.toml` and change the weights so that `add` gets 80% of the traffic while the other tools split the remaining 20%. Run the test again and notice how the per-tool metrics change:

```toml
[[scenario]]
type = "tools/call"
weight = 80
tool = "add"
arguments = { a = 42, b = 17 }

[[scenario]]
type = "tools/call"
weight = 7
tool = "subtract"
arguments = { a = 100, b = 37 }

[[scenario]]
type = "tools/call"
weight = 7
tool = "multiply"
arguments = { a = 6, b = 7 }

[[scenario]]
type = "tools/call"
weight = 6
tool = "divide"
arguments = { dividend = 100, divisor = 4 }
```

### Writing Your Own Config from Scratch

You don't have to use `loadtest init` -- you can write a config file from scratch. Here's a step-by-step guide:

**1.** Create a file at `.pmcp/loadtest.toml` (or any path -- you'll pass it with `--config`).

**2.** Add the required `[settings]` block with all three required fields:

```toml
[settings]
virtual_users = 5        # Start small -- you can always increase
duration_secs = 30       # 30 seconds is enough for a quick check
timeout_ms = 5000        # 5 second timeout
```

**3.** Add at least one `[[scenario]]` block:

```toml
[[scenario]]
type = "tools/call"
weight = 100             # Only one scenario, so weight doesn't matter
tool = "add"
arguments = { a = 1, b = 2 }
```

**4.** Run with your custom config:

```bash
cargo pmcp loadtest run http://localhost:3000/mcp --config .pmcp/loadtest.toml
```

**Validation rules the tool enforces:**

- At least one `[[scenario]]` must be defined
- Total weight across all scenarios must be greater than zero
- If `[[stage]]` blocks are present, each stage must have `duration_secs > 0`

If any rule is violated, you'll get a clear error message telling you what to fix.

## Staged Load Profiles

So far, you've been running **flat load** tests -- all virtual users start immediately and hammer the server at a constant rate for the entire duration. This is useful for measuring baseline performance, but what if you want to find your server's limits? That's where **staged load profiles** come in.

### Flat Load vs Staged Load

Here's the difference visually:

**Flat load** -- constant pressure:

```
VUs
 10 |##################################################
    |##################################################
    |##################################################
  0 └──────────────────────────────────────────────────
    0s                                              60s
```

All 10 VUs start at t=0 and run until t=60s. You get a constant, steady load.

**Staged load** -- controlled ramp:

```
VUs
 50 |                  ┌────────────────────┐
    |                /                        \
    |              /                            \
 25 |            /                                \
    |          /                                    \
 10 |        /                                        \
    |      /                                            \
  0 └────┬──────────┬────────────────────┬──────────┬────
    0s   10s        30s                 90s        120s
         ramp-up        hold             ramp-down
```

VUs start at 0, ramp up to 50 over 30 seconds, hold at 50 for 60 seconds, then ramp back down. This lets you observe how your server behaves as load increases and decreases.

**When to use each:**

| Goal | Use |
|------|-----|
| Measure baseline throughput at a known load level | Flat load |
| Find your server's breaking point | Staged load (ramp up) |
| Simulate realistic traffic patterns (morning ramp-up) | Staged load |
| Run a quick smoke test | Flat load with low VUs |
| Stress test graceful degradation | Staged load (ramp to extreme) |
| Soak test for memory leaks | Flat load over a long duration |

### Building Your First Staged Profile

Add `[[stage]]` blocks to your config to define a staged load profile. Each stage specifies a `target_vus` (where to end) and `duration_secs` (how long to get there). The engine **linearly ramps** the VU count from the previous stage's level to the new target over the stage's duration.

Here's a classic ramp-up, hold, ramp-down pattern:

```toml
[settings]
virtual_users = 10       # Ignored when stages are present
duration_secs = 60       # Ignored when stages are present
timeout_ms = 5000

[[scenario]]
type = "tools/call"
weight = 100
tool = "add"
arguments = { a = 42, b = 17 }

# Stage 1: Ramp up from 0 to 20 VUs over 30 seconds
[[stage]]
target_vus = 20
duration_secs = 30

# Stage 2: Hold at 20 VUs for 60 seconds
[[stage]]
target_vus = 20
duration_secs = 60

# Stage 3: Ramp down from 20 to 0 VUs over 30 seconds
[[stage]]
target_vus = 0
duration_secs = 30
```

**Important:** When `[[stage]]` blocks are present, `settings.virtual_users` and `settings.duration_secs` are ignored -- the stages control everything. The effective test duration is the sum of all stage durations (in this example: 30 + 60 + 30 = 120 seconds). A warning is printed if you have both settings and stages.

**How staged ramp works internally:**

- **Ramp up** (target > current): New VUs are spawned with linear stagger over the stage duration. Each VU gets its own cancellation token for independent lifecycle management.
- **Ramp down** (target < current): VU cancellation tokens are cancelled in LIFO order -- the most recently spawned VU is killed first.
- **Hold** (target == current): No VUs are added or removed; the engine waits for the remaining stage duration.

**Try this:** Save the config above and run it against your calculator server:

```bash
cargo pmcp loadtest run http://localhost:3000/mcp
```

Watch the live terminal display -- you'll see a `[stage 2/3]` indicator showing which stage the test is currently in. Notice how the throughput numbers change as VUs ramp up and down.

### Designing Profiles for Different Goals

Different testing goals call for different stage shapes:

| Goal | Profile Shape | Example Stages |
|------|--------------|----------------|
| **Capacity planning** | Gradual ramp to find limits | 0->10 (30s), 10->50 (60s), 50->100 (60s), 100->0 (30s) |
| **Soak testing** | Quick ramp to steady state, long hold | 0->20 (10s), 20->20 (300s), 20->0 (10s) |
| **Stress testing** | Aggressive ramp to extreme levels | 0->50 (10s), 50->200 (30s), 200->0 (10s) |
| **Realistic traffic** | Morning ramp, peak hours, evening decline | 0->10 (30s), 10->30 (60s), 30->30 (120s), 30->5 (60s) |

For capacity planning, the most useful pattern is a **gradual ramp from low to high VUs** combined with breaking point detection (covered in the next section). This tells you exactly where your server starts degrading.

<!-- CONTINUED IN TASK 2 -->
