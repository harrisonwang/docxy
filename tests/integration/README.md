# Integration Tests

这套测试用例用于验证 Docker 代理服务是否正常工作。

## 目录结构

```
tests/integration/
├── bin/
│   └── run-tests.sh          # 主入口脚本
├── configs/
│   └── registries.env        # registry 地址配置
├── suites/
│   ├── docker-hub.sh         # Docker Hub 测试集
│   ├── ghcr.sh               # GHCR 测试集
│   ├── quay.sh               # Quay.io 测试集
│   └── digest.sh             # Digest 测试集
└── README.md
```

## 快速开始

### 1. 配置代理地址

编辑 `configs/registries.env` 文件，设置你的代理地址：

```bash
DOCKERHUB=docker.example.com
GHCR=ghcr.example.com
QUAY=quay.example.com
```

### 2. 运行所有测试

```bash
cd tests/integration
chmod +x bin/run-tests.sh suites/*.sh
./bin/run-tests.sh
```

### 3. 运行特定测试套件

```bash
# 只测试 Docker Hub
./bin/run-tests.sh --suite docker-hub

# 只测试 GHCR
./bin/run-tests.sh --suite ghcr

# 只测试 Quay.io
./bin/run-tests.sh --suite quay

# 只测试 digest 拉取
./bin/run-tests.sh --suite digest
```

## 测试覆盖

| 测试套件 | 覆盖内容 |
|---------|---------|
| `docker-hub.sh` | Docker Hub 官方镜像、短名、library/ 命名空间、非官方 namespace、多平台 |
| `ghcr.sh` | GitHub Container Registry 镜像、多平台支持 |
| `quay.sh` | Quay.io 镜像、多平台支持 |
| `digest.sh` | 基于 SHA256 digest 的内容寻址拉取，覆盖所有 registry |

## 添加新的测试用例

1. 在 `suites/` 目录下创建新的测试脚本
2. 遵循现有的命名规范（如 `new-registry.sh`）
3. 确保脚本开头正确加载配置

```bash
#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONFIG_DIR="$(dirname "$SCRIPT_DIR")/configs"

source "$CONFIG_DIR/registries.env"
```
