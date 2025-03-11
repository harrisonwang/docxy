#!/bin/bash
# Script to deploy Docker Registry Proxy to Fly.io without authentication

# Install Fly.io CLI
curl -L https://fly.io/install.sh | sh
export FLYCTL_INSTALL="$HOME/.fly"
export PATH="$FLYCTL_INSTALL/bin:$PATH"

# Login to Fly.io (this will open a browser)
echo "Please login to Fly.io..."
fly auth login

# Launch the application
echo "Creating Fly.io application..."
fly launch --name docxy-public --no-deploy

# Deploy the application
echo "Deploying application to Fly.io..."
fly deploy

# Get the deployment URL
echo "Deployment complete. Opening application URL..."
fly open

echo "Your Docker Registry Proxy is now deployed to Fly.io without authentication."
echo "You can use the URL in your Docker configuration:"
echo '{
  "registry-mirrors": ["https://docxy-public.fly.dev"]
}'
echo "Add this to /etc/docker/daemon.json and restart Docker with: sudo systemctl restart docker"
