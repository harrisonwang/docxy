use actix_web::{HttpRequest, HttpResponse};
use log::{info, warn};

// 处理非法请求的函数
pub async fn handle_invalid_request(req: HttpRequest) -> HttpResponse {
    let path = req.uri().path();
    warn!("拦截非法请求: {} {}", req.method(), path);
    
    HttpResponse::Forbidden()
        .content_type("text/plain; charset=utf-8")
        .body("非法访问路径")
}

// 新增HTTP到HTTPS的重定向处理函数
pub async fn redirect_to_https(req: HttpRequest) -> HttpResponse {
    let host = req.connection_info().host().split(':').next().unwrap_or("").to_string();
    let uri = req.uri().to_string();
    
    // 构建重定向URL (HTTP -> HTTPS)
    let redirect_url = format!("https://{host}{uri}");
    
    info!("接收请求: \"{} {} HTTP/{:?}\" 301 Moved Permanently", 
        req.method(), 
        req.uri(), 
        req.version());
    
    info!("重定向到: {}", redirect_url);
    
    HttpResponse::MovedPermanently()
        .append_header(("Location", redirect_url))
        .finish()
}
