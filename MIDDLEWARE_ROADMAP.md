# Middleware Implementation Roadmap

## ✅ Phase 1: HTTP Middleware Foundation (COMPLETE)

**Status**: Merged in PR #89 (Resolves #82, #83)

**What was delivered**:
- ✅ HttpMiddleware trait with lifecycle hooks (on_request, on_response, on_error)
- ✅ HttpMiddlewareChain with priority ordering
- ✅ OAuthClientMiddleware for bearer token injection
- ✅ Case-insensitive HTTP header handling
- ✅ Integration with StreamableHttpTransport
- ✅ 23 comprehensive integration tests
- ✅ Complete documentation (Chapter 10.3, Chapter 11)

---

## 📋 Remaining Work - Priority Matrix

### Priority 0: Critical Fixes & Improvements (QUICK WINS)

#### HeaderMap Migration (NEW - From Code Review)
**Status**: Not started - Interim HashMap solution in place

**Why needed**:
- Current implementation uses `HashMap<String, String>` with manual lowercase normalization
- `http::HeaderMap` is the proper type for HTTP headers
- Better performance and standard compliance
- Enables proper multi-value header support

**Work required**:
- Replace `HttpRequest::headers` and `HttpResponse::headers` with `http::HeaderMap`
- Update all header methods to use HeaderMap API
- Update tests to use HeaderMap
- **Effort**: Low (1 day)
- **Value**: High - Correctness and performance
- **Risk**: Low - Well-defined change

**Recommendation**: Do this before Phase 2 for clean foundation

---

### Priority 1: Core Integration & Ergonomics (HIGH VALUE, LOW-MEDIUM EFFORT)

#### Issue #80: First-class middleware integration (PARTIAL)
**Status**: HTTP layer ✅ complete, Builder/Protocol ⚠️ partial

**Clarification**: `Client::send_request` already invokes `EnhancedMiddlewareChain` for request/response!

**What's actually missing**:
1. **ClientBuilder API** (HIGH PRIORITY)
   - Add `.with_http_middleware()` and `.with_protocol_middleware()` builder methods
   - Mirror on `StreamableHttpTransportConfig` for clean HTTP chain passing
   - Example: `Client::builder().with_http_middleware(chain).with_protocol_middleware(proto_chain).build()`
   - **Effort**: Low-Medium (2 days)
   - **Value**: High - Much better ergonomics

2. **Protocol middleware for inbound events** (MEDIUM PRIORITY)
   - Inbound notifications not tied to `send_request`
   - Streamed events (SSE)
   - Background protocol flows (progress notifications, subscriptions)
   - **Effort**: Medium (2-3 days)
   - **Value**: Medium - Completeness for server-initiated flows

3. **Unified configuration** (LOW PRIORITY)
   - Clear separation: HTTP middleware vs Protocol middleware
   - Documentation on when to use each layer
   - **Effort**: Low (1 day)
   - **Value**: Medium - Clarity

**Recommendation**: Start with #1 (ClientBuilder API) for immediate ergonomic wins

---

### Priority 2: Production-Grade Features (HIGH VALUE, MEDIUM-HIGH EFFORT)

#### Issue #84: Enhanced LoggingMiddleware and RetryMiddleware
**Status**: Basic implementations exist, production features missing

**LoggingMiddleware enhancements**:
- [ ] **Default-on header redaction** (CRITICAL)
  - Redact `authorization`, `cookie`, `set-cookie`, `x-api-key` by default
  - Allow per-field overrides
  - Prevent accidental PII/secret leaks
- [ ] Status threshold filtering (only log 4xx/5xx)
- [ ] Stdio-safe mode (disable for stdio transports to avoid corrupting framing)
- [ ] Body size limits and truncation
- [ ] Timing information
- [ ] Method exclusion list
- **Effort**: Medium (3-4 days, start with redaction in week 1)
- **Value**: High - Production safety
- **Risk**: High if not done - Secret leaks

**RetryMiddleware enhancements with clear boundaries**:
- [ ] **Define retry coordination boundaries** (CRITICAL)
  - HTTP layer: Connection failures, 5xx errors, timeouts
  - Transport layer: SSE reconnection, WebSocket reconnection
  - Protocol layer: Request-level retries (non-idempotent awareness)
  - **No double-retry**: Use metadata signals (`oauth.retry_used`, `http.retry_count`)
- [ ] Exponential backoff with jitter (avoid thundering herd)
- [ ] Idempotency awareness (don't retry mutations without explicit opt-in)
- [ ] Per-method retry configuration
- [ ] Honor OAuth middleware metadata to prevent infinite loops
- **Effort**: Medium (3-4 days)
- **Value**: High - Reliability without double-retry bugs
- **Risk**: Medium - Coordination complexity

**Retry Coordination Metadata Contract**:
```rust
// OAuth sets when it detects 401:
context.set_metadata("auth_failure", "true");
context.set_metadata("oauth.retry_used", "true");  // After first retry

// HTTP retry sets:
context.set_metadata("http.retry_count", retry_count.to_string());

// Protocol retry checks:
if context.get_metadata("http.retry_count").is_some() { /* skip */ }
```

**Recommendation**: Start with logging redaction (week 1), then retry boundaries (week 2)

---

#### Issue #85: Enhanced CircuitBreaker and RateLimit
**Status**: Basic implementations exist, scoping and metrics missing

**CircuitBreakerMiddleware enhancements**:
- [ ] **Scoping: Start with server+method** (RECOMMENDED)
  - Per-server scope (track each upstream separately)
  - Per-method scope (track `tools/call` vs `resources/read` separately)
  - Layer session scoping later if needed
  - Avoid over-granularity initially
- [ ] State persistence (file, then Redis)
- [ ] Metrics aggregation (integrate with existing MetricsMiddleware)
- [ ] Dynamic threshold adjustment (optional, later)
- **Effort**: Medium-High (4-5 days)
- **Value**: High - Production resilience
- **Approach**: Incremental (server scope → method scope → session scope)

**RateLimitMiddleware enhancements**:
- [ ] Scoping strategies (global, per-endpoint, per-session)
- [ ] Persistence across restarts
- [ ] Distributed coordination (Redis)
- [ ] Backpressure signals
- **Effort**: High (5-6 days)
- **Value**: High - Multi-tenant safety

**Recommendation**: Circuit breaker scoping is medium priority, rate limit is lower

---

### Priority 3: Advanced Features (MEDIUM VALUE, MEDIUM-HIGH EFFORT)

#### Issue #86: CompressionMiddleware
**Status**: Not implemented

**Features needed**:
- [ ] **Content-Type gating** (CRITICAL)
  - Only compress `application/json` by default
  - Configurable allowlist
  - Skip binary/already-compressed content
- [ ] **Minimum size threshold** (e.g., 1KB)
  - Don't compress small payloads (overhead > savings)
  - Configurable threshold
- [ ] **Transport awareness** (CRITICAL)
  - **NEVER compress on stdio** (breaks line-delimited JSON framing)
  - Only enable for HTTP/WebSocket
  - Auto-detect transport type
- [ ] Support multiple algorithms (Gzip, Deflate, Brotli, Zstd)
- [ ] Compression level configuration
- [ ] **Automatic decompression** (transparent to downstream middleware)
- [ ] Compression metrics (bytes saved, ratio, time)
- **Effort**: Medium-High (4-5 days)
- **Value**: Medium - Bandwidth savings for large payloads
- **Risk**: Medium - Stdio corruption if not transport-aware

**Recommendation**: Medium priority - Useful for edge/mobile deployments, but ensure stdio safety

---

#### Issue #87: Prometheus and OpenTelemetry exporters
**Status**: Not implemented

**Prometheus exporter**:
- [ ] HTTP /metrics endpoint
- [ ] Standard metric types (counters, histograms, gauges)
- [ ] Request/response metrics
- [ ] Transport metrics
- [ ] Middleware-specific metrics (circuit breaker, rate limit)
- **Effort**: Medium (3-4 days)
- **Value**: High - Observability

**OpenTelemetry exporter**:
- [ ] Distributed tracing integration
- [ ] Span creation for requests/middleware
- [ ] **Request ID propagation** from HttpMiddlewareContext and Client RequestId
- [ ] Context propagation across boundaries
- [ ] OTLP export
- **Effort**: High (5-6 days)
- **Value**: High - Distributed systems observability

**Recommendation**: Prometheus first (week 2-3), OpenTelemetry later (week 4-6)

---

### Priority 4: Developer Experience (MEDIUM VALUE, LOW-MEDIUM EFFORT)

#### Issue #88: Middleware ergonomics
**Status**: Not implemented

**Features needed**:
1. **`create_middleware!` macro**
   - Concise custom middleware creation
   - Reduces boilerplate
   - **Effort**: Low (1-2 days)
   - **Value**: Medium - Better DX

2. **Pre-configured middleware stacks**
   - Stdio preset (logging, validation, no compression)
   - HTTP preset (auth, logging, retry, compression)
   - WebSocket preset (reconnection, heartbeat)
   - **Effort**: Low (1-2 days)
   - **Value**: High - Quick start

3. **Comprehensive examples**
   - Real-world middleware stacks
   - Custom middleware patterns
   - Testing strategies
   - **Effort**: Low (2-3 days)
   - **Value**: High - Documentation

**Recommendation**: Medium priority - Improves adoption

---

## 🎯 Recommended Implementation Order (REVISED)

### Immediate (Week 1) - Foundation & Safety
**Time estimate**: 1 week (5 days)
**Priority**: CRITICAL - Do this first

1. **HeaderMap Migration** (1 day)
   - Replace `HashMap<String, String>` with `http::HeaderMap`
   - Update all header methods
   - Fix tests
   - **Value**: High - Correctness and performance
   - **Risk**: Low - Well-defined change

2. **ClientBuilder API** (2 days)
   - Add `.with_http_middleware()` and `.with_protocol_middleware()`
   - Mirror on `StreamableHttpTransportConfig` for clean HTTP chain passing
   - **Value**: High - Ergonomics
   - **Risk**: Low - Additive change

3. **Middleware Presets** (1 day)
   - stdio preset (logging, validation, NO compression)
   - http preset (OAuth, logging, retry, compression)
   - websocket preset (reconnection, heartbeat)
   - **Value**: High - Quick start for users
   - **Risk**: Low - Configuration only

4. **Basic Logging Redaction** (1 day)
   - Default-on redaction for `authorization`, `cookie`, `set-cookie`, `x-api-key`
   - Allow per-field overrides
   - **Value**: CRITICAL - Prevent secret leaks
   - **Risk**: Low - Additive security

**Deliverables**: Clean foundation, safe defaults, better UX

---

### Next (Weeks 2-3) - Protocol Coverage & Observability
**Time estimate**: 2 weeks
**Priority**: HIGH - Complete the integration story

1. **Protocol Middleware for Inbound Events** (3 days)
   - Wrap notifications and streaming paths
   - Add tests for unsolicited events (server-initiated)
   - Background protocol flows (progress, subscriptions)
   - **Value**: Medium - Completeness
   - **Risk**: Medium - Need to identify all paths

2. **Retry Enhancements with Coordination** (4 days)
   - Define clear boundaries (HTTP vs Transport vs Protocol)
   - Exponential backoff with jitter
   - Idempotency awareness (opt-in for mutations)
   - Honor OAuth metadata (`oauth.retry_used`, `http.retry_count`)
   - **Value**: High - Reliability without double-retry bugs
   - **Risk**: Medium - Coordination complexity

3. **Prometheus Exporter** (3 days)
   - HTTP /metrics endpoint
   - Counters/histograms for request/response
   - Middleware timings (piggyback on existing MetricsMiddleware)
   - **Value**: High - Observability
   - **Risk**: Low - Standard pattern

**Deliverables**: Complete middleware coverage, production observability

---

### Following (Weeks 4-6) - Production Hardening
**Time estimate**: 3 weeks
**Priority**: MEDIUM - Production-grade features

1. **Enhanced Logging** (1 week)
   - Status threshold filtering (only log 4xx/5xx)
   - Stdio-safe mode (detect transport type)
   - Body truncation and size limits
   - Timing information
   - Method exclusion list
   - **Value**: High - Production safety
   - **Risk**: Low - Additive features

2. **Circuit Breaker Scoping** (1 week)
   - Start with server+method scoping
   - State persistence (file first, Redis later)
   - Metrics aggregation
   - **Value**: High - Resilience
   - **Risk**: Medium - Persistence complexity

3. **OpenTelemetry Spans** (1 week)
   - Span creation with request ID propagation
   - Context propagation from HttpMiddlewareContext and Client RequestId
   - OTLP export
   - **Value**: High - Distributed tracing
   - **Risk**: Medium - Integration complexity

**Deliverables**: Production-hardened middleware

---

### Future (Weeks 7+) - Advanced Features
**Time estimate**: 2-3 weeks
**Priority**: LOW - Nice-to-have

1. **Compression** (1 week)
   - Content-Type gating (only `application/json`)
   - Minimum size threshold (e.g., 1KB)
   - Transport awareness (NEVER on stdio)
   - Automatic decompression
   - **Value**: Medium - Bandwidth savings
   - **Risk**: Medium - Stdio corruption if not careful

2. **RateLimit Enhancements** (1 week)
   - Scoping and persistence
   - Distributed coordination (Redis)
   - **Value**: Medium - Multi-tenant safety
   - **Risk**: Medium - Distributed state

3. **Developer Experience** (1 week)
   - `create_middleware!` macro
   - Middleware groups (enable/disable together)
   - **Value**: Medium - Better DX
   - **Risk**: Low - Quality-of-life

**Deliverables**: Advanced features for edge cases

---

## 📊 Effort vs Value Matrix

```
High Value │ #80 (Builder)  │ #87 (Prom)      │ #84 (Logging)
          │ #88 (Presets)  │ #85 (Circuit)   │ #84 (Retry)
          │                │                 │
────────────┼────────────────┼─────────────────┼──────────────
          │ #80 (Protocol) │ #85 (RateLimit) │
Medium    │ #88 (Macro)    │ #86 (Compress)  │ #87 (OTel)
Value     │                │                 │
────────────┼────────────────┼─────────────────┼──────────────
          │                │                 │
Low Value │                │                 │
          │                │                 │
          └────────────────┴─────────────────┴──────────────
            Low Effort      Medium Effort      High Effort
```

---

## 🚀 Quick Wins

**If we want immediate impact with low effort:**

1. **Middleware presets** (Issue #88) - 2 days
   - Pre-configured stacks for common use cases
   - Immediate value for new users

2. **ClientBuilder API** (Issue #80) - 3 days
   - Much better ergonomics
   - Easy migration path

3. **Basic header filtering** (Issue #84) - 2 days
   - Security/privacy win
   - Low complexity

**Total**: 1 week for significant UX improvements

---

## 🎓 Future Enhancement Opportunities

Beyond the current issues, consider:

1. **Middleware testing utilities**
   - Mock middleware for testing
   - Assertion helpers
   - Test fixtures

2. **Middleware composition patterns**
   - Conditional middleware (apply based on context)
   - Middleware groups (enable/disable together)
   - Async middleware chains

3. **Performance optimizations**
   - Middleware caching
   - Zero-copy middleware
   - Parallel middleware execution where safe

4. **Additional built-in middleware**
   - CachingMiddleware (response caching)
   - ValidationMiddleware (schema validation)
   - TimeoutMiddleware (request timeouts)
   - CORSMiddleware (HTTP CORS handling)

---

## 📝 Notes

**Current state:**
- HTTP middleware layer is complete and production-ready
- Protocol middleware exists but requires manual invocation
- Built-in middleware (Logging, Retry, Circuit Breaker, RateLimit, Metrics) have basic implementations
- OAuth client middleware is production-ready

**Key decisions needed:**
1. Which phase should we prioritize? (Recommend Phase 2 for adoption)
2. Should we implement all of #84/#85 or start with just logging/retry?
3. Do we need compression (Issue #86) before observability (Issue #87)?

**Dependencies:**
- Issue #80 (Protocol auto-invoke) should come before Issue #88 (examples) to show complete patterns
- Issue #87 (metrics exporters) depends on enhanced metrics from #84 and #85
- Issue #86 (compression) is independent and can be done anytime

---

## 📞 Stakeholder Questions

1. **What are the top pain points users are experiencing?**
   - If it's "hard to set up middleware" → Prioritize #80 + #88
   - If it's "logs leak secrets" → Prioritize #84 (logging)
   - If it's "no observability" → Prioritize #87 (Prometheus)

2. **What deployment scenarios are most common?**
   - Edge/mobile → Prioritize #86 (compression)
   - Multi-tenant SaaS → Prioritize #85 (rate limiting with scoping)
   - Enterprise/cloud → Prioritize #87 (observability)

3. **What's the timeline for production readiness?**
   - Need production-ready soon → Prioritize #84 (logging safety) + #87 (Prometheus)
   - Can wait for better UX → Prioritize #80 + #88

**Recommended starting point**: Phase 2 (#80 + #88) for adoption, then reassess based on user feedback.
