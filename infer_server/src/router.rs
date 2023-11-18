use std::{collections::HashMap, sync::Mutex};

use anyhow::{bail, Result};
use common::protocol::ProtoMsg;

use crate::{
    broadcast_channel, hashed, BroadcastReceiver, BroadcastSender, StaticFrameReceiver,
    StaticImageSender,
};

use super::as_jpeg_stream_item;

pub struct FrameRouter {
    frames_broadcast_map: Mutex<HashMap<u64, BroadcastSender>>,
    infered_broadcast_map: Mutex<HashMap<u64, BroadcastSender>>,
    infer_tx: StaticImageSender,
}

impl FrameRouter {
    pub fn new(infer_tx: StaticImageSender) -> Self {
        Self {
            frames_broadcast_map: Mutex::new(HashMap::new()),
            infered_broadcast_map: Mutex::new(HashMap::new()),
            infer_tx,
        }
    }

    pub async fn run(&self, rx: StaticFrameReceiver) -> Result<()> {
        let mut frames_sender_map = HashMap::new();
        let mut infered_sender_map = HashMap::new();

        loop {
            {
                let mut frames_broadcast_map = self.frames_broadcast_map.lock().unwrap();
                frames_broadcast_map.retain(|_id, sender| sender.receiver_count() > 0);

                for (id, sender) in frames_broadcast_map.iter() {
                    frames_sender_map.insert(*id, sender.clone());
                }
                frames_sender_map.retain(|id, _sender| frames_broadcast_map.contains_key(id))
            }
            {
                let mut infered_broadcast_map = self.infered_broadcast_map.lock().unwrap();
                infered_broadcast_map.retain(|_id, sender| sender.receiver_count() > 0);

                for (id, sender) in infered_broadcast_map.iter() {
                    infered_sender_map.insert(*id, sender.clone());
                }
                infered_sender_map.retain(|id, _sender| infered_broadcast_map.contains_key(id))
            }

            for _ in 0..4 {
                match rx.recv_ref().await {
                    None => bail!("incoming frames channel closed"),
                    Some(data) => {
                        if let Ok(ProtoMsg::FrameMsg(proto_msg)) = ProtoMsg::deserialize(&data[..])
                        {
                            let id = hashed(&proto_msg.id);

                            if let Some(sender) = frames_sender_map.get(&id) {
                                sender.send(as_jpeg_stream_item(&proto_msg.data)).ok();
                            }

                            if let Some(sender) = infered_sender_map.get(&id) {
                                if let Ok(mut frame) = self.infer_tx.try_send_ref() {
                                    frame.0 = 1280;
                                    frame.1 = 720;
                                    frame.2.clear();
                                    frame.2.extend_from_slice(&proto_msg.data);
                                    frame.3 = Some(sender.clone());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn get_broadcast_receiver(&self, name: &str) -> BroadcastReceiver {
        let id = hashed(name);
        let mut frames_broadcast_map = self.frames_broadcast_map.lock().unwrap();

        if let Some(tx) = frames_broadcast_map.get(&id) {
            tx.subscribe()
        } else {
            let (tx, rx) = broadcast_channel();
            frames_broadcast_map.insert(id, tx);

            rx
        }
    }

    pub fn get_broadcast_sender(&self, name: &str) -> BroadcastSender {
        let id = hashed(name);
        let mut frames_broadcast_map = self.frames_broadcast_map.lock().unwrap();

        frames_broadcast_map
            .entry(id)
            .or_insert_with(|| {
                let (tx, _rx) = broadcast_channel();
                tx
            })
            .clone()
    }

    pub fn get_infered_receiver(&self, name: &str) -> BroadcastReceiver {
        let id = hashed(name);
        self.get_infered_receiver_by_id(id)
    }

    pub fn get_infered_sender(&self, name: &str) -> BroadcastSender {
        let id = hashed(name);
        self.get_infered_sender_by_id(id)
    }

    pub fn get_infered_receiver_by_id(&self, id: u64) -> BroadcastReceiver {
        let mut infered_broadcast_map = self.infered_broadcast_map.lock().unwrap();

        if let Some(tx) = infered_broadcast_map.get(&id) {
            tx.subscribe()
        } else {
            let (tx, rx) = broadcast_channel();
            infered_broadcast_map.insert(id, tx);

            rx
        }
    }

    pub fn get_infered_sender_by_id(&self, id: u64) -> BroadcastSender {
        let mut infered_broadcast_map = self.infered_broadcast_map.lock().unwrap();

        infered_broadcast_map
            .entry(id)
            .or_insert_with(|| {
                let (tx, _rx) = broadcast_channel();
                tx
            })
            .clone()
    }
}
