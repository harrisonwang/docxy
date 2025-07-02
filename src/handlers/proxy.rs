use actix_web::{web, HttpRequest, HttpResponse, Result};
use futures::stream::StreamExt;
use log::{info, error};

use crate::{DOCKER_REGISTRY_URL, HTTP_CLIENT};

pub async fn handle_request(
    req: HttpRequest,
    path: web::Path<(String, String, String)>,
) -> Result<HttpResponse> {
    // 获取路径参数
    let (image_name, path_type, reference) = path.into_inner();

    // 使用常量构建目标URL
    let path = format!("/v2/{image_name}/{path_type}/{reference}");
    
    // 构建请求，根据原始请求的方法选择 HEAD 或 GET
    let target_url = format!("{DOCKER_REGISTRY_URL}{path}");
    let mut request_builder = if req.method() == actix_web::http::Method::HEAD {
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
                .body(format!("无法连接到 Docker Registry: {e}")))
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
    if req.method() == actix_web::http::Method::HEAD {
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
