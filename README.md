# Docker Registry 代理

这是一个轻量级的 Docker Registry 代理服务，用于加速 Docker 镜像的拉取过程。它通过代理 Docker Hub 的请求，提供本地缓存和重定向功能，从而提高 Docker 镜像的下载速度。

## 功能特点

- 支持 HTTP 和 HTTPS 协议
- 自动将 HTTP 请求重定向到 HTTPS
- 支持 Docker Hub 认证
- 自动为不带 `library/` 前缀的镜像名添加前缀
- 支持自定义 TLS 证书
- 健康检查接口

## 安装与运行

### 前提条件

- Rust 开发环境
- TLS 证书（用于 HTTPS）

### 编译

```bash
cargo build --release
```

### 运行

```bash
# 使用默认证书路径
./target/release/docxy

# 使用自定义证书路径
CERT_PATH=/path/to/your/cert.pem KEY_PATH=/path/to/your/key.pem ./target/release/docxy
```

## 配置选项

### 环境变量

| 环境变量 | 描述 | 默认值 |
|----------|------|--------|
| `CERT_PATH` | TLS 证书文件路径 | `/root/.acme.sh/example.com_ecc/fullchain.cer` |
| `KEY_PATH` | TLS 私钥文件路径 | `/root/.acme.sh/example.com_ecc/example.com.key` |

### 证书支持

支持多种私钥格式：
- ECC 私钥
- RSA 私钥
- PKCS8 格式私钥

## 使用方法

### 配置 Docker 客户端

在 Docker 配置文件中添加代理设置：

```json
{
  "registry-mirrors": ["https://your-proxy-domain.com"]
}
```

对于 Linux 系统，配置文件通常位于 `/etc/docker/daemon.json`。

**注意**: 
- Docker 不允许在 registry-mirrors URL 中包含用户名和密码
- 如果使用 expose_port 命令暴露服务，会添加 HTTP Basic 认证，这会导致 Docker 无法使用
- 建议使用公共云平台部署，或者使用本地 IP 地址直接访问服务

### 健康检查

可以通过访问以下端点检查服务是否正常运行：

```
https://your-proxy-domain.com/health
```

## 无认证版本

为了与 Docker 的 registry-mirrors 配置兼容，我们提供了一个无需认证的版本。此版本修改了服务的行为，使其在收到认证请求时返回空结果而不是要求认证。

### 使用方法

1. 克隆仓库并切换到无认证分支：
   ```bash
   git clone https://github.com/harrisonwang/docxy.git
   cd docxy
   git checkout devin/1741594099-remove-auth-requirement-v2
   ```

2. 构建并运行服务：
   ```bash
   cargo build --release
   PORT=8080 ./target/release/docxy
   ```

   或者使用提供的脚本：
   ```bash
   ./deploy-without-auth.sh
   ```

3. 在 Docker 配置中使用本地 IP 地址：
   ```json
   {
     "registry-mirrors": ["http://your-server-ip:8080"]
   }
   ```

4. 重启 Docker 服务：
   ```bash
   sudo systemctl restart docker
   ```

## 部署到云平台

要获得一个公共可访问的 URL，您可以将服务部署到云平台。

### 部署到 Render.com

1. 创建 render.yaml 配置文件:
   ```yaml
   services:
     - type: web
       name: docxy-registry
       env: docker
       dockerfilePath: ./Dockerfile
       plan: free
       healthCheckPath: /health
       envVars:
         - key: PORT
           value: 8080
   ```

2. 在 Render.com 上创建账户并部署:
   - 创建账户: https://render.com/signup
   - 创建新服务: https://dashboard.render.com/new/web-service
   - 选择 "Build and deploy from a Git repository"
   - 连接 GitHub 仓库
   - 选择 "Docker" 作为环境
   - 部署服务

3. 获取部署 URL:
   - 部署完成后，Render.com 会提供一个公共 URL
   - 例如: https://docxy-registry.onrender.com

4. 在 Docker 配置中使用此 URL:
   ```json
   {
     "registry-mirrors": ["https://docxy-registry.onrender.com"]
   }
   ```

### 部署到 Fly.io

Fly.io 是另一个不需要认证的云平台选项，详细部署步骤请参阅 [DEPLOYMENT.md](./DEPLOYMENT.md) 文件。

## 关于认证问题

### expose_port 命令的认证问题

使用 expose_port 命令暴露服务时，会添加 nginx 认证层，这会导致 Docker 无法使用该服务作为 registry-mirrors。nginx 认证配置类似于：

```nginx
auth_basic "Restricted Area";
auth_basic_user_file /path/to/.htpasswd;
```

这种认证无法通过 expose_port 命令的参数移除。要解决此问题，有两种方法：

1. 使用本地 IP 地址直接访问服务（不经过 nginx）
2. 部署到不添加认证的云平台，如 Fly.io

详细的部署说明请参阅 [DEPLOYMENT.md](./DEPLOYMENT.md) 文件。

更多部署选项和详细说明，请参阅 [DEPLOYMENT.md](./DEPLOYMENT.md) 文件。

## API 端点

| 端点 | 描述 |
|------|------|
| `/health` | 健康检查接口 |
| `/v2/` | Docker Registry API v2 入口 |
| `/auth/token` | 认证令牌获取接口 |
| `/v2/library/{image}/{path_type}/{reference}` | 镜像资源访问接口 |

## 开发

### 依赖

本项目使用 Cargo.toml 中定义的依赖：
- actix-web: Web 框架
- reqwest: HTTP 客户端
- rustls: TLS 实现
- tokio: 异步运行时
- 其他辅助库

### 构建与测试

```bash
# 构建
cargo build

# 测试
cargo test

# 运行（开发模式）
cargo run
```

## 许可证

[MIT License](LICENSE)
