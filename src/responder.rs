//! Objects responding to endpoint calls.
//!
//! There are two main objects, both implement the `futures_core::Stream` trait:
//! - `StreamableCamera` initializes the webcam and captures a new frame in its `poll_next` method.
//! - `InferCamera` initializes both the webcam and a neural network model from an `.onnx` file.
//!   In the `poll_next` method, every frame is passed through the network, the output postprocessed
//!   and bounding boxes drawn onto the original frame.

use actix_web::web::Bytes;
use actix_web::Error;
use futures_core::task::{Context, Poll};
use futures_core::Stream;
use image::codecs::jpeg::JpegEncoder;
use image::{ColorType, Rgb, RgbImage};
use imageproc::drawing::draw_hollow_rect;
use imageproc::rect::Rect;
use rscam::Frame;
use std::io::Cursor;
use std::pin::Pin;
use tract_onnx::prelude::{tvec, Arc, TVec, Tensor, TractResult};

use super::nn::postproc_ultraface;

/// Keep a handle to the capture function of an initialized camera.
pub struct StreamableCamera {
    gen_frame: Box<dyn Fn() -> Frame>,
}

impl StreamableCamera {
    /// Create a new instance.
    pub fn new(gen_frame: Box<dyn Fn() -> Frame>) -> StreamableCamera {
        StreamableCamera { gen_frame }
    }
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

        log::debug!("Streaming...");

        Poll::Ready(Some(Ok(body)))
    }
}

pub struct InferCamera {
    gen_frame: Box<dyn Fn() -> Frame>,
    infer_frame: Box<dyn Fn(TVec<Tensor>) -> TractResult<TVec<Arc<Tensor>>>>,
    preproc_frame: Box<dyn Fn(RgbImage) -> Tensor>,
}

impl InferCamera {
    /// Create a new instance.
    pub fn new(
        gen_frame: Box<dyn Fn() -> Frame>,
        infer_frame: Box<dyn Fn(TVec<Tensor>) -> TractResult<TVec<Arc<Tensor>>>>,
        preproc_frame: Box<dyn Fn(RgbImage) -> Tensor>,
    ) -> InferCamera {
        InferCamera {
            gen_frame,
            infer_frame,
            preproc_frame,
        }
    }
}

impl Stream for InferCamera {
    type Item = Result<Bytes, Error>;

    fn poll_next(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        log::debug!("Entering poll");
        let frame = (*self.gen_frame)().to_vec();
        let frame: RgbImage = RgbImage::from_raw(1280, 720, frame).unwrap();
        log::debug!("Image read");

        let (width, height) = frame.dimensions();

        let infer_result =
            (*self.infer_frame)(tvec!((*self.preproc_frame)(frame.clone()))).unwrap();
        log::debug!("Inference done");

        let bboxes_with_conf = postproc_ultraface(infer_result);
        log::debug!("Found {} faces in image", bboxes_with_conf.len());

        let frame = draw_bboxes_on_image(frame, bboxes_with_conf, width, height);

        let mut buf = Cursor::new(Vec::new());

        JpegEncoder::new_with_quality(&mut buf, 70)
            .encode(&frame, width, height, ColorType::Rgb8)
            .unwrap();

        let bytes = buf.into_inner();

        log::debug!("Image encoded");

        let body: Bytes = Bytes::copy_from_slice(
            &[
                "--frame\r\nContent-Type: image/jpeg\r\n\r\n".as_bytes(),
                &bytes[..],
                "\r\n\r\n".as_bytes(),
            ]
            .concat(),
        );

        Poll::Ready(Some(Ok(body)))
    }
}

/// Draw bounding boxes on the image.
fn draw_bboxes_on_image(
    mut frame: RgbImage,
    bboxes_with_confidences: Vec<([f32; 4], f32)>,
    width: u32,
    height: u32,
) -> RgbImage {
    let (width, height) = (width as f32, height as f32);

    for (bbox, _confidence) in bboxes_with_confidences.iter() {
        // Coordinates of top-left and bottom-right points
        // Coordinate frame basis is on the top left corner
        let (x_tl, y_tl) = (bbox[0] * width, bbox[1] * height);
        let (x_br, y_br) = (bbox[2] * width, bbox[3] * height);
        let rect_width = x_br - x_tl;
        let rect_height = y_br - y_tl;

        let face_rect =
            Rect::at(x_tl as i32, y_tl as i32).of_size(rect_width as u32, rect_height as u32);

        frame = draw_hollow_rect(&frame, face_rect, Rgb::from([0, 255, 0]));
    }

    frame
}
