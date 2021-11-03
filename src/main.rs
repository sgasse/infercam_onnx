use actix_web::App;
use actix_web::HttpServer;
use libwebcam_onnx::routes::{face_detection, index, video_stream};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    HttpServer::new(|| {
        App::new()
            .service(index)
            .service(video_stream)
            .service(face_detection)
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
