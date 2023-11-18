//! Inference server library.
//!

use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use bytes::{Bytes, BytesMut};
use thingbuf::mpsc::{StaticChannel, StaticReceiver, StaticSender};

pub mod data_socket;
pub mod endpoints;
pub mod inferer;
pub mod meter;
pub mod nn;
pub mod router;
pub mod utils;

pub type StaticFrameSender = StaticSender<BytesMut>;
pub type StaticFrameReceiver = StaticReceiver<BytesMut>;

pub static INCOMING_FRAMES_CHANNEL: StaticChannel<BytesMut, 200> = StaticChannel::new();

pub type BroadcastSender = tokio::sync::broadcast::Sender<Bytes>;
pub type BroadcastReceiver = tokio::sync::broadcast::Receiver<Bytes>;

pub fn broadcast_channel() -> (BroadcastSender, BroadcastReceiver) {
    tokio::sync::broadcast::channel(20)
}

pub type StaticImage = (u32, u32, Vec<u8>, Option<BroadcastSender>);

pub type StaticImageSender = StaticSender<StaticImage>;
pub type StaticImageReceiver = StaticReceiver<StaticImage>;

pub static INFER_IMAGES_CHANNEL: StaticChannel<StaticImage, 10> = StaticChannel::new();

fn hashed<T>(data: T) -> u64
where
    T: Hash,
{
    let mut hasher = DefaultHasher::new();
    data.hash(&mut hasher);
    hasher.finish()
}

fn as_jpeg_stream_item(data: &[u8]) -> Bytes {
    Bytes::copy_from_slice(
        &[
            "--frame\r\nContent-Type: image/jpeg\r\n\r\n".as_bytes(),
            &data[..],
            "\r\n\r\n".as_bytes(),
        ]
        .concat(),
    )
}
