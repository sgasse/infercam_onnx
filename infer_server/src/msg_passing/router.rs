use std::{collections::HashMap, time::Duration};

use bytes::Bytes;
use futures::{stream::FuturesUnordered, StreamExt};
use tokio::sync::mpsc::{self, Receiver, Sender};

use crate::hashed;

pub struct RegistryComm {
    pub frame_stream_listener_tx: Sender<(String, Sender<Bytes>)>,
    pub infered_stream_listener_tx: Sender<(String, Sender<Bytes>)>,
    pub tcp_tasks_comm_tx: TcpTaskDataSender,
}

pub type TcpTaskDataSender = Sender<(String, Sender<Vec<Sender<Bytes>>>)>;

/// Router registry.
///
/// In this approach, every HTTP stream creates its own tx/rx channel pair. The
/// tx end is stored in a map. Infer streams also generate a tx/rx channel pair
/// with the tx end stored in a separate map.
/// When a new data socket is connected, it creates a channel to receive updates
/// from the registry. Initially, it gets a clone of all the tx ends of streams
/// which want to receive updates of this data socket.
/// For every tx/rx channel pair, we spawn a future with a clone of the tx
/// end that completes when the rx end is dropped. This way, we notice if a
/// channel is closed and we should remove the sender from the map.
pub struct Registry {
    frames_sender_map: HashMap<u64, Vec<Sender<Bytes>>>,
    infered_frames_sender_map: HashMap<u64, Vec<Sender<Bytes>>>,
    tcp_tasks_comm: HashMap<u64, Sender<Vec<Sender<Bytes>>>>,

    frame_stream_listener_tx: Sender<(String, Sender<Bytes>)>,
    frame_stream_listener_rx: Option<Receiver<(String, Sender<Bytes>)>>,

    infered_stream_listener_tx: Sender<(String, Sender<Bytes>)>,
    infered_stream_listener_rx: Option<Receiver<(String, Sender<Bytes>)>>,

    tcp_tasks_comm_tx: Sender<(String, Sender<Vec<Sender<Bytes>>>)>,
    tcp_tasks_comm_rx: Option<Receiver<(String, Sender<Vec<Sender<Bytes>>>)>>,
}

impl Registry {
    pub fn new() -> Self {
        let (frame_stream_listener_tx, frame_stream_listener_rx) = mpsc::channel(20);
        let (infered_stream_listener_tx, infered_stream_listener_rx) = mpsc::channel(20);
        let (tcp_tasks_comm_tx, tcp_tasks_comm_rx) = mpsc::channel(20);

        Self {
            frames_sender_map: HashMap::new(),
            infered_frames_sender_map: HashMap::new(),
            tcp_tasks_comm: HashMap::new(),
            frame_stream_listener_tx,
            frame_stream_listener_rx: Some(frame_stream_listener_rx),
            infered_stream_listener_tx,
            infered_stream_listener_rx: Some(infered_stream_listener_rx),
            tcp_tasks_comm_tx,
            tcp_tasks_comm_rx: Some(tcp_tasks_comm_rx),
        }
    }

    pub fn get_comm(&self) -> RegistryComm {
        RegistryComm {
            frame_stream_listener_tx: self.frame_stream_listener_tx.clone(),
            infered_stream_listener_tx: self.infered_stream_listener_tx.clone(),
            tcp_tasks_comm_tx: self.tcp_tasks_comm_tx.clone(),
        }
    }

    pub async fn run(&mut self) {
        if let (
            Some(mut frame_stream_listener_rx),
            Some(mut infered_stream_listener_rx),
            Some(mut tcp_tasks_comm_rx),
        ) = (
            self.frame_stream_listener_rx.take(),
            self.infered_stream_listener_rx.take(),
            self.tcp_tasks_comm_rx.take(),
        ) {
            let mut expired_sender_ids = FuturesUnordered::new();

            loop {
                tokio::select! {
                    res = expired_sender_ids.next() => {
                        match res {
                            None => {
                                tokio::time::sleep(Duration::from_millis(500)).await;
                            },
                            Some(map_id) => match map_id {
                                MapId::Frame(id) => {
                                    log::info!("Removing expired frame sender for ID {}", id);
                                    self.frames_sender_map.remove(&id);
                                }
                                MapId::Infered(id) => {
                                    log::info!("Removing expired infered frame sender for ID {}", id);
                                    self.infered_frames_sender_map.remove(&id);
                                }
                                MapId::Tcp(id) => {
                                    log::info!("Removing expired TCP communicator for ID {}", id);
                                    self.tcp_tasks_comm.remove(&id);
                                }
                            }
                        }

                    }

                    res = frame_stream_listener_rx.recv() => {
                        match res {
                            None => panic!("stream ended"),
                            Some((name, tx)) => {
                                let id = hashed(&name);
                                {
                                    let tx = tx.clone();
                                    expired_sender_ids.push(
                                        Box::pin(async move {
                                            tx.closed().await;
                                            MapId::Frame(id)
                                        }) as Pin<Box<dyn Future<Output = MapId> + Send + 'static>>
                                    );
                                }
                                self.frames_sender_map.entry(id).or_insert_with(|| Vec::with_capacity(1)).push(tx.clone());
                                if let Some(tcp_task) = self.tcp_tasks_comm.get(&id) {
                                    // TODO: handle error
                                    tcp_task.send(vec![tx]).await.ok();
                                }
                            }
                        }

                    },

                    res = infered_stream_listener_rx.recv() => {
                        match res {
                            None => panic!("stream ended"),
                            Some((name, tx)) => {
                                let id = hashed(&name);
                                {
                                    let tx = tx.clone();
                                    expired_sender_ids.push(
                                        Box::pin(async move {
                                            tx.closed().await;
                                            MapId::Infered(id)
                                        }) as Pin<Box<dyn Future<Output = MapId> + Send + 'static>>
                                    );
                                }
                                self.infered_frames_sender_map.entry(id).or_insert_with(|| Vec::with_capacity(1)).push(tx.clone());
                                if let Some(tcp_task) = self.tcp_tasks_comm.get(&id) {
                                    // TODO: handle error
                                    tcp_task.send(vec![tx]).await.ok();
                                }
                            }
                        }

                    },

                    res = tcp_tasks_comm_rx.recv() => {
                        match res {
                            None => panic!("stream ended"),
                            Some((name, tx)) => {
                                let id = hashed(&name);
                                {
                                    let tx = tx.clone();
                                    expired_sender_ids.push(
                                        Box::pin(async move {
                                            tx.closed().await;
                                            MapId::Tcp(id)
                                        }) as Pin<Box<dyn Future<Output = MapId> + Send + 'static>>
                                    );
                                }
                                self.tcp_tasks_comm.insert(id, tx.clone());
                                if let Some(senders) = self.frames_sender_map.get(&id) {
                                    tx.send(senders.clone()).await.ok();
                                }
                                if let Some(senders) = self.infered_frames_sender_map.get(&id) {
                                    tx.send(senders.clone()).await.ok();
                                }
                            }
                        }

                    },
                }
            }
        } else {
            panic!("trying to run uninitialized registry");
        }
    }
}

enum MapId {
    Frame(u64),
    Infered(u64),
    Tcp(u64),
}
