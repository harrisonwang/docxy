#!/usr/bin/env bash
#
# Docker Registry Proxy - Integration Test Runner
#
# Usage:
#   ./bin/run-tests.sh           # Run all test suites
#   ./bin/run-tests.sh --suite docker-hub  # Run specific suite
#

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SUITES_DIR="$(dirname "$SCRIPT_DIR")/suites"
CONFIGS_DIR="$(dirname "$SCRIPT_DIR")/configs"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Source configuration
if [[ -f "$CONFIGS_DIR/registries.env" ]]; then
    source "$CONFIGS_DIR/registries.env"
else
    log_error "Configuration file not found: $CONFIGS_DIR/registries.env"
    exit 1
fi

# Parse arguments
SUITE_FILTER=""
if [[ "${1:-}" == "--suite" ]]; then
    SUITE_FILTER="${2:-}"
    if [[ -z "$SUITE_FILTER" ]]; then
        log_error "Usage: $0 --suite <suite-name>"
        exit 1
    fi
fi

echo "=========================================="
echo "  Docker Registry Proxy Integration Tests"
echo "=========================================="
echo ""
log_info "Docker Hub:  ${DOCKERHUB}"
log_info "GHCR:        ${GHCR}"
log_info "Quay.io:     ${QUAY}"
echo ""

FAILED_SUITES=()

run_suite() {
    local suite_path="$1"
    local suite_name=$(basename "$suite_path" .sh)

    # Skip non-shell files and this script
    if [[ ! "$suite_path" =~ \.sh$ ]] || [[ "$suite_name" == "run-tests" ]]; then
        return 0
    fi

    # Apply filter if specified
    if [[ -n "$SUITE_FILTER" ]] && [[ "$suite_name" != "$SUITE_FILTER" ]]; then
        return 0
    fi

    echo ""
    echo "----------------------------------------"
    echo "Running: $suite_name"
    echo "----------------------------------------"

    if bash "$suite_path"; then
        log_info "✓ $suite_name passed"
    else
        log_error "✗ $suite_name failed"
        FAILED_SUITES+=("$suite_name")
    fi
}

# Run all suites
for suite in "$SUITES_DIR"/*.sh; do
    run_suite "$suite"
done

# Summary
echo ""
echo "=========================================="
if [[ ${#FAILED_SUITES[@]} -eq 0 ]]; then
    log_info "All tests passed! ✓"
    echo "=========================================="
    exit 0
else
    log_error "Some tests failed:"
    for suite in "${FAILED_SUITES[@]}"; do
        echo "  - $suite"
    done
    echo "=========================================="
    exit 1
fi
