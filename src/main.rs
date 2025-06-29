use actix_web::{web, guard, App, HttpRequest, HttpResponse, HttpServer, Responder, Result};
use std::collections::HashMap;
use rustls::{Certificate, PrivateKey, ServerConfig};
use rustls_pemfile;
use std::fs::File;
use std::io::BufReader;
use futures::stream::StreamExt;
use std::time::Duration;
use lazy_static::lazy_static;
use std::env;
use log::{info, error, debug, warn};

// 将 Docker Registry URL 定义为常量
const DOCKER_REGISTRY_URL: &str = "https://registry-1.docker.io";

lazy_static! {
    static ref HTTP_CLIENT: reqwest::Client = reqwest::Client::builder()
        .pool_max_idle_per_host(10)  // 根据负载调整
        .pool_idle_timeout(Duration::from_secs(90))
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap();
}

// 处理非法请求的函数
async fn handle_invalid_request(req: HttpRequest) -> HttpResponse {
    let path = req.uri().path();
    warn!("拦截非法请求: {} {}", req.method(), path);
    
    HttpResponse::Forbidden()
        .content_type("text/plain; charset=utf-8")
        .body("非法访问路径")
}

async fn handle_request(
    req: HttpRequest,
    path: web::Path<(String, String, String)>,
) -> Result<HttpResponse> {
    // 获取路径参数
    let (image_name, path_type, reference) = path.into_inner();

    // 使用常量构建目标URL
    let path = format!("/v2/{}/{}/{}", image_name, path_type, reference);
    
    // 构建请求，根据原始请求的方法选择 HEAD 或 GET
    let target_url = format!("{}{}", DOCKER_REGISTRY_URL, path);
    let mut request_builder = if req.method() == &actix_web::http::Method::HEAD {
        HTTP_CLIENT.head(&target_url)
    } else {
        HTTP_CLIENT.get(&target_url)
    };

    // 添加认证头
    if let Some(auth) = req.headers().get("Authorization") {
        if let Ok(auth_str) = auth.to_str() {
            request_builder = request_builder.header("Authorization", auth_str);
        }
    }

    // 添加所有 Accept 头
    for accept in req.headers().get_all("Accept") {
        if let Ok(accept_str) = accept.to_str() {
            request_builder = request_builder.header("Accept", accept_str);
        }
    }

    // 发送请求到 Docker Registry
    let method = req.method().as_str();
    let response = match request_builder.send().await {
        Ok(resp) => {
            info!("{} {} {:?} {} {}", 
                method, 
                target_url, 
                req.version(),
                resp.status().as_u16(), 
                resp.status().canonical_reason().unwrap_or("Unknown"));
            resp
        },
        Err(e) => {
            error!("{} {} {:?} 失败: {}", method, target_url, req.version(), e);
            return Ok(HttpResponse::InternalServerError()
                .body(format!("无法连接到 Docker Registry: {}", e)))
        }
    };

    // 获取状态码和响应头
    let status = response.status();
    let mut builder = HttpResponse::build(actix_web::http::StatusCode::from_u16(status.as_u16()).unwrap());

    // 复制所有响应头
    for (name, value) in response.headers() {
        if let Ok(value_str) = value.to_str() {
            builder.append_header((name.as_str(), value_str));
        }
    }

    // 记录响应日志
    info!("{} {} {:?} {} {}", 
        req.method(), 
        req.uri(), 
        req.version(),
        status.as_u16(), 
        status.canonical_reason().unwrap_or("Unknown"));

    // 根据请求方法处理响应
    if req.method() == &actix_web::http::Method::HEAD {
        // HEAD 请求，不需要返回响应体
        Ok(builder.finish())
    } else {
        // GET 请求，使用流式传输响应体
        let stream = response
            .bytes_stream()
            .map(|result| {
                result.map_err(|err| {
                    error!("流读取错误: {}", err);
                    actix_web::error::ErrorInternalServerError(err)
                })
            });
            
        Ok(builder.streaming(stream))
    }
}

// 获取 Token 的处理函数
async fn get_token(req: HttpRequest) -> Result<HttpResponse> {
    // 1. 尝试解析查询参数，失败则返回 400
    let query_params = match web::Query::<HashMap<String, String>>::from_query(req.query_string()) {
        Ok(q) => q,
        Err(_) => {
            return Ok(HttpResponse::BadRequest().body("无效的查询参数"));
        }
    };

    // 2. 构建 Docker Hub 认证服务 URL
    let mut auth_url = reqwest::Url::parse("https://auth.docker.io/token").unwrap();
    {
        let mut query_pairs = auth_url.query_pairs_mut();
        // service 必须是 registry.docker.io
        query_pairs.append_pair("service", "registry.docker.io");

        // 3. 透传所有客户端提供的查询参数（包含 account、client_id、offline_token、scope 等）
        //    避免重复 service
        for (k, v) in query_params.iter() {
            if k != "service" {
                query_pairs.append_pair(k, v);
            }
        }
    }

    info!("转发 token 请求至: {}", auth_url);

    // 构造向上游的请求构建器
    let mut request_builder = HTTP_CLIENT.get(auth_url.clone());

    // 检查并代理 Authorization 头
    if let Some(auth_header) = req.headers().get("Authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            info!("代理 Authorization 头: {}", auth_str);
            request_builder = request_builder.header("Authorization", auth_str);
        }
    }

    // 发送请求到 Docker Hub 认证服务
    let response = match request_builder.send().await {
        Ok(resp) => {
            info!("GET {} {:?} {} {}", 
                auth_url, 
                req.version(), 
                resp.status().as_u16(), 
                resp.status().canonical_reason().unwrap_or("Unknown"));
            resp
        },
        Err(e) => {
            error!("GET {} {:?} 失败: {}", auth_url, req.version(), e);
            return Ok(HttpResponse::InternalServerError()
                .body("无法连接到 Docker Hub 认证服务"))
        }
    };

    // 获取状态码和响应头
    let status = response.status();
    let mut builder = HttpResponse::build(actix_web::http::StatusCode::from_u16(status.as_u16()).unwrap());

    // 复制所有响应头
    for (name, value) in response.headers() {
        if let Ok(value_str) = value.to_str() {
            builder.append_header((name.as_str(), value_str));
        }
    }

    // 获取响应体并返回
    match response.bytes().await {
        Ok(bytes) => {
            info!("{} {} {:?} {} {}", 
                req.method(), 
                req.uri(), 
                req.version(),
                status.as_u16(), 
                status.canonical_reason().unwrap_or("Unknown"));
            Ok(builder.body(bytes))
        },
        Err(e) => {
            error!("读取认证服务响应失败: {}", e);
            Ok(HttpResponse::InternalServerError().body("无法读取认证服务响应"))
        }
    }
}

async fn proxy_challenge(req: HttpRequest) -> Result<HttpResponse> {
    let host = match req.connection_info().host() {
        host if host.contains(':') => host.to_string(),
        host => format!("{}", host)
    };

    let request_url = format!("{}/v2/", DOCKER_REGISTRY_URL);
    
    // 构建请求，检查是否有 Authorization 头
    let mut request_builder = HTTP_CLIENT.get(&request_url);
    
    // 如果客户端提供了 Authorization 头，转发给上游
    if let Some(auth) = req.headers().get("Authorization") {
        if let Ok(auth_str) = auth.to_str() {
            info!("代理 Authorization 头到 /v2/: {}", auth_str);
            request_builder = request_builder.header("Authorization", auth_str);
        }
    }

    let response = match request_builder.send().await {
        Ok(resp) => {
            info!("GET {} {:?} {} {}", 
                request_url,
                req.version(), 
                resp.status().as_u16(), 
                resp.status().canonical_reason().unwrap_or("Unknown"));
            resp
        },
        Err(e) => {
            error!("GET {} {:?} 失败: {}", request_url, req.version(), e);
            return Ok(HttpResponse::InternalServerError()
                      .body("无法连接到上游 Docker Registry"))
        }
    };

    let status = response.status().as_u16();
    let mut builder = HttpResponse::build(actix_web::http::StatusCode::from_u16(status).unwrap());

    // 只有在返回 401 时才设置 WWW-Authenticate 头
    if status == 401 {
        let auth_header = format!(
            "Bearer realm=\"https://{}/auth/token\",service=\"registry.docker.io\"",
            host
        );
        info!("设置认证头: {}", auth_header);
        
        builder.append_header((
            "WWW-Authenticate",
            auth_header
        ));
    }

    let body = match response.text().await {
        Ok(text) => text,
        Err(e) => {
            error!("读取上游响应内容失败: {}", e);
            String::from("无法读取上游响应内容")
        }
    };

    info!("{} {} {:?} {} {}", 
        req.method(), 
        req.uri(), 
        req.version(),
        status, 
        actix_web::http::StatusCode::from_u16(status).unwrap().canonical_reason().unwrap_or("Unknown"));
    
    Ok(builder.body(body))
}

async fn health_check(req: HttpRequest) -> impl Responder {
    info!("{} {} {:?} 200 OK", 
        req.method(), 
        req.uri(), 
        req.version());
        
    HttpResponse::Ok()
        .content_type("text/plain; charset=utf-8")
        .body("服务正常运行\n")
}

// 新增HTTP到HTTPS的重定向处理函数
async fn redirect_to_https(req: HttpRequest) -> HttpResponse {
    let host = req.connection_info().host().split(':').next().unwrap_or("").to_string();
    let uri = req.uri().to_string();
    
    // 构建重定向URL (HTTP -> HTTPS)
    let redirect_url = format!("https://{}{}", host, uri);
    
    info!("接收请求: \"{} {} HTTP/{:?}\" 301 Moved Permanently", 
        req.method(), 
        req.uri(), 
        req.version());
    
    info!("重定向到: {}", redirect_url);
    
    HttpResponse::MovedPermanently()
        .append_header(("Location", redirect_url))
        .finish()
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // 使用env_logger的Builder直接设置日志级别
    env_logger::Builder::from_env(env_logger::Env::default()
        .default_filter_or("actix_web=info"))
        .format(|buf, record| {
            use std::io::Write;
            use chrono::Local;
            
            let level = record.level();
            let mut style_binding = buf.style(); // 先创建绑定
            let level_style = style_binding  // 使用绑定
                .set_bold(true)
                .set_color(match level {
                    log::Level::Error => env_logger::fmt::Color::Red,
                    log::Level::Warn => env_logger::fmt::Color::Yellow,
                    log::Level::Info => env_logger::fmt::Color::Green,
                    log::Level::Debug => env_logger::fmt::Color::Blue,
                    log::Level::Trace => env_logger::fmt::Color::Cyan,
                });
                
            let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
            
            writeln!(
                buf,
                "[{} {} {}] {}",
                timestamp,
                level_style.value(format!("{:5}", level)),
                record.target(),
                record.args()
            )
        })
        .init();
    
    // 检查是否在代理模式下运行
    let behind_proxy = env::var("DOCXY_BEHIND_PROXY")
        .unwrap_or_else(|_| "false".to_string()) == "true";
    
    // 从环境变量获取端口配置
    let http_enabled = env::var("DOCXY_HTTP_ENABLED")
        .unwrap_or_else(|_| "true".to_string()) == "true";
    
    // 在代理模式下默认使用9000端口
    let default_http_port = if behind_proxy { 9000 } else { 80 };
    let http_port = get_env_port("DOCXY_HTTP_PORT", default_http_port);
    
    // 在代理模式下自动禁用HTTPS，否则使用环境变量
    let https_enabled = if behind_proxy {
        false
    } else {
        env::var("DOCXY_HTTPS_ENABLED")
            .unwrap_or_else(|_| "true".to_string()) == "true"
    };
    
    let https_port = get_env_port("DOCXY_HTTPS_PORT", 443);
    
    // 输出配置信息
    info!("服务器配置:");
    info!("HTTP 端口: {}", http_port);
    
    if https_enabled {
        info!("HTTPS 端口: {}", https_port);
    } else {
        info!("HTTPS 服务: 已禁用");
    }
    
    if behind_proxy {
        info!("代理模式: 已启用");
    }
    
    // 创建应用配置
    let app = || {
        App::new()
            .route("/v2/", web::get().to(proxy_challenge))
            .route("/auth/token", web::get().to(get_token))
            .route("/health", web::get().to(health_check))
            .route("/v2/{image_name:.*}/{path_type}/{reference:.+}",
                   web::route()
                   .guard(guard::Any(guard::Get()).or(guard::Head()))
                   .to(handle_request))
            .default_service(web::route().to(handle_invalid_request))  // 添加默认服务处理非法请求
    };
    
    // 创建HTTP重定向应用配置，特殊情况下我们可能仍然希望重定向，而不是拒绝访问
    let http_redirect_app = || {
        App::new()
            .service(
                web::scope("/v2")
                    .route("", web::get().to(redirect_to_https))
                    .route("/{tail:.*}", web::route().to(redirect_to_https))
            )
            .route("/auth/token", web::get().to(redirect_to_https))
            .route("/health", web::get().to(redirect_to_https))
            .default_service(web::route().to(handle_invalid_request))  // 非法路径直接拒绝
    };
    
    // 创建服务器实例
    let mut servers = Vec::new();
    
    // 启动HTTP服务器（如果启用）
    if http_enabled {
        let http_server = if !behind_proxy && https_enabled {
            // 如果启用了HTTPS且不在代理后面，HTTP只做重定向
            HttpServer::new(http_redirect_app)
                .bind(("0.0.0.0", http_port))?
                .run()
        } else {
            // 否则HTTP提供完整功能
            HttpServer::new(app)
                .bind(("0.0.0.0", http_port))?
                .run()
        };
        
        servers.push(http_server);
    }
    
    // 启动HTTPS服务器（如果启用）
    if https_enabled {
        // 加载TLS配置
        match load_rustls_config() {
            Ok(rustls_config) => {
                let https_server = HttpServer::new(app)
                    .bind_rustls(("0.0.0.0", https_port), rustls_config)?
                    .run();
                
                servers.push(https_server);
            },
            Err(e) => {
                error!("无法加载TLS配置: {}", e);
                if !http_enabled {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Other, 
                        "HTTPS配置加载失败且HTTP服务已禁用，无法启动服务器"
                    ));
                }
            }
        }
    }
    
    // 确保至少有一个服务器在运行
    if servers.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other, 
            "HTTP和HTTPS服务均已禁用，无法启动服务器"
        ));
    }
    
    // 等待所有服务器完成
    futures::future::join_all(servers).await;
    
    Ok(())
}

// 添加辅助函数获取端口配置
fn get_env_port(name: &str, default: u16) -> u16 {
    match env::var(name) {
        Ok(val) => match val.parse::<u16>() {
            Ok(port) => port,
            Err(_) => default,
        },
        Err(_) => default,
    }
}

// 修改证书加载函数，使用环境变量配置证书路径
fn load_rustls_config() -> Result<ServerConfig, Box<dyn std::error::Error>> {
    // 从环境变量获取证书路径，如果未设置则使用默认值
    let cert_path = env::var("DOCXY_CERT_PATH")
        .unwrap_or_else(|_| "/root/.acme.sh/example.com_ecc/fullchain.cer".to_string());
    
    let key_path = env::var("DOCXY_KEY_PATH")
        .unwrap_or_else(|_| "/root/.acme.sh/example.com_ecc/example.com.key".to_string());
    
    info!("正在加载证书: {}", cert_path);
    info!("正在加载私钥: {}", key_path);
    
    // 读取证书和密钥文件
    let cert_file = &mut BufReader::new(File::open(&cert_path)
        .map_err(|e| format!("无法打开证书文件 {}: {}", cert_path, e))?);
    
    let key_file = &mut BufReader::new(File::open(&key_path)
        .map_err(|e| format!("无法打开私钥文件 {}: {}", key_path, e))?);
    
    // 解析证书
    let cert_chain = rustls_pemfile::certs(cert_file)?
        .into_iter()
        .map(Certificate)
        .collect();
    
    // 尝试解析私钥（支持多种格式）
    let mut keys = rustls_pemfile::ec_private_keys(key_file)?;
    
    // 如果没有找到 ECC 私钥，尝试读取 RSA 私钥
    if keys.is_empty() {
        // 需要重新打开文件，因为前面的读取已经消耗了文件内容
        let key_file = &mut BufReader::new(File::open(&key_path)?);
        keys = rustls_pemfile::rsa_private_keys(key_file)?;
    }
    
    // 如果仍然没有找到私钥，尝试读取 PKCS8 格式的私钥
    if keys.is_empty() {
        let key_file = &mut BufReader::new(File::open(&key_path)?);
        keys = rustls_pemfile::pkcs8_private_keys(key_file)?;
    }
    
    if keys.is_empty() {
        return Err("无法读取私钥，支持的格式：ECC、RSA 或 PKCS8".into());
    }
    
    // 构建 TLS 配置
    let config = ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(cert_chain, PrivateKey(keys[0].clone()))?;
    
    info!("成功加载证书和私钥");
    Ok(config)
}
