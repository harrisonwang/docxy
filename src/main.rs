use crate::error::AppError;
use actix_web::{web, guard, App, HttpServer, Result};
use rustls::{Certificate, PrivateKey, ServerConfig};
use std::fs::File;
use std::io::BufReader;
use std::time::Duration;
use lazy_static::lazy_static;
use log::{info, error};

mod config;
mod error;
mod handlers;



lazy_static! {
    pub static ref HTTP_CLIENT: reqwest::Client = reqwest::Client::builder()
        .pool_max_idle_per_host(10)  // 根据负载调整
        .pool_idle_timeout(Duration::from_secs(90))
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap();
}



#[actix_web::main]
async fn main() -> Result<(), AppError> {
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
                level_style.value(format!("{level:5}")),
                record.target(),
                record.args()
            )
        })
        .init();
    
    let settings = config::Settings::new().map_err(|e| AppError::TlsConfig(format!("无法加载配置: {}", e)))?;

    // 输出配置信息
    info!("服务器配置:");
    info!("HTTP 端口: {}", settings.server.http_port);
    
    if settings.server.https_enabled {
        info!("HTTPS 端口: {}", settings.server.https_port);
    } else {
        info!("HTTPS 服务: 已禁用");
    }
    
    if settings.server.behind_proxy {
        info!("代理模式: 已启用");
    }
    
    

    // 创建应用配置
    let http_app_data = web::Data::new(settings.registry.upstream_registry.clone());
    

    let http_app = move || {
        App::new()
            .app_data(http_app_data.clone())
            .route("/v2/", web::get().to(handlers::proxy_challenge))
            .route("/auth/token", web::get().to(handlers::get_token))
            .route("/health", web::get().to(handlers::health_check))
            .route("/v2/{image_name:.*}/{path_type}/{reference:.+}",
                   web::route()
                   .guard(guard::Any(guard::Get()).or(guard::Head()))
                   .to(handlers::handle_request))
            .default_service(web::route().to(handlers::handle_invalid_request))  // 添加默认服务处理非法请求
    };
    
    // 创建HTTP重定向应用配置，特殊情况下我们可能仍然希望重定向，而不是拒绝访问
    let http_redirect_app = move || {
        App::new()
            .service(
                web::scope("/v2")
                    .route("", web::get().to(handlers::redirect_to_https))
                    .route("/{tail:.*}", web::route().to(handlers::redirect_to_https))
            )
            .route("/auth/token", web::get().to(handlers::redirect_to_https))
            .route("/health", web::get().to(handlers::redirect_to_https))
            .default_service(web::route().to(handlers::handle_invalid_request))  // 非法路径直接拒绝
    };
    
    // 创建服务器实例
    let mut servers = Vec::new();
    
    // 启动HTTP服务器（如果启用）
    if settings.server.http_enabled {
        let http_server = if !settings.server.behind_proxy && settings.server.https_enabled {
            // 如果启用了HTTPS且不在代理后面，HTTP只做重定向
            HttpServer::new(http_redirect_app)
                .bind(("0.0.0.0", settings.server.http_port))?
                .run()
        } else {
            // 否则HTTP提供完整功能
            HttpServer::new(http_app)
                .bind(("0.0.0.0", settings.server.http_port))?
                .run()
        };
        
        servers.push(http_server);
    }
    
    // 启动HTTPS服务器（如果启用）
    if settings.server.https_enabled {
        // 加载TLS配置
        match load_rustls_config(&settings) {
            Ok(rustls_config) => {
                let https_server = HttpServer::new(move || {
                    App::new()
                        .app_data(web::Data::new(settings.registry.upstream_registry.clone()))
                        .route("/v2/", web::get().to(handlers::proxy_challenge))
                        .route("/auth/token", web::get().to(handlers::get_token))
                        .route("/health", web::get().to(handlers::health_check))
                        .route("/v2/{image_name:.*}/{path_type}/{reference:.+}",
                               web::route()
                               .guard(guard::Any(guard::Get()).or(guard::Head()))
                               .to(handlers::handle_request))
                        .default_service(web::route().to(handlers::handle_invalid_request))  // 添加默认服务处理非法请求
                })
                    .bind_rustls(("0.0.0.0", settings.server.https_port), rustls_config)?
                    .run();
                
                servers.push(https_server);
            },
            Err(e) => {
                error!("无法加载TLS配置: {}", e);
                if !settings.server.http_enabled {
                    return Err(AppError::TlsConfig(
                        "HTTPS配置加载失败且HTTP服务已禁用，无法启动服务器".to_string(),
                    ));
                }
            }
        }
    }
    
    // 确保至少有一个服务器在运行
    if servers.is_empty() {
        return Err(AppError::TlsConfig(
            "HTTP和HTTPS服务均已禁用，无法启动服务器".to_string(),
        ));
    }
    
    // 等待所有服务器完成
    futures::future::join_all(servers).await;
    
    Ok(())
}



// 修改证书加载函数，使用环境变量配置证书路径
fn load_rustls_config(settings: &config::Settings) -> Result<ServerConfig, AppError> {
    // 从环境变量获取证书路径，如果未设置则使用默认值
    let cert_path = &settings.tls.cert_path;
    let key_path = &settings.tls.key_path;
    
    info!("正在加载证书: {}", cert_path);
    info!("正在加载私钥: {}", key_path);
    
    // 读取证书和密钥文件
    let cert_file = &mut BufReader::new(File::open(&cert_path)
        .map_err(|e| AppError::TlsConfig(format!("无法打开证书文件 {cert_path}: {e}")))?);
    
    let key_file = &mut BufReader::new(File::open(&key_path)
        .map_err(|e| AppError::TlsConfig(format!("无法打开私钥文件 {key_path}: {e}")))?);
    
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
        return Err(AppError::TlsConfig("无法读取私钥，支持的格式：ECC、RSA 或 PKCS8".into()));
    }
    
    // 构建 TLS 配置
    let config = ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(cert_chain, PrivateKey(keys[0].clone()))?;
    
    info!("成功加载证书和私钥");
    Ok(config)
}
