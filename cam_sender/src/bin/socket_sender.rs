use cam_sender::{sensors::get_capture_fn, Error};
use clap::Parser;
use env_logger::TimestampPrecision;
use futures::sink::SinkExt;
use infer_server::protocol::{FrameMsg, ProtoMsg};
use tokio::net::TcpStream;
use tokio_util::codec::{Framed, LengthDelimitedCodec};

#[derive(Parser, Debug)]
#[clap(author, version)]
struct Args {
    /// Address of the infer server to connect to
    #[clap(long, default_value = "127.0.0.1:3001")]
    address: String,

    /// Channel name that this sender publishes to
    #[clap(long, default_value = "simon")]
    channel: String,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let args = Args::parse();

    env_logger::builder()
        .format_timestamp(Some(TimestampPrecision::Millis))
        .init();

    log::info!("Launching socket sender for channel {}", &args.channel);

    let capture_fn = get_capture_fn("/dev/video0", (1280, 720), "MJPG", (1, 10))?;

    match TcpStream::connect(&args.address).await {
        Ok(stream) => {
            log::info!("Client connected to {}", &args.channel);

            let mut transport = Framed::new(stream, LengthDelimitedCodec::new());
            loop {
                let frame = capture_fn().unwrap();
                let data =
                    ProtoMsg::FrameMsg(FrameMsg::new(args.channel.clone(), frame[..].to_vec()));
                let data: Vec<u8> = bincode::serialize(&data).unwrap();
                let data = bytes::Bytes::from(data);
                transport.send(data).await.unwrap();
            }
        }
        Err(err) => {
            println!("Error connecting to {}\n{}", &args.channel, err);
        }
    }

    Ok(())
}
