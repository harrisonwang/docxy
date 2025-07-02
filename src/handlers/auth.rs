
use actix_web::{web, HttpRequest, HttpResponse, Result};
use std::collections::HashMap;
use log::{info, error};

use crate::HTTP_CLIENT;

// 获取 Token 的处理函数
pub async fn get_token(req: HttpRequest) -> Result<HttpResponse> {
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

pub async fn proxy_challenge(req: HttpRequest) -> Result<HttpResponse> {
    let host = match req.connection_info().host() {
        host if host.contains(':') => host.to_string(),
        host => host.to_string()
    };

    let request_url = format!("{}/v2/", crate::DOCKER_REGISTRY_URL);
    
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
