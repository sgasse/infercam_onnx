use axum::{
    routing::{get, post},
    Extension, Router,
};
use env_logger::TimestampPrecision;
use futures::StreamExt;
use infer_server::{endpoints::named_stream, nn::UltrafaceModel, protocol::ProtoMsg};
use infer_server::{endpoints::recv_named_jpg_streams, pubsub::NamedPubSub};
use std::{net::SocketAddr, sync::Arc};
use tokio::net::{TcpListener, TcpStream};
use tokio_util::codec::{Framed, LengthDelimitedCodec};

#[tokio::main]
async fn main() {
    println!("Let's get started!");

    env_logger::builder()
        .format_timestamp(Some(TimestampPrecision::Millis))
        .init();

    // let model = UltrafaceModel::new(infer_server::nn::UltrafaceVariant::W320H240)
    //     .await
    //     .expect("Initialize model");
    let pubsub = Arc::new(NamedPubSub::new());

    let pubsub_ = Arc::clone(&pubsub);
    let handle = tokio::spawn(async move {
        let addr = "127.0.0.1:3001";
        let listener = TcpListener::bind(addr).await?;

        loop {
            let (socket, _) = listener.accept().await?;
            let pubsub__ = Arc::clone(&pubsub_);
            tokio::spawn(async move {
                handle_incoming(socket, pubsub__).await?;
                Ok::<_, std::io::Error>(())
            });
        }

        Ok::<_, std::io::Error>(())
    });

    let app = Router::new()
        .route("/healthcheck", get(healthcheck))
        .route("/post_jpgs", post(recv_named_jpg_streams))
        .route("/stream", get(named_stream))
        .layer(Extension(pubsub));

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn healthcheck() -> &'static str {
    "Healthy"
}

pub async fn handle_incoming(stream: TcpStream, pubsub: Arc<NamedPubSub>) -> std::io::Result<()> {
    println!("{}: New connection", stream.peer_addr()?);

    let mut transport = Framed::new(stream, LengthDelimitedCodec::new());

    while let Some(Ok(frame)) = transport.next().await {
        let data = frame;
        let proto_msg: ProtoMsg = bincode::deserialize(&data[..]).unwrap();
        if let ProtoMsg::FrameMsg(frame_msg) = proto_msg {
            let sender = pubsub.get_sender(&frame_msg.id).await;
            if let Err(e) = sender.send(frame_msg.data) {
                println!("Send error");
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod test {
    #[test]
    fn test_identifying_parts() {
        let id = b"--blabla\r\n";
        assert!(id.ends_with("\r\n".as_bytes()));
    }
}
