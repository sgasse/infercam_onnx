use std::{net::SocketAddr, sync::Arc};

use axum::{
    routing::{get, post},
    Extension, Router,
};
use env_logger::TimestampPrecision;
use infer_server::{
    data_socket::spawn_data_socket,
    endpoints::{face_stream, healthcheck, named_stream, recv_named_jpg_streams},
    inferer::InferBroker,
    pubsub::NamedPubSub,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Let's get started!");

    env_logger::builder()
        .format_timestamp(Some(TimestampPrecision::Millis))
        .init();

    let pubsub = Arc::new(NamedPubSub::new());

    let inferer = Arc::new(InferBroker::new().await);

    let inferer_ = Arc::clone(&inferer);
    let handle_inferer = tokio::spawn(async move {
        loop {
            inferer_.run().await;
        }
    });

    let handle = spawn_data_socket(pubsub.clone()).await;

    let app = Router::new()
        .route("/healthcheck", get(healthcheck))
        .route("/stream", get(named_stream))
        .route("/face_stream", get(face_stream))
        .route("/post_jpgs", post(recv_named_jpg_streams))
        .layer(Extension(pubsub))
        .layer(Extension(inferer));

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();

    Ok(())
}
