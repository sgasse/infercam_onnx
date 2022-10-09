use std::sync::Arc;

use futures::StreamExt;
use tokio::{
    net::{TcpListener, TcpStream},
    task::JoinHandle,
};
use tokio_util::codec::{Framed, LengthDelimitedCodec};

use crate::{protocol::ProtoMsg, pubsub::NamedPubSub};

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

        Ok::<_, std::io::Error>(())
    })
}

async fn handle_incoming(stream: TcpStream, pubsub: Arc<NamedPubSub>) -> std::io::Result<()> {
    println!("{}: New connection", stream.peer_addr()?);

    let mut transport = Framed::new(stream, LengthDelimitedCodec::new());

    while let Some(Ok(frame)) = transport.next().await {
        let data = frame;
        let proto_msg: ProtoMsg = bincode::deserialize(&data[..]).unwrap();
        if let ProtoMsg::FrameMsg(frame_msg) = proto_msg {
            let sender = pubsub.get_sender(&frame_msg.id).await;
            if let Err(_) = sender.send(frame_msg.data) {
                log::debug!("Send error for id {} - probably no listener", &frame_msg.id);
            }
        }
    }

    Ok(())
}
