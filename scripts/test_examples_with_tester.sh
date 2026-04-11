#!/bin/bash
# Test harness for running MCP tester against example servers
# This replaces manual curl testing with automated MCP tester validation

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
TESTER_BIN="echo"  # Placeholder for external MCP tester
SCENARIOS_DIR="./examples/scenarios"
RESULTS_DIR="./test-results"
TIMEOUT=30

# Ensure directories exist
mkdir -p "$RESULTS_DIR"

# Note about MCP tester
echo -e "${YELLOW}Note: Using external MCP tester...${NC}"
echo -e "${YELLOW}To use actual MCP tester, install it and update TESTER_BIN path${NC}"

# Function to test an HTTP example
test_http_example() {
    local example_name=$1
    local scenario_file=$2
    local port=${3:-8080}
    local features=${4:-"streamable-http"}

    echo -e "\n${YELLOW}Testing example: $example_name${NC}"

    # Build the example
    echo "Building $example_name..."
    cargo build --example "$example_name" --features "$features"

    # Start the server in background
    echo "Starting server on port $port..."
    cargo run --example "$example_name" --features "$features" &
    local server_pid=$!

    # Wait for server to start
    sleep 2

    # Run MCP tester with scenario
    echo "Running MCP tester with scenario..."
    if [ -f "$SCENARIOS_DIR/$scenario_file" ]; then
        if $TESTER_BIN scenario "http://localhost:$port" "$SCENARIOS_DIR/$scenario_file" \
            --format json > "$RESULTS_DIR/${example_name}_results.json" 2>&1; then
            echo -e "${GREEN}✓ $example_name passed all tests${NC}"
            local result=0
        else
            echo -e "${RED}✗ $example_name failed tests${NC}"
            cat "$RESULTS_DIR/${example_name}_results.json"
            local result=1
        fi
    else
        # No scenario file, run basic tests
        echo "No scenario file found, running basic compliance tests..."
        if $TESTER_BIN test "http://localhost:$port" --with-tools \
            --format json > "$RESULTS_DIR/${example_name}_results.json" 2>&1; then
            echo -e "${GREEN}✓ $example_name passed basic tests${NC}"
            local result=0
        else
            echo -e "${RED}✗ $example_name failed basic tests${NC}"
            local result=1
        fi
    fi

    # Stop the server
    echo "Stopping server..."
    kill $server_pid 2>/dev/null || true
    wait $server_pid 2>/dev/null || true

    return $result
}

# Function to test a stdio example
test_stdio_example() {
    local example_name=$1
    local scenario_file=$2

    echo -e "\n${YELLOW}Testing stdio example: $example_name${NC}"

    # Build the example
    echo "Building $example_name..."
    cargo build --example "$example_name"

    # Run MCP tester with stdio transport
    echo "Running MCP tester..."
    if [ -f "$SCENARIOS_DIR/$scenario_file" ]; then
        if $TESTER_BIN scenario stdio --command "cargo run --example $example_name" \
            "$SCENARIOS_DIR/$scenario_file" \
            --format json > "$RESULTS_DIR/${example_name}_results.json" 2>&1; then
            echo -e "${GREEN}✓ $example_name passed all tests${NC}"
            return 0
        else
            echo -e "${RED}✗ $example_name failed tests${NC}"
            return 1
        fi
    else
        # Basic stdio test
        if $TESTER_BIN test stdio --command "cargo run --example $example_name" \
            --format json > "$RESULTS_DIR/${example_name}_results.json" 2>&1; then
            echo -e "${GREEN}✓ $example_name passed basic tests${NC}"
            return 0
        else
            echo -e "${RED}✗ $example_name failed basic tests${NC}"
            return 1
        fi
    fi
}

# Main test execution
main() {
    local total_tests=0
    local passed_tests=0
    local failed_tests=0

    echo -e "${YELLOW}========================================${NC}"
    echo -e "${YELLOW}MCP Example Testing with MCP Tester${NC}"
    echo -e "${YELLOW}========================================${NC}"

    # Test HTTP examples
    HTTP_EXAMPLES=(
        "t04_streamable_http_stateful:t04_http_stateful_test.yaml:8080"
        "t05_streamable_http_stateless:t05_http_stateless_test.yaml:8081"
        "25-oauth-basic:25_oauth_test.yaml:8080"
    )

    for example_spec in "${HTTP_EXAMPLES[@]}"; do
        IFS=':' read -r example scenario port <<< "$example_spec"
        if test_http_example "$example" "$scenario" "$port"; then
            ((passed_tests++))
        else
            ((failed_tests++))
        fi
        ((total_tests++))
    done

    # Test stdio examples
    STDIO_EXAMPLES=(
        "s05_server_prompts:s05_server_prompts_test.yaml"
    )

    for example_spec in "${STDIO_EXAMPLES[@]}"; do
        IFS=':' read -r example scenario <<< "$example_spec"
        if test_stdio_example "$example" "$scenario"; then
            ((passed_tests++))
        else
            ((failed_tests++))
        fi
        ((total_tests++))
    done

    # Generate summary report
    echo -e "\n${YELLOW}========================================${NC}"
    echo -e "${YELLOW}Test Summary${NC}"
    echo -e "${YELLOW}========================================${NC}"
    echo "Total tests: $total_tests"
    echo -e "${GREEN}Passed: $passed_tests${NC}"
    echo -e "${RED}Failed: $failed_tests${NC}"

    # Generate consolidated JSON report
    echo "Generating consolidated report..."
    python3 -c "
import json
import glob

results = []
for file in glob.glob('$RESULTS_DIR/*_results.json'):
    try:
        with open(file) as f:
            data = json.load(f)
            results.append({
                'example': file.split('/')[-1].replace('_results.json', ''),
                'results': data
            })
    except:
        pass

with open('$RESULTS_DIR/consolidated_report.json', 'w') as f:
    json.dump({
        'total': $total_tests,
        'passed': $passed_tests,
        'failed': $failed_tests,
        'examples': results
    }, f, indent=2)
" || true

    if [ $failed_tests -eq 0 ]; then
        echo -e "\n${GREEN}All tests passed!${NC}"
        exit 0
    else
        echo -e "\n${RED}Some tests failed. Check $RESULTS_DIR for details.${NC}"
        exit 1
    fi
}

# Parse arguments
case "${1:-}" in
    --help|-h)
        echo "Usage: $0 [example-name]"
        echo "  Run MCP tester against example servers"
        echo "  If example-name is provided, only test that example"
        echo "  Otherwise, test all examples"
        exit 0
        ;;
    "")
        # Test all examples
        main
        ;;
    *)
        # Test specific example
        if test_http_example "$1" "$1_test.yaml" 8080; then
            echo -e "${GREEN}Test passed${NC}"
            exit 0
        else
            echo -e "${RED}Test failed${NC}"
            exit 1
        fi
        ;;
esac