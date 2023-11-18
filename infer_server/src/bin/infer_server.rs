//! Infer server binary.
//!
use std::{net::SocketAddr, sync::Arc};

use anyhow::Result;
use axum::{routing::get, Extension, Router};
use clap::Parser;
use env_logger::TimestampPrecision;
use infer_server::{
    data_socket::spawn_data_socket,
    endpoints::{faces_stream, healthcheck, named_stream},
    inferer::Inferer,
    meter::spawn_meter_logger,
    router::FrameRouter,
    INCOMING_FRAMES_CHANNEL, INFER_IMAGES_CHANNEL,
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
async fn main() -> Result<()> {
    let args = Args::parse();

    // Setup logger
    env_logger::builder()
        .format_timestamp(Some(TimestampPrecision::Millis))
        .init();

    let (incoming_tx, incoming_rx) = INCOMING_FRAMES_CHANNEL.split();
    let (infer_tx, infer_rx) = INFER_IMAGES_CHANNEL.split();
    let frame_router = Arc::new(FrameRouter::new(infer_tx));

    {
        let frame_router = frame_router.clone();
        tokio::spawn(async move { frame_router.run(incoming_rx).await });
    }

    {
        tokio::spawn(async move { Inferer::new(infer_rx).await.run().await });
    }

    // Create socket to receive image streams via network
    spawn_data_socket(incoming_tx, &args.socket_address).await?;

    spawn_meter_logger();

    // Build HTTP server with endpoints
    let app = Router::new()
        .route("/healthcheck", get(healthcheck))
        .route("/stream", get(named_stream))
        .route("/face_stream", get(faces_stream))
        .layer(Extension(frame_router));

    // Serve HTTP server
    let addr: SocketAddr = args.server_address.parse()?;
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}
