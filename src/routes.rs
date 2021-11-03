use actix_web::web::Bytes;
use actix_web::{get, Error, HttpResponse, Responder};
use futures_core::task::{Context, Poll};
use futures_core::Stream;
use rscam::Frame;
use std::pin::Pin;

use super::responder::InferCamera;
use crate::nn::{get_model_run_func, get_preproc_func};

use super::sensors::get_frame_fn;

#[get("/index")]
async fn index() -> impl Responder {
    let resp = r#"
    <body>
    <div class="container">
        <div class="row">
            <div class="col-lg-8  offset-lg-2">
                <h3 class="mt-5">Live Streaming</h3>
                <img src="./video" width="100%">
            </div>
        </div>
    </div>
    </body>
    "#;
    resp
}

#[get("/video_stream")]
async fn video_stream() -> HttpResponse {
    let cam_stream = StreamableCamera {
        gen_frame: get_frame_fn(),
    };

    HttpResponse::Ok()
        .content_type("multipart/x-mixed-replace; boundary=frame")
        .streaming(cam_stream)
}

struct StreamableCamera {
    gen_frame: Box<dyn Fn() -> Frame>,
}

impl Stream for StreamableCamera {
    type Item = Result<Bytes, Error>;

    fn poll_next(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let frame = (*self.gen_frame)();
        let body: Bytes = Bytes::copy_from_slice(
            &[
                "--frame\r\nContent-Type: image/jpeg\r\n\r\n".as_bytes(),
                &frame[..],
                "\r\n\r\n".as_bytes(),
            ]
            .concat(),
        );

        println!("Streaming...");

        Poll::Ready(Some(Ok(body)))
    }
}

#[get("/face_detection")]
async fn face_detection() -> HttpResponse {
    let infer_stream = InferCamera::new(
        get_frame_fn(),
        get_model_run_func("ultraface-RFB-320").unwrap(),
        get_preproc_func("ultraface-RFB-320").unwrap(),
    );

    HttpResponse::Ok()
        .content_type("multipart/x-mixed-replace; boundary=frame")
        .streaming(infer_stream)
}
