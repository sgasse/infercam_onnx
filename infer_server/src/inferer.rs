use std::collections::HashMap;

use tokio::sync::{broadcast, Mutex};

use crate::pubsub::{BytesReceiver, BytesSender};

pub type RecvSendPair = (BytesReceiver, BytesSender);

pub struct Inferer {
    channel_map: Mutex<HashMap<String, RecvSendPair>>,
}

impl Inferer {
    pub fn new() -> Self {
        Self {
            channel_map: Mutex::new(HashMap::new()),
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

    pub async fn run(&self) {
        loop {
            {
                let mut channel_map = self.channel_map.lock().await;
                let mut names_to_remove = Vec::with_capacity(0);
                {
                    for (name, (img_rx, infered_tx)) in channel_map.iter_mut() {
                        if let Ok(img) = img_rx.recv().await {
                            let infered = img;
                            if let Err(_) = infered_tx.send(infered) {
                                log::info!("No listener for {}", name);
                                names_to_remove.push(name.clone());
                            }
                        }
                    }
                }

                for name in names_to_remove {
                    log::info!("Removing channel {}", &name);
                    channel_map.remove(&name);
                }
            }
        }
    }
}
