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
HTTP_PORT=80
HTTPS_PORT=443
DEPLOYMENT_MODE=""

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

# 添加部署模式询问函数
ask_deployment_mode() {
  echo -e "${YELLOW}请选择您的部署方式:${NC}"
  echo "1. [默认] 独立运行模式 (HTTPS，自动申请或提供证书)"
  echo "2. Nginx 反向代理模式 (您需要手动配置Nginx)"
  echo "3. 独立运行模式 (仅 HTTP，用于CDN回源)"
  echo -ne "${YELLOW}请输入选项 [1-3] (默认: 1): ${NC}"
  read -r DEPLOYMENT_CHOICE
  DEPLOYMENT_CHOICE=${DEPLOYMENT_CHOICE:-1}

  case "$DEPLOYMENT_CHOICE" in
    1)
      DEPLOYMENT_MODE="standalone_https"
      ;;
    2)
      DEPLOYMENT_MODE="nginx"
      ;;
    3)
      DEPLOYMENT_MODE="standalone_http"
      ;;
    *)
      echo -e "${RED}无效的选项，脚本退出。${NC}"
      exit 1
      ;;
  esac
}

# 设置模式参数
set_mode_parameters() {
  case "$DEPLOYMENT_MODE" in
    "standalone_https")
      BEHIND_PROXY=false
      HTTPS_ENABLED=true
      HTTP_ENABLED=true # HTTP用于重定向
      HTTP_PORT=80
      HTTPS_PORT=443
      echo -e "${GREEN}将直接提供服务，使用标准端口 80/443${NC}"
      ask_certificate_option
      ;;
    "nginx")
      BEHIND_PROXY=true
      HTTPS_ENABLED=false # docxy本身不处理https
      HTTP_ENABLED=true
      echo -e "${YELLOW}请输入 docxy 后端监听的HTTP端口 (默认: 9000):${NC}"
      read -r HTTP_PORT_INPUT
      HTTP_PORT=${HTTP_PORT_INPUT:-9000}
      echo -e "${GREEN}docxy 将在代理模式下运行，监听端口: ${HTTP_PORT}${NC}"
      ask_certificate_option
      ;;
    "standalone_http")
      BEHIND_PROXY=true # 设为true以处理CDN/代理的X-Forwarded-*头
      HTTPS_ENABLED=false
      HTTP_ENABLED=true
      echo -e "${YELLOW}请输入要监听的HTTP端口 (默认: 80):${NC}"
      read -r HTTP_PORT_INPUT
      HTTP_PORT=${HTTP_PORT_INPUT:-80}
      HTTPS_PORT=443 # 保持一个值，即使禁用
      echo -e "${GREEN}将在独立模式下运行，HTTP端口: ${HTTP_PORT}${NC}"
      ;;
  esac
}

# 修改端口检查函数
check_ports() {
  echo -e "${YELLOW}检查端口 ${HTTP_PORT} 和 ${HTTPS_PORT} 是否可用...${NC}"
  
  # 检查HTTP端口
  if netstat -tuln | grep -q ":${HTTP_PORT} "; then
    echo -e "${RED}端口 ${HTTP_PORT} 已被占用，请关闭占用该端口的服务后重试${NC}"
    exit 1
  fi
  
  # 检查HTTPS端口
  if [ "$DEPLOYMENT_MODE" = "standalone_https" ] && netstat -tuln | grep -q ":${HTTPS_PORT} "; then
    echo -e "${RED}端口 ${HTTPS_PORT} 已被占用，请关闭占用该端口的服务后重试${NC}"
    exit 1
  fi
  
  echo -e "${GREEN}端口 ${HTTP_PORT} 和 ${HTTPS_PORT} 可用${NC}"
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

# 复制默认配置文件
copy_default_config() {
  echo -e "${YELLOW}正在复制和配置默认配置文件...${NC}"
  mkdir -p /etc/docxy/config/
  curl -Ls https://raw.githubusercontent.com/harrisonwang/docxy/main/config/default.toml -o /etc/docxy/config/default.toml || {
    echo -e "${RED}下载默认配置文件失败${NC}"
    exit 1
  }

  # 修改 default.toml 中的配置
  sed -i "s/^http_port = .*/http_port = ${HTTP_PORT}/" /etc/docxy/config/default.toml
  sed -i "s/^https_port = .*/https_port = ${HTTPS_PORT}/" /etc/docxy/config/default.toml
  sed -i "s/^http_enabled = .*/http_enabled = ${HTTP_ENABLED}/" /etc/docxy/config/default.toml
  sed -i "s/^https_enabled = .*/https_enabled = ${HTTPS_ENABLED}/" /etc/docxy/config/default.toml
  sed -i "s/^behind_proxy = .*/behind_proxy = ${BEHIND_PROXY}/" /etc/docxy/config/default.toml
  sed -i "s#^cert_path = .*#cert_path = \"${DOCXY_CERT_PATH}\"#" /etc/docxy/config/default.toml
  sed -i "s#^key_path = .*#key_path = \"${DOCXY_KEY_PATH}\"#" /etc/docxy/config/default.toml

  echo -e "${GREEN}默认配置文件已复制并配置到 /etc/docxy/config/default.toml${NC}"
}

# 修改systemd服务创建函数
create_service() {
  echo -e "${YELLOW}正在创建 systemd 服务...${NC}"
  
  cat > /etc/systemd/system/docxy.service << EOF
[Unit]
Description=Docker Registry Proxy
After=network.target

[Service]
Type=simple
User=root
Environment="RUST_LOG=info"
ExecStart=/usr/local/bin/docxy
Restart=on-failure
RestartSec=5s
WorkingDirectory=/etc/docxy

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

# 修改显示说明函数
show_instructions() {
  echo -e "\n${GREEN}=== Docker Registry 代理安装完成 ===${NC}"
  
  if [ "$DEPLOYMENT_MODE" = "nginx" ]; then
    echo -e "\n${YELLOW}Nginx 反向代理配置示例 (请将其保存到您的Nginx配置中):${NC}"
    echo -e "server {"
    echo -e "    listen 80;"
    echo -e "    server_name ${DOMAIN};"
    echo -e "    return 301 https://\$host\$request_uri;"
    echo -e "}"
    echo -e ""
    echo -e "server {"
    echo -e "    listen 443 ssl http2;"
    echo -e "    server_name ${DOMAIN};"
    echo -e ""
    echo -e "    # SSL 配置 (请确保路径正确)"
    echo -e "    ssl_certificate ${DOCXY_CERT_PATH};"
    echo -e "    ssl_certificate_key ${DOCXY_KEY_PATH};"
    echo -e ""
    echo -e "    location / {"
    echo -e "        proxy_pass http://127.0.0.1:${HTTP_PORT};"
    echo -e "        proxy_set_header Host \$host;"
    echo -e "        proxy_set_header X-Real-IP \$remote_addr;"
    echo -e "        proxy_set_header X-Forwarded-For \$proxy_add_x_forwarded_for;"
    echo -e "        proxy_set_header X-Forwarded-Proto \$scheme;"
    echo -e "    }"
    echo -e "}"
    echo -e "\n${YELLOW}配置好 Nginx 后，请运行: nginx -t && systemctl reload nginx${NC}"
  fi
  
  echo -e "\n${YELLOW}使用说明:${NC}"
  echo -e "1. 在 Docker 客户端配置文件中添加以下内容:"
  echo -e "   ${GREEN}编辑 /etc/docker/daemon.json:${NC}"
  
  if [ "$DEPLOYMENT_MODE" = "standalone_http" ]; then
    # 独立HTTP模式
    echo -e "   ${YELLOW}{\"registry-mirrors\": [\"http://${DOMAIN}:${HTTP_PORT}\"], \"insecure-registries\": [\"${DOMAIN}:${HTTP_PORT}\"]}${NC}"
  else
    # standalone_https 和 nginx 模式都通过https访问
    echo -e "   ${YELLOW}{\"registry-mirrors\": [\"https://${DOMAIN}\"]}${NC}"
  fi
  
  echo -e "\n2. 重启 Docker 服务:"
  echo -e "   ${YELLOW}systemctl restart docker${NC}\n"
  echo -e "3. 服务管理命令:"
  echo -e "   ${YELLOW}启动: systemctl start docxy${NC}"
  echo -e "   ${YELLOW}停止: systemctl stop docxy${NC}"
  echo -e "   ${YELLOW}重启: systemctl restart docxy${NC}"
  echo -e "   ${YELLOW}查看状态: systemctl status docxy${NC}"
  echo -e "   ${YELLOW}查看日志: journalctl -u docxy${NC}\n"
  echo -e "4. 健康检查:"
  if [ "$DEPLOYMENT_MODE" = "standalone_http" ]; then
    echo -e "   ${YELLOW}curl http://${DOMAIN}:${HTTP_PORT}/health${NC}\n"
  else
    echo -e "   ${YELLOW}curl https://${DOMAIN}/health${NC}\n"
  fi
}

# 创建 Nginx 配置文件
create_nginx_config() {
  echo -e "${YELLOW}正在创建 Nginx 配置文件...${NC}"
  
  # 获取Nginx配置目录
  echo -e "${YELLOW}请输入Nginx配置文件目录 (默认: /etc/nginx/conf.d):${NC}"
  read -r NGINX_CONF_INPUT
  NGINX_CONF_DIR=${NGINX_CONF_INPUT:-/etc/nginx/conf.d}
  
  # 确认目录存在
  if [ ! -d "$NGINX_CONF_DIR" ]; then
    echo -e "${RED}目录 ${NGINX_CONF_DIR} 不存在${NC}"
    echo -e "${YELLOW}是否创建该目录? (y/n):${NC}"
    read -r CREATE_DIR
    if [[ "$CREATE_DIR" =~ ^[Yy]$ ]]; then
      mkdir -p "$NGINX_CONF_DIR"
    else
      echo -e "${RED}无法创建Nginx配置${NC}"
      return
    fi
  fi
  
  local NGINX_CONF_FILE="$NGINX_CONF_DIR/${DOMAIN}.conf"
  
  cat > "$NGINX_CONF_FILE" << EOF
# Docker Registry Proxy 配置
# 为域名 $DOMAIN 生成的配置文件

server {
    listen 80;
    listen [::]:80;
    server_name $DOMAIN;
    
    # 将 HTTP 请求重定向到 HTTPS
    location / {
        return 301 https://\$host\$request_uri;
    }
}

server {
    listen 443 ssl http2;
    listen [::]:443 ssl http2;
    server_name $DOMAIN;
    
    # SSL 配置
    ssl_certificate $DOCXY_CERT_PATH;
    ssl_certificate_key $DOCXY_KEY_PATH;
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_prefer_server_ciphers on;
    ssl_ciphers 'ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-RSA-AES128-GCM-SHA256:ECDHE-ECDSA-AES256-GCM-SHA384:ECDHE-RSA-AES256-GCM-SHA384';
    
    # 代理设置
    location / {
        proxy_pass http://127.0.0.1:$HTTP_PORT;
        proxy_set_header Host \$host;
        proxy_set_header X-Real-IP \$remote_addr;
        proxy_set_header X-Forwarded-For \$proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto \$scheme;
        
        proxy_read_timeout 300;
        proxy_connect_timeout 60;
        proxy_send_timeout 60;
    }
}
EOF

  echo -e "${GREEN}Nginx 配置文件已创建: $NGINX_CONF_FILE${NC}"
}

# 主函数
main() {
  echo -e "${GREEN}=== Docker Registry 代理安装脚本 ===${NC}\n"
  
  get_domain
  ask_deployment_mode
  set_mode_parameters

  # 检查依赖 (仅在需要申请证书时)
  if ( [ "$DEPLOYMENT_MODE" = "standalone_https" ] || [ "$DEPLOYMENT_MODE" = "nginx" ] ) && [ "$USE_EXISTING_CERT" = false ]; then
    check_dependencies
    install_acme
  fi

  # 获取证书
  if [ "$DEPLOYMENT_MODE" = "standalone_https" ] || [ "$DEPLOYMENT_MODE" = "nginx" ]; then
    if [ "$USE_EXISTING_CERT" = true ]; then
      get_certificate_paths
    else
      get_certificate
    fi
  fi

  # 端口检查
  check_ports

  # 下载、配置和启动服务
  download_docxy
  create_service
  copy_default_config
  start_service

  # 如果是Nginx模式，创建配置文件
  if [ "$DEPLOYMENT_MODE" = "nginx" ]; then
    create_nginx_config
  fi

  show_instructions
}

# 执行主函数
main