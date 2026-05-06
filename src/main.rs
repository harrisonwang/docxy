use crate::error::AppError;
use actix_web::{App, HttpRequest, HttpServer, Result, guard, http::header, web};
use lazy_static::lazy_static;
use log::{error, info, warn};
use rustls::{Certificate, PrivateKey, ServerConfig};
use std::env;
use std::fs::File;
use std::io::BufReader;
use std::process;
use std::time::Duration;

mod config;
mod error;
mod handlers;

#[derive(Clone)]
pub struct AppState {
    pub registries: Vec<RegistryTarget>,
    pub default_registry: String,
    pub public_base_url: String,
}

#[derive(Debug, Clone)]
pub struct RegistryTarget {
    pub name: String,
    pub hosts: Vec<String>,
    pub upstream_registry: String,
    pub auth_realm: String,
    pub auth_service: String,
    pub auto_library_prefix: bool,
    pub public_base_url: String,
}

impl AppState {
    pub fn default_registry(&self) -> RegistryTarget {
        self.registry_by_name(&self.default_registry)
            .or_else(|| self.registries.first().cloned())
            .expect("at least one registry target must be configured")
    }

    pub fn registry_by_name(&self, name: &str) -> Option<RegistryTarget> {
        self.registries
            .iter()
            .find(|registry| registry.name.eq_ignore_ascii_case(name))
            .cloned()
    }

    pub fn registry_for_request(&self, req: &HttpRequest) -> RegistryTarget {
        request_host(req)
            .and_then(|host| {
                self.registries
                    .iter()
                    .find(|registry| {
                        registry
                            .hosts
                            .iter()
                            .any(|configured| host_matches(configured, &host))
                    })
                    .cloned()
            })
            .unwrap_or_else(|| self.default_registry())
    }
}

// 超时配置常量
const CONNECT_TIMEOUT_SECS: u64 = 30; // 连接超时：30秒
const REQUEST_TIMEOUT_SECS: u64 = 3600; // 请求总超时：1小时，适用于大镜像下载
const CLIENT_TIMEOUT_SECS: u64 = 3600; // 客户端超时：1小时
const CLIENT_DISCONNECT_TIMEOUT_SECS: u64 = 3600; // 客户端断开超时：1小时
const KEEP_ALIVE_SECS: u64 = 75; // Keep-alive：75秒
const POOL_IDLE_TIMEOUT_SECS: u64 = 90; // 连接池空闲超时：90秒

const DOCKER_HUB_REGISTRY: &str = "https://registry-1.docker.io";
const DOCKER_HUB_AUTH_REALM: &str = "https://auth.docker.io/token";
const DOCKER_HUB_AUTH_SERVICE: &str = "registry.docker.io";

#[derive(Debug, PartialEq, Eq)]
enum CliAction {
    Run(CliOptions),
    PrintHelp,
    PrintVersion,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct CliOptions {
    config_path: Option<String>,
    log_filter: Option<String>,
}

lazy_static! {
    pub static ref HTTP_CLIENT: reqwest::Client = reqwest::Client::builder()
        .pool_max_idle_per_host(10)  // 根据负载调整
        .pool_idle_timeout(Duration::from_secs(POOL_IDLE_TIMEOUT_SECS))
        .connect_timeout(Duration::from_secs(CONNECT_TIMEOUT_SECS))
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .build()
        .unwrap();
}

fn configure_proxy_routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/v2/", web::get().to(handlers::proxy_challenge))
        .route("/auth/{registry}/token", web::get().to(handlers::get_token))
        .route("/auth/token", web::get().to(handlers::get_token))
        .route("/health", web::get().to(handlers::health_check))
        .route(
            "/v2/{image_name:.*}/{path_type}/{reference:.+}",
            web::route()
                .guard(guard::Any(guard::Get()).or(guard::Head()))
                .to(handlers::handle_request),
        )
        .route("/generate_204", web::get().to(handlers::generate_204));
}

fn request_host(req: &HttpRequest) -> Option<String> {
    req.headers()
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .map(normalize_host)
        .filter(|host| !host.is_empty())
}

fn normalize_host(host: &str) -> String {
    let host = host.trim().trim_end_matches('.').to_ascii_lowercase();

    if let Some(ipv6_host) = host
        .strip_prefix('[')
        .and_then(|rest| rest.split(']').next())
    {
        return ipv6_host.to_string();
    }

    if host.matches(':').count() == 1 {
        host.split(':').next().unwrap_or("").to_string()
    } else {
        host
    }
}

fn host_matches(configured: &str, request_host: &str) -> bool {
    normalize_host(configured) == request_host
}

fn build_registry_targets(
    registry_settings: &config::RegistrySettings,
    default_public_base_url: &str,
) -> Result<Vec<RegistryTarget>, AppError> {
    let default_registry = validate_registry_name(&registry_settings.default)?;
    let registries = if registry_settings.upstreams.is_empty() {
        vec![RegistryTarget {
            name: default_registry.clone(),
            hosts: Vec::new(),
            upstream_registry: normalize_registry_base_url(
                registry_settings
                    .upstream_registry
                    .as_deref()
                    .unwrap_or(DOCKER_HUB_REGISTRY),
                "registry.upstream_registry",
            )?,
            auth_realm: normalize_auth_realm(
                registry_settings
                    .auth_realm
                    .as_deref()
                    .unwrap_or(DOCKER_HUB_AUTH_REALM),
                "registry.auth_realm",
            )?,
            auth_service: normalize_non_empty(
                registry_settings
                    .auth_service
                    .as_deref()
                    .unwrap_or(DOCKER_HUB_AUTH_SERVICE),
                "registry.auth_service",
            )?,
            auto_library_prefix: registry_settings.auto_library_prefix,
            public_base_url: normalize_optional_public_base_url(
                registry_settings.public_base_url.as_deref(),
                default_public_base_url,
                "registry.public_base_url",
            )?,
        }]
    } else {
        registry_settings
            .upstreams
            .iter()
            .map(|upstream| {
                Ok(RegistryTarget {
                    name: validate_registry_name(&upstream.name)?,
                    hosts: upstream
                        .hosts
                        .iter()
                        .map(|host| normalize_host(host))
                        .collect(),
                    upstream_registry: normalize_registry_base_url(
                        &upstream.upstream_registry,
                        "registry.upstreams[].upstream_registry",
                    )?,
                    auth_realm: normalize_auth_realm(
                        &upstream.auth_realm,
                        "registry.upstreams[].auth_realm",
                    )?,
                    auth_service: normalize_non_empty(
                        &upstream.auth_service,
                        "registry.upstreams[].auth_service",
                    )?,
                    auto_library_prefix: upstream.auto_library_prefix,
                    public_base_url: normalize_optional_public_base_url(
                        upstream.public_base_url.as_deref(),
                        default_public_base_url,
                        "registry.upstreams[].public_base_url",
                    )?,
                })
            })
            .collect::<Result<Vec<_>, AppError>>()?
    };

    if registries.is_empty() {
        return Err(AppError::TlsConfig(
            "registry configuration must include at least one upstream".to_string(),
        ));
    }

    if !registries
        .iter()
        .any(|registry| registry.name.eq_ignore_ascii_case(&default_registry))
    {
        return Err(AppError::TlsConfig(format!(
            "registry.default '{}' does not match any configured upstream",
            registry_settings.default
        )));
    }

    Ok(registries)
}

fn validate_registry_name(name: &str) -> Result<String, AppError> {
    let trimmed = name.trim();
    if trimmed.is_empty()
        || !trimmed
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
    {
        return Err(AppError::TlsConfig(
            "registry upstream name can only contain ASCII letters, digits, '-', '_' or '.'"
                .to_string(),
        ));
    }

    Ok(trimmed.to_string())
}

fn normalize_registry_base_url(raw_url: &str, field_name: &str) -> Result<String, AppError> {
    let trimmed = raw_url.trim().trim_end_matches('/');
    let parsed = reqwest::Url::parse(trimmed)
        .map_err(|e| AppError::TlsConfig(format!("{field_name} invalid: {e}")))?;

    match parsed.scheme() {
        "http" | "https" => {}
        _ => {
            return Err(AppError::TlsConfig(format!(
                "{field_name} only supports http or https"
            )));
        }
    }

    if parsed.host_str().is_none() {
        return Err(AppError::TlsConfig(format!(
            "{field_name} must include a host"
        )));
    }

    Ok(trimmed.to_string())
}

fn normalize_auth_realm(raw_url: &str, field_name: &str) -> Result<String, AppError> {
    let trimmed = raw_url.trim();
    let parsed = reqwest::Url::parse(trimmed)
        .map_err(|e| AppError::TlsConfig(format!("{field_name} invalid: {e}")))?;

    match parsed.scheme() {
        "http" | "https" => {}
        _ => {
            return Err(AppError::TlsConfig(format!(
                "{field_name} only supports http or https"
            )));
        }
    }

    if parsed.host_str().is_none() {
        return Err(AppError::TlsConfig(format!(
            "{field_name} must include a host"
        )));
    }

    Ok(trimmed.to_string())
}

fn normalize_non_empty(value: &str, field_name: &str) -> Result<String, AppError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::TlsConfig(format!("{field_name} cannot be empty")));
    }

    Ok(trimmed.to_string())
}

fn normalize_optional_public_base_url(
    configured: Option<&str>,
    fallback: &str,
    field_name: &str,
) -> Result<String, AppError> {
    match configured.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => normalize_public_base_url(value)
            .map_err(|e| AppError::TlsConfig(format!("{field_name}: {e}"))),
        None => Ok(fallback.to_string()),
    }
}

fn parse_cli_args<I, S>(args: I) -> Result<CliAction, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let args = args
        .into_iter()
        .map(|arg| arg.as_ref().to_string())
        .collect::<Vec<_>>();

    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        return Ok(CliAction::PrintHelp);
    }

    let mut options = CliOptions::default();
    let mut iter = args.into_iter();

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-V" | "--version" => return Ok(CliAction::PrintVersion),
            "-c" | "--config" => {
                let value = iter
                    .next()
                    .ok_or_else(|| format!("选项 '{arg}' 需要一个配置文件路径"))?;
                if value.starts_with('-') {
                    return Err(format!("选项 '{arg}' 需要一个配置文件路径"));
                }
                options.config_path = Some(parse_non_empty_option_value(&arg, value)?);
            }
            "--log-level" => {
                let value = iter
                    .next()
                    .ok_or_else(|| format!("选项 '{arg}' 需要一个日志级别或过滤器"))?;
                if value.starts_with('-') {
                    return Err(format!("选项 '{arg}' 需要一个日志级别或过滤器"));
                }
                options.log_filter = Some(parse_non_empty_option_value(&arg, value)?);
            }
            _ if arg.starts_with("--config=") => {
                options.config_path = Some(parse_non_empty_option_value(
                    "--config",
                    arg.trim_start_matches("--config=").to_string(),
                )?);
            }
            _ if arg.starts_with("-c=") => {
                options.config_path = Some(parse_non_empty_option_value(
                    "-c",
                    arg.trim_start_matches("-c=").to_string(),
                )?);
            }
            _ if arg.starts_with("--log-level=") => {
                options.log_filter = Some(parse_non_empty_option_value(
                    "--log-level",
                    arg.trim_start_matches("--log-level=").to_string(),
                )?);
            }
            _ => return Err(format!("未知选项 '{arg}'")),
        }
    }

    Ok(CliAction::Run(options))
}

fn parse_non_empty_option_value(option_name: &str, value: String) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!("选项 '{option_name}' 的值不能为空"));
    }

    Ok(trimmed.to_string())
}

fn resolve_config_path(cli_config_path: Option<String>) -> String {
    cli_config_path
        .or_else(|| {
            env::var("WHARF_CONFIG")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
        .unwrap_or_else(|| config::DEFAULT_CONFIG_FILE.to_string())
}

fn init_logger(log_filter: Option<&str>) {
    let mut builder = match log_filter {
        Some(filter) => {
            let mut builder = env_logger::Builder::new();
            builder.parse_filters(filter);
            builder
        }
        None => env_logger::Builder::from_env(
            env_logger::Env::default().default_filter_or("actix_web=info"),
        ),
    };

    builder
        .format(|buf, record| {
            use chrono::Local;
            use std::io::Write;

            let level = record.level();
            let mut style_binding = buf.style(); // 先创建绑定
            let level_style = style_binding // 使用绑定
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
}

fn help_text() -> String {
    format!(
        "\
{name} {version}

Docker Registry 代理，用于容器镜像加速。

用法:
  {name} [选项]

选项:
  -c, --config <路径>       指定配置文件路径，可省略 .toml 后缀
      --log-level <过滤器>  指定日志级别或 RUST_LOG 过滤器，例如 info、debug、wharf=debug
  -h, --help               打印帮助信息
  -V, --version            打印版本信息

优先级:
  配置文件: --config/-c > WHARF_CONFIG > config/default
  日志级别: --log-level > RUST_LOG > actix_web=info

兼容说明:
  不传参数时会继续读取当前工作目录下的 config/default。
  现有 systemd 配置 WorkingDirectory=/etc/wharf 会继续读取 /etc/wharf/config/default.toml。
",
        name = env!("CARGO_PKG_NAME"),
        version = env!("CARGO_PKG_VERSION")
    )
}

#[actix_web::main]
async fn main() {
    let cli_options = match parse_cli_args(env::args().skip(1)) {
        Ok(CliAction::Run(options)) => options,
        Ok(CliAction::PrintHelp) => {
            print!("{}", help_text());
            return;
        }
        Ok(CliAction::PrintVersion) => {
            println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
            return;
        }
        Err(message) => {
            eprintln!("{message}\n\n{}", help_text());
            process::exit(2);
        }
    };

    // 使用env_logger的Builder直接设置日志级别
    init_logger(cli_options.log_filter.as_deref());

    if let Err(error) = run_server(cli_options).await {
        eprintln!("{}", startup_error_message(&error));
        process::exit(1);
    }
}

async fn run_server(cli_options: CliOptions) -> Result<(), AppError> {
    let config_path = resolve_config_path(cli_options.config_path);
    let settings = if config_path == config::DEFAULT_CONFIG_FILE {
        config::Settings::new()
    } else {
        config::Settings::from_file(&config_path)
    }
    .map_err(|source| AppError::ConfigLoad {
        path: config_path.clone(),
        source,
    })?;
    let public_base_url = resolve_public_base_url(&settings.server)?;
    let registries = build_registry_targets(&settings.registry, &public_base_url)?;

    // 输出配置信息
    info!("服务器配置:");
    info!("配置文件: {}", config_path);
    info!("HTTP 端口: {}", settings.server.http_port);
    info!("对外基准地址: {}", public_base_url);

    if settings.server.https_enabled {
        info!("HTTPS 端口: {}", settings.server.https_port);
    } else {
        info!("HTTPS 服务: 已禁用");
    }

    if settings.server.behind_proxy {
        info!("代理模式: 已启用");
    }
    if public_base_url.starts_with("http://") {
        warn!("server.public_base_url 使用 HTTP，这会降低认证链路安全性，建议改为 HTTPS");
    }

    // 创建应用配置
    for registry in &registries {
        info!(
            "Registry {} -> {} (service: {})",
            registry.name, registry.upstream_registry, registry.auth_service
        );
    }

    let app_state = web::Data::new(AppState {
        registries,
        default_registry: settings.registry.default.trim().to_string(),
        public_base_url: public_base_url.clone(),
    });
    let http_app_data = app_state.clone();
    let http_redirect_app_data = app_state.clone();
    let https_app_data = app_state.clone();

    let http_app = move || {
        App::new()
            .app_data(http_app_data.clone())
            .configure(configure_proxy_routes)
            .default_service(web::route().to(handlers::handle_invalid_request)) // 添加默认服务处理非法请求
    };

    // 创建HTTP重定向应用配置，特殊情况下我们可能仍然希望重定向，而不是拒绝访问
    let http_redirect_app = move || {
        App::new()
            .app_data(http_redirect_app_data.clone())
            .service(
                web::scope("/v2")
                    .route("", web::get().to(handlers::redirect_to_https))
                    .route("/{tail:.*}", web::route().to(handlers::redirect_to_https)),
            )
            .route(
                "/auth/{registry}/token",
                web::get().to(handlers::redirect_to_https),
            )
            .route("/auth/token", web::get().to(handlers::redirect_to_https))
            .route("/health", web::get().to(handlers::redirect_to_https))
            .default_service(web::route().to(handlers::handle_invalid_request)) // 非法路径直接拒绝
    };

    // 创建服务器实例
    let mut servers = Vec::new();

    // 启动HTTP服务器（如果启用）
    if settings.server.http_enabled {
        let http_server = if !settings.server.behind_proxy && settings.server.https_enabled {
            // 如果启用了HTTPS且不在代理后面，HTTP只做重定向
            HttpServer::new(http_redirect_app)
                .bind(("0.0.0.0", settings.server.http_port))?
                .client_request_timeout(Duration::from_secs(CLIENT_TIMEOUT_SECS))
                .client_disconnect_timeout(Duration::from_secs(CLIENT_DISCONNECT_TIMEOUT_SECS))
                .keep_alive(Duration::from_secs(KEEP_ALIVE_SECS))
                .run()
        } else {
            // 否则HTTP提供完整功能
            HttpServer::new(http_app)
                .bind(("0.0.0.0", settings.server.http_port))?
                .client_request_timeout(Duration::from_secs(CLIENT_TIMEOUT_SECS))
                .client_disconnect_timeout(Duration::from_secs(CLIENT_DISCONNECT_TIMEOUT_SECS))
                .keep_alive(Duration::from_secs(KEEP_ALIVE_SECS))
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
                        .app_data(https_app_data.clone())
                        .configure(configure_proxy_routes)
                        .default_service(web::route().to(handlers::handle_invalid_request)) // 添加默认服务处理非法请求
                })
                .bind_rustls(("0.0.0.0", settings.server.https_port), rustls_config)?
                .client_request_timeout(Duration::from_secs(CLIENT_TIMEOUT_SECS))
                .client_disconnect_timeout(Duration::from_secs(CLIENT_DISCONNECT_TIMEOUT_SECS))
                .keep_alive(Duration::from_secs(KEEP_ALIVE_SECS))
                .run();

                servers.push(https_server);
            }
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

fn startup_error_message(error: &AppError) -> String {
    match error {
        AppError::ConfigLoad { path, source } => config_load_error_message(path, source),
        AppError::TlsConfig(message) => {
            format!("启动失败: 配置无效\n\n原因: {message}\n\n可运行 `wharf -h` 查看启动选项。")
        }
        AppError::Io(error) => format!(
            "启动失败: I/O 错误\n\n原因: {error}\n\n请检查端口占用、文件权限或 systemd 服务权限。"
        ),
        AppError::Rustls(error) => {
            format!("启动失败: TLS 配置无效\n\n原因: {error}\n\n请检查证书和私钥是否匹配。")
        }
        AppError::UpstreamRequest(error) => {
            format!("启动失败: 上游 Registry 请求失败\n\n原因: {error}")
        }
        AppError::InvalidRequest(message) => format!("启动失败: 请求配置无效\n\n原因: {message}"),
    }
}

fn config_load_error_message(path: &str, source: &::config::ConfigError) -> String {
    let source_message = source.to_string();
    let mut message = format!("配置文件加载失败\n\n路径: {path}\n原因: {source_message}\n");

    if path == config::DEFAULT_CONFIG_FILE {
        message.push_str("\n默认路径: config/default（会自动查找 config/default.toml）\n");
    }

    if is_missing_config_error(&source_message) {
        message.push_str(
            "\n处理方式:\n  1. 创建默认配置文件:\n     cp config/default.toml.example config/default.toml\n  2. 或显式指定配置文件:\n     wharf --config /etc/wharf/config/default.toml\n\nsystemd 兼容说明:\n  当前默认仍兼容 WorkingDirectory=/etc/wharf，此时会读取 /etc/wharf/config/default.toml。\n",
        );
    } else {
        message
            .push_str("\n请检查配置文件 TOML 格式、字段名称和必填字段。可运行 `wharf -h` 查看配置路径优先级。\n");
    }

    message
}

fn is_missing_config_error(message: &str) -> bool {
    message.contains("not found") || message.contains("No such file or directory")
}
fn resolve_public_base_url(server: &config::ServerSettings) -> Result<String, AppError> {
    if let Some(configured) = server.public_base_url.as_deref() {
        let trimmed = configured.trim();
        if !trimmed.is_empty() {
            return normalize_public_base_url(trimmed);
        }
    }

    let fallback = if server.https_enabled {
        if server.https_port == 443 {
            "https://localhost".to_string()
        } else {
            format!("https://localhost:{}", server.https_port)
        }
    } else if server.http_enabled {
        if server.http_port == 80 {
            "http://localhost".to_string()
        } else {
            format!("http://localhost:{}", server.http_port)
        }
    } else {
        return Err(AppError::TlsConfig(
            "server.public_base_url 未配置，且 HTTP/HTTPS 均禁用，无法推导默认值".to_string(),
        ));
    };

    warn!(
        "server.public_base_url 未配置，已回退为 {}。建议尽快在配置中显式设置公网访问地址。",
        fallback
    );
    Ok(fallback)
}

// 校验并规范化对外基准地址，避免将请求头中的 Host 用于安全敏感响应
fn normalize_public_base_url(raw_url: &str) -> Result<String, AppError> {
    let mut parsed = reqwest::Url::parse(raw_url)
        .map_err(|e| AppError::TlsConfig(format!("server.public_base_url 无效: {e}")))?;

    match parsed.scheme() {
        "http" | "https" => {}
        _ => {
            return Err(AppError::TlsConfig(
                "server.public_base_url 仅支持 http 或 https 协议".to_string(),
            ));
        }
    }

    if parsed.host_str().is_none() {
        return Err(AppError::TlsConfig(
            "server.public_base_url 必须包含主机名".to_string(),
        ));
    }

    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(AppError::TlsConfig(
            "server.public_base_url 不能包含用户认证信息".to_string(),
        ));
    }

    if parsed.query().is_some() || parsed.fragment().is_some() {
        return Err(AppError::TlsConfig(
            "server.public_base_url 不能包含查询参数或锚点".to_string(),
        ));
    }

    if parsed.path() != "/" {
        return Err(AppError::TlsConfig(
            "server.public_base_url 不能包含路径".to_string(),
        ));
    }

    parsed.set_path("");
    Ok(parsed.as_str().trim_end_matches('/').to_string())
}

// 修改证书加载函数，使用环境变量配置证书路径
fn load_rustls_config(settings: &config::Settings) -> Result<ServerConfig, AppError> {
    // 从环境变量获取证书路径，如果未设置则使用默认值
    let cert_path = &settings.tls.cert_path;
    let key_path = &settings.tls.key_path;

    info!("正在加载证书: {}", cert_path);
    info!("正在加载私钥: {}", key_path);

    // 读取证书和密钥文件
    let cert_file = &mut BufReader::new(
        File::open(cert_path)
            .map_err(|e| AppError::TlsConfig(format!("无法打开证书文件 {cert_path}: {e}")))?,
    );

    let key_file = &mut BufReader::new(
        File::open(key_path)
            .map_err(|e| AppError::TlsConfig(format!("无法打开私钥文件 {key_path}: {e}")))?,
    );

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
        let key_file = &mut BufReader::new(File::open(key_path)?);
        keys = rustls_pemfile::rsa_private_keys(key_file)?;
    }

    // 如果仍然没有找到私钥，尝试读取 PKCS8 格式的私钥
    if keys.is_empty() {
        let key_file = &mut BufReader::new(File::open(key_path)?);
        keys = rustls_pemfile::pkcs8_private_keys(key_file)?;
    }

    if keys.is_empty() {
        return Err(AppError::TlsConfig(
            "无法读取私钥，支持的格式：ECC、RSA 或 PKCS8".into(),
        ));
    }

    // 构建 TLS 配置
    let config = ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(cert_chain, PrivateKey(keys[0].clone()))?;

    info!("成功加载证书和私钥");
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::test as actix_test;

    fn server_settings() -> config::ServerSettings {
        config::ServerSettings {
            http_port: 80,
            https_port: 443,
            http_enabled: true,
            https_enabled: true,
            behind_proxy: false,
            public_base_url: Some("https://docker.example.com".to_string()),
        }
    }

    #[test]
    fn legacy_registry_config_builds_docker_hub_target() {
        let registry_settings = config::RegistrySettings {
            default: "dockerhub".to_string(),
            upstream_registry: Some("https://registry-1.docker.io".to_string()),
            auth_realm: None,
            auth_service: None,
            auto_library_prefix: true,
            public_base_url: None,
            upstreams: vec![],
        };

        let registries = build_registry_targets(&registry_settings, "https://docker.example.com")
            .expect("legacy registry config should resolve");

        assert_eq!(registries.len(), 1);
        assert_eq!(registries[0].name, "dockerhub");
        assert_eq!(registries[0].auth_service, "registry.docker.io");
        assert!(registries[0].auto_library_prefix);
    }

    #[test]
    fn multi_registry_config_builds_ghcr_and_quay_targets() {
        let registry_settings = config::RegistrySettings {
            default: "dockerhub".to_string(),
            upstream_registry: None,
            auth_realm: None,
            auth_service: None,
            auto_library_prefix: true,
            public_base_url: None,
            upstreams: vec![
                config::RegistryUpstreamSettings {
                    name: "dockerhub".to_string(),
                    hosts: vec!["docker.example.com".to_string()],
                    upstream_registry: "https://registry-1.docker.io".to_string(),
                    auth_realm: "https://auth.docker.io/token".to_string(),
                    auth_service: "registry.docker.io".to_string(),
                    auto_library_prefix: true,
                    public_base_url: Some("https://docker.example.com".to_string()),
                },
                config::RegistryUpstreamSettings {
                    name: "ghcr".to_string(),
                    hosts: vec!["ghcr.example.com".to_string()],
                    upstream_registry: "https://ghcr.io".to_string(),
                    auth_realm: "https://ghcr.io/token".to_string(),
                    auth_service: "ghcr.io".to_string(),
                    auto_library_prefix: false,
                    public_base_url: Some("https://ghcr.example.com".to_string()),
                },
                config::RegistryUpstreamSettings {
                    name: "quay".to_string(),
                    hosts: vec!["quay.example.com".to_string()],
                    upstream_registry: "https://quay.io".to_string(),
                    auth_realm: "https://quay.io/v2/auth".to_string(),
                    auth_service: "quay.io".to_string(),
                    auto_library_prefix: false,
                    public_base_url: Some("https://quay.example.com".to_string()),
                },
            ],
        };

        let registries = build_registry_targets(&registry_settings, "https://docker.example.com")
            .expect("multi-registry config should resolve");

        assert_eq!(registries.len(), 3);
        assert_eq!(registries[1].name, "ghcr");
        assert_eq!(registries[1].auth_realm, "https://ghcr.io/token");
        assert_eq!(registries[2].name, "quay");
        assert_eq!(registries[2].auth_realm, "https://quay.io/v2/auth");
    }

    #[test]
    fn host_header_selects_matching_registry() {
        let registries = vec![
            RegistryTarget {
                name: "dockerhub".to_string(),
                hosts: vec!["docker.example.com".to_string()],
                upstream_registry: "https://registry-1.docker.io".to_string(),
                auth_realm: "https://auth.docker.io/token".to_string(),
                auth_service: "registry.docker.io".to_string(),
                auto_library_prefix: true,
                public_base_url: "https://docker.example.com".to_string(),
            },
            RegistryTarget {
                name: "ghcr".to_string(),
                hosts: vec!["ghcr.example.com".to_string()],
                upstream_registry: "https://ghcr.io".to_string(),
                auth_realm: "https://ghcr.io/token".to_string(),
                auth_service: "ghcr.io".to_string(),
                auto_library_prefix: false,
                public_base_url: "https://ghcr.example.com".to_string(),
            },
        ];
        let app_state = AppState {
            registries,
            default_registry: "dockerhub".to_string(),
            public_base_url: "https://docker.example.com".to_string(),
        };
        let req = actix_test::TestRequest::default()
            .insert_header(("Host", "ghcr.example.com:443"))
            .to_http_request();

        assert_eq!(app_state.registry_for_request(&req).name, "ghcr");
    }

    #[test]
    fn public_base_url_still_resolves_for_server_config() {
        assert_eq!(
            resolve_public_base_url(&server_settings()).unwrap(),
            "https://docker.example.com"
        );
    }

    #[test]
    fn cli_without_args_runs_server() {
        assert_eq!(
            parse_cli_args(std::iter::empty::<&str>()).unwrap(),
            CliAction::Run(CliOptions::default())
        );
    }

    #[test]
    fn cli_help_flags_short_circuit_startup() {
        assert!(matches!(
            parse_cli_args(["-h"]).unwrap(),
            CliAction::PrintHelp
        ));
        assert!(matches!(
            parse_cli_args(["--help", "-V"]).unwrap(),
            CliAction::PrintHelp
        ));
    }

    #[test]
    fn cli_version_flags_short_circuit_startup() {
        assert!(matches!(
            parse_cli_args(["-V"]).unwrap(),
            CliAction::PrintVersion
        ));
        assert!(matches!(
            parse_cli_args(["--version"]).unwrap(),
            CliAction::PrintVersion
        ));
    }

    #[test]
    fn cli_rejects_unknown_args() {
        assert_eq!(
            parse_cli_args(["--missing"]).unwrap_err(),
            "未知选项 '--missing'"
        );
    }

    #[test]
    fn cli_parses_config_path() {
        assert_eq!(
            parse_cli_args(["--config", "/etc/wharf/config/default.toml"]).unwrap(),
            CliAction::Run(CliOptions {
                config_path: Some("/etc/wharf/config/default.toml".to_string()),
                log_filter: None,
            })
        );
        assert_eq!(
            parse_cli_args(["-c=/etc/wharf/config/default.toml"]).unwrap(),
            CliAction::Run(CliOptions {
                config_path: Some("/etc/wharf/config/default.toml".to_string()),
                log_filter: None,
            })
        );
        assert_eq!(
            parse_cli_args(["--config=/etc/wharf/config/default"]).unwrap(),
            CliAction::Run(CliOptions {
                config_path: Some("/etc/wharf/config/default".to_string()),
                log_filter: None,
            })
        );
    }

    #[test]
    fn cli_parses_log_filter() {
        assert_eq!(
            parse_cli_args(["--log-level", "debug"]).unwrap(),
            CliAction::Run(CliOptions {
                config_path: None,
                log_filter: Some("debug".to_string()),
            })
        );
        assert_eq!(
            parse_cli_args(["--log-level=wharf=debug,actix_web=info"]).unwrap(),
            CliAction::Run(CliOptions {
                config_path: None,
                log_filter: Some("wharf=debug,actix_web=info".to_string()),
            })
        );
    }

    #[test]
    fn cli_rejects_missing_option_values() {
        assert_eq!(
            parse_cli_args(["--config"]).unwrap_err(),
            "选项 '--config' 需要一个配置文件路径"
        );
        assert_eq!(
            parse_cli_args(["--config", "--log-level"]).unwrap_err(),
            "选项 '--config' 需要一个配置文件路径"
        );
        assert_eq!(
            parse_cli_args(["--log-level="]).unwrap_err(),
            "选项 '--log-level' 的值不能为空"
        );
    }

    #[test]
    fn cli_help_text_is_chinese() {
        let help = help_text();
        assert!(help.contains("用法:"));
        assert!(help.contains("优先级:"));
        assert!(help.contains("兼容说明:"));
    }

    #[test]
    fn startup_config_error_is_human_readable() {
        let error = AppError::ConfigLoad {
            path: config::DEFAULT_CONFIG_FILE.to_string(),
            source: ::config::ConfigError::Message(
                "configuration file \"config/default\" not found".to_string(),
            ),
        };
        let message = startup_error_message(&error);

        assert!(message.contains("配置文件加载失败"));
        assert!(message.contains("路径: config/default"));
        assert!(message.contains("cp config/default.toml.example config/default.toml"));
        assert!(message.contains("wharf --config /etc/wharf/config/default.toml"));
        assert!(!message.contains("TlsConfig"));
    }
}
