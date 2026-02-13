use std::{collections::HashMap, sync::{Arc, Weak}, time::Instant};
use dashmap::DashMap;
use futures::{SinkExt, StreamExt};
use khronos_runtime::{primitives::event::CreateEvent, utils::khronos_value::KhronosValue};
use rand::distr::{Alphanumeric, SampleString};
use tokio::{select, spawn, sync::{mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel}, oneshot::{Receiver, Sender, channel}}};
use tokio_util::sync::CancellationToken;

use crate::{mesophyll::{dbstate::DbState, message::{ClientMessage, GlobalKeyValueOp, KeyValueOp, PublicGlobalKeyValueOp, ServerMessage}}, worker::{workerstate::TenantState, workervmmanager::Id}};

use axum::{
    Router, body::Bytes, extract::{Query, State, WebSocketUpgrade, ws::{Message, WebSocket}}, http::StatusCode, response::{IntoResponse, Response}, routing::{get, post}
};

#[derive(Clone)]
pub struct MesophyllServer {
    idents: Arc<HashMap<usize, String>>,
    conns: Arc<DashMap<usize, MesophyllServerConn>>,
    db_state: DbState,
}

impl MesophyllServer {
    const TOKEN_LENGTH: usize = 64;

    pub async fn new(addr: String, num_idents: usize, pool: sqlx::PgPool) -> Result<Self, crate::Error> {
        let mut idents = HashMap::new();
        for i in 0..num_idents {
            let ident = Alphanumeric.sample_string(&mut rand::rng(), Self::TOKEN_LENGTH);
            idents.insert(i, ident);
        }
        Self::new_with(addr, idents, pool).await
    }

    pub async fn new_with(addr: String, idents: HashMap<usize, String>, pool: sqlx::PgPool) -> Result<Self, crate::Error> {
        let s = Self {
            db_state: DbState::new(idents.len(), pool).await?,
            idents: Arc::new(idents),
            conns: Arc::new(DashMap::new()),
        };

        // Axum Router
        let app = Router::new()
            .route("/ws", get(ws_handler))
            .route("/db/tenant-states", get(list_tenant_states))
            .route("/db/tenant-state", post(set_tenant_state_for))
            .route("/db/kv", post(kv_handler))
            .route("/db/public-global-kv", post(public_global_kv_handler))
            .route("/db/global-kv", post(global_kv_handler))
            .with_state(s.clone());

        println!("Binding Mesophyll server to address: {}", addr);
        let listener = tokio::net::TcpListener::bind(addr).await
        .map_err(|e| format!("Failed to bind Mesophyll server to address: {}", e))?;
        
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

    pub fn db_state(&self) -> &DbState {
        &self.db_state
    }
}

#[derive(serde::Deserialize)]
pub struct WorkerQuery {
    id: usize,
    token: String,
}

impl WorkerQuery {
    /// Validates the worker query against the Mesophyll server state
    fn validate(&self, state: &MesophyllServer) -> Option<Response> {
        let Some(expected_key) = state.idents.get(&self.id) else {
            log::warn!("Connection attempt from unknown worker ID: {}", self.id);
            return Some(StatusCode::NOT_FOUND.into_response());
        };

        // Verify token of the worker trying to connect to us before upgrading
        if self.token != *expected_key {
            log::warn!("Invalid token for worker {}", self.id);
            return Some(StatusCode::UNAUTHORIZED.into_response());
        }

        None
    }
}

/// DB API to fetch the tenant states assigned to the given worker
async fn list_tenant_states(
    Query(worker_query): Query<WorkerQuery>,
    State(state): State<MesophyllServer>,
) -> impl IntoResponse {
    if let Some(resp) = worker_query.validate(&state) {
        return resp;
    }

    encode_db_resp(&state.db_state.tenant_state_cache_for(worker_query.id).await)
}

#[derive(serde::Deserialize)]
pub struct WorkerQueryWithTenant {
    id: usize,
    token: String,
    tenant_type: String,
    tenant_id: String,
}

impl WorkerQueryWithTenant {
    fn parse(self) -> Result<(Id, WorkerQuery), Response> {
        let Some(id) = Id::from_parts(&self.tenant_type, &self.tenant_id) else {
            log::error!("Failed to parse tenant ID from tenant_type: {}, tenant_id: {}", self.tenant_type, self.tenant_id);
            return Err((StatusCode::BAD_REQUEST, "Invalid tenant ID".to_string()).into_response());
        };

        Ok((id, WorkerQuery { id: self.id, token: self.token }))
    }
}

/// DB API to set tenant state
#[axum::debug_handler]
async fn set_tenant_state_for(
    Query(worker_query): Query<WorkerQueryWithTenant>,
    State(state): State<MesophyllServer>,
    body: Bytes,
) -> impl IntoResponse {
    let (id, worker_query) = match worker_query.parse() {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    if let Some(resp) = worker_query.validate(&state) {
        return resp;
    }

    let tstate: TenantState = match decode_db_req(&body) {
        Ok(s) => s,
        Err(resp) => return resp,
    };

    match state.db_state.set_tenant_state_for(id, tstate).await {
        Ok(_) => (StatusCode::OK).into_response(),
        Err(e) => {
            log::error!("Failed to set tenant state: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

/// DB API to perform a KV op
async fn kv_handler(
    Query(worker_query): Query<WorkerQueryWithTenant>,
    State(state): State<MesophyllServer>,
    body: Bytes,
) -> impl IntoResponse {
    let (id, worker_query) = match worker_query.parse() {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    if let Some(resp) = worker_query.validate(&state) {
        return resp;
    }

    let kvop: KeyValueOp = match decode_db_req(&body) {
        Ok(s) => s,
        Err(resp) => return resp,
    };

    match kvop {
        KeyValueOp::Get { scopes, key } => {
            match state.db_state.kv_get(id, scopes, key).await {
                Ok(rec) => encode_db_resp(&rec),
                Err(e) => {
                    log::error!("Failed to get KV record: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
                }
            }
        }
        KeyValueOp::ListScopes {} => {
            match state.db_state.kv_list_scopes(id).await {
                Ok(scopes) => encode_db_resp(&scopes),
                Err(e) => {
                    log::error!("Failed to list KV scopes: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
                }
            }
        }
        KeyValueOp::Set { scopes, key, value } => {
            match state.db_state.kv_set(id, scopes, key, value).await {
                Ok(_) => (StatusCode::OK).into_response(),
                Err(e) => {
                    log::error!("Failed to set KV record: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
                }
            }
        }
        KeyValueOp::Delete { scopes, key } => {
            match state.db_state.kv_delete(id, scopes, key).await {
                Ok(_) => (StatusCode::OK).into_response(),
                Err(e) => {
                    log::error!("Failed to delete KV record: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
                }
            }
        }
        KeyValueOp::Find { scopes, prefix } => {
            match state.db_state.kv_find(id, scopes, prefix).await {
                Ok(records) => encode_db_resp(&records),
                Err(e) => {
                    log::error!("Failed to find KV records: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
                }
            }
        }
    }
}

/// DB API to perform a KV op
async fn public_global_kv_handler(
    Query(worker_query): Query<WorkerQuery>,
    State(state): State<MesophyllServer>,
    body: Bytes,
) -> impl IntoResponse {
    if let Some(resp) = worker_query.validate(&state) {
        return resp;
    }

    let kvop: PublicGlobalKeyValueOp = match decode_db_req(&body) {
        Ok(s) => s,
        Err(resp) => return resp,
    };

    match kvop {
        PublicGlobalKeyValueOp::Find { query, scope } => {
            match state.db_state.global_kv_find(scope, query).await {
                Ok(records) => encode_db_resp(&records),
                Err(e) => {
                    log::error!("Failed to list global KV records: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
                }
            }
        }
        PublicGlobalKeyValueOp::Get { key, version, scope, id } => {
            match state.db_state.global_kv_get(key, version, scope, id).await {
                Ok(record) => encode_db_resp(&record),
                Err(e) => {
                    log::error!("Failed to get global KV record: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
                }
            }
        }
    }
}

/// DB API to perform a KV op
async fn global_kv_handler(
    Query(worker_query): Query<WorkerQueryWithTenant>,
    State(state): State<MesophyllServer>,
    body: Bytes,
) -> impl IntoResponse {
    let (id, worker_query) = match worker_query.parse() {
        Ok(v) => v,
        Err(resp) => return resp,
    };

    if let Some(resp) = worker_query.validate(&state) {
        return resp;
    }

    let kvop: GlobalKeyValueOp = match decode_db_req(&body) {
        Ok(s) => s,
        Err(resp) => return resp,
    };

    match kvop {
        GlobalKeyValueOp::Create { entry } => {
            match state.db_state.global_kv_create(id, entry).await {
                Ok(_) => (StatusCode::OK).into_response(),
                Err(e) => {
                    log::error!("Failed to create global KV record: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
                }
            }
        }
        GlobalKeyValueOp::Delete { key, version, scope } => {
            match state.db_state.global_kv_delete(id, key, version, scope).await {
                Ok(_) => (StatusCode::OK).into_response(),
                Err(e) => {
                    log::error!("Failed to delete global KV record: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
                }
            }
        }
    }
}

/// WebSocket handler for Mesophyll server
async fn ws_handler(
    Query(worker_query): Query<WorkerQuery>,
    State(state): State<MesophyllServer>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    //println!("Worker {} attempting to connect to Mesophyll server...", worker_query.id);
    let worker_id = worker_query.id;
    if let Some(resp) = worker_query.validate(&state) {
        return resp;
    }
    
    ws.on_upgrade(move |socket| handle_socket(socket, worker_id, state))
}

/// Handles a new Mesophyll server WebSocket connection
async fn handle_socket(socket: WebSocket, id: usize, state: MesophyllServer) {
    if state.conns.contains_key(&id) {
        log::debug!("Worker {id} reconnection - overwriting old connection.");
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

                    let Ok(client_msg) = decode_message::<ClientMessage>(&msg) else {
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

    // Runs a script and waits for a response
    pub async fn run_script(&self, id: Id, name: String, code: String, event: CreateEvent) -> Result<KhronosValue, crate::Error> {
        let (req_id, rx) = self.register_dispatch_response_handler();

        // Upon return, remove the handler from the map
        let _guard = DispatchHandlerDropGuard::new(&self.dispatch_response_handlers, req_id);

        let message = ServerMessage::RunScript { id, name, code, event, req_id };
        self.send(&message)?;
        Ok(rx.await.map_err(|e| format!("Failed to receive dispatch event response: {}", e))??)
    }

    /// Dispatches an event without waiting for a response
    pub fn dispatch_event_nowait(&self, id: Id, event: CreateEvent) -> Result<(), crate::Error> {
        let message = ServerMessage::DispatchEvent { id, event, req_id: None };
        self.send(&message)?;
        Ok(())
    }

    /// Drops a tenant from the worker
    pub async fn drop_tenant(&self, id: Id) -> Result<(), crate::Error> {
        let (req_id, rx) = self.register_dispatch_response_handler();

        // Upon return, remove the handler from the map
        let _guard = DispatchHandlerDropGuard::new(&self.dispatch_response_handlers, req_id);

        let message = ServerMessage::DropWorker { id, req_id };
        self.send(&message)?;
        rx.await.map_err(|e| format!("Failed to receive dispatch event response: {}", e))??;
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
    let bytes = rmp_serde::encode::to_vec(msg)
        .map_err(|e| format!("Failed to serialize Mesophyll message: {}", e))?;
    Ok(Message::Binary(bytes.into()))
}

fn decode_message<T: for<'de> serde::Deserialize<'de>>(msg: &Message) -> Result<T, crate::Error> {
    match msg {
        Message::Binary(b) => {
            let decoded: T = rmp_serde::from_slice(b)
                .map_err(|e| format!("Failed to deserialize Mesophyll message: {}", e))?;
            Ok(decoded)
        }
        _ => Err("Invalid Mesophyll message type".into()),
    }
}

fn encode_db_resp<T: serde::Serialize>(resp: &T) -> Response {
    let encoded = rmp_serde::encode::to_vec(resp);
    match encoded {
        Ok(v) => (StatusCode::OK, axum::body::Bytes::from(v)).into_response(),
        Err(e) => {
            log::error!("Failed to encode tenant states: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

fn decode_db_req<T: for<'de> serde::Deserialize<'de>>(body: &Bytes) -> Result<T, Response> {
    rmp_serde::from_slice(body)
        .map_err(|e| {
            log::error!("Failed to decode DB request: {}", e);
            (StatusCode::BAD_REQUEST, format!("Failed to decode request: {}", e)).into_response()
        })
}