use actix_web::web::Bytes;
use actix_web::Error;
use futures_core::task::{Context, Poll};
use futures_core::Stream;
use image::codecs::jpeg::JpegEncoder;
use image::{load_from_memory_with_format, ImageBuffer, ImageFormat, Rgb};
use imageproc::drawing::draw_hollow_rect;
use imageproc::rect::Rect;
use rscam::Frame;
use std::io::BufWriter;
use std::pin::Pin;
use tract_onnx::prelude::{tvec, Arc, TVec, Tensor, TractResult};

use super::nn::get_top_bbox_from_ultraface;

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

        println!("Streaming...");

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
        let frame = (*self.gen_frame)();
        let frame = load_from_memory_with_format(&frame, ImageFormat::Jpeg)
            .unwrap()
            .to_rgb8();

        let (width, height) = frame.dimensions();

        let infer_result =
            (*self.infer_frame)(tvec!((*self.preproc_frame)(frame.clone()))).unwrap();

        let (top_bbox, top_confidence) = get_top_bbox_from_ultraface(infer_result);

        // Coordinates of top-left and bottom-right point
        let (x_tl, y_tl) = (top_bbox[0] * width as f32, top_bbox[1] * height as f32);
        let (x_br, y_br) = (top_bbox[2] * width as f32, top_bbox[3] * height as f32);
        println!(
            "Confidence {}: top-left ({}, {}), bottom-right ({}, {})",
            top_confidence, x_tl, y_tl, x_br, y_br
        );
        let face_rect =
            Rect::at(x_tl as i32, y_tl as i32).of_size((x_br - x_tl) as u32, (y_br - y_tl) as u32);

        let frame = draw_hollow_rect(&frame, face_rect, Rgb::from([255, 0, 0]));

        let mut buf = BufWriter::new(Vec::new());

        JpegEncoder::new(&mut buf)
            .encode(&frame, width, height, image::ColorType::Rgb8)
            .unwrap();

        let bytes = buf.into_inner().unwrap();

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
