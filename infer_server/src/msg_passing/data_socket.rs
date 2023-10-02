//! Data socket module to receive image streams via network.
//!
use std::net::SocketAddr;

use anyhow::{bail, Result};
use bytes::Bytes;
use common::protocol::ProtoMsg;
use futures::StreamExt;
use tokio::{
    net::{TcpListener, TcpStream},
    sync::mpsc,
    task::JoinHandle,
};
use tokio_util::codec::{Framed, LengthDelimitedCodec};

use crate::msg_passing::router::TcpTaskDataSender;

/// Spawn a data socket and register the stream with the Pub/Sub-Engine.
pub async fn spawn_data_socket(
    registry_tx: TcpTaskDataSender,
    addr: &str,
) -> Result<JoinHandle<Result<()>>> {
    let socket: SocketAddr = addr.parse()?;
    Ok(tokio::spawn(async move {
        let listener = TcpListener::bind(socket).await?;

        loop {
            let (socket, _peer_addr) = listener.accept().await?;
            let registry_tx = registry_tx.clone();
            tokio::spawn(async move {
                handle_incoming(registry_tx, socket).await?;
                Ok::<_, anyhow::Error>(())
            });
        }
    }))
}

async fn handle_incoming(registry_tx: TcpTaskDataSender, stream: TcpStream) -> Result<()> {
    let addr = stream.peer_addr()?;
    log::info!("{}: New TCP connection", &addr);

    let mut transport = Framed::new(stream, LengthDelimitedCodec::new());

    let channel_name = {
        if let Some(Ok(data)) = transport.next().await {
            if let Ok(ProtoMsg::ConnectReq(channel)) = ProtoMsg::deserialize(&data) {
                channel
            } else {
                bail!("no channel name");
            }
        } else {
            bail!("no channel name");
        }
    };

    let (senders_tx, mut senders_rx) = mpsc::channel(20);
    registry_tx.send((channel_name, senders_tx)).await?;

    let mut listeners = Vec::new();
    let mut failed_senders = Vec::new();

    loop {
        tokio::select! {
            res = senders_rx.recv() => {
                match res {
                    None => panic!("registry closed"),
                    Some(new_listeners) => {
                        listeners.extend(new_listeners.into_iter());
                    }
                }
            }
            res = transport.next() => match res {
                None => {
                    log::info!("TCP stream ended");
                }
                Some(Ok(data)) => {
                    if let Ok(ProtoMsg::FrameMsg(msg)) = ProtoMsg::deserialize(&data) {
                    let data = as_jpeg_stream_item(&msg.data);
                    // Send
                    for (idx, sender) in listeners.iter().enumerate() {
                        if sender.send(data.clone()).await.is_err() {
                            failed_senders.push(idx);
                        }
                    }

                    for idx in failed_senders.iter().rev() {
                        listeners.swap_remove(*idx);
                    }
                    }
                }
                Some(Err(e)) => {
                    log::warn!("Error in TCP codec: {e}");
                }
            }
        }
    }
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
