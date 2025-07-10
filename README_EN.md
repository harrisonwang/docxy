# Docxy

![og-image](og-image.png)

[![English](https://img.shields.io/badge/English-Click-orange)](README_EN.md)
[![简体中文](https://img.shields.io/badge/简体中文-点击查看-blue)](README.md)
[![Русский](https://img.shields.io/badge/Русский-Нажмите-orange)](README_RU.md)
[![Español](https://img.shields.io/badge/Español-Clic-blue)](README_ES.md)
[![한국어](https://img.shields.io/badge/한국어-클릭-orange)](README_KR.md)
[![العربية](https://img.shields.io/badge/العربية-انقر-blue)](README_AR.md)
[![Türkçe](https://img.shields.io/badge/Türkçe-Tıkla-orange)](README_TR.md)

A lightweight Docker image proxy service, designed to solve the problem of restricted access to Docker Hub in mainland China.

> 📢 **Blog Tutorial:** [**Say Goodbye to Docker Hub Connection Timeouts! Build Your Exclusive Image Accelerator with Docxy**](https://voxsay.com/posts/docxy-docker-proxy-tutorial-for-china/)

## Core Features

*   🚀 **One-Click Deployment**: Provides an `install.sh` automation script for one-click environment setup, certificate application (Let's Encrypt), and service deployment, requiring no manual intervention.

*   📦 **Multiple Deployment Modes**:
    *   **Standalone**: Built-in TLS functionality, directly provides HTTPS service.
    *   **Nginx Proxy**: Can work with Nginx as a backend service.
    *   **CDN Origin**: Supports HTTP mode, convenient for CDN integration.

*   ⚡ **Login for Increased Pull Rate**: Allows users to authenticate with their personal Docker Hub accounts via `docker login`, increasing the pull rate limit from anonymous users (10 pulls/hour/IP) to authenticated users (100 pulls/hour/account).

*   💎 **Completely Transparent Proxy**: Fully compatible with Docker Registry V2 API. Clients only need to modify the mirror source address, with no additional learning curve or changes in usage habits.

*   🛡️ **High Performance & Security**: Built with **Rust** and **Actix Web**, offering excellent performance and memory safety. Uses streaming for image transfer, with minimal overhead.

## Installation and Deployment

We provide a one-click installation script to simplify the deployment process. Before starting, please ensure your domain name is resolved to the target host.

```bash
bash <(curl -Ls https://raw.githubusercontent.com/harrisonwang/docxy/main/install.sh)
```

The script will guide you through the installation and offers the following three deployment modes:

---

### Mode One: Standalone (HTTPS)

This is the simplest and most recommended mode. Docxy will directly listen on ports 80 and 443, providing a complete HTTPS proxy service.

**Features:**
- No need for additional web server configuration.
- Automatically handles HTTP to HTTPS redirection.
- Option to automatically apply for Let's Encrypt certificates or use your own certificates.

**Installation Process:**
1.  Run the one-click installation script.
2.  When prompted for mode selection, enter `1` or simply press Enter.
3.  Follow the prompts to enter your domain name and choose the certificate handling method.
4.  The script will automatically complete all configurations and start the service.

---

<details>
<summary>Mode Two: Nginx Reverse Proxy (Advanced)</summary>

### Mode Two: Nginx Reverse Proxy

This mode is suitable if you already have Nginx and wish to manage web services centrally through it.

**Features:**
- Nginx handles HTTPS encryption and certificate management, with Docxy running as a plain HTTP backend.
- Docxy runs as a backend HTTP service on a specified port (e.g., 9000).
- Convenient for integration with other services.

**Installation Process:**
1.  Run the one-click installation script.
2.  When prompted for mode selection, enter `2`.
3.  Follow the prompts to enter your domain name, Docxy backend listening port, and certificate information.
4.  The script will automatically generate an example Nginx configuration file for you. You will need to manually add it to your Nginx configuration and reload the Nginx service.

</details>

---

<details>
<summary>Mode Three: CDN Origin (HTTP) (Advanced)</summary>

### Mode Three: CDN Origin (HTTP)

This mode is suitable if you want to use Docxy as the origin for a CDN to achieve better global acceleration.

**Features:**
- Docxy only listens on HTTP ports.
- The CDN provider handles HTTPS requests and certificates.
- Docxy trusts and processes `X-Forwarded-*` headers to correctly identify client IP and protocol.

**Installation Process:**
1.  Run the one-click installation script.
2.  When prompted for mode selection, enter `3`.
3.  Follow the prompts to enter the HTTP port Docxy should listen on.
4.  Configure your CDN service to point its origin to the Docxy service address and port.

</details>


## Docker Client Usage

Configure your Docker client to use your proxy service.

### Method One: Anonymous Usage (Basic Configuration)

This is the basic configuration, pointing Docker's default requests to your proxy service.

1.  **Configure Docker Daemon**

    Edit the `/etc/docker/daemon.json` file (create if it doesn't exist) and add the following content. Replace `your-domain.com` with your domain name.

    ```json
    {
      "registry-mirrors": ["https://your-domain.com"]
    }
    ```

2.  **Restart Docker Service**

    ```bash
    sudo systemctl restart docker
    ```
    Now, `docker pull` will pull images through your proxy.

<details>
<summary>Method Two: Login Usage (Increased Pull Rate)</summary>

This method allows you to get a higher image pull rate by logging in with your Docker Hub account, in addition to anonymous usage.

1.  **Complete Basic Configuration**

    Please ensure you have completed all steps in **Method One**.

2.  **Login to Proxy Service**

    Use the `docker login` command and enter your Docker Hub username and password.

    ```bash
    docker login your-domain.com
    ```

3.  **Synchronize Authentication Information**

    After successful login, you need to manually edit the `~/.docker/config.json` file. Copy the `auth` information generated for `your-domain.com` and paste it for `https://index.docker.io/v1/`.

    Before modification:
    ```json
    {
        "auths": {
            "your-domain.com": {
                "auth": "aBcDeFgHiJkLmNoPqRsTuVwXyZ..."
            }
        }
    }
    ```

    After modification:
    ```json
    {
        "auths": {
            "your-domain.com": {
                "auth": "aBcDeFgHiJkLmNoPqRsTuVwXyZ..."
            },
            "https://index.docker.io/v1/": {
                "auth": "aBcDeFgHiJkLmNoPqRsTuVwXyZ..."
            }
        }
    }
    ```
    After saving the file, your `docker pull` requests will be sent as an authenticated user, thus enjoying higher rate limits.

</details>

## Development

> [!NOTE]
> For detailed technical background, system architecture, and implementation principles, please refer to the [**Technical Architecture & Principles Document**](docs/ARCHITECTURE.md).

1.  **Clone Repository**
    ```bash
    git clone https://github.com/harrisonwang/docxy.git
    cd docxy
    ```

2.  **Modify Configuration File**
    Open `config/default.toml` and modify the `[server]` section to ensure HTTP service is enabled and HTTPS service is disabled. You can set the port to 8080 to avoid using privileged ports in the development environment.

    ```toml
    # config/default.toml

    [server]
    http_port = 8080      # Use non-privileged port
    https_port = 8443
    http_enabled = true   # Enable HTTP
    https_enabled = false # Disable HTTPS
    behind_proxy = true
    ```

3.  **Run Project**
    Now, you can directly run the project with `cargo`.
    ```bash
    cargo run
    ```
    The service will start and listen on `http://0.0.0.0:8080`.

4.  **Build Release Version**
    ```bash
    cargo build --release
    ```

## License

This project is licensed under the MIT License. See [LICENSE](LICENSE) for more information.