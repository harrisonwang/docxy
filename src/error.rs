use actix_web::{http::StatusCode, HttpResponse, ResponseError};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Upstream registry request failed")]
    UpstreamRequest(#[from] reqwest::Error),

    #[error("TLS configuration failed to load: {0}")]
    TlsConfig(String),

    #[error("Rustls error")]
    Rustls(#[from] rustls::Error),

    #[error("Invalid client request: {0}")]
    InvalidRequest(String),

    #[error("I/O error")]
    Io(#[from] std::io::Error),
}

impl ResponseError for AppError {
    fn status_code(&self) -> StatusCode {
        match self {
            AppError::UpstreamRequest(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::TlsConfig(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::InvalidRequest(_) => StatusCode::BAD_REQUEST,
            AppError::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AppError::Rustls(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code())
            .content_type("text/plain; charset=utf-8")
            .body(self.to_string())
    }
}
