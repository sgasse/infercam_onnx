use std::collections::HashMap;

use tokio::sync::{broadcast, mpsc, Mutex};

pub type BytesSender = broadcast::Sender<Vec<u8>>;
pub type BytesReceiver = broadcast::Receiver<Vec<u8>>;
pub type MpscBytesSender = mpsc::Sender<Vec<u8>>;
pub type MpscBytesReceiver = mpsc::Receiver<Vec<u8>>;

pub struct NamedPubSub {
    broadcast_map: Mutex<HashMap<String, BytesSender>>,
    mpsc_map: Mutex<HashMap<String, (MpscBytesSender, Option<MpscBytesReceiver>)>>,
}

impl NamedPubSub {
    pub fn new() -> Self {
        Self {
            broadcast_map: Mutex::new(HashMap::new()),
            mpsc_map: Mutex::new(HashMap::new()),
        }
    }

    pub async fn get_broadcast_sender(&self, name: &str) -> BytesSender {
        let mut map = self.broadcast_map.lock().await;
        match map.get(name) {
            Some(tx) => tx.clone(),
            None => {
                let (tx, _) = broadcast::channel(20);
                map.insert(name.to_owned(), tx.clone());
                tx
            }
        }
    }

    pub async fn get_broadcast_receiver(&self, name: &str) -> BytesReceiver {
        let mut map = self.broadcast_map.lock().await;
        match map.get(name) {
            Some(tx) => tx.subscribe(),
            None => {
                let (tx, rx) = broadcast::channel(20);
                map.insert(name.to_owned(), tx);
                rx
            }
        }
    }

    pub async fn get_mpsc_sender(&self, name: &str) -> MpscBytesSender {
        let mut map = self.mpsc_map.lock().await;
        match map.get(name) {
            Some((tx, _)) => tx.clone(),
            None => {
                let (tx, rx) = mpsc::channel(1);
                map.insert(name.to_owned(), (tx.clone(), Some(rx)));
                tx
            }
        }
    }

    pub async fn get_mpsc_receiver(&self, name: &str) -> Option<MpscBytesReceiver> {
        let mut map = self.mpsc_map.lock().await;
        match map.get_mut(name) {
            Some((_, rx_opt)) => rx_opt.take(),
            None => {
                let (tx, rx) = mpsc::channel(1);
                map.insert(name.to_owned(), (tx, None));
                Some(rx)
            }
        }
    }

    pub async fn return_mpsc_receiver(&self, name: &str, rx: MpscBytesReceiver) {
        let mut map = self.mpsc_map.lock().await;
        if let Some((_, rx_opt)) = map.get_mut(name) {
            rx_opt.replace(rx);
        }
    }
}

impl Default for NamedPubSub {
    fn default() -> Self {
        Self::new()
    }
}
