# Docxy Technical Architecture & Principles

This document details the background, technical principles, system architecture, and implementation flow of the Docxy project.

## Background

### Introduction to Docker Image Registries

A Docker image registry is a service for storing and distributing Docker container images, providing centralized storage for containerized applications. These registries allow developers to push, store, manage, and pull container images, simplifying the application distribution and deployment process.

### Types of Image Registries

- **Official Registry**: Docker Hub, the official registry maintained by Docker, Inc.
- **Third-party Standalone Registries**: Such as AWS ECR, Google GCR, Aliyun ACR, etc., used for publishing and sharing proprietary images.
- **Mirror Services**: Such as the TUNA mirror site at Tsinghua University, Aliyun's mirror accelerator, etc., which provide acceleration for Docker Hub.

> [!NOTE]
> Due to network restrictions, direct access to Docker Hub from within mainland China is difficult, and most mirror services have ceased operation.

### Why a Registry Proxy is Needed

An image proxy is an intermediary service that connects the Docker client with Docker Hub. It does not store the actual images but only forwards requests, effectively solving:

- Network access restriction issues
- Improving image download speeds

Docxy is such an image proxy service, aiming to bypass network blockades and accelerate image downloads by self-hosting a proxy.

### Usage Limits of an Image Proxy

Docker Hub imposes strict rate-limiting policies on image pulls. When using a proxy service, the following limits apply:

- For unauthenticated users, a maximum of 10 image pulls per hour per IP address is allowed.
- For users logged in with a personal account, 100 image pulls per hour are allowed.
- For limits on other account types, please refer to the table below:

| User Type                    | Pull Rate Limit        |
| ---------------------------- | ---------------------- |
| Business (authenticated)     | Unlimited              |
| Team (authenticated)         | Unlimited              |
| Pro (authenticated)          | Unlimited              |
| **Personal (authenticated)** | **100/hour/account**   |
| **Unauthenticated users**    | **10/hour/IP**         |

## Technical Principles

Docxy implements a complete proxy for the Docker Registry API, requiring only the addition of a proxy configuration in the Docker client to be used.

### System Architecture

```mermaid
graph TD
    Client[Docker Client] -->|Sends Request| HttpServer[HTTP Server]
    
    subgraph "Docker Image Proxy Service"
        HttpServer -->|Routes Request| RouterHandler[Route Handler]
        
        RouterHandler -->|/v2/| ChallengeHandler[Challenge Handler<br>proxy_challenge]
        RouterHandler -->|/auth/token| TokenHandler[Token Handler<br>get_token]
        RouterHandler -->|/v2/namespace/image/path_type| RequestHandler[Request Handler<br>handle_request]
        RouterHandler -->|/health| HealthCheck[Health Check<br>health_check]
        
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
    autonumber
    actor Client
    participant Proxy as Docxy Proxy
    participant Registry as Docker Registry
    participant Auth as Docker Auth Service
    
    %% Challenge Request Handling
    Client->>Proxy: GET /v2/
    Proxy->>+Registry: GET /v2/
    Registry-->>-Proxy: 401 Unauthorized (WWW-Authenticate)
    Proxy->>Proxy: Modify WWW-Authenticate header, point realm to local /auth/token
    Proxy-->>Client: 401 Return modified auth header
    
    %% Token Acquisition
    Client->>Proxy: GET /auth/token?scope=repository:library/cirros:pull
    Proxy->>+Auth: GET /token?service=registry.docker.io&scope=repository:library/cirros:pull
    Auth-->>-Proxy: 200 Return token
    Proxy-->>Client: 200 Return original token response
    
    %% Image Manifest Request (for digest)
    Client->>Proxy: HEAD /v2/library/cirros/manifests/latest
    Proxy->>+Registry: Forward request (with auth and Accept headers)
    Registry-->>-Proxy: Return image digest
    Proxy-->>Client: Return image digest (preserving original headers and status)

    %% Image Manifest Request (for metadata)
    Client->>Proxy: GET /v2/library/cirros/manifests/{docker-content-digest}
    Proxy->>+Registry: Forward request (with auth and Accept headers)
    Registry-->>-Proxy: Return image metadata
    Proxy-->>Client: Return image metadata (preserving original headers and status)

    %% Image Layers and Config Request
    Client->>Proxy: GET /v2/library/cirros/manifests/{digest}
    Proxy->>+Registry: Forward request (with auth and Accept headers)
    Registry-->>-Proxy: Return image config and layer info for the specified architecture
    Proxy-->>Client: Return image config and layer info (preserving original headers and status)

    %% Image Config Details Request
    Client->>Proxy: GET /v2/library/cirros/blobs/{digest}
    Proxy->>+Registry: Forward request (with auth and Accept headers)
    Registry-->>-Proxy: Return image config details
    Proxy-->>Client: Return image config details (preserving original headers and status)
    
    %% Image Layer Blob Request (loop for each layer)
    loop For each layer
        Client->>Proxy: GET /v2/library/cirros/blobs/{digest}
        Proxy->>+Registry: Forward blob request
        Registry-->>-Proxy: Return blob data
        Proxy-->>Client: Stream blob data back
    end
```

## Other Solutions

- [Cloudflare Worker for Image Proxy](https://voxsay.com/posts/china-docker-registry-proxy-guide/): Use with caution, as it may lead to your Cloudflare account being banned.
- [Nginx for Image Proxy](https://voxsay.com/posts/china-docker-registry-proxy-guide/): This only proxies `registry-1.docker.io`. Requests to `auth.docker.io` are still made directly, so if `auth.docker.io` is also blocked, this solution will fail.
