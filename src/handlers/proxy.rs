use actix_web::{HttpRequest, HttpResponse, Result, http::header, web};
use futures::stream::{StreamExt, empty};
use log::{error, info};

use crate::AppState;
use crate::HTTP_CLIENT;
use crate::error::AppError;

fn append_forward_headers(
    mut request_builder: reqwest::RequestBuilder,
    req: &HttpRequest,
) -> reqwest::RequestBuilder {
    if let Some(auth) = req.headers().get("Authorization") {
        if let Ok(auth_str) = auth.to_str() {
            request_builder = request_builder.header("Authorization", auth_str);
        }
    }

    for accept in req.headers().get_all("Accept") {
        if let Ok(accept_str) = accept.to_str() {
            request_builder = request_builder.header("Accept", accept_str);
        }
    }

    request_builder
}

pub async fn handle_request(
    req: HttpRequest,
    path: web::Path<(String, String, String)>,
) -> Result<HttpResponse, AppError> {
    let app_state = req.app_data::<web::Data<AppState>>().unwrap();
    let upstream_registry = app_state.upstream_registry.as_str();
    let (image_name, path_type, reference) = path.into_inner();

    let processed_image_name = if !image_name.contains('/') {
        format!("library/{}", image_name)
    } else {
        image_name
    };

    let is_manifest_request = path_type == "manifests";
    let client_is_head = req.method() == actix_web::http::Method::HEAD;
    // Docker 29 对 manifest descriptor 的 size 校验更严格。
    // 对客户端 HEAD manifest 请求，直接对上游发 GET，更稳定地拿到 manifest 元信息。
    let use_get_for_upstream = client_is_head && is_manifest_request;

    let path = format!("/v2/{processed_image_name}/{path_type}/{reference}");
    let target_url = format!("{upstream_registry}{path}");

    let request_builder = if use_get_for_upstream {
        HTTP_CLIENT.get(&target_url)
    } else if client_is_head {
        HTTP_CLIENT.head(&target_url)
    } else {
        HTTP_CLIENT.get(&target_url)
    };
    let request_builder = append_forward_headers(request_builder, &req);

    let method_for_log = if use_get_for_upstream {
        "GET"
    } else {
        req.method().as_str()
    };
    let response = match request_builder.send().await {
        Ok(resp) => {
            info!(
                "{} {} {:?} {} {}",
                method_for_log,
                target_url,
                req.version(),
                resp.status().as_u16(),
                resp.status().canonical_reason().unwrap_or("Unknown")
            );
            resp
        }
        Err(e) => {
            error!(
                "{} {} {:?} 失败: {}",
                method_for_log,
                target_url,
                req.version(),
                e
            );
            return Ok(HttpResponse::InternalServerError()
                .body(format!("无法连接到 Docker Registry: {e}")));
        }
    };

    let status = response.status();
    let response_headers: Vec<(String, String)> = response
        .headers()
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|value_str| (name.as_str().to_string(), value_str.to_string()))
        })
        .collect();

    if client_is_head {
        let mut builder =
            HttpResponse::build(actix_web::http::StatusCode::from_u16(status.as_u16()).unwrap());
        let mut effective_content_length = response.content_length();

        // 极端情况下上游 GET 也未提供 Content-Length，且我们正要返回 HEAD。
        // 此时读取 manifest body 仅用于计算长度，不会把 body 返回给客户端。
        if is_manifest_request && status.is_success() && effective_content_length.unwrap_or(0) == 0
        {
            match response.bytes().await {
                Ok(bytes) => {
                    let computed_len = bytes.len() as u64;
                    if computed_len > 0 {
                        effective_content_length = Some(computed_len);
                        info!(
                            "manifest Content-Length 缺失或为 0，已通过上游 GET 计算长度: {}",
                            computed_len
                        );
                    }
                }
                Err(e) => {
                    error!("读取 manifest 内容用于长度计算失败: {}", e);
                }
            }
        }

        for (name, value) in response_headers {
            if name.eq_ignore_ascii_case("content-length") {
                continue;
            }
            builder.append_header((name, value));
        }

        if let Some(content_length) = effective_content_length {
            builder.insert_header((header::CONTENT_LENGTH, content_length.to_string()));
            builder.no_chunking(content_length);
        }

        info!(
            "{} {} {:?} {} {}",
            req.method(),
            req.uri(),
            req.version(),
            status.as_u16(),
            status.canonical_reason().unwrap_or("Unknown")
        );

        let empty_stream = empty::<Result<web::Bytes, actix_web::Error>>();
        return Ok(builder.streaming(empty_stream));
    }

    let mut builder =
        HttpResponse::build(actix_web::http::StatusCode::from_u16(status.as_u16()).unwrap());
    for (name, value) in response_headers {
        builder.append_header((name, value));
    }

    info!(
        "{} {} {:?} {} {}",
        req.method(),
        req.uri(),
        req.version(),
        status.as_u16(),
        status.canonical_reason().unwrap_or("Unknown")
    );

    let stream = response.bytes_stream().map(|result| {
        result.map_err(|err| {
            let error_msg = err.to_string();
            if error_msg.contains("timeout") || error_msg.contains("deadline") {
                error!("流读取超时错误 (可能是镜像过大): {}", err);
            } else {
                error!("流读取错误: {}", err);
            }
            actix_web::error::ErrorInternalServerError(format!("数据传输错误: {}", err))
        })
    });

    Ok(builder.streaming(stream))
}
