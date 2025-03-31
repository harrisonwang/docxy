#!/bin/bash

# 设置颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
NC='\033[0m' # 无颜色

# 初始化变量
USE_EXISTING_CERT=false
DOCXY_CERT_PATH=""
DOCXY_KEY_PATH=""

# 检查是否以 root 权限运行
if [ "$EUID" -ne 0 ]; then
  echo -e "${RED}请以 root 权限运行此脚本${NC}"
  exit 1
fi

# 检查依赖
check_dependencies() {
  echo -e "${YELLOW}正在检查依赖...${NC}"
  
  # 检查 curl
  if ! command -v curl &> /dev/null; then
    echo -e "${YELLOW}正在安装 curl...${NC}"
    apt-get update && apt-get install -y curl || {
      echo -e "${RED}安装 curl 失败${NC}"
      exit 1
    }
  fi
  
  # 检查 socat (acme.sh 需要)
  if ! command -v socat &> /dev/null; then
    echo -e "${YELLOW}正在安装 socat...${NC}"
    apt-get update && apt-get install -y socat || {
      echo -e "${RED}安装 socat 失败${NC}"
      exit 1
    }
  fi
  
  echo -e "${GREEN}所有依赖已满足${NC}"
}

# 获取域名
get_domain() {
  echo -e "${YELLOW}请输入您的域名 (例如: example.com):${NC}"
  read -r DOMAIN
  
  if [ -z "$DOMAIN" ]; then
    echo -e "${RED}域名不能为空${NC}"
    exit 1
  fi
  
  echo -e "${GREEN}将使用域名: ${DOMAIN}${NC}"
}

# 询问是否使用已有证书
ask_certificate_option() {
  echo -e "${YELLOW}您是否已有 SSL 证书? (y/n):${NC}"
  read -r CERT_OPTION
  
  if [[ "$CERT_OPTION" =~ ^[Yy]$ ]]; then
    USE_EXISTING_CERT=true
    echo -e "${GREEN}将使用您提供的证书${NC}"
  else
    USE_EXISTING_CERT=false
    echo -e "${GREEN}将为您自动申请证书${NC}"
  fi
}

# 获取已有证书路径
get_certificate_paths() {
  echo -e "${YELLOW}请输入证书完整路径 (fullchain.cer 或 .pem):${NC}"
  read -r DOCXY_CERT_PATH
  
  echo -e "${YELLOW}请输入私钥完整路径 (.key):${NC}"
  read -r DOCXY_KEY_PATH
  
  # 验证文件是否存在
  if [ ! -f "$DOCXY_CERT_PATH" ]; then
    echo -e "${RED}证书文件不存在: $DOCXY_CERT_PATH${NC}"
    exit 1
  fi
  
  if [ ! -f "$DOCXY_KEY_PATH" ]; then
    echo -e "${RED}私钥文件不存在: $DOCXY_KEY_PATH${NC}"
    exit 1
  fi
  
  echo -e "${GREEN}将使用以下证书文件:${NC}"
  echo -e "证书: ${YELLOW}$DOCXY_CERT_PATH${NC}"
  echo -e "私钥: ${YELLOW}$DOCXY_KEY_PATH${NC}"
}

# 检查端口可用性
check_ports() {
  echo -e "${YELLOW}检查端口 80 和 443 是否可用...${NC}"
  
  # 检查端口 80
  if netstat -tuln | grep -q ":80 "; then
    echo -e "${RED}端口 80 已被占用，请关闭占用该端口的服务后重试${NC}"
    exit 1
  fi
  
  # 检查端口 443
  if netstat -tuln | grep -q ":443 "; then
    echo -e "${RED}端口 443 已被占用，请关闭占用该端口的服务后重试${NC}"
    exit 1
  fi
  
  echo -e "${GREEN}端口 80 和 443 可用${NC}"
}

# 安装 acme.sh
install_acme() {
  echo -e "${YELLOW}正在安装 acme.sh...${NC}"
  
  if [ -f ~/.acme.sh/acme.sh ]; then
    echo -e "${GREEN}acme.sh 已安装，跳过安装步骤${NC}"
  else
    curl https://get.acme.sh | sh || {
      echo -e "${RED}安装 acme.sh 失败${NC}"
      exit 1
    }
    echo -e "${GREEN}acme.sh 安装成功${NC}"
  fi
  
  # 设置 acme.sh 别名
  source ~/.bashrc
  alias acme.sh=~/.acme.sh/acme.sh
}

# 申请证书
get_certificate() {
  echo -e "${YELLOW}正在为 ${DOMAIN} 申请 SSL 证书...${NC}"
  
  # 停止可能占用 80 端口的服务
  systemctl stop nginx 2>/dev/null
  systemctl stop apache2 2>/dev/null
  
  # 使用 acme.sh 申请证书
  ~/.acme.sh/acme.sh --issue -d "$DOMAIN" --standalone --force --server letsencrypt || {
    echo -e "${RED}申请证书失败${NC}"
    exit 1
  }
  
  echo -e "${GREEN}证书申请成功${NC}"
  
  # 检查证书文件是否存在
  if [ ! -f ~/.acme.sh/"$DOMAIN"_ecc/fullchain.cer ] || [ ! -f ~/.acme.sh/"$DOMAIN"_ecc/"$DOMAIN".key ]; then
    echo -e "${RED}证书文件不存在，请检查 acme.sh 的输出${NC}"
    exit 1
  fi
  
  # 设置证书路径变量
  DOCXY_CERT_PATH=~/.acme.sh/"$DOMAIN"_ecc/fullchain.cer
  DOCXY_KEY_PATH=~/.acme.sh/"$DOMAIN"_ecc/"$DOMAIN".key
  
  echo -e "${GREEN}证书文件已生成:${NC}"
  echo -e "证书: ${YELLOW}$DOCXY_CERT_PATH${NC}"
  echo -e "私钥: ${YELLOW}$DOCXY_KEY_PATH${NC}"
}

# 下载 docxy
download_docxy() {
  echo -e "${YELLOW}正在下载 docxy...${NC}"
  
  # 创建目录
  mkdir -p /usr/local/bin
  
  # 检测系统架构
  ARCH=$(uname -m)
  if [ "$ARCH" = "x86_64" ]; then
    BINARY="docxy-linux-amd64"
  elif [ "$ARCH" = "aarch64" ] || [ "$ARCH" = "arm64" ]; then
    BINARY="docxy-linux-arm"
  else
    echo -e "${RED}不支持的系统架构: $ARCH${NC}"
    exit 1
  fi
  
  # 获取最新版本号
  echo -e "${YELLOW}正在获取最新版本...${NC}"
  LATEST_VERSION=$(curl -s https://api.github.com/repos/harrisonwang/docxy/releases/latest | grep -oP '"tag_name": "\K(.*)(?=")' || echo "v0.2.0")
  if [ -z "$LATEST_VERSION" ]; then
    LATEST_VERSION="v0.2.0"
    echo -e "${YELLOW}无法获取最新版本，使用默认版本: $LATEST_VERSION${NC}"
  else
    echo -e "${GREEN}找到最新版本: $LATEST_VERSION${NC}"
  fi
  
  # 下载二进制文件
  curl -L "https://github.com/harrisonwang/docxy/releases/download/$LATEST_VERSION/$BINARY" -o /usr/local/bin/docxy || {
    echo -e "${RED}下载 docxy 失败${NC}"
    exit 1
  }
  
  # 设置执行权限
  chmod +x /usr/local/bin/docxy
  
  echo -e "${GREEN}docxy 下载成功到 /usr/local/bin/docxy${NC}"
}

# 创建 systemd 服务
create_service() {
  echo -e "${YELLOW}正在创建 systemd 服务...${NC}"
  
  cat > /etc/systemd/system/docxy.service << EOF
[Unit]
Description=Docker Registry Proxy
After=network.target

[Service]
Type=simple
User=root
Environment="DOCXY_CERT_PATH=$DOCXY_CERT_PATH"
Environment="DOCXY_KEY_PATH=$DOCXY_KEY_PATH"
ExecStart=/usr/local/bin/docxy
Restart=on-failure
RestartSec=5s

[Install]
WantedBy=multi-user.target
EOF

  # 重新加载 systemd
  systemctl daemon-reload
  
  echo -e "${GREEN}systemd 服务创建成功${NC}"
}

# 启动服务
start_service() {
  echo -e "${YELLOW}正在启动 docxy 服务...${NC}"
  
  systemctl enable docxy
  systemctl start docxy
  
  # 检查服务状态
  if systemctl is-active --quiet docxy; then
    echo -e "${GREEN}docxy 服务已成功启动${NC}"
  else
    echo -e "${RED}docxy 服务启动失败，请检查日志: journalctl -u docxy${NC}"
    exit 1
  fi
}

# 显示使用说明
show_instructions() {
  echo -e "\n${GREEN}=== Docker Registry 代理安装完成 ===${NC}"
  echo -e "\n${YELLOW}使用说明:${NC}"
  echo -e "1. 在 Docker 客户端配置文件中添加以下内容:"
  echo -e "   ${GREEN}编辑 /etc/docker/daemon.json:${NC}"
  echo -e "   ${YELLOW}{\"registry-mirrors\": [\"https://${DOMAIN}\"]}\n${NC}"
  echo -e "2. 重启 Docker 服务:"
  echo -e "   ${YELLOW}systemctl restart docker${NC}\n"
  echo -e "3. 服务管理命令:"
  echo -e "   ${YELLOW}启动: systemctl start docxy${NC}"
  echo -e "   ${YELLOW}停止: systemctl stop docxy${NC}"
  echo -e "   ${YELLOW}重启: systemctl restart docxy${NC}"
  echo -e "   ${YELLOW}查看状态: systemctl status docxy${NC}"
  echo -e "   ${YELLOW}查看日志: journalctl -u docxy${NC}\n"
  echo -e "4. 健康检查:"
  echo -e "   ${YELLOW}curl -k https://${DOMAIN}/health${NC}\n"
}

# 主函数
main() {
  echo -e "${GREEN}=== Docker Registry 代理安装脚本 ===${NC}\n"
  
  get_domain
  ask_certificate_option
  
  if [ "$USE_EXISTING_CERT" = true ]; then
    # 使用已有证书的流程
    get_certificate_paths
    # 只检查端口443
    if netstat -tuln | grep -q ":443 "; then
      echo -e "${RED}端口 443 已被占用，请关闭占用该端口的服务后重试${NC}"
      exit 1
    fi
    echo -e "${GREEN}端口 443 可用${NC}"
  else
    # 申请新证书的流程
    check_dependencies
    check_ports
    install_acme
    get_certificate
  fi
  
  download_docxy
  create_service
  start_service
  show_instructions
}

# 执行主函数
main
