# Docker Registry 代理服务 (无认证版本)

这是一个修改版的 Docker Registry 代理服务，它不需要认证即可使用。这个版本特别适合用作 Docker 的 registry-mirrors 配置。

## 特点

- 不需要认证即可访问
- 兼容 Docker 的 registry-mirrors 配置
- 自动处理 Docker Hub 的认证
- 提供健康检查端点

## 使用方法

### 构建和运行

```bash
# 构建服务
cargo build --release

# 运行服务
PORT=8080 ./target/release/docxy
```

或者使用提供的脚本:

```bash
./deploy-without-auth.sh
```

### 配置 Docker 客户端

在 Docker 配置文件中添加代理设置：

```json
{
  "registry-mirrors": ["http://your-server-ip:8080"]
}
```

对于 Linux 系统，配置文件通常位于 `/etc/docker/daemon.json`。

**注意**: 
- Docker 不允许在 registry-mirrors URL 中包含用户名和密码
- 此版本已修改为不需要认证即可访问

### 验证配置

重启 Docker 服务后，可以通过拉取镜像来验证配置是否生效：

```bash
sudo systemctl restart docker
docker pull hello-world
```

如果配置正确，Docker 将通过您的代理服务拉取镜像。

## 健康检查

可以通过访问以下端点检查服务是否正常运行：

```
http://your-server-ip:8080/health
```

## 暴露服务

如果需要将服务暴露到公网，可以使用 ngrok 或其他反向代理工具：

```bash
ngrok http 8080
```

然后使用 ngrok 提供的 URL 作为 registry-mirrors 配置。
