use actix_web::body::Body;
use actix_web::{get, Error, HttpRequest, HttpResponse, Responder};
use rscam::Frame;
use std::future::{ready, Ready};

use super::sensors::get_frame_fn;

#[get("/index")]
async fn index() -> impl Responder {
    format!("Hello from actix!")
}

struct VideoFrame {
    frame: Frame,
}

impl Responder for VideoFrame {
    type Error = Error;
    type Future = Ready<Result<HttpResponse, Error>>;

    fn respond_to(self, _req: &HttpRequest) -> Self::Future {
        let body: Vec<u8> = [
            "--frame\r\nContent-Type: image/jpeg\r\n\r\n".as_bytes(),
            &self.frame[..],
            "\r\n\r\n".as_bytes(),
        ]
        .concat();

        ready(Ok(HttpResponse::Ok()
            .content_type("multipart/x-mixed-replace; boundary=frame")
            .body(Body::from_slice(&body[..]))))
    }
}

#[get("/video")]
async fn video() -> impl Responder {
    let frame_fn = get_frame_fn();
    VideoFrame { frame: frame_fn() }
}
