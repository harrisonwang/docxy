use actix_web::{HttpRequest, HttpResponse, Responder};
use log::info;

pub async fn health_check(req: HttpRequest) -> impl Responder {
    info!("{} {} {:?} 200 OK", 
        req.method(), 
        req.uri(), 
        req.version());
        
    HttpResponse::Ok()
        .content_type("text/plain; charset=utf-8")
        .body("服务正常运行\n")
}
