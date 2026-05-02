use actix_web::{HttpRequest, HttpResponse, Result, http::header, web};
use futures::stream::{StreamExt, empty};
use log::{error, info};

use super::auth::build_auth_challenge;
use crate::HTTP_CLIENT;
use crate::error::AppError;
use crate::{AppState, RegistryTarget};

fn is_hop_by_hop_header(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
    )
}

fn should_forward_response_header(name: &str) -> bool {
    !is_hop_by_hop_header(name) && !name.eq_ignore_ascii_case("content-length")
}

fn header_value<'a>(headers: &'a [(String, String)], header_name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(header_name))
        .map(|(_, value)| value.as_str())
}

fn parse_content_length(headers: &[(String, String)]) -> Option<u64> {
    header_value(headers, "content-length")?
        .trim()
        .parse::<u64>()
        .ok()
}

fn parse_content_range_total(value: &str) -> Option<u64> {
    let value = value.trim();
    let range = value
        .strip_prefix("bytes ")
        .or_else(|| value.strip_prefix("Bytes "))?;
    let (_, total) = range.rsplit_once('/')?;
    let total = total.trim();
    if total == "*" {
        None
    } else {
        total.parse::<u64>().ok()
    }
}

fn effective_content_length(
    response: &reqwest::Response,
    headers: &[(String, String)],
) -> Option<u64> {
    parse_content_length(headers).or_else(|| response.content_length())
}

fn append_response_headers(
    builder: &mut actix_web::HttpResponseBuilder,
    response_headers: Vec<(String, String)>,
    registry: &RegistryTarget,
) {
    for (name, value) in response_headers {
        if should_forward_response_header(&name) {
            if name.eq_ignore_ascii_case("www-authenticate") {
                builder.append_header((
                    "WWW-Authenticate",
                    build_auth_challenge(registry, Some(&value)),
                ));
            } else {
                builder.append_header((name, value));
            }
        }
    }
}

fn append_forward_headers(
    mut request_builder: reqwest::RequestBuilder,
    req: &HttpRequest,
) -> reqwest::RequestBuilder {
    // Docker pull 在断点/重试场景会大量依赖 Range。
    // 若不透传，httpReadSeeker 可能拿不到预期分片，导致后续拉层失败。
    const FORWARDED_HEADERS: [&str; 8] = [
        "Authorization",
        "Accept",
        "Range",
        "If-Range",
        "If-Match",
        "If-None-Match",
        "If-Modified-Since",
        "If-Unmodified-Since",
    ];

    for header_name in FORWARDED_HEADERS {
        for value in req.headers().get_all(header_name) {
            if let Ok(value_str) = value.to_str() {
                request_builder = request_builder.header(header_name, value_str);
            }
        }
    }

    request_builder
}

async fn probe_blob_content_length(target_url: &str, req: &HttpRequest) -> Option<u64> {
    let request_builder =
        append_forward_headers(HTTP_CLIENT.get(target_url), req).header("Range", "bytes=0-0");

    let response = match request_builder.send().await {
        Ok(resp) => resp,
        Err(e) => {
            error!("GET {} Range bytes=0-0 失败: {}", target_url, e);
            return None;
        }
    };

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

    let length_from_range = header_value(&response_headers, "content-range")
        .and_then(parse_content_range_total)
        .filter(|content_length| *content_length > 0);

    if length_from_range.is_some() {
        return length_from_range;
    }

    effective_content_length(&response, &response_headers)
        .filter(|content_length| *content_length > 0)
}

pub async fn handle_request(
    req: HttpRequest,
    path: web::Path<(String, String, String)>,
) -> Result<HttpResponse, AppError> {
    let app_state = req.app_data::<web::Data<AppState>>().unwrap();
    let registry = app_state.registry_for_request(&req);
    let upstream_registry = registry.upstream_registry.as_str();
    let (image_name, path_type, reference) = path.into_inner();

    let processed_image_name = if registry.auto_library_prefix && !image_name.contains('/') {
        format!("library/{}", image_name)
    } else {
        image_name
    };

    let is_manifest_request = path_type == "manifests";
    let is_blob_request = path_type == "blobs";
    let client_is_head = req.method() == actix_web::http::Method::HEAD;
    // Docker 29 对 manifest descriptor 的 size 校验更严格。
    // 对客户端 HEAD manifest 请求，直接对上游发 GET，更稳定地拿到 manifest 元信息。
    let use_get_for_upstream = client_is_head && is_manifest_request;

    let mut path = format!("/v2/{processed_image_name}/{path_type}/{reference}");
    if let Some(query) = req.uri().query() {
        path.push('?');
        path.push_str(query);
    }
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
        let mut effective_content_length = effective_content_length(&response, &response_headers);

        if is_blob_request && status.is_success() && effective_content_length.unwrap_or(0) == 0 {
            effective_content_length = probe_blob_content_length(&target_url, &req).await;
            if let Some(content_length) = effective_content_length {
                info!(
                    "blob HEAD 响应缺少有效 Content-Length，已通过 Range 探测补齐为 {}",
                    content_length
                );
            } else {
                info!("blob HEAD 响应缺少有效 Content-Length，Range 探测也未能获取总长度");
            }
        }

        // 不在代理层读取整个 manifest 来手动计算长度，避免额外复杂度和开销。
        // 我们仅使用上游返回的 Content-Length；若上游缺失该头，则不在此处强行补算。
        if is_manifest_request && status.is_success() && effective_content_length.unwrap_or(0) == 0
        {
            info!("manifest 响应缺少有效 Content-Length，保持上游语义透传，不进行代理侧补算");
        }

        append_response_headers(&mut builder, response_headers, &registry);

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
    let effective_content_length = effective_content_length(&response, &response_headers);

    append_response_headers(&mut builder, response_headers, &registry);

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_content_range_total() {
        assert_eq!(parse_content_range_total("bytes 0-0/123456"), Some(123456));
        assert_eq!(
            parse_content_range_total("bytes 1024-2047/4096"),
            Some(4096)
        );
        assert_eq!(parse_content_range_total("bytes */4096"), Some(4096));
        assert_eq!(parse_content_range_total("bytes 0-0/*"), None);
        assert_eq!(parse_content_range_total("invalid"), None);
    }
}
