//! Endpoints of HTTP server.
//!
use std::sync::Arc;

use axum::{body::StreamBody, extract::Query, http::header, response::IntoResponse, Extension};
use futures::StreamExt;
use serde::Deserialize;
use tokio_stream::wrappers::BroadcastStream;

use crate::{hour_glass::router::FrameRouter, meter::METER};

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
    Extension(frame_router): Extension<Arc<FrameRouter>>,
    Query(params): Query<StreamParams>,
) -> impl IntoResponse {
    let name = params.name.unwrap_or_else(|| "unknown".into());
    log::info!("Stream for {} requested", &name);

    // Subscribe to a broadcasted received image stream.
    let rx = frame_router.get_broadcast_receiver(&name);

    let stream = BroadcastStream::from(rx).map(|x| {
        METER.tick_raw();
        x
    });

    // Set body and headers for multipart streaming
    let body = StreamBody::new(stream);
    let headers = [(
        header::CONTENT_TYPE,
        "multipart/x-mixed-replace; boundary=frame",
    )];

    (headers, body)
}

pub async fn faces_stream(
    Extension(frame_router): Extension<Arc<FrameRouter>>,
    Query(params): Query<StreamParams>,
) -> impl IntoResponse {
    let name = params.name.unwrap_or_else(|| "unknown".into());
    log::info!("Infered stream for {} requested", &name);

    // Subscribe to a broadcasted received image stream.
    let rx = frame_router.get_infered_receiver(&name);

    let stream = BroadcastStream::from(rx).map(|x| {
        METER.tick_infered();
        x
    });

    // Set body and headers for multipart streaming
    let body = StreamBody::new(stream);
    let headers = [(
        header::CONTENT_TYPE,
        "multipart/x-mixed-replace; boundary=frame",
    )];

    (headers, body)
}
