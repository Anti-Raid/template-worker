use std::{collections::HashMap, sync::{Arc, Weak}, time::Instant};
use rand::{distr::{Alphanumeric, SampleString}};
use dashmap::DashMap;
use futures::{SinkExt, StreamExt};
use khronos_runtime::{primitives::event::CreateEvent, utils::khronos_value::KhronosValue};
use tokio::{select, spawn, sync::{mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel}, oneshot::{Receiver, Sender, channel}}};
use tokio_util::sync::CancellationToken;

use crate::{mesophyll::message::{ClientMessage, ServerMessage}, worker::workervmmanager::Id};

use axum::{
    Router, extract::{Path, Query, State, WebSocketUpgrade, ws::{Message, WebSocket}}, http::StatusCode, response::IntoResponse, routing::get
};

#[derive(Clone)]
pub struct MesophyllServer {
    idents: Arc<HashMap<usize, String>>,
    conns: Arc<DashMap<usize, MesophyllServerConn>>,
}

impl MesophyllServer {
    const TOKEN_LENGTH: usize = 64;

    pub async fn new(addr: String, num_idents: usize) -> Result<Self, crate::Error> {
        let mut idents = HashMap::new();
        for i in 0..num_idents {
            let ident = Alphanumeric.sample_string(&mut rand::rng(), Self::TOKEN_LENGTH);
            idents.insert(i, ident);
        }
        Self::new_with(addr, idents).await
    }

    pub async fn new_with(addr: String, idents: HashMap<usize, String>) -> Result<Self, crate::Error> {
        let s = Self {
            idents: Arc::new(idents),
            conns: Arc::new(DashMap::new()),
        };

        // Axum Router
        let app = Router::new()
            .route("/conn/:worker_id", get(ws_handler))
            .with_state(s.clone());

        let listener = tokio::net::TcpListener::bind(addr).await?;
        
        log::info!("Mesophyll server listening...");
        
        // Spawn the server task
        spawn(async move {
            axum::serve(listener, app).await.expect("Mesophyll server failed");
        });

        Ok(s)
    }

    pub fn get_token_for_worker(&self, worker_id: usize) -> Option<&String> {
        self.idents.get(&worker_id)
    }

    pub fn get_connection(&self, worker_id: usize) -> Option<MesophyllServerConn> {
        self.conns.get(&worker_id).map(|r| r.value().clone())
    }
}

#[derive(serde::Deserialize)]
pub struct WorkerQuery {
    token: String,
}

/// WebSocket handler for Mesophyll server
async fn ws_handler(
    Path(worker_id): Path<usize>,
    Query(worker_query): Query<WorkerQuery>,
    State(state): State<MesophyllServer>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    let Some(expected_key) = state.idents.get(&worker_id) else {
        log::warn!("Connection attempt from unknown worker ID: {}", worker_id);
        return StatusCode::NOT_FOUND.into_response();
    };

    // Verify token of the worker trying to connect to us before upgrading
    if worker_query.token != *expected_key {
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

enum MesophyllServerQueryMessage {
    GetLastHeartbeat { tx: Sender<Option<Instant>> }
}

/// A Mesophyll server connection
#[derive(Clone)]
pub struct MesophyllServerConn {
    id: usize,
    send_queue: UnboundedSender<Message>,
    query_queue: UnboundedSender<MesophyllServerQueryMessage>,
    dispatch_response_handlers: Arc<DashMap<u64, Sender<Result<KhronosValue, String>>>>,
    cancel: CancellationToken,
}

impl Drop for MesophyllServerConn {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

struct DispatchHandlerDropGuard<'a> {
    map: &'a DashMap<u64, Sender<Result<KhronosValue, String>>>,
    req_id: u64,
}

impl<'a> DispatchHandlerDropGuard<'a> {
    fn new(map: &'a DashMap<u64, Sender<Result<KhronosValue, String>>>, req_id: u64) -> Self {
        Self { map, req_id }
    }
}

impl<'a> Drop for DispatchHandlerDropGuard<'a> {
    fn drop(&mut self) {
        self.map.remove(&self.req_id);
    }
}

impl MesophyllServerConn {
    fn new(id: usize, stream: WebSocket, conns_map: Weak<DashMap<usize, MesophyllServerConn>>) -> Self {
        let (send_queue, recv_queue) = unbounded_channel();
        let (query_queue_tx, query_queue_rx) = unbounded_channel();
        let s = Self {
            id,
            send_queue,
            dispatch_response_handlers: Arc::new(DashMap::new()),
            cancel: CancellationToken::new(),
            query_queue: query_queue_tx,
        };

        let s_ref = s.clone();
        spawn(async move {
            s_ref.run_loop(stream, recv_queue, query_queue_rx).await;

            if let Some(map) = conns_map.upgrade() {
                map.remove(&id);
            }
        });
        s
    }

    async fn run_loop(&self, socket: WebSocket, mut rx: UnboundedReceiver<Message>, mut query_queue_rx: UnboundedReceiver<MesophyllServerQueryMessage>) {
        let (mut stream_tx, mut stream_rx) = socket.split();
        let mut last_hb = None;
        loop {
            select! {
                Some(msg) = rx.recv() => {
                    if let Err(e) = stream_tx.send(msg).await {
                        log::error!("Failed to send Mesophyll message to worker {}: {}", self.id, e);
                    }
                }
                Some(msg) = query_queue_rx.recv() => {
                    match msg {
                        MesophyllServerQueryMessage::GetLastHeartbeat { tx } => {
                            let _ = tx.send(last_hb);
                        }
                    }
                }
                Some(Ok(msg)) = stream_rx.next() => {
                    if let Message::Close(_) = msg {
                        log::info!("Mesophyll server connection {} closed by client", self.id);
                        break;
                    }

                    let Some(client_msg) = decode_message::<ClientMessage>(&msg) else {
                        continue;
                    };

                    match client_msg {
                        ClientMessage::DispatchResponse { req_id, result } => {
                            if let Some((_, tx)) = self.dispatch_response_handlers.remove(&req_id) {
                                let _ = tx.send(result);
                            }
                        }
                        ClientMessage::Heartbeat { } => {
                            last_hb = Some(Instant::now());
                        }
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

    // Sends a message to the connected client
    fn send(&self, msg: &ServerMessage) -> Result<(), crate::Error> {
        let msg = encode_message(&msg)?;
        self.send_queue.send(msg).map_err(|e| format!("Failed to send Mesophyll server message: {}", e).into())
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

        // Upon return, remove the handler from the map
        let _guard = DispatchHandlerDropGuard::new(&self.dispatch_response_handlers, req_id);

        let message = ServerMessage::DispatchEvent { id, event, req_id: Some(req_id) };
        self.send(&message)?;
        Ok(rx.await.map_err(|e| format!("Failed to receive dispatch event response: {}", e))??)
    }

    /// Dispatches an event without waiting for a response
    pub fn dispatch_event_nowait(&self, id: Id, event: CreateEvent) -> Result<(), crate::Error> {
        let message = ServerMessage::DispatchEvent { id, event, req_id: None };
        self.send(&message)?;
        Ok(())
    }

    /// Returns the last time a heartbeat was recieved from this client
    #[allow(dead_code)]
    pub async fn get_last_hb_instant(&self) -> Result<Option<Instant>, crate::Error> {
        let (tx, rx) = channel();
        self.query_queue.send(MesophyllServerQueryMessage::GetLastHeartbeat { tx })?;
        Ok(rx.await?)
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