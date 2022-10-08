use cam_sender::{
    sensors::{get_capture_fn, StreamableCamera},
    Error,
};
use env_logger::TimestampPrecision;
use reqwest::{multipart, Body};

#[tokio::main]
async fn main() -> Result<(), Error> {
    println!("Let's send some stuff!");

    env_logger::builder()
        .format_timestamp(Some(TimestampPrecision::Millis))
        .init();

    let capture_fn = get_capture_fn("/dev/video0", (1280, 720), "MJPG", (1, 10))?;
    let s_cam = StreamableCamera::new(capture_fn);

    let chunk = multipart::Part::stream(Body::wrap_stream(s_cam));

    let form = multipart::Form::new().part("chunk", chunk);

    reqwest::Client::new()
        .post("http://127.0.0.1:3000/post_jpgs?name=simon")
        .multipart(form)
        .send()
        .await
        .unwrap();

    Ok(())
}
