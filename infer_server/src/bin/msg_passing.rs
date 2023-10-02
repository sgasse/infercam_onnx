//! Infer server binary.
//!
use std::{net::SocketAddr, sync::Arc};

use anyhow::Result;
use axum::{routing::get, Extension, Router};
use clap::Parser;
use env_logger::TimestampPrecision;
use infer_server::{
    meter::spawn_meter_logger,
    msg_passing::{
        data_socket::spawn_data_socket,
        endpoints::{healthcheck, named_stream},
        router::Registry,
    },
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

    let mut registry = Registry::new();
    let comm = registry.get_comm();

    tokio::spawn(async move { registry.run().await });

    // Create socket to receive image streams via network
    spawn_data_socket(comm.tcp_tasks_comm_tx.clone(), &args.socket_address).await?;

    spawn_meter_logger();

    // Build HTTP server with endpoints
    let app = Router::new()
        .route("/healthcheck", get(healthcheck))
        .route("/stream", get(named_stream))
        .layer(Extension(Arc::new(comm)));

    // Serve HTTP server
    let addr: SocketAddr = args.server_address.parse()?;
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}
