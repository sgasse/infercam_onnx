use super::nn::{get_model_run_func, get_preproc_func};
use super::responder::{InferCamera, StreamableCamera};
use super::sensors::get_frame_fn;
use actix_web::{get, HttpResponse, Responder};

#[get("/index")]
async fn index() -> impl Responder {
    let resp = r#"
<!DOCTYPE html>
<html>
<head>
<title>Index</title>
</head>
<body>
<div class="container">
    <h3>Streaming</h3>
    <img src="./video_stream" width="100%">
</div>
</body>
</html>
"#;
    HttpResponse::Ok().content_type("text/html").body(resp)
}

#[get("/video_stream")]
async fn video_stream() -> HttpResponse {
    let cam_stream = StreamableCamera::new(get_frame_fn());

    HttpResponse::Ok()
        .content_type("multipart/x-mixed-replace; boundary=frame")
        .streaming(cam_stream)
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
