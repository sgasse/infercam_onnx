use cam_sender::{
    sensors::{get_capture_fn, StreamableCamera},
    Error,
};
use env_logger::TimestampPrecision;
use futures::sink::SinkExt;
use infer_server::protocol::{FrameMsg, ProtoMsg};
use reqwest::{multipart, Body};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio_util::codec::{Framed, LengthDelimitedCodec};

#[tokio::main]
async fn main() -> Result<(), Error> {
    println!("Let's send some stuff!");

    env_logger::builder()
        .format_timestamp(Some(TimestampPrecision::Millis))
        .init();

    let capture_fn = get_capture_fn("/dev/video0", (1280, 720), "MJPG", (1, 10))?;
    // let s_cam = StreamableCamera::new(capture_fn);

    // let chunk = multipart::Part::stream(Body::wrap_stream(s_cam));

    // let form = multipart::Form::new().part("chunk", chunk);

    // reqwest::Client::new()
    //     .post("http://127.0.0.1:3000/post_jpgs?name=simon")
    //     .multipart(form)
    //     .send()
    //     .await
    //     .unwrap();

    match TcpStream::connect("127.0.0.1:3001").await {
        Ok(stream) => {
            println!("Client connected");
            let mut counter = 0;

            let mut transport = Framed::new(stream, LengthDelimitedCodec::new());
            loop {
                let frame = capture_fn().unwrap();
                let data =
                    ProtoMsg::FrameMsg(FrameMsg::new("simon".to_owned(), frame[..].to_vec()));
                let data: Vec<u8> = bincode::serialize(&data).unwrap();
                let data = bytes::Bytes::from(data);
                transport.send(data).await.unwrap();

                counter += 1;

                // std::thread::sleep(std::time::Duration::from_secs(1));
            }
        }
        Err(err) => {
            println!("Error connecting");
        }
    }

    Ok(())
}
