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
                            tokio::time::timeout(std::time::Duration::from_millis(500), async {
                                img_rx.recv().await
                            });
                        match recv_with_timeout.await {
                            Ok(Ok(img)) => {
                                let infered = img;
                                if let Err(_) = infered_tx.send(infered) {
                                    log::info!("No listener for {}", name);
                                    names_to_remove.push(name.clone());
                                }
                            }
                            // Elapsed or failed to receive
                            // Handle granularily?
                            _ => (),
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
