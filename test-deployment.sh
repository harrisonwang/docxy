#!/bin/bash
# Script to test a deployed Docker Registry Proxy

# Usage: ./test-deployment.sh https://your-deployed-service-url

if [ -z "$1" ]; then
  echo "Usage: ./test-deployment.sh https://your-deployed-service-url"
  exit 1
fi

URL=$1

echo "Testing health check endpoint..."
curl -s $URL/health
echo ""

echo "Testing v2 endpoint..."
curl -s $URL/v2/
echo ""

echo "Testing with Docker..."
echo "{\"registry-mirrors\": [\"$URL\"]}" > /tmp/daemon.json
echo "Created Docker configuration:"
cat /tmp/daemon.json
echo ""
echo "To use this configuration, copy it to /etc/docker/daemon.json and restart Docker:"
echo "sudo cp /tmp/daemon.json /etc/docker/daemon.json"
echo "sudo systemctl restart docker"
