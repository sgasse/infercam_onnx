use axum::{
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use env_logger::TimestampPrecision;
use infer_server::endpoints::recv_jpgs;
use infer_server::nn::UltrafaceModel;
use serde::Deserialize;
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    println!("Let's get started!");

    env_logger::builder()
        .format_timestamp(Some(TimestampPrecision::Millis))
        .init();

    // let model = UltrafaceModel::new(infer_server::nn::UltrafaceVariant::W320H240)
    //     .await
    //     .expect("Initialize model");

    let app = Router::new()
        .route("/healthcheck", get(healthcheck))
        .route("/post_frame", post(post_frames))
        .route("/post_jpgs", post(recv_jpgs));

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn healthcheck() -> &'static str {
    "Healthy"
}

async fn post_frames(Json(payload): Json<Frame>) -> impl IntoResponse {
    println!("Got payload {:?}", payload.name);
    StatusCode::ACCEPTED
}

#[derive(Deserialize)]
struct Frame {
    name: String,
    data: Vec<u8>,
}

#[cfg(test)]
mod test {

    #[test]
    fn test_identifying_parts() {
        let id = b"--blabla\r\n";
        assert!(id.ends_with("\r\n".as_bytes()));
    }
}
