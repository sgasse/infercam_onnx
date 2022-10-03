use actix_web::App;
use actix_web::HttpServer;
use env_logger::TimestampPrecision;
use libinfercam_onnx::routes::{face_detection, index, video_stream};
use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(name = "InferCam")]
struct Opts {
    /// Port on which to serve
    #[structopt(short, long, default_value = "8080")]
    port: u32,

    /// Bind to all IP addresses
    #[structopt(short, long)]
    bindall: bool,
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::builder()
        .format_timestamp(Some(TimestampPrecision::Millis))
        .init();

    let opts = Opts::from_args();
    let bind_ip = match opts.bindall {
        true => "0.0.0.0",
        false => "127.0.0.1",
    };

    HttpServer::new(|| {
        App::new()
            .service(index)
            .service(video_stream)
            .service(face_detection)
    })
    .bind(format!("{}:{}", bind_ip, opts.port))?
    .run()
    .await
}
