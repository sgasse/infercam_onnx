use cam_sender::{
    sensors::{get_capture_fn, StreamableCamera},
    Error,
};
use clap::Parser;
use env_logger::TimestampPrecision;
use reqwest::{multipart, Body};

#[derive(Parser, Debug)]
#[clap(author, version)]
struct Args {
    /// Address of the infer server to connect to
    #[clap(long, default_value = "127.0.0.1:3000")]
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

    log::info!("Launching multipart sender for channel {}", &args.channel);

    let capture_fn = get_capture_fn("/dev/video0", (1280, 720), "MJPG", (1, 10))?;
    let s_cam = StreamableCamera::new(capture_fn);

    let chunk = multipart::Part::stream(Body::wrap_stream(s_cam));

    let form = multipart::Form::new().part("chunk", chunk);

    reqwest::Client::new()
        .post(&format!(
            "http://{}/post_jpgs?name={}",
            &args.address, &args.channel
        ))
        .multipart(form)
        .send()
        .await?;

    Ok(())
}
