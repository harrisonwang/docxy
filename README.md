# Docker Registry 代理

这是一个轻量级的 Docker Registry 代理服务，它通过代理 Docker Hub 的请求，从而解决国内无法下载 Docker 镜像的问题。

## 一键安装

TODO

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

```bash
curl https://your-proxy-domain.com/health
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
