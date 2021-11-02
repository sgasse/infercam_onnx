use actix_web::App;
use actix_web::HttpServer;
use libwebcam_onnx::routes::{index, video};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| App::new().service(video))
        .bind("127.0.0.1:8080")?
        .run()
        .await
}
