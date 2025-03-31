# Docxy

[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)
[![Docker](https://img.shields.io/badge/docker-28%2B-blue.svg)](https://www.docker.com)
[![GitHub release](https://img.shields.io/github/v/release/harrisonwang/docxy)](https://github.com/harrisonwang/docxy/releases)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

<div align="center">
  <a href="./README.md"><img alt="README in English" src="https://img.shields.io/badge/English-d9d9d9"></a>
  <a href="./README_CN.md"><img alt="简体中文版自述文件" src="https://img.shields.io/badge/简体中文-d9d9d9"></a>
  <a href="./README_RU.md"><img alt="README на русском" src="https://img.shields.io/badge/Русский-d9d9d9"></a>
  <a href="./README_ES.md"><img alt="README en Español" src="https://img.shields.io/badge/Español-d9d9d9"></a>
  <a href="./README_KR.md"><img alt="README in Korean" src="https://img.shields.io/badge/한국어-d9d9d9"></a>
  <a href="./README_AR.md"><img alt="README بالعربية" src="https://img.shields.io/badge/العربية-d9d9d9"></a>
  <a href="./README_TR.md"><img alt="Türkçe README" src="https://img.shields.io/badge/Türkçe-d9d9d9"></a>
</div>

Lightweight Docker image proxy service that solves Docker Hub access restriction issues in certain regions.

## Background

### Introduction to Docker Image Repositories

Docker image repositories are services for storing and distributing Docker container images, providing centralized storage for containerized applications. These repositories allow developers to push, store, manage, and pull container images, simplifying the distribution and deployment process of applications.

### Types of Image Repositories

- **Official Image Repository**: Docker Hub, the official repository maintained by Docker Inc.
- **Third-party Independent Image Repositories**: Such as AWS ECR, Google GCR, Alibaba Cloud ACR, etc., used for publishing and sharing proprietary images
- **Image Acceleration Services**: Such as Tsinghua TUNA Mirror, Alibaba Cloud Image Accelerator, etc., which provide image acceleration services for Docker Hub

> [!NOTE]
> Due to network restrictions, direct access to Docker Hub is difficult in certain regions, and most image acceleration services have also been discontinued.

### Why Image Proxies are Needed

Image proxies are middleware services that connect Docker clients with Docker Hub. They don't store actual images but only forward requests, effectively solving:

- Network access restriction issues
- Improving image download speed

Docxy is such an image proxy service, aiming to bypass network blockages and accelerate image downloads through a self-hosted image proxy.

### Usage Limitations of Image Proxies

Docker Hub implements strict rate limiting policies for image pulls. When using proxy services, the following limitations exist:

- If not logged in, each IP address is limited to 10 image pulls per hour
- If logged in with a personal account, you can pull 100 images per hour
- For other account types, please refer to the table below:

| User Type                   | Pull Rate Limit       |
| --------------------------- | --------------------- |
| Business (authenticated)    | Unlimited             |
| Team (authenticated)        | Unlimited             |
| Pro (authenticated)         | Unlimited             |
| **Personal (authenticated)**| **100/hour/account**  |
| **Unauthenticated users**   | **10/hour/IP**        |

> [!WARNING]
> Note: This limitation will take effect from April 1, 2025

## Technical Principles

Docxy implements a complete Docker Registry API proxy, which only requires adding Docker client proxy configuration to use.

### System Architecture

```mermaid
graph TD
    Client[Docker Client] -->|Send Request| HttpServer[HTTP Server]
    
    subgraph "Docker Image Proxy Service"
        HttpServer -->|Route Request| RouterHandler[Router Handler]
        
        RouterHandler -->|/v2/| ChallengeHandler[Challenge Handler<br>proxy_challenge]
        RouterHandler -->|/auth/token| TokenHandler[Token Handler<br>get_token]
        RouterHandler -->|/v2/namespace/image/path_type| RequestHandler[Request Handler<br>handle_request]
        RouterHandler -->|/health| HealthCheck[Health Check]
        
        ChallengeHandler --> HttpClient
        TokenHandler --> HttpClient
        RequestHandler --> HttpClient
        
    end
    
    HttpClient[HTTP Client<br>reqwest]
    
    HttpClient -->|Auth Request| DockerAuth[Docker Auth<br>auth.docker.io]
    HttpClient -->|Image Request| DockerRegistry[Docker Registry<br>registry-1.docker.io]
```

### Request Flow

```mermaid
sequenceDiagram
    actor Client as Docker Client
    participant Proxy as Docxy Proxy
    participant Registry as Docker Registry
    participant Auth as Docker Auth Service
    
    %% Challenge Request Handling
    Client->>Proxy: GET /v2/
    Proxy->>+Registry: GET /v2/
    Registry-->>-Proxy: 401 Unauthorized (WWW-Authenticate)
    Proxy->>Proxy: Modify WWW-Authenticate header, pointing to local /auth/token
    Proxy-->>Client: 401 Return modified authentication header
    
    %% Token Acquisition
    Client->>Proxy: GET /auth/token?scope=repository:redis:pull
    Proxy->>+Auth: GET /token?service=registry.docker.io&scope=repository:library/redis:pull
    Auth-->>-Proxy: 200 Return token
    Proxy-->>Client: 200 Return original token response
    
    %% Image Metadata Request Handling
    Client->>Proxy: GET /v2/library/redis/manifests/latest
    Proxy->>+Registry: Forward request (with auth header and Accept header)
    Registry-->>-Proxy: Return image manifest
    Proxy-->>Client: Return image manifest (preserving original response headers and status code)
    
    %% Binary Data Handling
    Client->>Proxy: GET /v2/library/redis/blobs/{digest}
    Proxy->>+Registry: Forward blob request
    Registry-->>-Proxy: Return blob data
    Proxy-->>Client: Stream blob data back
```

### Certificate Handling Process

```mermaid
flowchart LR
    A[Start Service] --> B{Check Environment Variables}
    B -->|Exist| C[Use Specified Certificate Path]
    B -->|Don't Exist| D[Use Default Certificate Path]
    C --> E[Load Certificate Files]
    D --> E
    E --> F{Certificate Type Determination}
    F -->|ECC| G[Load ECC Private Key]
    F -->|RSA| H[Load RSA Private Key]
    F -->|PKCS8| I[Load PKCS8 Private Key]
    G --> J[Initialize TLS Configuration]
    H --> J
    I --> J
    J --> K[Start HTTPS Service]
```

## Features

- **Transparent Proxy**: Fully compatible with Docker Registry API v2
- **Seamless Integration**: Only requires configuring the mirror source, no change in usage habits
- **High-Performance Transfer**: Uses streaming processing for response data, supports large image downloads
- **TLS Encryption**: Built-in HTTPS support, ensuring secure data transmission
- **Accelerated Official Image Downloads**: Provides more stable connections
- **Bypassing Network Blockages**: Solves access restriction issues in certain regions

## Quick Start

> [!TIP]
> Before deployment, please resolve your domain to the target host in advance.

### One-Click Deployment

```bash
bash <(curl -Ls https://raw.githubusercontent.com/harrisonwang/docxy/main/install.sh)
```

> [!WARNING]
> Note: ZeroSSL certificate authority requires account registration before issuing certificates. For convenience, the script forces the use of Let's Encrypt as the certificate authority and forces certificate reissuance.

### Development

1. Clone the repository

   ```bash
   cd /opt
   git clone https://github.com/harrisonwang/docxy.git
   ```

2. Enter the project directory

   ```bash
   cd /opt/docxy
   ```

3. Configure certificates (using test.com domain as an example)

   ```bash
   export CERT_PATH=/root/.acme.sh/test.com_ecc/fullchain.cer
   export KEY_PATH=/root/.acme.sh/test.com_ecc/test.com.key
   ```

> [!TIP]
> Please apply for TLS certificates in advance using acme.sh

4. Start the service

   ```bash
   cargo run
   ```

5. Build the binary package

   ```bash
   cargo build --release
   ```

### Docker Client Configuration

Edit the `/etc/docker/daemon.json` configuration file and add the following proxy settings:

```json
{
  "registry-mirrors": ["https://test.com"]
}
```

### Health Check

You can check if the service is running properly by accessing the following endpoint:

```bash
curl https://test.com/health
```

## API Reference

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/health` | GET | Health check interface |
| `/v2/` | GET | Docker Registry API v2 entry point and authentication challenge |
| `/auth/token` | GET | Authentication token acquisition interface |
| `/v2/{namespace}/{image}/{path_type}/{reference}` | GET/HEAD | Image resource access interface, supporting manifests and blobs, etc. |

## Other Solutions

- [Cloudflare Worker Implementation of Image Proxy](https://voxsay.com/posts/china-docker-registry-proxy-guide/): Use with caution, may lead to Cloudflare account suspension.
- [Nginx Implementation of Image Proxy](https://voxsay.com/posts/china-docker-registry-proxy-guide/): Only proxies registry-1.docker.io, but still has requests sent to auth.docker.io. Once auth.docker.io is also blocked, it will not function properly.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
