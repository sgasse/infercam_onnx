use std::{net::SocketAddr, sync::Arc};

use axum::{
    routing::{get, post},
    Extension, Router,
};
use env_logger::TimestampPrecision;
use infer_server::{
    data_socket::spawn_data_socket,
    endpoints::{healthcheck, named_stream, recv_named_jpg_streams},
    nn::UltrafaceModel,
    pubsub::NamedPubSub,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Let's get started!");

    env_logger::builder()
        .format_timestamp(Some(TimestampPrecision::Millis))
        .init();

    // let model = UltrafaceModel::new(infer_server::nn::UltrafaceVariant::W320H240)
    //     .await
    //     .expect("Initialize model");
    let pubsub = Arc::new(NamedPubSub::new());

    let handle = spawn_data_socket(pubsub.clone()).await;

    let app = Router::new()
        .route("/healthcheck", get(healthcheck))
        .route("/stream", get(named_stream))
        .route("/post_jpgs", post(recv_named_jpg_streams))
        .layer(Extension(pubsub));

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();

    Ok(())
}
