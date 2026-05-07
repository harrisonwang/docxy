#!/usr/bin/env bash
#
# Digest-based Pull Test Suite
# Tests that registry proxy correctly handles content-addressable pulls
#

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONFIG_DIR="$(dirname "$SCRIPT_DIR")/configs"

source "$CONFIG_DIR/registries.env"

echo "==> Testing Docker Hub digest pulls..."

# Cleanup function
cleanup() {
    docker rmi \
        ${DOCKERHUB}/alpine@sha256:d9e853e87e55526f6b2917df91a2115c36dd7c696a35be12163d44e6e2a4b6bc \
        ${DOCKERHUB}/nginx@sha256:65645c7bb6a0661892a8b03b89d0743208a18dd2f3f17a54ef4b76fb8e2f2a10 \
        ${GHCR}/oras-project/oras@sha256:0087224dd0decc354b5b0689068fbbc40cd5dc3dbf65fcb3868dfbd363dc790b \
        ${GHCR}/kyverno/kyverno@sha256:ab21e9fd054d2f4197fc1e46285bd6e7b1353a7ed7749f80f75a1ac04f3426e7 \
        ${QUAY}/prometheus/prometheus@sha256:f6639335d34a77d9d9db382b92eeb7fc00934be8eae81dbc03b31cfe90411a94 \
        ${QUAY}/centos/centos@sha256:64cacfe1346daa9fedec8c59ce4dd6940a56ca4b29023cdb627bc414ac87029c \
        2>/dev/null || true
}
trap cleanup EXIT

# Docker Hub digests
docker pull ${DOCKERHUB}/alpine@sha256:d9e853e87e55526f6b2917df91a2115c36dd7c696a35be12163d44e6e2a4b6bc
docker pull ${DOCKERHUB}/library/alpine@sha256:d9e853e87e55526f6b2917df91a2115c36dd7c696a35be12163d44e6e2a4b6bc
docker pull ${DOCKERHUB}/nginx@sha256:65645c7bb6a0661892a8b03b89d0743208a18dd2f3f17a54ef4b76fb8e2f2a10

echo "==> Testing GHCR digest pulls..."

# GHCR digests
docker pull ${GHCR}/oras-project/oras@sha256:0087224dd0decc354b5b0689068fbbc40cd5dc3dbf65fcb3868dfbd363dc790b
docker pull ${GHCR}/kyverno/kyverno@sha256:ab21e9fd054d2f4197fc1e46285bd6e7b1353a7ed7749f80f75a1ac04f3426e7

echo "==> Testing Quay.io digest pulls..."

# Quay.io digests
docker pull ${QUAY}/prometheus/prometheus@sha256:f6639335d34a77d9d9db382b92eeb7fc00934be8eae81dbc03b31cfe90411a94
docker pull ${QUAY}/centos/centos@sha256:64cacfe1346daa9fedec8c59ce4dd6940a56ca4b29023cdb627bc414ac87029c

echo "Digest tests passed!"
