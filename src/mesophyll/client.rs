use std::time::Duration;

use futures::{SinkExt, StreamExt};
use tokio::time::interval;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::{mesophyll::{MESOPHYLL_DEFAULT_HEARTBEAT_MS, message::{ClientMessage, ServerMessage}}, worker::workerdispatch::WorkerDispatch};

/// Mesophyll client, NOT THREAD SAFE
#[derive(Clone)]
pub struct MesophyllClient {
    dispatch: WorkerDispatch,
    addr: String,
}

#[allow(dead_code)]
impl MesophyllClient {
    /// Creates a new Mesophyll client
    pub fn new(addr: String, token: String, dispatch: WorkerDispatch) -> Self {
        let s = Self {
            dispatch,
            addr: format!("{}?token={}", addr, token),
        };

        let self_ref = s.clone();
        tokio::task::spawn_local(async move {
            loop {
                if let Err(e) = self_ref.handle_task().await {
                    log::error!("Mesophyll client task error: {}", e);
                }
            }
        });

        s
    }

    async fn handle_task(&self) -> Result<(), crate::Error> {
        // Connect to the masters IP/port
        let (ws_stream, _) = connect_async(&self.addr).await.map_err(|e| format!("Failed to connect: {:?}", e))?;
        let (mut stream_tx, mut stream_rx) = ws_stream.split();
        let mut hb_timer = interval(Duration::from_millis(MESOPHYLL_DEFAULT_HEARTBEAT_MS));
        loop {
            tokio::select! {
                Some(Ok(msg)) = stream_rx.next() => {
                    match msg {
                        Message::Text(text) => {
                            if let Ok(server_msg) = serde_json::from_str::<ServerMessage>(&text) {
                                match server_msg {
                                    ServerMessage::DispatchEvent { id, event, req_id } => {
                                        let result = self.dispatch.dispatch_event(id, event).await;
                                        let response = ClientMessage::DispatchResponse {
                                            req_id,
                                            result: result.map_err(|e| e.to_string()),
                                        };
                                        let json = serde_json::to_string(&response)
                                            .map_err(|e| format!("Failed to serialize DispatchResponse: {}", e))?;
                                        stream_tx.send(Message::Text(json.into())).await
                                            .map_err(|e| format!("Failed to send DispatchResponse: {}", e))?;
                                    },
                                    ServerMessage::Hello { heartbeat_interval_ms } => {
                                        log::info!("Mesophyll client received Hello, heartbeat interval: {} ms", heartbeat_interval_ms);
                                        hb_timer = interval(Duration::from_millis(heartbeat_interval_ms));
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                _ = hb_timer.tick() => {
                    let heartbeat = ClientMessage::Heartbeat {
                        vm_count: self.dispatch.vm_manager().len(),
                    };
                    let json = serde_json::to_string(&heartbeat)
                        .map_err(|e| format!("Failed to serialize Heartbeat: {}", e))?;
                    stream_tx.send(Message::Text(json.into())).await
                        .map_err(|e| format!("Failed to send Heartbeat: {}", e))?;
                }
            }
        }
    }
}