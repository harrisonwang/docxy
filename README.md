# Docxy

![og-image](og-image.png)

[![English](https://img.shields.io/badge/English-Click-orange)](README_EN.md)
[![简体中文](https://img.shields.io/badge/简体中文-点击查看-blue)](README.md)
[![Русский](https://img.shields.io/badge/Русский-Нажмите-orange)](README_RU.md)
[![Español](https://img.shields.io/badge/Español-Clic-blue)](README_ES.md)
[![한국어](https://img.shields.io/badge/한국어-클릭-orange)](README_KR.md)
[![العربية](https://img.shields.io/badge/العربية-انقر-blue)](README_AR.md)
[![Türkçe](https://img.shields.io/badge/Türkçe-Tıkla-orange)](README_TR.md)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-blue.svg)](https://www.rust-lang.org)
[![Docker](https://img.shields.io/badge/docker-28%2B-orange.svg)](https://www.docker.com)

轻量级 Docker 镜像代理服务，旨在解决国内访问 Docker Hub 受限问题。

> 📢 **使用教程:** [**跟 Docker Hub 连接超时说拜拜！用 Docxy 自建专属镜像加速器**](https://voxsay.com/posts/docxy-docker-proxy-tutorial-for-china/)

## 核心特性

*   🚀 **一键部署**: 提供 `install.sh` 自动化脚本，可一键完成环境配置、证书申请 (Let's Encrypt)、服务部署，无需手动干预。

*   📦 **多种部署模式**:
    *   **独立运行**: 内置 TLS 功能，直接对外提供 HTTPS 服务。
    *   **Nginx 代理**: 可配合 Nginx 作为后端服务运行。
    *   **CDN 回源**: 支持 HTTP 模式，方便接入 CDN。

*   ⚡ **支持登录提升速率**: 允许用户通过 `docker login` 使用个人账户认证，将匿名用户的拉取速率限制（10次/小时/IP）提升至认证用户的（100次/小时/账户）。

*   💎 **完全透明的代理**: 完美兼容 Docker Registry V2 API，客户端仅需修改镜像源地址，无额外学习成本和使用习惯的改变。

*   🛡️ **高性能与安全**: 基于 **Rust** 和 **Actix Web** 构建，性能卓越、内存安全。采用流式传输处理镜像，开销极小。

## 安装与部署

我们提供了一键安装脚本来简化部署流程，在开始前，请提前将您的域名解析到目标主机。

```bash
bash <(curl -Ls https://raw.githubusercontent.com/harrisonwang/docxy/main/install.sh)
```

脚本将引导您完成安装，并提供以下三种部署模式：

---

### 模式一：独立运行 (HTTPS)

这是最简单、最推荐的模式。Docxy 将直接监听 80 和 443 端口，对外提供完整的 HTTPS 代理服务。

**特点:**
- 无需额外配置 Web 服务器。
- 自动处理 HTTP 到 HTTPS 的重定向。
- 可选择自动申请 Let's Encrypt 证书或使用您自己的证书。

**安装流程:**
1.  运行一键安装脚本。
2.  在模式选择时，输入 `1` 或直接回车。
3.  根据提示输入您的域名，并选择证书处理方式。
4.  脚本将自动完成所有配置并启动服务。

---

<details>
<summary>模式二：Nginx 反向代理 (高级)</summary>

### 模式二：Nginx 反向代理

此模式适用于您已经拥有并希望通过 Nginx 统一管理 Web 服务的场景。

**特点:**
- 由 Nginx 统一处理 HTTPS 加密和证书管理，Docxy 在后端以普通 HTTP 模式运行。
- Docxy 作为后端 HTTP 服务运行在一个指定端口上 (如: 9000)。
- 方便与其他服务集成。

**安装流程:**
1.  运行一键安装脚本。
2.  在模式选择时，输入 `2`。
3.  根据提示输入您的域名、Docxy 后端监听端口以及证书信息。
4.  脚本会自动为您生成一份 Nginx 配置文件示例，您需要手动将其添加到您的 Nginx 配置中，并重载 Nginx 服务。

</details>

---

<details>
<summary>模式三：CDN 回源 (HTTP) (高级)</summary>

### 模式三：CDN 回源 (HTTP)

此模式适用于您希望将 Docxy 作为 CDN 的源站，以获得更好的全球加速效果。

**特点:**
- Docxy 仅监听 HTTP 端口。
- 由 CDN 服务商负责处理 HTTPS 请求和证书。
- Docxy 会信任并处理 `X-Forwarded-*` 头，以正确识别客户端 IP 和协议。

**安装流程:**
1.  运行一键安装脚本。
2.  在模式选择时，输入 `3`。
3.  根据提示输入 Docxy 需要监听的 HTTP 端口。
4.  配置您的 CDN 服务，将源站指向 Docxy 服务的地址和端口。

</details>


## Docker 客户端使用

配置 Docker 客户端以使用您的代理服务。

### 方式一：匿名使用 (基础配置)

这是最基础的配置，将 Docker 的默认请求指向您的代理服务。

1.  **配置 Docker Daemon**

    编辑 `/etc/docker/daemon.json` 文件 (如果不存在则创建)，并添加以下内容。将 `your-domain.com` 替换为您的域名。

    ```json
    {
      "registry-mirrors": ["https://your-domain.com"]
    }
    ```

2.  **重启 Docker 服务**

    ```bash
    sudo systemctl restart docker
    ```
    现在，`docker pull` 将通过您的代理进行拉取。

<details>
<summary>方式二：登录使用 (提升拉取速率)</summary>

此方式可以在匿名使用的基础上，通过登录您的 Docker Hub 账户来获取更高的镜像拉取速率。

1.  **完成基础配置**

    请确保您已经完成了 **方式一** 中的所有步骤。

2.  **登录代理服务**

    使用 `docker login` 命令并输入您的 Docker Hub 用户名和密码。

    ```bash
    docker login your-domain.com
    ```

3.  **同步认证信息**

    登录成功后，需要手动编辑 `~/.docker/config.json` 文件，将您刚刚为 `your-domain.com` 生成的 `auth` 信息，复制一份给 `https://index.docker.io/v1/`。

    修改前：
    ```json
    {
        "auths": {
            "your-domain.com": {
                "auth": "aBcDeFgHiJkLmNoPqRsTuVwXyZ..."
            }
        }
    }
    ```

    修改后：
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
    保存文件后，您的 `docker pull` 请求就会以认证用户的方式发送，从而享受更高的速率限制。

</details>

## 开发

> [!NOTE]
> 详细的技术背景、系统架构和实现流程，请参阅 [**技术架构与原理文档**](docs/ARCHITECTURE.md)。

1.  **克隆仓库**
    ```bash
    git clone https://github.com/harrisonwang/docxy.git
    cd docxy
    ```

2.  **修改配置文件**
    打开 `config/default.toml`，修改 `[server]` 部分，确保 HTTP 服务被启用，HTTPS 服务被禁用。您可以将端口设置为 8080，以避免在开发环境中使用特权端口。

    ```toml
    # config/default.toml

    [server]
    http_port = 8080      # 使用非特权端口
    https_port = 8443
    http_enabled = true   # 启用 HTTP
    https_enabled = false # 禁用 HTTPS
    behind_proxy = true
    ```

3.  **运行项目**
    现在，可以直接用 `cargo` 运行项目。
    ```bash
    cargo run
    ```
    服务将启动并监听在 `http://0.0.0.0:8080`。

4.  **构建发布版本**
    ```bash
    cargo build --release
    ```

## 许可证

本项目采用 MIT 许可证，查看 [LICENSE](LICENSE) 了解更多信息。