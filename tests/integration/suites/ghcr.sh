#!/usr/bin/env bash
#
# GHCR (GitHub Container Registry) Test Suite
#

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONFIG_DIR="$(dirname "$SCRIPT_DIR")/configs"

source "$CONFIG_DIR/registries.env"

echo "==> Testing GHCR images..."

# Cleanup function
cleanup() {
    docker rmi ${GHCR}/oras-project/oras:v1.2.0 \
        ${GHCR}/kyverno/kyverno:v1.13.0 \
        ${GHCR}/helm/chartmuseum:v0.16.2 \
        ${GHCR}/fluxcd/flux-cli:v2.4.0 2>/dev/null || true
}
trap cleanup EXIT

# Popular project images
docker pull ${GHCR}/oras-project/oras:v1.2.0
docker pull ${GHCR}/kyverno/kyverno:v1.13.0
docker pull ${GHCR}/helm/chartmuseum:v0.16.2
docker pull ${GHCR}/fluxcd/flux-cli:v2.4.0

echo "==> Testing GHCR multi-platform images..."

# Multi-platform tests
docker pull --platform linux/amd64 ${GHCR}/oras-project/oras:v1.2.0
docker pull --platform linux/arm64 ${GHCR}/oras-project/oras:v1.2.0

echo "GHCR tests passed!"
