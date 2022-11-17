use crate::{
    nn::{Bbox, InferModel, UltrafaceModel},
    pubsub::{BytesReceiver, BytesSender, MpscBytesReceiver, NamedPubSub},
    Error,
};
use image::{Rgb, RgbImage};
use imageproc::drawing::{draw_hollow_rect, draw_text};
use imageproc::rect::Rect;
use lazy_static::lazy_static;
use simple_error::SimpleError;
use std::collections::HashMap;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    sync::{broadcast, mpsc, Mutex},
    task::JoinHandle,
};

pub type RecvSendPair = (MpscBytesReceiver, BytesSender);

pub struct InferBroker {
    channel_map: Mutex<HashMap<String, RecvSendPair>>,
    infer_queue_tx: mpsc::Sender<(Box<Vec<u8>>, BytesSender, String)>,
    unsendable_rx: Mutex<mpsc::Receiver<String>>,
    infer_task: JoinHandle<()>,
    pubsub: Arc<NamedPubSub>,
}

impl InferBroker {
    pub async fn new(pubsub: Arc<NamedPubSub>) -> Self {
        let model = UltrafaceModel::new(crate::nn::UltrafaceVariant::W320H240)
            .await
            .expect("Initialize model");
        let (infer_queue_tx, infer_queue_rx) = mpsc::channel(1);
        let (unsendable_tx, unsendable_rx) = mpsc::channel(20);
        let infer_task = tokio::spawn(async move {
            let mut inferer = Inferer::new(model, infer_queue_rx, unsendable_tx);
            loop {
                inferer.run().await;
            }

            ()
        });
        Self {
            channel_map: Mutex::new(HashMap::new()),
            infer_queue_tx,
            infer_task,
            unsendable_rx: Mutex::new(unsendable_rx),
            pubsub,
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
        let mut interval = tokio::time::interval(Duration::from_millis(10));
        loop {
            {
                let mut channel_map = self.channel_map.lock().await;
                {
                    for (name, (img_rx, infered_tx)) in channel_map.iter_mut() {
                        // TODO: Parallel await?
                        let recv_with_timeout =
                            tokio::time::timeout(Duration::from_millis(200), async {
                                img_rx.recv().await
                            });
                        match recv_with_timeout.await {
                            Ok(Some(img)) => {
                                match self
                                    .infer_queue_tx
                                    .send((img, infered_tx.clone(), name.clone()))
                                    .await
                                {
                                    Ok(()) => log::debug!("Send frame of {} to inferer", &name),
                                    Err(err) => log::debug!(
                                        "Could not end fame of {} inferer: {}",
                                        &name,
                                        &err,
                                    ),
                                }
                            }
                            Ok(None) => {
                                log::warn!("data socket closed for {}", &name);
                            }
                            Err(elapsed) => {
                                use std::error::Error;
                                match elapsed.source() {
                                    None => log::info!("Receive timed out for {}", &name),
                                    Some(err) => log::warn!("Receive error for {}: {}", &name, err),
                                }
                            }
                        }
                    }
                }

                let mut unsendable_rx = self.unsendable_rx.lock().await;
                while let Ok(name_to_remove) = unsendable_rx.try_recv() {
                    log::info!("Returning channel {} to pubsub", &name_to_remove);
                    if let Some((img_rx, _)) = channel_map.remove(&name_to_remove) {
                        self.pubsub
                            .return_mpsc_receiver(&name_to_remove, img_rx)
                            .await;
                    }
                }
            }
            interval.tick().await;
        }
    }
}

pub struct Inferer {
    model: UltrafaceModel,
    infer_queue_rx: mpsc::Receiver<(Box<Vec<u8>>, BytesSender, String)>,
    unsendable_tx: mpsc::Sender<String>,
}

impl Inferer {
    pub fn new(
        model: UltrafaceModel,
        infer_queue_rx: mpsc::Receiver<(Box<Vec<u8>>, BytesSender, String)>,
        unsendable_tx: mpsc::Sender<String>,
    ) -> Self {
        Self {
            model,
            infer_queue_rx,
            unsendable_tx,
        }
    }

    pub async fn run(&mut self) {
        loop {
            if let Some((frame, infered_tx, name)) = self.infer_queue_rx.recv().await {
                let (width, height) = (1280, 720);
                // This is a case of the compiler being too strict. If we move the
                // `unsendable_tx.send(...).await` into the error case, it complains
                // about the `err` variable of type `dyn StdError` not being Send.
                // We scope the evaluation so that the variable is freed before we
                // await on sending.
                let remove_name = {
                    match self.process_frame(frame, width, height) {
                        Err(err) => {
                            log::error!("Error in process frame: {}", err);
                            false
                        }
                        Ok(infered) => {
                            if let Err(err) = infered_tx.send(infered) {
                                log::info!("Could not send infered image of {}: {}", &name, err);
                                true
                            } else {
                                false
                            }
                        }
                    }
                };

                if remove_name {
                    if let Err(_) = self.unsendable_tx.send(name).await {
                        log::error!("Could not send name to remove");
                    }
                }
            }
        }
    }

    fn process_frame(
        &self,
        frame: Box<Vec<u8>>,
        width: u32,
        height: u32,
    ) -> Result<Vec<u8>, Error> {
        let start = Instant::now();
        let frame: RgbImage = turbojpeg::decompress_image(frame.as_slice())?;
        log::debug!("Decode frame took {:?}", start.elapsed());
        let start = Instant::now();
        let bboxes_with_confidences = self.infer_faces(frame.clone())?;
        log::debug!("Infer frame took {:?}", start.elapsed());

        let start = Instant::now();
        let frame = draw_bboxes_on_image(frame, bboxes_with_confidences, width, height);
        log::debug!("draw_bbox took {:?}", start.elapsed());

        let start = Instant::now();
        let buf = turbojpeg::compress_image(&frame, 95, turbojpeg::Subsamp::Sub2x2)?;
        log::debug!("Encode as JPG took {:?}", start.elapsed());

        Ok(buf.to_vec())
    }

    fn infer_faces(&self, frame: RgbImage) -> Result<Vec<(Bbox, f32)>, Error> {
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
        assert_eq!(
            result,
            Ok(Err::<(), String>("This is a real error!".to_owned()))
        );
    }
}
