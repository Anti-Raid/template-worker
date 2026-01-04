use std::{sync::Arc, time::Duration};

use futures::{SinkExt, StreamExt, stream::FuturesUnordered};
use tokio::time::interval;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::{mesophyll::{MESOPHYLL_DEFAULT_HEARTBEAT_MS, message::{ClientMessage, ServerMessage}}, worker::{workerlike::WorkerLike, workerthread::WorkerThread}};

/// Mesophyll client, NOT THREAD SAFE
#[derive(Clone)]
pub struct MesophyllClient {
    wt: Arc<WorkerThread>,
    addr: String,
}

#[allow(dead_code)]
impl MesophyllClient {
    /// Creates a new Mesophyll client
    pub fn new(addr: String, token: String, wt: Arc<WorkerThread>) -> Self {
        let worker_id = wt.id();
        let s = Self {
            wt,
            addr: format!("{}/conn/{}?token={}", addr, worker_id, token),
        };

        let self_ref = s.clone();
        tokio::task::spawn_local(async move {
            loop {
                if let Err(e) = self_ref.handle_task().await {
                    log::error!("Mesophyll client task error: {}", e);
                }
                
                log::debug!("Mesophyll client reconnecting in 5 seconds...");
                tokio::time::sleep(Duration::from_secs(5)).await;
                log::debug!("Mesophyll client reconnecting now");
            }
        });

        s
    }

    async fn handle_task(&self) -> Result<(), crate::Error> {
        // Connect to the masters IP/port
        let (ws_stream, _) = connect_async(&self.addr).await.map_err(|e| format!("Failed to connect: {:?}", e))?;
        let (mut stream_tx, mut stream_rx) = ws_stream.split();
        let mut hb_timer = interval(Duration::from_millis(MESOPHYLL_DEFAULT_HEARTBEAT_MS));
        let mut dispatches = FuturesUnordered::new();
        loop {
            tokio::select! {
                Some(Ok(msg)) = stream_rx.next() => {
                    if let Message::Close(_) = msg {
                        log::info!("Mesophyll client connection closed by server");
                        return Ok(());
                    }

                    let Some(server_msg) = decode_message::<ServerMessage>(&msg) else {
                        continue;
                    };

                    match server_msg {
                        ServerMessage::Hello { heartbeat_interval_ms } => {
                            log::info!("Mesophyll client received Hello, heartbeat interval: {} ms", heartbeat_interval_ms);
                            hb_timer = interval(Duration::from_millis(heartbeat_interval_ms));
                        }
                        ServerMessage::DispatchEvent { id, event, req_id } => {
                            let fut = self.wt.dispatch_event(id, event);
                            dispatches.push(async move {
                                let resp = fut.await;
                                (req_id, resp)
                            });
                        },
                    }
                }
                Some((req_id, result)) = dispatches.next() => {
                    let Some(req_id) = req_id else {
                        continue;
                    };
                    let response = encode_message(&ClientMessage::DispatchResponse {
                        req_id,
                        result: result.map_err(|e| e.to_string()),
                    })?;
                    stream_tx.send(response).await
                        .map_err(|e| format!("Failed to send DispatchResponse: {}", e))?;
                }
                _ = hb_timer.tick() => {
                    let heartbeat = encode_message(&ClientMessage::Heartbeat {})?;
                    stream_tx.send(heartbeat).await
                        .map_err(|e| format!("Failed to send Heartbeat: {}", e))?;
                }
            }
        }
    }
}

fn encode_message<T: serde::Serialize>(msg: &T) -> Result<Message, crate::Error> {
    let json = serde_json::to_string(msg)
        .map_err(|e| format!("Failed to serialize Mesophyll message: {}", e))?;
    Ok(Message::Text(json.into()))
}

fn decode_message<T: for<'de> serde::Deserialize<'de>>(msg: &Message) -> Option<T> {
    match msg {
        Message::Text(text) => {
            serde_json::from_str::<T>(text).ok()
        }
        _ => None,
    }
}