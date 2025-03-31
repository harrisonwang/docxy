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

async fn handle_no_namespace_request(
    req: HttpRequest,
    path: web::Path<(String, String, String)>,
) -> Result<HttpResponse> {
    let (image_name, path_type, reference) = path.into_inner();

    // 获取主机信息和协议
    let connection_info = req.connection_info();
    let host = connection_info.host().to_string();
    let scheme = connection_info.scheme();

    // 构建重定向URL (添加library命名空间)
    let redirect_url = format!("{}://{}/v2/library/{}/{}/{}",
                              scheme, host, image_name, path_type, reference);

    // 复制请求头到重定向
    let mut builder = HttpResponse::MovedPermanently();
    builder.append_header(("Location", redirect_url));

    // 复制Authorization和Accept头（如果存在）
    if let Some(auth) = req.headers().get("Authorization") {
        if let Ok(auth_str) = auth.to_str() {
            builder.append_header(("Authorization", auth_str));
        }
    }

    if let Some(accept) = req.headers().get("Accept") {
        if let Ok(accept_str) = accept.to_str() {
            builder.append_header(("Accept", accept_str));
        }
    }

    Ok(builder.finish())
}

async fn handle_request(
    req: HttpRequest,
    path: web::Path<(String, String, String)>,
) -> Result<HttpResponse> {
    // 获取路径参数
    let (image_name, path_type, reference) = path.into_inner();

    // 使用常量构建目标URL
    let path = format!("/v2/library/{}/{}/{}", image_name, path_type, reference);

    // 构建请求，根据原始请求的方法选择 HEAD 或 GET
    let mut request_builder = if req.method() == &actix_web::http::Method::HEAD {
        HTTP_CLIENT.head(format!("{}{}", DOCKER_REGISTRY_URL, path))
    } else {
        HTTP_CLIENT.get(format!("{}{}", DOCKER_REGISTRY_URL, path))
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
    let response = match request_builder.send().await {
        Ok(resp) => resp,
        Err(e) => {
            eprintln!("请求上游失败: {}", e);
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
                    eprintln!("流读取错误: {}", err);
                    actix_web::error::ErrorInternalServerError(err)
                })
            });
            
        Ok(builder.streaming(stream))
    }
}

// 获取 Token 的处理函数
async fn get_token(req: HttpRequest) -> Result<HttpResponse> {
    // 获取请求中的查询参数
    let query_params = web::Query::<HashMap<String, String>>::from_query(req.query_string()).unwrap();

    // 处理 scope 参数
    let scope = match query_params.get("scope") {
        Some(s) => process_scope(s),
        None => "".to_string()
    };

    // 构建请求 Docker Hub 认证服务的 URL
    let mut auth_url = reqwest::Url::parse("https://auth.docker.io/token").unwrap();

    // 添加查询参数
    {
        let mut query_pairs = auth_url.query_pairs_mut();
        query_pairs.append_pair("service", "registry.docker.io");
        query_pairs.append_pair("scope", &scope);
        // 如果有其他参数也可以添加，例如 client_id 等
    }

    // 发送请求到 Docker Hub 认证服务
    let response = match HTTP_CLIENT.get(auth_url).send().await {
        Ok(resp) => resp,
        Err(_) => {
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
        Ok(bytes) => Ok(builder.body(bytes)),
        Err(_) => Ok(HttpResponse::InternalServerError().body("无法读取认证服务响应"))
    }
}

// 处理 scope 参数的辅助函数
fn process_scope(scope: &str) -> String {
    let parts: Vec<&str> = scope.split(':').collect();

    if parts.len() == 3 && !parts[1].contains('/') {
        // 如果是以 repository:name:action 格式，并且 name 不包含 /，
        // 则添加 library/ 前缀
        return format!("{}:library/{}:{}", parts[0], parts[1], parts[2]);
    }

    scope.to_string()
}

async fn proxy_challenge(req: HttpRequest) -> Result<HttpResponse> {
    let host = match req.connection_info().host() {
        host if host.contains(':') => host.to_string(),
        host => format!("{}", host)
    };

    let response = match HTTP_CLIENT.get(format!("{}/v2/", DOCKER_REGISTRY_URL)).send().await {
        Ok(resp) => resp,
        Err(_) => {
            return Ok(HttpResponse::InternalServerError()
                      .body("无法连接到上游 Docker Registry"))
        }

    };

    let status = response.status().as_u16();

    let mut builder = HttpResponse::build(actix_web::http::StatusCode::from_u16(status).unwrap());

    builder.append_header((
        "WWW-Authenticate",
        format!("Bearer realm=\"https://{}/auth/token\",service=\"docker-registry-proxy\"", host)
    ));


    let body = match response.text().await {
        Ok(text) => text,
        Err(_) => String::from("无法读取上游响应内容")
    };

    Ok(builder.body(body))
}

async fn health_check() -> impl Responder {
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
    
    HttpResponse::MovedPermanently()
        .append_header(("Location", redirect_url))
        .finish()
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // 使用env_logger的Builder直接设置日志级别，不需要修改环境变量
    env_logger::Builder::from_env(env_logger::Env::default()
        .default_filter_or("actix_web=info"))
        .init();
    
    println!("服务器启动在 HTTP 端口 80 和 HTTPS 端口 443");
    
    // 创建HTTPS应用配置
    let https_app = || {
        App::new()
            .route("/v2/", web::get().to(proxy_challenge))
            .route("/auth/token", web::get().to(get_token))
            .route("/health", web::get().to(health_check))
            .route("/v2/library/{image_name}/{path_type}/{reference:.+}",
                   web::route()
                   .guard(guard::Any(guard::Get()).or(guard::Head()))
                   .to(handle_request))
            .route("/v2/{image_name}/{path_type}/{reference:.+}",
                   web::route()
                   .guard(guard::Any(guard::Get()).or(guard::Head()))
                   .to(handle_no_namespace_request))
    };
    
    // 创建HTTP重定向应用配置
    let http_app = || {
        App::new()
            .default_service(web::route().to(redirect_to_https))
    };
    
    // 加载TLS配置
    let rustls_config = load_rustls_config().expect("无法加载TLS配置");
    
    // 同时启动HTTP和HTTPS服务器
    let http_server = HttpServer::new(http_app)
        .bind(("0.0.0.0", 80))?
        .run();
        
    let https_server = HttpServer::new(https_app)
        .bind_rustls(("0.0.0.0", 443), rustls_config)?
        .run();
    
    // 等待两个服务器都完成
    futures::future::try_join(http_server, https_server).await?;
    
    Ok(())
}

// 修改证书加载函数，使用环境变量配置证书路径
fn load_rustls_config() -> Result<ServerConfig, Box<dyn std::error::Error>> {
    // 从环境变量获取证书路径，如果未设置则使用默认值
    let cert_path = env::var("DOCXY_CERT_PATH")
        .unwrap_or_else(|_| "/root/.acme.sh/example.com_ecc/fullchain.cer".to_string());
    
    let key_path = env::var("DOCXY_KEY_PATH")
        .unwrap_or_else(|_| "/root/.acme.sh/example.com_ecc/example.com.key".to_string());
    
    println!("正在加载证书: {}", cert_path);
    println!("正在加载私钥: {}", key_path);
    
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
    
    println!("成功加载证书和私钥");
    Ok(config)
}
