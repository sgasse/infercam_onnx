use cam_sender::{
    sensors::{get_capture_fn, StreamableCamera},
    Error,
};
use env_logger::TimestampPrecision;
use futures_core::Stream;
use reqwest::{multipart, Body};
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> Result<(), Error> {
    println!("Let's send some stuff!");

    env_logger::builder()
        .format_timestamp(Some(TimestampPrecision::Millis))
        .init();

    let capture_fn = get_capture_fn("/dev/video0", (1280, 720), "MJPG", (1, 30))?;
    let s_cam = StreamableCamera::new(capture_fn);

    // println!("Got through cam init");

    // let (tx, mut rx) = mpsc::channel(100);

    // tokio::spawn(async move {
    //     loop {
    //         match s_cam.capture() {
    //             Ok(frame) => {
    //                 log::info!("Got frame");
    //                 tx.send(frame).await;
    //             }
    //             Err(e) => {
    //                 log::error!("Error getting frame {:?}", e);
    //             }
    //         }
    //     }
    // });

    // let stream = async_stream::stream! {
    //     while let Some(item) = rx.recv().await {
    //         yield item;
    //     }
    // };

    // let chunks = vec!["hello", " ", "you", " ", "coder"];

    // let stream = futures_util::stream::iter(
    //     chunks
    //         .into_iter()
    //         .map(|v| Ok(v))
    //         .collect::<Vec<Result<_, std::io::Error>>>(),
    // );

    // let chunk = multipart::Part::stream(Body::wrap_stream(stream));
    // let (mut tx, mut rx) = futures::channel::mpsc::channel(100);
    // use futures::SinkExt;
    // tx.send(Ok::<_, std::io::Error>(1)).await?;
    // tx.send(Ok::<_, std::io::Error>(2)).await?;

    let chunk = multipart::Part::stream(Body::wrap_stream(s_cam));

    let form = multipart::Form::new().part("chunk", chunk);

    reqwest::Client::new()
        .post("http://127.0.0.1:3000/chunks")
        .multipart(form)
        .send()
        .await
        .unwrap();

    Ok(())
}
