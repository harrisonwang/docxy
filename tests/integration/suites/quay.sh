#!/usr/bin/env bash
#
# Quay.io Registry Test Suite
#

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONFIG_DIR="$(dirname "$SCRIPT_DIR")/configs"

source "$CONFIG_DIR/registries.env"

echo "==> Testing Quay.io images..."

# Cleanup function
cleanup() {
    docker rmi ${QUAY}/prometheus/prometheus:v2.54.1 \
        ${QUAY}/centos/centos:stream9 \
        ${QUAY}/jetstack/cert-manager-controller:v1.15.3 \
        ${QUAY}/podman/stable:latest 2>/dev/null || true
}
trap cleanup EXIT

# Popular project images
docker pull ${QUAY}/prometheus/prometheus:v2.54.1
docker pull ${QUAY}/centos/centos:stream9
docker pull ${QUAY}/jetstack/cert-manager-controller:v1.15.3
docker pull ${QUAY}/podman/stable:latest

echo "==> Testing Quay.io multi-platform images..."

# Multi-platform tests
docker pull --platform linux/amd64 ${QUAY}/prometheus/prometheus:v2.54.1
docker pull --platform linux/arm64 ${QUAY}/prometheus/prometheus:v2.54.1

echo "Quay.io tests passed!"
