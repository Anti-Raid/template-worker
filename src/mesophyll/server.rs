use std::sync::Arc;

use dashmap::DashMap;
use futures::{SinkExt, StreamExt};
use khronos_runtime::primitives::event::CreateEvent;
use tokio::{net::{TcpListener, TcpStream}, select, spawn, sync::{mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel}, oneshot::{Receiver, Sender, channel}}};
use tokio_tungstenite::{WebSocketStream, accept_async, tungstenite::{Message, Utf8Bytes, protocol::{CloseFrame, frame::coding::CloseCode}}};
use tokio_util::sync::CancellationToken;

use crate::{mesophyll::message::MesophyllMessage, templatedb::attached_templates::TemplateOwner, worker::workerprocesscomm::WorkerProcessCommDispatchResult};

/// Data associated with a Mesophyll connection
struct ConnectionData {
    queue: UnboundedSender<MesophyllMessage>,
    id: usize,
}

#[derive(Clone)]
pub struct MesophyllServer {
    cancel: CancellationToken,
    idents: Arc<DashMap<usize, String>>,
    send_queues: Arc<DashMap<usize, ConnectionData>>,
    response_handlers: Arc<DashMap<u64, Sender<MesophyllClientServerResponse>>>,
}

#[allow(dead_code)]
impl MesophyllServer {
    /// Create a new Mesophyll server
    pub async fn new(addr: String) -> Result<Self, crate::Error> {
        let s = Self {
            cancel: CancellationToken::new(),
            idents: Arc::new(DashMap::new()),
            send_queues: Arc::new(DashMap::new()),
            response_handlers: Arc::new(DashMap::new()),
        };

        let s_ref = s.clone();
        let listener = TcpListener::bind(addr).await?;

        spawn(async move {
            s_ref.serve(listener).await;
        });

        Ok(s)
    }

    pub fn set_ident(&self, id: usize, session_key: String) {
        self.idents.insert(id, session_key);
    }

    async fn serve(&self, listener: TcpListener) {
        loop {
            select! {
                Ok((stream, _)) = listener.accept() => {
                    log::info!("New Mesophyll transport connection established");

                    let (tx, rx) = unbounded_channel();

                    // Attempt a websocket connection handshake
                    let s = match accept_async(stream).await {
                        Ok(s) => s,
                        Err(e) => {
                            log::error!("Failed to accept Mesophyll websocket connection: {}", e);
                            continue;
                        }
                    };

                    let s_ref = self.clone();
                    spawn(async move {
                        s_ref.handle(s, tx, rx).await;
                    });
                }
                _ = self.cancel.cancelled() => {
                    log::info!("Mesophyll transport shutting down");
                    return;
                }
            }
        }
    }

    async fn handle(&self, stream: WebSocketStream<TcpStream>, tx: UnboundedSender<MesophyllMessage>, mut rx: UnboundedReceiver<MesophyllMessage>) {
        let (mut stream_tx, mut stream_rx) = stream.split();
        let mut id = None;
        loop {
            select! {
                Some(msg) = rx.recv() => {
                    let Ok(bytes) = serde_json::to_string(&msg) else {
                        log::error!("Failed to serialize Mesophyll message");
                        continue;
                    };

                    stream_tx.send(Message::Text(Utf8Bytes::from(bytes))).await.unwrap_or_else(|e| {
                        log::error!("Failed to send Mesophyll message: {}", e);
                    });
                }
                Some(Ok(msg)) = stream_rx.next() => {
                    match msg {
                        Message::Text(txt) => {
                            let Ok(msg) = serde_json::from_str::<MesophyllMessage>(&txt) else {
                                log::error!("Failed to deserialize Mesophyll message");
                                continue;
                            };

                            let self_ref = self.clone();

                            // TODO: Decide between spawning a new task or just doing it directly not in task
                            match self_ref.process_message(tx.clone(), msg).await {
                                Ok(MesophyllStateChange::Identified { id: new_id }) => {
                                    id = Some(new_id);
                                }
                                Ok(MesophyllStateChange::None) => {}
                                Err(e) => {
                                    log::error!("Error processing Mesophyll message: {}", e);
                                }
                            }
                        }
                        Message::Close(frame) => {
                            log::info!("Mesophyll transport connection closed: {:?}", frame);
                            if let Some(conn_id) = id {
                                self.send_queues.remove(&conn_id);
                            }
                            return;
                        }
                        _ => {
                            log::warn!("Mesophyll transport received unsupported message type");
                        }
                    }
                }
                _ = self.cancel.cancelled() => {
                    log::info!("Mesophyll transport connection shutting down");
                    if let Some(conn_id) = id {
                        self.send_queues.remove(&conn_id);
                    }
                    let mut stream = stream_tx.reunite(stream_rx).expect("Failed to reunite websocket stream");
                    if let Err(e) = stream.close(Some(CloseFrame {
                        code: CloseCode::Again,
                        reason: "Server shutting down".into(),
                    })).await {
                        log::error!("Error shutting down Mesophyll transport connection: {}", e);
                    }
                    return;
                }
            }
        }
    }

    async fn process_message(&self, tx: UnboundedSender<MesophyllMessage>, event: MesophyllMessage) -> Result<MesophyllStateChange, crate::Error> {
        match event {
            MesophyllMessage::Identify { id, session_key } => {
                let Some(stored_key) = self.idents.get(&id) else {
                    log::warn!("Mesophyll server received Identify message with unknown id: {}", id);
                    return Ok(MesophyllStateChange::None);
                };

                if *stored_key.value() != session_key {
                    log::warn!("Mesophyll server received Identify message with invalid session key for id: {}", id);
                    return Ok(MesophyllStateChange::None);
                }

                log::info!("Mesophyll server identified client with id: {}", id);
                self.send_queues.insert(
                    id,
                    ConnectionData {
                        queue: tx,
                        id
                    },
                );

                return Ok(MesophyllStateChange::Identified { id });
            }
            MesophyllMessage::Ready {} => {
                // Nothing server can do.
                log::warn!("Mesophyll server received unexpected Ready message");
            }
            MesophyllMessage::Relay { msg, req_id } => {
                // Relay message to all connected clients
                for entry in self.send_queues.iter() {
                    let conn = entry.value();
                    match conn.queue.send(MesophyllMessage::Relay { msg: msg.clone(), req_id }) {
                        Ok(_) => {}
                        Err(e) => {
                            log::error!("Failed to relay Mesophyll message to client {}: {}", conn.id, e);
                        }
                    }
                }
            }
            MesophyllMessage::DispatchEvent { .. } => {
                // Nothing server can do.
                log::warn!("Mesophyll server received unsupported DispatchEvent message");
            }
            MesophyllMessage::DispatchScopedEvent { .. } => {
                // Nothing server can do.
                log::warn!("Mesophyll server received unsupported DispatchScopedEvent message");
            }
            MesophyllMessage::ResponseAck { req_id } => {
                if let Some(handler) = self.response_handlers.remove(&req_id) {
                    let _ = handler.1.send(MesophyllClientServerResponse::Ack);
                } else {
                    log::warn!("Mesophyll server received ResponseAck for unknown req_id: {}", req_id);
                }
            }
            MesophyllMessage::ResponseError { error, req_id } => {
                if let Some(handler) = self.response_handlers.remove(&req_id) {
                    let _ = handler.1.send(MesophyllClientServerResponse::Error(error));
                } else {
                    log::warn!("Mesophyll server received ResponseError for unknown req_id: {}", req_id);
                }
            }
            MesophyllMessage::ResponseDispatchResult { result, req_id } => {
                if let Some(handler) = self.response_handlers.remove(&req_id) {
                    let _ = handler.1.send(MesophyllClientServerResponse::DispatchResult(result));
                } else {
                    log::warn!("Mesophyll server received ResponseDispatchResult for unknown req_id: {}", req_id);
                }
            }
        }

        Ok(MesophyllStateChange::None)
    }

    // Sends a message to a specific Mesophyll client
    pub async fn send(&self, id: usize, message: MesophyllMessage) -> Result<(), crate::Error> {
        let Some(conn) = self.send_queues.get(&id) else {
            return Err("Mesophyll client not connected".into());
        };

        match conn.queue.send(message) {
            Ok(_) => Ok(()),
            Err(e) => {
                Err(format!("Failed to send Mesophyll message to client {}: {}", id, e).into())
            }
        }
    }

    // Internal helper method to register a response handler for the next req_id, returning the req_id
    fn register_response_handler(&self) -> (u64, Receiver<MesophyllClientServerResponse>) {
        let mut req_id = rand::random::<u64>();
        loop {
            if !self.response_handlers.contains_key(&req_id) {
                break;
            } else {
                req_id = rand::random::<u64>();
            }
        }

        let (tx, rx) = channel();
        self.response_handlers.insert(req_id, tx);
        (req_id, rx)
    }

    // Dispatches an event to a specific Mesophyll client and waits for a response
    pub async fn dispatch_event(&self, worker_id: usize, owner: TemplateOwner, event: CreateEvent, scopes: Option<Vec<String>>) -> Result<WorkerProcessCommDispatchResult, crate::Error> {
        let (req_id, rx) = self.register_response_handler();

        let message = if let Some(scopes) = scopes {
            MesophyllMessage::DispatchScopedEvent { id: owner, event, scopes, req_id }
        } else {
            MesophyllMessage::DispatchEvent { id: owner, event, req_id }
        };

        self.send(worker_id, message).await?;

        match rx.await? {
            MesophyllClientServerResponse::DispatchResult(result) => Ok(result),
            MesophyllClientServerResponse::Error(err) => Err(err.into()),
            MesophyllClientServerResponse::Ack => Err("Unexpected Ack response for DispatchEvent".into()),
        }
    }
}

/// Result of processing a Mesophyll message
enum MesophyllStateChange {
    None,
    Identified { id: usize },
}

/// Response from Mesophyll client to server
enum MesophyllClientServerResponse {
    Ack,
    Error(String),
    DispatchResult(WorkerProcessCommDispatchResult)
}