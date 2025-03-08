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
./target/release/docker-registry-proxy

# 使用自定义证书路径
CERT_PATH=/path/to/your/cert.pem KEY_PATH=/path/to/your/key.pem ./target/release/docker-registry-proxy
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

### 健康检查

可以通过访问以下端点检查服务是否正常运行：

```
https://your-proxy-domain.com/health
```

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
