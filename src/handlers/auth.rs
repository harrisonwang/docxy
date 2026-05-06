use actix_web::{HttpRequest, HttpResponse, Result, web};
use log::{error, info};

use crate::error::AppError;
use crate::{AppState, HTTP_CLIENT, RegistryTarget};

fn process_scope_parameter(scope: &str, auto_library_prefix: bool) -> String {
    if !auto_library_prefix {
        return scope.to_string();
    }

    let parts: Vec<&str> = scope.split(':').collect();
    if parts.len() == 3 && parts[0] == "repository" {
        let image_name = parts[1];
        let action = parts[2];
        let processed_image_name = if !image_name.contains('/') {
            format!("library/{}", image_name)
        } else {
            image_name.to_string()
        };

        format!("repository:{}:{}", processed_image_name, action)
    } else {
        scope.to_string()
    }
}

fn query_pairs(query_string: &str) -> Result<Vec<(String, String)>, AppError> {
    let url = reqwest::Url::parse(&format!("http://localhost/?{query_string}"))
        .map_err(|_| AppError::InvalidRequest("invalid query parameters".to_string()))?;

    Ok(url
        .query_pairs()
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect())
}

fn bearer_param(auth_header: &str, param_name: &str) -> Option<String> {
    let mut input = auth_header.trim().strip_prefix("Bearer ")?.trim();

    while !input.is_empty() {
        input = input.trim_start_matches(|ch: char| ch == ',' || ch.is_ascii_whitespace());
        let (key, rest) = input.split_once('=')?;
        let key = key.trim();
        let rest = rest.trim_start();

        let (value, remaining) = if let Some(rest) = rest.strip_prefix('"') {
            let mut value = String::new();
            let mut escaped = false;
            let mut end_index = None;

            for (index, ch) in rest.char_indices() {
                if escaped {
                    value.push(ch);
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == '"' {
                    end_index = Some(index + ch.len_utf8());
                    break;
                } else {
                    value.push(ch);
                }
            }

            let end_index = end_index?;
            (value, &rest[end_index..])
        } else {
            let (value, remaining) = rest.split_once(',').unwrap_or((rest, ""));
            (value.trim().to_string(), remaining)
        };

        if key.eq_ignore_ascii_case(param_name) {
            return Some(value);
        }

        input = remaining;
    }

    None
}

fn escape_auth_param(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

pub(crate) fn build_auth_challenge(
    registry: &RegistryTarget,
    upstream_challenge: Option<&str>,
) -> String {
    let mut challenge = format!(
        "Bearer realm=\"{}/auth/{}/token\",service=\"{}\"",
        registry.public_base_url,
        registry.name,
        escape_auth_param(&registry.auth_service)
    );

    if let Some(scope) = upstream_challenge.and_then(|header| bearer_param(header, "scope")) {
        challenge.push_str(&format!(",scope=\"{}\"", escape_auth_param(&scope)));
    }

    challenge
}

fn resolve_registry(req: &HttpRequest, app_state: &AppState) -> Result<RegistryTarget, AppError> {
    match req.match_info().get("registry") {
        Some(registry_name) => app_state.registry_by_name(registry_name).ok_or_else(|| {
            AppError::InvalidRequest(format!("unknown registry target: {registry_name}"))
        }),
        None => Ok(app_state.registry_for_request(req)),
    }
}

pub async fn get_token(req: HttpRequest) -> Result<HttpResponse, AppError> {
    let app_state = req.app_data::<web::Data<AppState>>().unwrap();
    let registry = resolve_registry(&req, app_state)?;

    let mut auth_url = reqwest::Url::parse(&registry.auth_realm).map_err(|e| {
        AppError::InvalidRequest(format!("invalid auth realm for {}: {e}", registry.name))
    })?;
    {
        let mut upstream_query = auth_url.query_pairs_mut();
        upstream_query.append_pair("service", &registry.auth_service);

        for (key, value) in query_pairs(req.query_string())? {
            if key.eq_ignore_ascii_case("service") {
                continue;
            }

            if key.eq_ignore_ascii_case("scope") {
                let processed_scope = process_scope_parameter(&value, registry.auto_library_prefix);
                upstream_query.append_pair(&key, &processed_scope);
            } else {
                upstream_query.append_pair(&key, &value);
            }
        }
    }

    info!(
        "forwarding token request for registry {} to {}",
        registry.name, auth_url
    );

    let mut request_builder = HTTP_CLIENT.get(auth_url.clone());

    if let Some(auth_header) = req.headers().get("Authorization")
        && let Ok(auth_str) = auth_header.to_str()
    {
        request_builder = request_builder.header("Authorization", auth_str);
    }

    let response = match request_builder.send().await {
        Ok(resp) => {
            info!(
                "GET {} {:?} {} {}",
                auth_url,
                req.version(),
                resp.status().as_u16(),
                resp.status().canonical_reason().unwrap_or("Unknown")
            );
            resp
        }
        Err(e) => {
            error!("GET {} {:?} failed: {}", auth_url, req.version(), e);
            return Ok(HttpResponse::InternalServerError()
                .body(format!("unable to connect to registry auth service: {e}")));
        }
    };

    let status = response.status();
    let mut builder =
        HttpResponse::build(actix_web::http::StatusCode::from_u16(status.as_u16()).unwrap());

    for (name, value) in response.headers() {
        if let Ok(value_str) = value.to_str() {
            builder.append_header((name.as_str(), value_str));
        }
    }

    match response.bytes().await {
        Ok(bytes) => {
            info!(
                "{} {} {:?} {} {}",
                req.method(),
                req.uri(),
                req.version(),
                status.as_u16(),
                status.canonical_reason().unwrap_or("Unknown")
            );
            Ok(builder.body(bytes))
        }
        Err(e) => {
            error!("failed reading auth service response: {}", e);
            Ok(HttpResponse::InternalServerError()
                .body("unable to read registry auth service response"))
        }
    }
}

pub async fn proxy_challenge(req: HttpRequest) -> Result<HttpResponse, AppError> {
    let app_state = req.app_data::<web::Data<AppState>>().unwrap();
    let registry = app_state.registry_for_request(&req);
    let request_url = format!("{}/v2/", registry.upstream_registry);

    let mut request_builder = HTTP_CLIENT.get(&request_url);

    if let Some(auth) = req.headers().get("Authorization")
        && let Ok(auth_str) = auth.to_str()
    {
        request_builder = request_builder.header("Authorization", auth_str);
    }

    let response = match request_builder.send().await {
        Ok(resp) => {
            info!(
                "GET {} {:?} {} {}",
                request_url,
                req.version(),
                resp.status().as_u16(),
                resp.status().canonical_reason().unwrap_or("Unknown")
            );
            resp
        }
        Err(e) => {
            error!("GET {} {:?} failed: {}", request_url, req.version(), e);
            return Ok(HttpResponse::InternalServerError()
                .body(format!("unable to connect to upstream registry: {e}")));
        }
    };

    let status = response.status().as_u16();
    let mut builder = HttpResponse::build(actix_web::http::StatusCode::from_u16(status).unwrap());

    if status == 401 {
        builder.append_header(("WWW-Authenticate", build_auth_challenge(&registry, None)));
    }

    let body = match response.text().await {
        Ok(text) => text,
        Err(e) => {
            error!("failed reading upstream registry response: {}", e);
            String::from("unable to read upstream registry response")
        }
    };

    info!(
        "{} {} {:?} {} {}",
        req.method(),
        req.uri(),
        req.version(),
        status,
        actix_web::http::StatusCode::from_u16(status)
            .unwrap()
            .canonical_reason()
            .unwrap_or("Unknown")
    );

    Ok(builder.body(body))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_registry(auto_library_prefix: bool) -> RegistryTarget {
        RegistryTarget {
            name: "dockerhub".to_string(),
            hosts: vec![],
            upstream_registry: "https://registry-1.docker.io".to_string(),
            auth_realm: "https://auth.docker.io/token".to_string(),
            auth_service: "registry.docker.io".to_string(),
            auto_library_prefix,
            public_base_url: "https://proxy.example.com".to_string(),
        }
    }

    #[test]
    fn docker_hub_scope_adds_library_prefix() {
        assert_eq!(
            process_scope_parameter("repository:alpine:pull", true),
            "repository:library/alpine:pull"
        );
        assert_eq!(
            process_scope_parameter("repository:library/alpine:pull", true),
            "repository:library/alpine:pull"
        );
    }

    #[test]
    fn non_docker_hub_scope_is_unchanged() {
        assert_eq!(
            process_scope_parameter("repository:owner/image:pull", false),
            "repository:owner/image:pull"
        );
    }

    #[test]
    fn builds_registry_specific_auth_challenge() {
        let mut registry = test_registry(false);
        registry.name = "ghcr".to_string();
        registry.auth_service = "ghcr.io".to_string();
        registry.public_base_url = "https://ghcr.example.com".to_string();

        assert_eq!(
            build_auth_challenge(
                &registry,
                Some(
                    "Bearer realm=\"https://ghcr.io/token\",service=\"ghcr.io\",scope=\"repository:owner/image:pull\""
                )
            ),
            "Bearer realm=\"https://ghcr.example.com/auth/ghcr/token\",service=\"ghcr.io\",scope=\"repository:owner/image:pull\""
        );
    }

    #[test]
    fn parses_quoted_bearer_param() {
        assert_eq!(
            bearer_param(
                "Bearer realm=\"https://quay.io/v2/auth\",service=\"quay.io\",scope=\"repository:org/image:pull\"",
                "scope"
            ),
            Some("repository:org/image:pull".to_string())
        );
    }
}
