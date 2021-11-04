use actix_web::web::Bytes;
use actix_web::Error;
use futures_core::task::{Context, Poll};
use futures_core::Stream;
use image::codecs::jpeg::JpegEncoder;
use image::{ImageBuffer, Rgb};
use imageproc::definitions::Image;
use imageproc::drawing::draw_hollow_rect;
use imageproc::rect::Rect;
use rscam::Frame;
use std::io::Cursor;
use std::pin::Pin;
use tract_onnx::prelude::{tvec, Arc, TVec, Tensor, TractResult};

use super::nn::postproc_ultraface;

pub struct StreamableCamera {
    gen_frame: Box<dyn Fn() -> Frame>,
}

impl StreamableCamera {
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
    preproc_frame: Box<dyn Fn(ImageBuffer<Rgb<u8>, Vec<u8>>) -> Tensor>,
}

impl InferCamera {
    pub fn new(
        gen_frame: Box<dyn Fn() -> Frame>,
        infer_frame: Box<dyn Fn(TVec<Tensor>) -> TractResult<TVec<Arc<Tensor>>>>,
        preproc_frame: Box<dyn Fn(ImageBuffer<Rgb<u8>, Vec<u8>>) -> Tensor>,
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
        let frame: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::from_raw(1280, 720, frame).unwrap();
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
            .encode(&frame, width, height, image::ColorType::Rgb8)
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

fn draw_bboxes_on_image(
    mut frame: Image<Rgb<u8>>,
    bboxes_with_confidences: Vec<([f32; 4], f32)>,
    width: u32,
    height: u32,
) -> Image<Rgb<u8>> {
    let (width, height) = (width as f32, height as f32);

    for (bbox, _confidence) in bboxes_with_confidences.iter() {
        // Coordinates of top-left point (coordinate basis is on the top left corner)
        let (x_tl, y_tl) = (bbox[0] * width, bbox[1] * height);
        let (x_br, y_br) = (bbox[2] * width, bbox[3] * height);
        let rect_width = x_br - x_tl;
        let rect_height = y_br - y_tl;

        let face_rect =
            Rect::at(x_tl as i32, y_tl as i32).of_size(rect_width as u32, rect_height as u32);

        frame = draw_hollow_rect(&frame, face_rect, Rgb::from([255, 0, 0]));
    }

    frame
}
