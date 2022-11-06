use crate::{
    nn::{Bbox, InferModel, UltrafaceModel},
    pubsub::{BytesReceiver, BytesSender, MpscBytesReceiver, NamedPubSub},
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
use tokio::{
    sync::{broadcast, mpsc, Mutex},
    task::JoinHandle,
};

pub type RecvSendPair = (MpscBytesReceiver, BytesSender);

pub struct InferBroker {
    channel_map: Mutex<HashMap<String, RecvSendPair>>,
    infer_queue_tx: mpsc::Sender<(Vec<u8>, BytesSender)>,
    infer_task: JoinHandle<()>,
}

pub struct Inferer {
    model: UltrafaceModel,
    infer_queue_rx: mpsc::Receiver<(Vec<u8>, BytesSender)>,
}

impl Inferer {
    pub fn new(
        model: UltrafaceModel,
        infer_queue_rx: mpsc::Receiver<(Vec<u8>, BytesSender)>,
    ) -> Self {
        Self {
            model,
            infer_queue_rx,
        }
    }

    pub async fn run(&mut self) {
        loop {
            if let Some((frame, infered_tx)) = self.infer_queue_rx.recv().await {
                let (width, height) = (1280, 720);
                match self.process_frame(frame, width, height) {
                    Err(err) => log::error!("Error in process frame: {}", err),
                    Ok(infered) => {
                        if let Err(err) = infered_tx.send(infered) {
                            log::error!("infered_tx.send error: {}", err);
                        }
                    }
                }
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
}

impl InferBroker {
    pub async fn new() -> Self {
        let model = UltrafaceModel::new(crate::nn::UltrafaceVariant::W320H240)
            .await
            .expect("Initialize model");
        let (infer_queue_tx, infer_queue_rx) = mpsc::channel(10);
        let infer_task = tokio::spawn(async move {
            let mut inferer = Inferer::new(model, infer_queue_rx);
            loop {
                inferer.run().await;
            }

            ()
        });
        Self {
            channel_map: Mutex::new(HashMap::new()),
            infer_queue_tx,
            infer_task,
        }
    }

    pub async fn subscribe_img_stream(
        &self,
        name: &str,
        pubsub: &NamedPubSub,
    ) -> Result<BytesReceiver, Error> {
        let mut channel_map = self.channel_map.lock().await;

        match channel_map.get(name) {
            Some((_, infered_tx)) => Ok(infered_tx.subscribe()),
            None => {
                // We need to retrieve the receiver for this channel
                match pubsub.get_mpsc_receiver(name).await {
                    Some(img_rx) => {
                        let (infered_tx, infered_rx) = broadcast::channel(10);
                        channel_map.insert(name.to_owned(), (img_rx, infered_tx));
                        Ok(infered_rx)
                    }
                    None => Err(
                        SimpleError::new("Could not get mpsc recv end, cannot subscribe").into(),
                    ),
                }
            }
        }
    }

    pub async fn run(&self) {
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(10));
        loop {
            {
                let mut channel_map = self.channel_map.lock().await;
                // TODO: What to do about names to remove?
                // let mut names_to_remove = Vec::with_capacity(0);
                {
                    for (name, (img_rx, infered_tx)) in channel_map.iter_mut() {
                        // TODO: Parallel await?
                        let recv_with_timeout =
                            tokio::time::timeout(std::time::Duration::from_millis(1000), async {
                                img_rx.recv().await
                            });
                        match recv_with_timeout.await {
                            Ok(Some(img)) => {
                                match self.infer_queue_tx.send((img, infered_tx.clone())).await {
                                    Ok(()) => log::warn!("Send frame of {} to inferer", &name),
                                    Err(err) => log::debug!(
                                        "Could not end fame of {} inferer: {}",
                                        &name,
                                        &err,
                                    ),
                                }
                            }
                            Ok(None) => {
                                log::error!("data socket closed for {}", &name);
                            }
                            Err(elapsed) => {
                                use std::error::Error;
                                match elapsed.source() {
                                    None => log::info!("Receive timed out for {}", &name),
                                    Some(err) => log::info!("Receive error for {}: {}", &name, err),
                                }
                            }
                        }
                    }
                }

                // for name in names_to_remove {
                //     log::info!("Removing channel {}", &name);
                //     channel_map.remove(&name);
                // }
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
