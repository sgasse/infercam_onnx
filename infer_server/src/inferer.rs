use anyhow::Result;
use image::{Rgb, RgbImage};
use imageproc::{
    drawing::{draw_hollow_rect, draw_text},
    rect::Rect,
};
use lazy_static::lazy_static;

use crate::{
    nn::{Bbox, InferModel, UltrafaceModel},
    StaticImageReceiver,
};

use super::as_jpeg_stream_item;

pub struct Inferer {
    infer_rx: StaticImageReceiver,
    model: UltrafaceModel,
}

impl Inferer {
    pub async fn new(infer_rx: StaticImageReceiver) -> Self {
        let model = UltrafaceModel::new(crate::nn::UltrafaceVariant::W320H240, 0.5, 0.5)
            .await
            .expect("failed to initialized model");
        Self { infer_rx, model }
    }

    pub async fn run(&self) {
        loop {
            if let Some(recv_ref) = self.infer_rx.recv_ref().await {
                let width = recv_ref.0;
                let height = recv_ref.1;

                let image: RgbImage = turbojpeg::decompress_image(&recv_ref.2.as_slice())
                    .expect("failed to decompress");
                if let Ok(bboxes_with_confidences) = self.infer_faces(&image) {
                    let frame = draw_bboxes_on_image(image, bboxes_with_confidences, width, height);
                    let buf = turbojpeg::compress_image(&frame, 95, turbojpeg::Subsamp::Sub2x2)
                        .expect("failed to compress");
                    recv_ref
                        .3
                        .as_ref()
                        .unwrap()
                        .send(as_jpeg_stream_item(&buf))
                        .ok();
                }
            }
        }
    }

    fn infer_faces(&self, frame: &RgbImage) -> Result<Vec<(Bbox, f32)>> {
        self.model.run(frame)
    }
}

/// Draw bounding boxes with confidence scores on the image.
fn draw_bboxes_on_image(
    mut frame: RgbImage,
    bboxes_with_confidences: Vec<([f32; 4], f32)>,
    width: u32,
    height: u32,
) -> RgbImage {
    let (width, height) = (width as f32, height as f32);

    let color = Rgb::from([0, 255, 0]);

    for (bbox, confidence) in bboxes_with_confidences.iter() {
        // Coordinates of top-left and bottom-right points
        // Coordinate frame basis is on the top left corner
        let (x_tl, y_tl) = (bbox[0] * width, bbox[1] * height);
        let (x_br, y_br) = (bbox[2] * width, bbox[3] * height);
        let rect_width = x_br - x_tl;
        let rect_height = y_br - y_tl;

        let face_rect =
            Rect::at(x_tl as i32, y_tl as i32).of_size(rect_width as u32, rect_height as u32);

        frame = draw_hollow_rect(&frame, face_rect, color);
        frame = draw_text(
            &frame,
            color,
            x_tl as i32,
            y_tl as i32,
            rusttype::Scale { x: 16.0, y: 16.0 },
            &DEJAVU_MONO,
            &format!("{:.2}%", confidence * 100.0),
        );
    }

    frame
}

lazy_static! {
    static ref DEJAVU_MONO: rusttype::Font<'static> = {
        let font_data: &[u8] = include_bytes!("../../resources/DejaVuSansMono.ttf");
        let font: rusttype::Font<'static> =
            rusttype::Font::try_from_bytes(font_data).expect("failed to load font");
        font
    };
}
