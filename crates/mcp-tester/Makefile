.PHONY: all build run test clean help install

# Default target
all: build

# Build the server tester
build:
	@echo "Building MCP Server Tester..."
	@cargo build --release
	@echo "✓ Build complete: target/release/mcp-tester"

# Install to cargo bin directory
install: build
	@echo "Installing mcp-tester to cargo bin..."
	@cargo install --path .
	@echo "✓ Installed to ~/.cargo/bin/mcp-tester"

# Run quick test against local server
test-local:
	@echo "Testing local server at http://localhost:8080..."
	@cargo run -- test http://localhost:8080 --with-tools

# Run quick connectivity test
quick-test:
	@echo "Running quick connectivity test..."
	@cargo run -- quick http://localhost:8080

# Run compliance tests
compliance:
	@echo "Running protocol compliance tests..."
	@cargo run -- compliance http://localhost:8080 --strict

# Run diagnostics
diagnose:
	@echo "Running connection diagnostics..."
	@cargo run -- diagnose http://localhost:8080 --network

# Test against OAuth example
test-oauth:
	@echo "Testing OAuth example server..."
	@cd ../25-oauth-basic && make run-http &
	@sleep 2
	@cargo run -- test http://localhost:8080 --with-tools
	@pkill -f "oauth-basic" || true

# Test OAuth-protected server with access token
test-oauth-server:
	@if [ -z "$(TOKEN)" ]; then \
		echo "Error: ACCESS_TOKEN is required"; \
		echo ""; \
		echo "Usage: make test-oauth-server TOKEN=YOUR_ACCESS_TOKEN"; \
		echo ""; \
		echo "To get an access token:"; \
		echo "1. Open MCP Inspector"; \
		echo "2. Connect to the OAuth-protected MCP server"; \
		echo "3. Complete the OAuth login flow"; \
		echo "4. Copy the access token from the inspector"; \
		echo ""; \
		exit 1; \
	fi
	@echo "Testing OAuth-protected server with token..."
	@cargo run -- test $(SERVER_URL) --api-key "$(TOKEN)"

# Test AI-evals server with OAuth
test-ai-evals:
	@if [ -z "$(TOKEN)" ]; then \
		echo "Error: TOKEN is required"; \
		echo "Usage: make test-ai-evals TOKEN=YOUR_ACCESS_TOKEN"; \
		exit 1; \
	fi
	@echo "Testing AI-evals MCP server..."
	@cargo run -- test https://9nq2m33mi0.execute-api.us-west-2.amazonaws.com/mcp --api-key "$(TOKEN)"

# Test AI-evals with tools
test-ai-evals-tools:
	@if [ -z "$(TOKEN)" ]; then \
		echo "Error: TOKEN is required"; \
		echo "Usage: make test-ai-evals-tools TOKEN=YOUR_ACCESS_TOKEN"; \
		exit 1; \
	fi
	@echo "Testing AI-evals MCP server with tools..."
	@cargo run -- test https://9nq2m33mi0.execute-api.us-west-2.amazonaws.com/mcp --api-key "$(TOKEN)" --with-tools

# Diagnose OAuth server connection
diagnose-oauth:
	@if [ -z "$(TOKEN)" ]; then \
		echo "Error: TOKEN is required"; \
		echo "Usage: make diagnose-oauth TOKEN=YOUR_ACCESS_TOKEN SERVER_URL=https://..."; \
		exit 1; \
	fi
	@echo "Diagnosing OAuth server connection..."
	@cargo run -- diagnose $(SERVER_URL) --api-key "$(TOKEN)" --network

# Test with JSON output
test-json:
	@cargo run -- test http://localhost:8080 --format json

# Run all example tests
test-all-examples:
	@echo "Testing all example servers..."
	@# Test each example that provides an MCP server
	@for example in ../*/; do \
		if [ -f "$$example/Cargo.toml" ] && grep -q "pmcp" "$$example/Cargo.toml"; then \
			echo "Testing $$example..."; \
			cargo run -- quick stdio < /dev/null || true; \
		fi \
	done

# Compare two servers
compare:
	@echo "Comparing two servers..."
	@cargo run -- compare http://localhost:8080 http://localhost:8081 --with-perf

# Development run with verbose output
dev:
	@cargo run -- test http://localhost:8080 --format verbose -vvv

# Run with custom arguments
run:
	@cargo run -- $(ARGS)

# Clean build artifacts
clean:
	@echo "Cleaning build artifacts..."
	@cargo clean
	@echo "✓ Clean complete"

# Format code
fmt:
	@echo "Formatting code..."
	@cargo fmt
	@echo "✓ Format complete"

# Run clippy
clippy:
	@echo "Running clippy..."
	@cargo clippy -- -D warnings
	@echo "✓ Clippy complete"

# Run tests
test:
	@echo "Running unit tests..."
	@cargo test
	@echo "✓ Tests complete"

# Full quality check
quality: fmt clippy test
	@echo "✓ All quality checks passed"

# Show help
help:
	@echo "MCP Server Tester - Makefile Commands"
	@echo ""
	@echo "Build & Install:"
	@echo "  make build          - Build the server tester"
	@echo "  make install        - Install to ~/.cargo/bin"
	@echo ""
	@echo "Testing:"
	@echo "  make test-local     - Test local server at :8080"
	@echo "  make quick-test     - Quick connectivity test"
	@echo "  make compliance     - Run compliance tests"
	@echo "  make diagnose       - Run connection diagnostics"
	@echo "  make test-oauth     - Test OAuth example"
	@echo "  make test-json      - Test with JSON output"
	@echo "  make test-all       - Test all examples"
	@echo "  make compare        - Compare two servers"
	@echo ""
	@echo "OAuth Testing:"
	@echo "  make test-oauth-server TOKEN=... SERVER_URL=..."
	@echo "                      - Test OAuth-protected server"
	@echo "  make test-ai-evals TOKEN=..."
	@echo "                      - Test AI-evals MCP server"
	@echo "  make test-ai-evals-tools TOKEN=..."
	@echo "                      - Test AI-evals with tools"
	@echo "  make diagnose-oauth TOKEN=... SERVER_URL=..."
	@echo "                      - Diagnose OAuth server"
	@echo ""
	@echo "Development:"
	@echo "  make dev            - Run with verbose output"
	@echo "  make run ARGS=...   - Run with custom arguments"
	@echo "  make fmt            - Format code"
	@echo "  make clippy         - Run clippy linter"
	@echo "  make test           - Run unit tests"
	@echo "  make quality        - Run all quality checks"
	@echo "  make clean          - Clean build artifacts"
	@echo ""
	@echo "Examples:"
	@echo "  make run ARGS='test http://localhost:8080'"
	@echo "  make run ARGS='tools http://localhost:8080 --test-all'"
	@echo "  make test-ai-evals TOKEN=your_access_token"
	@echo "  make diagnose-oauth TOKEN=your_token SERVER_URL=https://api.example.com/mcp"