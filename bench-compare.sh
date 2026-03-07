#!/bin/bash
# Symphony Performance Comparison Script
# Compares Elixir vs Rust implementations fairly

set -e

echo "==================================="
echo "SYMPHONY PERFORMANCE COMPARISON"
echo "Elixir vs Rust Implementation"
echo "==================================="
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Check prerequisites
check_prerequisites() {
    echo -e "${BLUE}Checking prerequisites...${NC}"
    
    if ! command -v cargo &> /dev/null; then
        echo -e "${RED}ERROR: Rust/Cargo not found${NC}"
        exit 1
    fi
    
    if ! command -v mix &> /dev/null; then
        echo -e "${YELLOW}WARNING: Elixir/Mix not found - Elixir tests will be skipped${NC}"
        SKIP_ELIXIR=1
    else
        SKIP_ELIXIR=0
    fi
    
    echo -e "${GREEN}✓ Prerequisites OK${NC}"
    echo ""
}

# Run Rust benchmarks
run_rust_benchmarks() {
    echo -e "${BLUE}=== RUST BENCHMARKS ===${NC}"
    echo "Running comprehensive Rust benchmark suite..."
    echo ""
    
    cd rust
    
    # Unit test count
    echo "Test Count:"
    cargo test --workspace 2>&1 | grep "test result" | awk '{sum+=$5} END {print "  Total tests:", sum}'
    
    echo ""
    echo "Running Criterion.rs benchmarks..."
    cd ../benches
    cargo bench 2>&1 | tee /tmp/rust_benchmarks.txt
    
    echo ""
    echo "Summary:"
    grep "time:" /tmp/rust_benchmarks.txt | head -20
    
    cd ..
}

# Run Elixir benchmarks
run_elixir_benchmarks() {
    if [ "$SKIP_ELIXIR" -eq 1 ]; then
        echo -e "${YELLOW}Skipping Elixir benchmarks (Elixir not installed)${NC}"
        return
    fi
    
    echo -e "${BLUE}=== ELIXIR BENCHMARKS ===${NC}"
    echo "Running Elixir benchmark suite..."
    echo ""
    
    cd elixir
    
    # Unit test count
    echo "Test Count:"
    mix test 2>&1 | grep -E "(passed|failed)" | tail -1
    
    echo ""
    echo "Note: Elixir benchmarks require Benchee setup"
    echo "To add benchmarks, install: {:benchee, \"~> 1.0\", only: :dev}"
    
    cd ..
}

# Run load tests
run_load_tests() {
    echo ""
    echo -e "${BLUE}=== LOAD TESTS ===${NC}"
    
    if ! command -v k6 &> /dev/null; then
        echo -e "${YELLOW}WARNING: k6 not installed - load tests will be skipped${NC}"
        echo "Install with: brew install k6 (macOS) or see https://k6.io/docs/get-started/installation/"
        return
    fi
    
    echo "Starting Symphony for load testing..."
    
    # Build and start Rust version
    cd rust
    cargo build --release --package symphony-cli
    ./target/release/symphony-cli ../WORKFLOW.md --port 8080 &
    RUST_PID=$!
    
    sleep 3
    
    echo ""
    echo "Running k6 load test..."
    cd ../load-tests
    k6 run --summary-trend-stats="avg,min,med,max,p(95),p(99)" load-test.js 2>&1 | tee /tmp/load_test_results.txt
    
    # Cleanup
    kill $RUST_PID 2>/dev/null || true
    
    echo ""
    echo "Load Test Summary:"
    grep -A 5 "http_req_duration" /tmp/load_test_results.txt || echo "See /tmp/load_test_results.txt for details"
}

# Memory usage comparison
memory_comparison() {
    echo ""
    echo -e "${BLUE}=== MEMORY USAGE ===${NC}"
    echo "Binary sizes:"
    
    # Rust binary size
    if [ -f "rust/target/release/symphony-cli" ]; then
        RUST_SIZE=$(du -h rust/target/release/symphony-cli | cut -f1)
        echo "  Rust binary: $RUST_SIZE"
    fi
    
    # Elixir release size (if available)
    if [ -d "elixir/_build" ]; then
        ELIXIR_SIZE=$(du -sh elixir/_build 2>/dev/null | cut -f1)
        echo "  Elixir build: $ELIXIR_SIZE"
    fi
}

# Generate report
generate_report() {
    echo ""
    echo "==================================="
    echo "COMPARISON REPORT"
    echo "==================================="
    echo ""
    
    echo "RUST RESULTS:"
    echo "  - Test Suite: $(grep -c "test result" /tmp/rust_benchmarks.txt 2>/dev/null || echo "N/A") test files"
    
    if [ -f "/tmp/rust_benchmarks.txt" ]; then
        echo "  - Reducer (1000 events): $(grep -A1 "reducer_claim/1000" /tmp/rust_benchmarks.txt | grep "time:" | awk '{print $2}')"
        echo "  - State Clone (100 issues): $(grep -A1 "state_clone_100_issues" /tmp/rust_benchmarks.txt | grep "time:" | awk '{print $2}')"
        echo "  - State Serialize: $(grep -A1 "state_serialize_100_issues" /tmp/rust_benchmarks.txt | grep "time:" | awk '{print $2}')"
        echo "  - Workspace Sanitize: $(grep -A1 "sanitize_workspace_key" /tmp/rust_benchmarks.txt | grep "time:" | awk '{print $2}')"
    fi
    
    echo ""
    echo "ELIXIR RESULTS:"
    if [ "$SKIP_ELIXIR" -eq 1 ]; then
        echo "  - Skipped (Elixir not installed)"
    else
        echo "  - See elixir/test results above"
    fi
    
    echo ""
    echo "LOAD TEST:"
    if [ -f "/tmp/load_test_results.txt" ]; then
        grep "http_req_duration" /tmp/load_test_results.txt || echo "  - See /tmp/load_test_results.txt"
    else
        echo "  - Skipped (k6 not installed)"
    fi
    
    echo ""
    echo "==================================="
    echo "Detailed results saved to:"
    echo "  - /tmp/rust_benchmarks.txt"
    echo "  - /tmp/load_test_results.txt"
    echo "==================================="
}

# Main execution
main() {
    check_prerequisites
    run_rust_benchmarks
    run_elixir_benchmarks
    memory_comparison
    run_load_tests
    generate_report
}

main "$@"
