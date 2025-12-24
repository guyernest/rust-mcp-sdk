.PHONY: build run test inspector clean help

# Default content directory
CONTENT_DIR ?= ../../pmcp-course/src

# Default target
help:
	@echo "Course Server Minimal - Available Commands"
	@echo "==========================================="
	@echo ""
	@echo "Building:"
	@echo "  make build          - Build the course server"
	@echo "  make build-release  - Build optimized release version"
	@echo ""
	@echo "Running:"
	@echo "  make run            - Run server in STDIO mode"
	@echo "  make run CONTENT_DIR=/path/to/content - Run with custom content"
	@echo ""
	@echo "Testing:"
	@echo "  make test           - Test STDIO protocol"
	@echo "  make test-tools     - Test all tools"
	@echo "  make test-resources - Test resource reading"
	@echo ""
	@echo "MCP Inspector:"
	@echo "  make inspector      - Launch with MCP Inspector (STDIO)"
	@echo ""
	@echo "Utilities:"
	@echo "  make clean          - Clean build artifacts"
	@echo "  make watch          - Watch and rebuild on changes"

# Build targets
build:
	@echo "Building course server..."
	@cargo build --package course-server-minimal

build-release:
	@echo "Building course server (release)..."
	@cargo build --package course-server-minimal --release

# Run targets
run: build
	@echo "Starting course server with content from: $(CONTENT_DIR)"
	@CONTENT_DIR=$(CONTENT_DIR) ./../../target/debug/course-server-minimal

# Test targets
test: build
	@echo "Testing STDIO protocol..."
	@echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0.0"}}}' | \
		CONTENT_DIR=$(CONTENT_DIR) ./../../target/debug/course-server-minimal 2>/dev/null | \
		head -1 | jq . || echo "Error: No response or invalid JSON"

test-tools: build
	@echo "Testing tools/list..."
	@(echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0.0"}}}'; \
	  echo '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}') | \
		CONTENT_DIR=$(CONTENT_DIR) ./../../target/debug/course-server-minimal 2>/dev/null | \
		tail -1 | jq .
	@echo ""
	@echo "Testing list_chapters tool..."
	@(echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0.0"}}}'; \
	  echo '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"list_chapters","arguments":{}}}') | \
		CONTENT_DIR=$(CONTENT_DIR) ./../../target/debug/course-server-minimal 2>/dev/null | \
		tail -1 | jq .

test-resources: build
	@echo "Testing resources/list..."
	@(echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0.0"}}}'; \
	  echo '{"jsonrpc":"2.0","id":2,"method":"resources/list","params":{}}') | \
		CONTENT_DIR=$(CONTENT_DIR) ./../../target/debug/course-server-minimal 2>/dev/null | \
		tail -1 | jq .

# MCP Inspector
inspector: build
	@echo "Launching MCP Inspector with STDIO transport..."
	@echo "Content directory: $(CONTENT_DIR)"
	@CONTENT_DIR=$(CONTENT_DIR) npx @modelcontextprotocol/inspector $(shell pwd)/../../target/debug/course-server-minimal

# Utility targets
clean:
	@echo "Cleaning build artifacts..."
	@cargo clean --package course-server-minimal

watch:
	@echo "Watching for changes..."
	@CONTENT_DIR=$(CONTENT_DIR) cargo watch -x "build --package course-server-minimal"
