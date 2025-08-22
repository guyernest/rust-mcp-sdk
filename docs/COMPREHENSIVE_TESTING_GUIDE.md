# PMCP SDK Comprehensive Testing Guide

This guide demonstrates the complete testing infrastructure for the PMCP SDK, implementing Toyota Way quality principles with ALWAYS requirements.

## Testing Philosophy

We follow the **ALWAYS Requirements** for every new feature:

1. **FUZZ Testing** - Property-based fuzzing for robustness
2. **PROPERTY Testing** - Invariant verification with quickcheck/proptest
3. **UNIT Testing** - Comprehensive unit test coverage (80%+)
4. **EXAMPLE** - Working `cargo run --example` demonstration

Additionally:
- **Integration Testing** - Full client-server scenarios
- **Performance Benchmarks** - Regression prevention
- **Documentation Testing** - All doctests must pass

## Testing Infrastructure Overview

### 1. Property-Based Testing (`tests/property_tests.rs`)

Property-based testing verifies invariants that should hold across all valid inputs:

```bash
# Run property tests
cargo test property_tests

# Run with more test cases
PROPTEST_CASES=10000 cargo test property_tests
```

**Key Properties Tested:**
- JSON-RPC serialization round-trip stability
- URI template deterministic behavior
- Error code consistency
- Capability logical consistency
- Transport message ordering

### 2. Fuzz Testing (`fuzz/`)

Fuzz testing discovers edge cases and security vulnerabilities:

```bash
# List available fuzz targets
cargo fuzz list

# Run protocol parsing fuzzer
cargo fuzz run protocol_parsing

# Run transport layer fuzzer
cargo fuzz run transport_layer

# Run all fuzz targets (time-limited)
make test-fuzz
```

**Fuzz Targets:**
- `protocol_parsing` - JSON-RPC message parsing
- `transport_layer` - Transport framing and buffering
- `auth_flows` - Authentication workflows
- `jsonrpc_handling` - JSON-RPC request/response handling

### 3. Unit Testing (`tests/unit_tests.rs`)

Comprehensive unit tests for all modules:

```bash
# Run unit tests
cargo test unit_tests

# Run with coverage
cargo llvm-cov --html unit_tests
```

**Coverage Areas:**
- Error handling (all error types)
- URI template operations
- Capability management
- Authentication workflows
- Transport primitives
- Batching and debouncing
- JSON validation
- Protocol compliance

### 4. Integration Testing (`tests/integration_tests.rs`)

Full client-server integration scenarios:

```bash
# Run integration tests
cargo test integration_tests

# Run with specific test threads
cargo test integration_tests -- --test-threads=1
```

**Integration Scenarios:**
- Client-server communication
- Transport layer integration
- Error handling across boundaries
- Batching system integration
- Performance integration tests
- Memory safety validation

### 5. Example Testing

All examples serve as both documentation and tests:

```bash
# Run all examples
make test-examples

# Run specific examples
cargo run --example 25_property_testing_demo
cargo run --example 26_quality_gates_demo
```

**ALWAYS Requirement Examples:**
- Property testing demonstration
- Quality gates validation
- All major features demonstrated

### 6. Performance Benchmarks (`benches/`)

Comprehensive performance regression testing:

```bash
# Run all benchmarks
cargo bench

# Run specific benchmark group
cargo bench jsonrpc_serialization

# Generate HTML reports
cargo bench -- --output-format html
```

**Benchmark Categories:**
- JSON-RPC serialization/deserialization
- Error handling performance
- URI template operations
- Capability checking
- Transport operations
- Memory allocation patterns

## Quality Gates

### Toyota Way Quality Gate Process

```bash
# Pre-commit quality gate (fast)
make pre-commit-gate

# Comprehensive quality gate
make quality-gate

# Extreme quality gate (release)
make quality-gate-strict

# ALWAYS requirements validation
make validate-always
```

### Quality Standards

- **Zero Tolerance**: No defects, technical debt, or unwraps in production
- **Complexity**: ≤25 cognitive complexity per function
- **Coverage**: 80%+ test coverage maintained
- **Performance**: No regressions, continuous improvement
- **Documentation**: 100% public API coverage with examples

## Testing Best Practices

### 1. Test Organization

```rust
// Group related tests in modules
#[cfg(test)]
mod error_handling_tests {
    use super::*;
    
    #[test]
    fn test_specific_error_case() {
        // Test implementation
    }
}
```

### 2. Property Test Design

```rust
proptest! {
    #[test]
    fn property_name(
        input in strategy_for_input()
    ) {
        // Property assertion
        prop_assert!(invariant_holds(input));
    }
}
```

### 3. Integration Test Patterns

```rust
#[tokio::test]
async fn test_client_server_interaction() {
    // Setup
    let server = create_test_server().await;
    let client = create_test_client().await;
    
    // Exercise
    let result = client.call_method().await;
    
    // Verify
    assert!(result.is_ok());
}
```

### 4. Benchmark Design

```rust
fn bench_operation(c: &mut Criterion) {
    let mut group = c.benchmark_group("operation");
    
    for size in [10, 100, 1000] {
        group.bench_with_input(
            BenchmarkId::new("operation", size),
            &size,
            |b, &size| {
                b.iter(|| {
                    black_box(operation_under_test(size));
                });
            },
        );
    }
}
```

## Continuous Integration

### GitHub Actions Integration

The CI pipeline enforces all quality gates:

```yaml
# .github/workflows/ci.yml
name: CI

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          components: rustfmt, clippy
      
      - name: Run quality gates
        run: make quality-gate
      
      - name: Run ALWAYS validation
        run: make validate-always
      
      - name: Generate coverage report
        run: cargo llvm-cov --lcov --output-path lcov.info
      
      - name: Upload coverage
        uses: codecov/codecov-action@v3
        with:
          file: lcov.info
```

### Local Development Workflow

1. **Setup Development Environment**
   ```bash
   make setup  # Install tools
   make setup-quality  # Setup pre-commit hooks
   ```

2. **Development Cycle**
   ```bash
   # Make changes
   edit src/your_feature.rs
   
   # Run quick validation
   make pre-commit-gate
   
   # Run comprehensive tests
   make test-all
   
   # Validate ALWAYS requirements
   make validate-always
   
   # Commit changes
   git add -A
   git commit -m "feat: your feature"
   ```

3. **Release Process**
   ```bash
   # Run extreme quality validation
   make quality-gate-strict
   
   # Create release
   make release-minor
   
   # Push release
   git push origin main --tags
   ```

## Debugging Test Failures

### Common Issues and Solutions

1. **Property Test Failures**
   ```bash
   # Run with detailed output
   PROPTEST_VERBOSE=1 cargo test property_tests
   
   # Generate minimal failing case
   PROPTEST_SHRINK=1 cargo test property_tests
   ```

2. **Fuzz Test Issues**
   ```bash
   # Run with specific input
   cargo fuzz run target_name input_file
   
   # Debug with gdb
   cargo fuzz run target_name -- -debug
   ```

3. **Performance Regressions**
   ```bash
   # Compare with baseline
   cargo bench -- --save-baseline main
   cargo bench -- --baseline main
   
   # Profile specific benchmark
   cargo bench bench_name -- --profile-time=5
   ```

4. **Coverage Issues**
   ```bash
   # Generate detailed coverage report
   cargo llvm-cov --html --open
   
   # Show uncovered lines
   cargo llvm-cov --text | grep "0.00%"
   ```

## Quality Metrics Dashboard

### Key Metrics Tracked

- **Test Coverage**: 80%+ maintained
- **Property Test Cases**: 1000+ per property
- **Fuzz Testing**: 24/7 continuous fuzzing
- **Benchmark Stability**: <5% variance
- **Documentation Coverage**: 100% public APIs
- **Complexity Metrics**: ≤25 per function
- **Technical Debt**: 0 SATD comments

### Reporting

```bash
# Generate quality report
make quality-report

# View coverage dashboard
cargo llvm-cov --html --open

# View benchmark results
cargo bench -- --output-format html
```

## Advanced Testing Techniques

### 1. Mutation Testing

```bash
# Install cargo-mutants
cargo install cargo-mutants

# Run mutation tests
cargo mutants

# Check test quality
make mutants
```

### 2. Security Testing

```bash
# Security audit
cargo audit

# Dependency scanning
cargo deny check

# Memory safety validation
valgrind cargo test
```

### 3. Performance Profiling

```bash
# CPU profiling
cargo bench -- --profile-time=10

# Memory profiling
valgrind --tool=massif cargo test

# Flame graph generation
cargo flamegraph --bench benchmark_name
```

## Conclusion

The PMCP SDK testing infrastructure implements Toyota Way principles with zero tolerance for defects. Every feature must satisfy the ALWAYS requirements:

- ✅ **FUZZ Testing** - Robustness validation
- ✅ **PROPERTY Testing** - Invariant verification  
- ✅ **UNIT Testing** - Comprehensive coverage
- ✅ **EXAMPLE** - Working demonstrations

This comprehensive approach ensures the highest quality Rust MCP SDK with enterprise-grade reliability and performance.
