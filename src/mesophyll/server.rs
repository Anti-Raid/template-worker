use std::{collections::HashMap, sync::{Arc, RwLock, Weak}, time::{Duration, Instant}};

use dashmap::DashMap;
use futures::{SinkExt, StreamExt};
use khronos_runtime::{primitives::event::CreateEvent, utils::khronos_value::KhronosValue};
use tokio::{select, spawn, sync::{Notify, mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel}, oneshot::{Receiver, Sender, channel}}};
use tokio_util::sync::CancellationToken;

use crate::{mesophyll::message::{ClientMessage, ServerMessage}, worker::workervmmanager::Id};

use axum::{
    extract::{Path, State, WebSocketUpgrade, ws::WebSocket, ws::Message},
    http::{StatusCode, HeaderMap},
    response::IntoResponse,
    routing::get,
    Router,
};


#[derive(Clone)]
pub struct MesophyllServer {
    cancel: CancellationToken,
    idents: Arc<HashMap<usize, String>>,
    conns: Arc<DashMap<usize, MesophyllServerConn>>,
}

impl MesophyllServer {
    pub async fn new(addr: String, idents: HashMap<usize, String>) -> Result<Self, crate::Error> {
        let s = Self {
            cancel: CancellationToken::new(),
            idents: Arc::new(idents),
            conns: Arc::new(DashMap::new()),
        };

        // Axum Router
        let app = Router::new()
            .route("/:worker_id", get(ws_handler))
            .with_state(s.clone());

        let listener = tokio::net::TcpListener::bind(addr).await?;
        
        log::info!("Mesophyll server listening...");
        
        // Spawn the server task
        spawn(async move {
            axum::serve(listener, app).await.expect("Mesophyll server failed");
        });

        Ok(s)
    }

    pub fn get_connection(&self, worker_id: usize) -> Option<MesophyllServerConn> {
        self.conns.get(&worker_id).map(|r| r.value().clone())
    }
}

/// WebSocket handler for Mesophyll server
async fn ws_handler(
    Path(worker_id): Path<usize>,
    State(state): State<MesophyllServer>,
    headers: HeaderMap,           // <--- 1. Extract Headers
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    let token = headers
        .get("X-Mesophyll-Token")
        .and_then(|val| val.to_str().ok())
        .unwrap_or("");

    let Some(expected_key) = state.idents.get(&worker_id) else {
        log::warn!("Connection attempt from unknown worker ID: {}", worker_id);
        return StatusCode::NOT_FOUND.into_response();
    };

    // Verify token of the worker trying to connect to us before upgrading
    if token != expected_key {
        log::warn!("Invalid token for worker {}", worker_id);
        return StatusCode::UNAUTHORIZED.into_response();
    }

    ws.on_upgrade(move |socket| handle_socket(socket, worker_id, state))
}

/// Handles a new Mesophyll server WebSocket connection
async fn handle_socket(socket: WebSocket, id: usize, state: MesophyllServer) {
    if state.conns.contains_key(&id) {
        log::warn!("Worker {id} reconnection - overwriting old connection.");
    }

    let weak_map = Arc::downgrade(&state.conns);
    let conn = MesophyllServerConn::new(id, socket, weak_map);

    state.conns.insert(id, conn);
    log::info!("Worker {} connected", id);
}

pub struct HeartbeatInfo {
    last_heartbeat: Instant,
    vm_count: u64,
}

/// A Mesophyll server connection
#[derive(Clone)]
pub struct MesophyllServerConn {
    id: usize,
    send_queue: UnboundedSender<ServerMessage>,
    dispatch_response_handlers: Arc<DashMap<u64, Sender<Result<KhronosValue, String>>>>,
    heartbeat_info: Arc<RwLock<Option<HeartbeatInfo>>>,
    heartbeat_notify: Arc<Notify>,
    cancel: CancellationToken,
    conns_map: Weak<DashMap<usize, MesophyllServerConn>>,
}

impl Drop for MesophyllServerConn {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

impl MesophyllServerConn {
    fn new(id: usize, stream: WebSocket, conns_map: Weak<DashMap<usize, MesophyllServerConn>>) -> Self {
        let (send_queue, recv_queue) = unbounded_channel();
        let s = Self {
            id,
            send_queue,
            dispatch_response_handlers: Arc::new(DashMap::new()),
            heartbeat_info: Arc::new(RwLock::new(None)),
            heartbeat_notify: Arc::new(Notify::new()),
            cancel: CancellationToken::new(),
            conns_map: conns_map.clone(),
        };

        let s_ref = s.clone();
        spawn(async move {
            s_ref.run_loop(stream, recv_queue).await;

            if let Some(map) = conns_map.upgrade() {
                map.remove(&id);
            }
        });
        s
    }

    async fn run_loop(&self, socket: WebSocket, mut rx: UnboundedReceiver<ServerMessage>) {
        let (mut stream_tx, mut stream_rx) = socket.split();

        loop {
            select! {
                Some(msg) = rx.recv() => {
                    let Ok(json) = serde_json::to_string(&msg) else { 
                        continue 
                    };
                    if let Err(e) = stream_tx.send(Message::Text(json.into())).await {
                        log::error!("Failed to send Mesophyll message to worker {}: {}", self.id, e);
                    }
                }
                Some(Ok(msg)) = stream_rx.next() => {
                    match msg {
                        Message::Text(text) => {
                            if let Ok(client_msg) = serde_json::from_str::<ClientMessage>(&text) {
                                self.handle_message(client_msg).await;
                            }
                        }
                        Message::Close(_) => {
                            log::info!("Mesophyll server connection {} closed by client", self.id);
                            break;
                        },
                        _ => {}
                    }
                }
                _ = self.cancel.cancelled() => {
                    log::info!("Mesophyll server connection {} cancelling", self.id);
                    break;
                }
                else => {
                    log::info!("Mesophyll server connection {} shutting down", self.id);
                    break;
                }
            }
        }
    }

    async fn handle_message(&self, msg: ClientMessage) {
        match msg {
            ClientMessage::DispatchResponse { req_id, result } => {
                if let Some((_, tx)) = self.dispatch_response_handlers.remove(&req_id) {
                    let _ = tx.send(result);
                }
            }
            ClientMessage::Heartbeat { vm_count } => {
                // SAFETY: We do not hold the lock across an await point
                let mut lock = self.heartbeat_info.write().expect("Failed to acquire heartbeat info write lock");
                *lock = Some(HeartbeatInfo {
                    last_heartbeat: Instant::now(),
                    vm_count,
                });
            }
        }
    }

    // Sends a message to the connected client
    fn send(&self, message: ServerMessage) -> Result<(), crate::Error> {
        match self.send_queue.send(message) {
            Ok(_) => Ok(()),
            Err(e) => {
                Err(format!("Failed to send Mesophyll message to client: {}", e).into())
            }
        }
    }

    // Internal helper method to register a response handler for the next req_id, returning the req_id
    fn register_dispatch_response_handler(&self) -> (u64, Receiver<Result<KhronosValue, String>>) {
        let mut req_id = rand::random::<u64>();
        loop {
            if !self.dispatch_response_handlers.contains_key(&req_id) {
                break;
            } else {
                req_id = rand::random::<u64>();
            }
        }

        let (tx, rx) = channel();
        self.dispatch_response_handlers.insert(req_id, tx);
        (req_id, rx)
    }

    // Dispatches an event and waits for a response
    pub async fn dispatch_event(&self, id: Id, event: CreateEvent) -> Result<KhronosValue, crate::Error> {
        let (req_id, rx) = self.register_dispatch_response_handler();

        let message = ServerMessage::DispatchEvent { id, event, req_id };
        match self.send(message) {
            Ok(_) => {},
            Err(e) => {
                self.dispatch_response_handlers.remove(&req_id);
                return Err(e);
            }
        }
        match tokio::time::timeout(Duration::from_secs(30), rx).await {
            Ok(Ok(r)) => {
                Ok(r?)
            },
            Ok(Err(_)) => {
                self.dispatch_response_handlers.remove(&req_id);
                Err("Dispatch event response channel closed unexpectedly".into())
            }
            Err(e) => {
                self.dispatch_response_handlers.remove(&req_id);
                return Err(format!("Dispatch event timed out: {}", e).into());
            }
        }
    }
}
