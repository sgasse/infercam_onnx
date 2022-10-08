use std::net::SocketAddr;

use axum::{
    extract::{BodyStream, Multipart},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use futures::stream::StreamExt;
use infer_server::nn::UltrafaceModel;
use serde::Deserialize;

#[tokio::main]
async fn main() {
    println!("Let's get started!");

    // let model = UltrafaceModel::new(infer_server::nn::UltrafaceVariant::W320H240)
    //     .await
    //     .expect("Initialize model");

    let app = Router::new()
        .route("/healthcheck", get(healthcheck))
        .route("/post_frame", post(post_frames))
        .route("/chunks", post(recv_stream2));

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

async fn recv_stream(mut multipart: Multipart) {
    while let Some(mut field) = multipart.next_field().await.unwrap() {
        let name = field.name().unwrap().to_string();
        let data = field.bytes().await.unwrap();

        dbg!(name);
        dbg!(data);
    }
}

async fn recv_stream2(mut stream: BodyStream) {
    while let Some(chunk) = stream.next().await {
        dbg!(chunk);
    }
}
