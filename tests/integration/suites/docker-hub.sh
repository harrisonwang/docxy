#!/usr/bin/env bash
#
# Docker Hub Registry Test Suite
#

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONFIG_DIR="$(dirname "$SCRIPT_DIR")/configs"

source "$CONFIG_DIR/registries.env"

echo "==> Testing Docker Hub official images..."

# Cleanup function
cleanup() {
    docker rmi ${DOCKERHUB}/hello-world:latest \
        ${DOCKERHUB}/alpine:3.20 \
        ${DOCKERHUB}/library/alpine:3.20 \
        ${DOCKERHUB}/busybox:1.36 \
        ${DOCKERHUB}/nginx:1.27-alpine \
        ${DOCKERHUB}/curlimages/curl:8.10.1 2>/dev/null || true
}
trap cleanup EXIT

# Official library images - short names
docker pull ${DOCKERHUB}/hello-world:latest
docker pull ${DOCKERHUB}/alpine:3.20
docker pull ${DOCKERHUB}/busybox:1.36
docker pull ${DOCKERHUB}/nginx:1.27-alpine

# Official library images - explicit library/ namespace
docker pull ${DOCKERHUB}/library/alpine:3.20

echo "==> Testing Docker Hub multi-platform images..."

# Multi-platform tests
docker pull --platform linux/amd64 ${DOCKERHUB}/alpine:3.20
docker pull --platform linux/arm64 ${DOCKERHUB}/alpine:3.20

echo "==> Testing Docker Hub non-official namespaces..."

# Non-official namespaces
docker pull ${DOCKERHUB}/curlimages/curl:8.10.1

# Multi-platform for non-official
docker pull --platform linux/amd64 ${DOCKERHUB}/curlimages/curl:8.10.1
docker pull --platform linux/arm64 ${DOCKERHUB}/curlimages/curl:8.10.1

echo "Docker Hub tests passed!"
