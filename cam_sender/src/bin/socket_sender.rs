use anyhow::{bail, Result};
use argh::FromArgs;
use cam_sender::sensors::{get_max_res_mjpg_capture_fn, CameraWrapper};
use common::protocol::{FrameMsg, ProtoMsg};
use env_logger::TimestampPrecision;
use futures::sink::SinkExt;
use rscam::Camera;
use std::{str::FromStr, time::Duration};
use tokio::net::TcpStream;
use tokio_util::codec::{Framed, LengthDelimitedCodec};

#[derive(FromArgs)]
/// Send webcam stream to infer_server.
struct Cli {
    /// address of the infer server to connect to
    #[argh(option, default = "String::from(\"127.0.0.1:3001\")")]
    address: String,

    /// channel name that this sender publishes to
    #[argh(option, default = "String::from(\"simon\")")]
    channel: String,
}

#[derive(Clone, Debug)]
pub enum Protocol {
    Tcp,
}

impl FromStr for Protocol {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "tcp" | "Tcp" | "TCP" => Ok(Protocol::Tcp),
            _ => Err(s.to_owned()),
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Cli = argh::from_env();

    env_logger::builder()
        .format_timestamp(Some(TimestampPrecision::Millis))
        .init();

    log::info!("Launching socket sender for channel {}", &args.channel);

    // Initialize webcam to send image stream
    let cam = get_max_res_mjpg_capture_fn()?;

    loop {
        if let Err(e) = tcp_sender(&cam, &args).await {
            log::warn!("Error in sender: {e}. Reconnecting...");
        }

        tokio::time::sleep(Duration::from_secs(3)).await;
    }
}

async fn tcp_sender(cam: &CameraWrapper<Camera>, args: &Cli) -> Result<()> {
    match TcpStream::connect(&args.address).await {
        Ok(stream) => {
            log::info!("Client connected to {}", &args.channel);

            // Wrap stream in transport handler with length-delimited codec
            let mut transport = Framed::new(stream, LengthDelimitedCodec::new());

            // Send init message
            let init_msg = bytes::Bytes::from(bincode::serialize(&ProtoMsg::ConnectReq(
                args.channel.clone(),
            ))?);
            transport.send(init_msg).await?;

            // Send captured frames in a loop
            loop {
                match cam.get_frame() {
                    Some(frame) => {
                        let data = ProtoMsg::FrameMsg(FrameMsg::new(
                            args.channel.clone(),
                            frame[..].to_vec(),
                        ));
                        let data: Vec<u8> = bincode::serialize(&data)?;
                        let data = bytes::Bytes::from(data);
                        transport.send(data).await?;
                    }
                    None => log::error!("Unable to capture frame, trying again..."),
                }
            }
        }
        Err(err) => {
            bail!(
                "failed to connect to server with channel {}: {}",
                &args.channel,
                err
            );
        }
    }
}
