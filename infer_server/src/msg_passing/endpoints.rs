//! Endpoints of HTTP server.
//!
use std::sync::Arc;

use axum::{body::StreamBody, extract::Query, http::header, response::IntoResponse, Extension};
use futures::StreamExt;
use serde::Deserialize;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use crate::{meter::METER, msg_passing::router::RegistryComm};

/// Search parameters available to streams.
#[derive(Debug, Deserialize)]
pub struct StreamParams {
    #[serde(default)]
    name: Option<String>,
}

/// Health check endpoint.
pub async fn healthcheck() -> &'static str {
    "healthy"
}

// Endpoint of received image streams.
pub async fn named_stream(
    Extension(registry): Extension<Arc<RegistryComm>>,
    Query(params): Query<StreamParams>,
) -> impl IntoResponse {
    let name = params.name.unwrap_or_else(|| "unknown".into());
    log::info!("Stream for {} requested", &name);

    let (tx, rx) = mpsc::channel(20);
    registry
        .frame_stream_listener_tx
        .send((name, tx))
        .await
        .ok();

    let stream = ReceiverStream::new(rx).map(|x| {
        METER.tick_raw();
        Ok::<_, String>(x)
    });

    // Set body and headers for multipart streaming
    let body = StreamBody::new(stream);
    let headers = [(
        header::CONTENT_TYPE,
        "multipart/x-mixed-replace; boundary=frame",
    )];

    (headers, body)
}
