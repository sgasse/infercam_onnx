use crate::{
    nn::{Bbox, InferModel, UltrafaceModel},
    pubsub::{BytesReceiver, BytesSender},
    Error,
};
use image::ImageDecoder;
use image::{
    codecs::jpeg::{JpegDecoder, JpegEncoder},
    ColorType, Rgb, RgbImage,
};
use imageproc::drawing::{draw_hollow_rect, draw_text};
use imageproc::rect::Rect;
use lazy_static::lazy_static;
use simple_error::SimpleError;
use std::{collections::HashMap, io::Cursor};
use tokio::sync::{broadcast, Mutex};

pub type RecvSendPair = (BytesReceiver, BytesSender);

pub struct Inferer {
    channel_map: Mutex<HashMap<String, RecvSendPair>>,
    model: UltrafaceModel,
}

impl Inferer {
    pub async fn new() -> Self {
        let model = UltrafaceModel::new(crate::nn::UltrafaceVariant::W320H240)
            .await
            .expect("Initialize model");
        Self {
            channel_map: Mutex::new(HashMap::new()),
            model,
        }
    }

    pub async fn subscribe_img_stream(&self, name: &str, img_rx: BytesReceiver) -> BytesReceiver {
        let mut channel_map = self.channel_map.lock().await;

        match channel_map.get(name) {
            Some((_, infered_tx)) => infered_tx.subscribe(),
            None => {
                let (infered_tx, infered_rx) = broadcast::channel(10);
                channel_map.insert(name.to_owned(), (img_rx, infered_tx));
                infered_rx
            }
        }
    }

    fn process_frame(&self, frame: Vec<u8>, width: u32, height: u32) -> Result<Vec<u8>, Error> {
        let frame = decode_frame(width, height, frame)?;
        let bboxes_with_confidences = self.infer_faces(frame.clone())?;

        let frame = draw_bboxes_on_image(frame, bboxes_with_confidences, width, height);

        let mut buf = Cursor::new(Vec::new());

        JpegEncoder::new_with_quality(&mut buf, 70).encode(
            &frame,
            width,
            height,
            ColorType::Rgb8,
        )?;

        Ok(buf.into_inner())
    }

    fn infer_faces(&self, frame: RgbImage) -> Result<Vec<(Bbox, f32)>, Error> {
        self.model.run(frame)
    }

    pub async fn run(&self) {
        // TODO: Consider oneshot channel instead of Mutex?
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(10));
        loop {
            {
                let mut channel_map = self.channel_map.lock().await;
                let mut names_to_remove = Vec::with_capacity(0);
                {
                    for (name, (img_rx, infered_tx)) in channel_map.iter_mut() {
                        // TODO: Parallel await?
                        let recv_with_timeout =
                            tokio::time::timeout(std::time::Duration::from_millis(1000), async {
                                img_rx.recv().await
                            });
                        match recv_with_timeout.await {
                            Ok(Ok(img)) => {
                                let (width, height) = (1280, 720);
                                match self.process_frame(img, width, height) {
                                    Err(err) => log::error!("Error in process frame: {}", err),
                                    Ok(infered) => {
                                        if let Err(_) = infered_tx.send(infered) {
                                            log::info!("No listener for {}", name);
                                            names_to_remove.push(name.clone());
                                        }
                                    }
                                }
                            }
                            Ok(Err(err)) => {
                                log::error!("Receive error for {}: {}", &name, err);
                            }
                            Err(elapsed) => {
                                use std::error::Error;
                                match elapsed.source() {
                                    None => log::info!("Receive timed out for {}", &name),
                                    Some(err) => log::info!("Receive error for {}: {}", &name, err),
                                }
                                log::info!("Infer timed out or failed");
                            }
                        }
                    }
                }

                for name in names_to_remove {
                    log::info!("Removing channel {}", &name);
                    channel_map.remove(&name);
                }
            }
            interval.tick().await;
        }
    }
}

fn decode_frame(width: u32, height: u32, buffer: Vec<u8>) -> Result<RgbImage, Error> {
    let decoder = JpegDecoder::new(Cursor::new(buffer))?;
    let mut target = vec![0; decoder.total_bytes() as usize];

    decoder.read_image(&mut target)?;
    match RgbImage::from_raw(width, height, target) {
        None => Err(SimpleError::new("Could not decode frame").into()),
        Some(frame) => Ok(frame),
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

        frame = draw_hollow_rect(&frame, face_rect, Rgb::from([0, 255, 0]));
        frame = draw_text(
            &mut frame,
            color.clone(),
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
            rusttype::Font::try_from_bytes(font_data).expect("Load font");
        font
    };
}

#[cfg(test)]
mod test {
    use std::error::Error;

    #[tokio::test]
    async fn test_timeout_returns() {
        // Returning Ok before timeout
        let ok_timeout = tokio::time::timeout(std::time::Duration::from_millis(500), async {
            return Ok::<(), String>(());
        });
        let result = ok_timeout.await;
        assert_eq!(result, Ok(Ok::<(), String>(())));

        // No return before timeout
        let elapsed_timeout = tokio::time::timeout(std::time::Duration::from_millis(500), async {
            tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
        });
        let result = elapsed_timeout.await;
        match result {
            Ok(_) => panic!("Expected error type"),
            Err(elapsed) => {
                assert!(elapsed.source().is_none());
            }
        }

        // Error before timeout
        let err_before_timeout =
            tokio::time::timeout(std::time::Duration::from_millis(500), async {
                return Err::<(), String>("This is a real error!".to_owned());
            });
        let result = err_before_timeout.await;
        match result {
            Ok(_) => panic!("Expected error type"),
            Err(elapsed) => {
                assert!(elapsed.source().is_some());
            }
        }
    }
}
