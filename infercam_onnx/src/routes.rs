//! Route definitions.

use actix_web::{get, HttpResponse, Responder};

use super::nn::{get_model_run_func, get_preproc_func};
use super::responder::{InferCamera, StreamableCamera};
use super::sensors::get_capture_func;

/// Display index page with face detection stream.
#[get("/")]
async fn index() -> impl Responder {
    let resp = r#"
<!DOCTYPE html>
<html>
<head>
<title>InferCam</title>
<style>
body {
    background-color: black;
    color: white;
    text-align: center;
    font-family: sans-serif;
}
</style>
</head>
<body>
<div class="container">
    <h3>Deep Learning Face Detection</h3>
    <h4>Built with Rust, onnxruntime, and actix_web.</h4>
    <img src="./face_detection" width="100%">
</div>
</body>
</html>
"#;
    HttpResponse::Ok().content_type("text/html").body(resp)
}

/// Stream webcam without any processing on top.
#[get("/video_stream")]
async fn video_stream() -> HttpResponse {
    // Capture directly as `MJPG` to avoid costly encoding to serve as JPEG on the `html` page
    let cam_stream = StreamableCamera::new(get_capture_func((1280, 720), "MJPG"));

    HttpResponse::Ok()
        .content_type("multipart/x-mixed-replace; boundary=frame")
        .streaming(cam_stream)
}

/// Stream face detection.
#[get("/face_detection")]
async fn face_detection() -> HttpResponse {
    let infer_stream = InferCamera::new(
        // Capture as `RGB3` to avoid extra decoding step before preprocessing a frame
        get_capture_func((1280, 720), "RGB3"),
        get_model_run_func("ultraface-RFB-320").unwrap(),
        get_preproc_func("ultraface-RFB-320").unwrap(),
    );

    HttpResponse::Ok()
        .content_type("multipart/x-mixed-replace; boundary=frame")
        .streaming(infer_stream)
}
