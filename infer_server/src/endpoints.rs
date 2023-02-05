//! Endpoints of HTTP server.
//!
use std::sync::Arc;

use axum::{body::StreamBody, extract::Query, http::header, response::IntoResponse, Extension};
use bytes::Bytes;
use serde::Deserialize;

use crate::{inferer::InferBroker, pubsub::NamedPubSub};

/// Search parameters available to streams.
#[derive(Debug, Deserialize)]
pub struct StreamParams {
    #[serde(default)]
    name: Option<String>,
}

/// Health check endpoint.
pub async fn healthcheck() -> &'static str {
    "Healthy"
}

/// Endpoint of received image streams with faces+confidences infered.
pub async fn face_stream(
    Extension(pubsub): Extension<Arc<NamedPubSub>>,
    Extension(inferer): Extension<Arc<InferBroker>>,
    Query(params): Query<StreamParams>,
) -> Result<impl IntoResponse, String> {
    let name = params.name.unwrap_or_else(|| "unknown".into());
    log::info!("Face stream for {} requested", &name);

    // Subscribe to an infered image stream.
    // If there is already at least one client connected which receives the stream with the same
    // name, this will only clone the receiving end of the respective broadcast channel thus the
    // inference is only done once regardless of how many people subscribe in parallel.
    // If this client is the first to request the stream, the inferer will request the receiving
    // end of the MPSC channel of this name and add it to the channels which it periodically checks
    // for new data and infers.
    if let Ok(mut infered_rx) = inferer.subscribe_img_stream(&name, &pubsub).await {
        let stream = async_stream::stream! {
            while let Ok(item) = infered_rx.recv().await {
                // Wrap data with frame separator for multipart streaming
                let data: Bytes = Bytes::copy_from_slice(
                    &[
                        "--frame\r\nContent-Type: image/jpeg\r\n\r\n".as_bytes(),
                        &item[..],
                        "\r\n\r\n".as_bytes(),
                    ].concat()
                );
                yield Ok::<_, std::io::Error>(data);
            }

            log::info!("Exited stream for {}", &name);
        };

        // Set body and headers for multipart streaming
        let body = StreamBody::new(stream);
        let headers = [(
            header::CONTENT_TYPE,
            "multipart/x-mixed-replace; boundary=frame",
        )];

        return Ok((headers, body));
    }

    Err(format!("Could not setup face stream for {name}"))
}

/// Endpoint of received image streams.
pub async fn named_stream(
    Extension(pubsub): Extension<Arc<NamedPubSub>>,
    Query(params): Query<StreamParams>,
) -> impl IntoResponse {
    let name = params.name.unwrap_or_else(|| "unknown".into());
    log::info!("Stream for {} requested", &name);

    // Subscribe to a broadcasted received image stream.
    let mut rx = pubsub.get_broadcast_receiver(&name).await;

    let stream = async_stream::stream! {
        while let Ok(item) = rx.recv().await {
            // Wrap data with frame separator for multipart streaming
            let data: Bytes = Bytes::copy_from_slice(
                &[
                    "--frame\r\nContent-Type: image/jpeg\r\n\r\n".as_bytes(),
                    &item[..],
                    "\r\n\r\n".as_bytes(),
                ].concat()
            );
            yield Ok::<_, std::io::Error>(data);
        }
        log::info!("Exited stream for {}", &name);
    };

    // Set body and headers for multipart streaming
    let body = StreamBody::new(stream);
    let headers = [(
        header::CONTENT_TYPE,
        "multipart/x-mixed-replace; boundary=frame",
    )];

    (headers, body)
}
