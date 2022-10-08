use crate::pubsub::NamedPubSub;
use axum::{
    extract::{BodyStream, Query},
    Extension,
};
use futures::stream::StreamExt;
use serde::Deserialize;
use std::io::Cursor;
use std::io::Write;
use std::{fs::File, sync::Arc};

#[derive(Debug, Deserialize)]
pub struct RecvJpgsParams {
    #[serde(default)]
    name: Option<String>,
}

pub async fn recv_named_jpg_streams(
    Extension(pubsub): Extension<Arc<NamedPubSub>>,
    Query(params): Query<RecvJpgsParams>,
    mut stream: BodyStream,
) {
    let name = params.name.unwrap_or("unknown".into());
    log::info!("Receiving stream for name {}", &name);
    let sender = pubsub.get_sender(&name).await;

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
                        log::debug!("Sending buffer");

                        let send_data = buf.get_ref()[0..(buf.position() as usize)].to_vec();
                        if let Err(_) = sender.send(send_data) {
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

pub async fn recv_jpgs(mut stream: BodyStream) {
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

                        let mut frame_file =
                            File::create(&format!("frame-{}.jpg", counter)).unwrap();
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
