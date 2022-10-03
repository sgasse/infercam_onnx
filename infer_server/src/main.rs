use std::net::SocketAddr;

use axum::{
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;

#[tokio::main]
async fn main() {
    println!("Let's get started!");

    let app = Router::new()
        .route("/healthcheck", get(healthcheck))
        .route("/post_frame", post(post_frames));

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
