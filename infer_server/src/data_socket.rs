use std::sync::Arc;

use futures::StreamExt;
use tokio::{
    net::{TcpListener, TcpStream},
    sync::broadcast::Sender,
    task::JoinHandle,
};
use tokio_util::codec::{Framed, LengthDelimitedCodec};

use crate::pubsub::{BytesSender, MpscBytesSender, NamedPubSub};
use common::protocol::ProtoMsg;

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
        let sender_raw = pubsub.get_broadcast_sender(&name).await;
        let sender_infer = pubsub.get_mpsc_sender(&name).await;

        while let Some(Ok(frame)) = transport.next().await {
            let data = frame;
            let proto_msg: ProtoMsg = ProtoMsg::deserialize(&data[..]).unwrap();
            if let ProtoMsg::FrameMsg(frame_msg) = proto_msg {
                if let Err(_) = sender_raw.send(frame_msg.data.clone()) {
                    // log::debug!("Send error for id {} - probably no listener", &frame_msg.id);
                }

                let send_infer_with_timeout =
                    tokio::time::timeout(std::time::Duration::from_millis(10), async {
                        sender_infer.send(frame_msg.data).await
                    });
                if let Err(_) = send_infer_with_timeout.await {
                    // log::debug!(
                    //     "Send error infer for id {} - probably no listener",
                    //     &frame_msg.id
                    // );
                } else {
                    log::debug!("Data socket of {} sent to infer!", &name);
                }
            }
        }
    }

    log::info!("{}: Connection closed", &addr);

    Ok(())
}
