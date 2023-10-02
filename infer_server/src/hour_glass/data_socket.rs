//! Data socket module to receive image streams via network.
//!
use std::net::SocketAddr;

use anyhow::Result;
use futures::StreamExt;
use tokio::{
    net::{TcpListener, TcpStream},
    task::JoinHandle,
};
use tokio_util::codec::{Framed, LengthDelimitedCodec};

use crate::hour_glass::StaticFrameSender;

pub async fn spawn_data_socket(
    tx: StaticFrameSender,
    addr: &str,
) -> Result<JoinHandle<Result<()>>> {
    let socket: SocketAddr = addr.parse()?;
    Ok(tokio::spawn(async move {
        let listener = TcpListener::bind(socket).await?;

        loop {
            let (socket, _peer_addr) = listener.accept().await?;
            let tx = tx.clone();
            tokio::spawn(async move {
                handle_incoming(tx, socket).await?;
                Ok::<_, anyhow::Error>(())
            });
        }
    }))
}

async fn handle_incoming(tx: StaticFrameSender, stream: TcpStream) -> Result<()> {
    let addr = stream.peer_addr()?;
    log::info!("{}: New TCP connection", &addr);

    let mut transport = Framed::new(stream, LengthDelimitedCodec::new());

    while let Some(Ok(data)) = transport.next().await {
        tx.send(data)
            .await
            .map_err(|_| anyhow::anyhow!("failed to send frame"))?;
    }

    Ok(())
}
