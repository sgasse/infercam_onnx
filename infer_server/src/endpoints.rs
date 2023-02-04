use std::{
    fs::File,
    io::{Cursor, Write},
    sync::Arc,
};

use axum::{
    body::StreamBody,
    extract::{BodyStream, Query},
    http::header,
    response::IntoResponse,
    Extension,
};
use bytes::Bytes;
use futures::stream::StreamExt;
use serde::Deserialize;

use crate::{inferer::InferBroker, pubsub::NamedPubSub};

#[derive(Debug, Deserialize)]
pub struct StreamParams {
    #[serde(default)]
    name: Option<String>,
}

pub async fn healthcheck() -> &'static str {
    "Healthy"
}

pub async fn face_stream(
    Extension(pubsub): Extension<Arc<NamedPubSub>>,
    Extension(inferer): Extension<Arc<InferBroker>>,
    Query(params): Query<StreamParams>,
) -> Result<impl IntoResponse, String> {
    let name = params.name.unwrap_or_else(|| "unknown".into());
    log::info!("Face stream for {} requested", &name);

    if let Ok(mut infered_rx) = inferer.subscribe_img_stream(&name, &pubsub).await {
        let stream = async_stream::stream! {
            while let Ok(item) = infered_rx.recv().await {
                // log::debug!("Next iteration in face stream");
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

        let body = StreamBody::new(stream);
        let headers = [(
            header::CONTENT_TYPE,
            "multipart/x-mixed-replace; boundary=frame",
        )];

        return Ok((headers, body));
    }

    Err(format!("Could not setup face stream for {name}"))
}

pub async fn named_stream(
    Extension(pubsub): Extension<Arc<NamedPubSub>>,
    Query(params): Query<StreamParams>,
) -> impl IntoResponse {
    let name = params.name.unwrap_or_else(|| "unknown".into());
    log::info!("Stream for {} requested", &name);

    let mut rx = pubsub.get_broadcast_receiver(&name).await;

    let stream = async_stream::stream! {
        while let Ok(item) = rx.recv().await {
            // log::debug!("Next iteration in video stream");
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

    let body = StreamBody::new(stream);
    let headers = [(
        header::CONTENT_TYPE,
        "multipart/x-mixed-replace; boundary=frame",
    )];

    (headers, body)
}

pub async fn recv_named_jpg_streams(
    Extension(pubsub): Extension<Arc<NamedPubSub>>,
    Query(params): Query<StreamParams>,
    mut stream: BodyStream,
) {
    let name = params.name.unwrap_or_else(|| "unknown".into());
    log::info!("Receiving stream for name {}", &name);
    let sender = pubsub.get_broadcast_sender(&name).await;

    let mut buf = Cursor::new(vec![0_u8; 200000]);
    while let Some(Ok(data)) = stream.next().await {
        log::debug!("Data length {}", data.len());
        match data.ends_with("\r\n".as_bytes()) {
            true => {
                log::debug!("Skipping header {:?}", data);
            }
            false => {
                // No header
                match data.ends_with("\n\n".as_bytes()) {
                    true => {
                        // The last two bytes are the separation marker
                        buf.write_all(&data[..(data.len() - 2)]).expect("write");
                        log::debug!("Buf position {}", buf.position());
                        log::debug!("Sending buffer");

                        let send_data = buf.get_ref()[0..(buf.position() as usize)].to_vec();
                        if sender.send(send_data).is_err() {
                            log::warn!("Error sending for channel {}", &name);
                        }

                        buf.set_position(0);
                    }
                    false => {
                        log::debug!("Writing {} bytes", data.len());
                        buf.write_all(&data).expect("write");
                        log::debug!("Buf position {}", buf.position());
                    }
                }
            }
        }
    }
}

pub async fn recv_jpgs_to_files(mut stream: BodyStream) {
    let mut counter = 0;

    let mut buf = Cursor::new(vec![0_u8; 100000]);
    while let Some(Ok(data)) = stream.next().await {
        log::debug!("Data length {}", data.len());
        match data.ends_with("\r\n".as_bytes()) {
            true => {
                log::debug!("Skipping header {:?}", data);
            }
            false => {
                // No header
                match data.ends_with("\n\n".as_bytes()) {
                    true => {
                        // The last two bytes are the separation marker
                        buf.write_all(&data[..(data.len() - 2)]).expect("write");
                        log::debug!("Buf position {}", buf.position());
                        log::debug!("Writing file");

                        let mut frame_file = File::create(format!("frame-{counter}.jpg")).unwrap();
                        frame_file
                            .write_all(&buf.get_ref()[0..(buf.position() as usize)])
                            .expect("Write to file");
                        counter += 1;

                        buf.set_position(0);
                    }
                    false => {
                        log::debug!("Writing {} bytes", data.len());
                        buf.write_all(&data).expect("write");
                        log::debug!("Buf position {}", buf.position());
                    }
                }
            }
        }
    }
}
