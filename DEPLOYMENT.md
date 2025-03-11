# Deploying Docker Registry Proxy Without Authentication

This document provides instructions for deploying the Docker Registry Proxy service without authentication, making it suitable for use as a Docker registry mirror.

## Local Deployment

1. Clone the repository:
   ```bash
   git clone https://github.com/harrisonwang/docxy.git
   cd docxy
   git checkout devin/1741594099-remove-auth-requirement-v2
   ```

2. Build and run the service:
   ```bash
   cargo build --release
   PORT=8080 ./target/release/docxy
   ```

   Or use the provided script:
   ```bash
   ./deploy-without-auth.sh
   ```

3. The service will be available at `http://localhost:8080`

## Deployment to Render.com

1. Create a new Web Service on Render.com
2. Connect your GitHub repository
3. Use the following settings:
   - Build Command: `cargo build --release`
   - Start Command: `PORT=8080 ./target/release/docxy`
   - Environment Variables:
     - `PORT`: `8080`

4. Deploy the service
5. The service will be available at `https://your-app-name.onrender.com`

## Deployment to Fly.io

1. Install the Fly.io CLI:
   ```bash
   curl -L https://fly.io/install.sh | sh
   ```

2. Login to Fly.io:
   ```bash
   fly auth login
   ```

3. Deploy the application:
   ```bash
   fly launch --name docxy-public
   fly deploy
   ```

4. The service will be available at `https://docxy-public.fly.dev`

## Using the Deployed Service

Once deployed, you can use the service as a Docker registry mirror by adding it to your Docker configuration:

```json
{
  "registry-mirrors": ["https://your-deployed-service-url"]
}
```

For Linux systems, this configuration is typically located at `/etc/docker/daemon.json`.

After updating the configuration, restart the Docker service:

```bash
sudo systemctl restart docker
```

## Verifying the Deployment

You can verify that the service is working correctly by accessing the following endpoints:

- Health check: `https://your-deployed-service-url/health`
- Docker Registry API v2: `https://your-deployed-service-url/v2/`

The health check should return `服务正常运行 - Health check passed`, and the v2 endpoint should return `{"repositories":[]}`.
