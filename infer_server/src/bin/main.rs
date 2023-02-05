//! Infer server binary.
//!
use std::{net::SocketAddr, sync::Arc};

use axum::{routing::get, Extension, Router};
use clap::Parser;
use env_logger::TimestampPrecision;
use infer_server::{
    data_socket::spawn_data_socket,
    endpoints::{face_stream, healthcheck, named_stream},
    inferer::InferBroker,
    pubsub::NamedPubSub,
    Error,
};

#[derive(Parser, Debug)]
#[clap(author, version)]
struct Args {
    /// Address of the infer server to connect to
    #[clap(long, default_value = "127.0.0.1:3000")]
    server_address: String,

    /// Address of the infer server to connect to
    #[clap(long, default_value = "127.0.0.1:3001")]
    socket_address: String,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let args = Args::parse();

    // Setup logger
    env_logger::builder()
        .format_timestamp(Some(TimestampPrecision::Millis))
        .init();

    // Build Pub/Sub-Engine to communicate between data input, inference and serving via HTTP
    let pubsub = Arc::new(NamedPubSub::new());

    // Build inferer to determine faces with confidences on image streams
    let inferer = Arc::new(InferBroker::new(Arc::clone(&pubsub)).await);

    // Spawn separate task to run the inference on
    let inferer_ = Arc::clone(&inferer);
    tokio::spawn(async move {
        loop {
            inferer_.run().await;
        }
    });

    // Create socket to receive image streams via network
    spawn_data_socket(pubsub.clone(), &args.socket_address).await?;

    // Build HTTP server with endpoints
    let app = Router::new()
        .route("/healthcheck", get(healthcheck))
        .route("/stream", get(named_stream))
        .route("/face_stream", get(face_stream))
        .layer(Extension(pubsub))
        .layer(Extension(inferer));

    // Serve HTTP server
    let addr: SocketAddr = args.server_address.parse()?;
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}
