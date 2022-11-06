use std::collections::HashMap;

use tokio::sync::{broadcast, Mutex};

pub type BytesSender = broadcast::Sender<Vec<u8>>;
pub type BytesReceiver = broadcast::Receiver<Vec<u8>>;

pub struct NamedPubSub {
    map: Mutex<HashMap<String, BytesSender>>,
}

impl NamedPubSub {
    pub fn new() -> Self {
        Self {
            map: Mutex::new(HashMap::new()),
        }
    }

    pub async fn get_broadcast_sender(&self, name: &str) -> broadcast::Sender<Vec<u8>> {
        let mut map = self.map.lock().await;
        match map.get(name) {
            Some(tx) => tx.clone(),
            None => {
                let (tx, _) = broadcast::channel(20);
                map.insert(name.to_owned(), tx.clone());
                tx
            }
        }
    }

    pub async fn get_broadcast_receiver(&self, name: &str) -> broadcast::Receiver<Vec<u8>> {
        let mut map = self.map.lock().await;
        match map.get(name) {
            Some(tx) => tx.subscribe(),
            None => {
                let (tx, rx) = broadcast::channel(20);
                map.insert(name.to_owned(), tx);
                rx
            }
        }
    }
}
