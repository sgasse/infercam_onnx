//! Data socket module to receive image streams via network.
//!
use std::sync::Arc;

use common::protocol::ProtoMsg;
use futures::StreamExt;
use tokio::{
    net::{TcpListener, TcpStream},
    task::JoinHandle,
};
use tokio_util::codec::{Framed, LengthDelimitedCodec};

use crate::pubsub::NamedPubSub;

/// Spawn a data socket and register the stream with the Pub/Sub-Engine.
pub async fn spawn_data_socket(pubsub: Arc<NamedPubSub>) -> JoinHandle<Result<(), std::io::Error>> {
    tokio::spawn(async move {
        let addr = "127.0.0.1:3001";
        let listener = TcpListener::bind(addr).await?;

        loop {
            let (socket, _) = listener.accept().await?;
            let pubsub_ = Arc::clone(&pubsub);
            tokio::spawn(async move {
                handle_incoming(socket, pubsub_).await?;
                Ok::<_, std::io::Error>(())
            });
        }
    })
}

/// Handle an incoming image stream.
async fn handle_incoming(stream: TcpStream, pubsub: Arc<NamedPubSub>) -> std::io::Result<()> {
    let addr = stream.peer_addr()?;
    log::info!("{}: New connection", &addr);

    let mut transport = Framed::new(stream, LengthDelimitedCodec::new());

    let name = match transport.next().await {
        Some(Ok(data)) => {
            let proto_msg = ProtoMsg::deserialize(&data).unwrap();
            match proto_msg {
                ProtoMsg::ConnectReq(name) => Some(name),
                _ => None,
            }
        }
        _ => None,
    };

    if let Some(name) = name {
        // We send received frames **twice**. The reason behind this is that infering an image takes
        // a lot longer than just pushing it out via HTTP to the browser. If we use the same
        // broadcast channel for inference and serving the stream on the web, we get a large slack
        // between the receivers which ultimately leads to the inferer only iterating through errors
        // due to being so far behind.
        // By using two different channels, the raw HTTP stream can have a high frame rate while
        // the infered stream with a necessarily lower frame rate will still infer quite recent
        // images. We ensure this by having a very small buffer in the infer channel, which leads
        // the sending end to reject frames often and only pushing through very recent frames
        // when the inferer is ready to receive a new frame.
        let sender_raw = pubsub.get_broadcast_sender(&name).await;
        let sender_infer = pubsub.get_mpsc_sender(&name).await;

        while let Some(Ok(frame)) = transport.next().await {
            let data = frame;
            let proto_msg: ProtoMsg = ProtoMsg::deserialize(&data[..]).unwrap();
            if let ProtoMsg::FrameMsg(frame_msg) = proto_msg {
                if sender_raw.send(frame_msg.data.clone()).is_err() {
                    // Error in sending usually means no listener
                }

                let send_infer_with_timeout =
                    tokio::time::timeout(std::time::Duration::from_millis(10), async {
                        sender_infer.send(frame_msg.data).await
                    });
                if send_infer_with_timeout.await.is_err() {
                    // Error in sending usually means no listener
                } else {
                    log::debug!("Data socket of {} sent to infer!", &name);
                }
            }
        }
    }

    log::info!("{}: Connection closed", &addr);

    Ok(())
}
