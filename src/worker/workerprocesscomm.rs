use std::{collections::HashMap, sync::{Arc, Mutex}};

use khronos_runtime::{primitives::event::CreateEvent, utils::khronos_value::KhronosValue};
use tokio_tungstenite::{tungstenite::{client::IntoClientRequest, handshake::server::Request}, MaybeTlsStream, WebSocketStream};

use crate::worker::{workerdispatch::{DispatchTemplateResult, TemplateResult}, workerlike::WorkerLike, workervmmanager::Id};
use futures::StreamExt;
use futures::SinkExt;
use rand::{distr::{Alphanumeric, SampleString}, Rng};
use tokio::{net::TcpStream, sync::mpsc::{UnboundedSender, UnboundedReceiver, unbounded_channel}, sync::{oneshot::Sender as OneShotSender, oneshot::channel as oneshot_channel}};

#[async_trait::async_trait]
pub trait WorkerProcessCommServer {
    /// Dispatch an event to the templates managed by this worker
    async fn dispatch_event_to_templates(&self, id: Id, event: CreateEvent) -> DispatchTemplateResult;

    /// Dispatch a scoped event to the templates managed by this worker
    async fn dispatch_scoped_event_to_templates(&self, id: Id, event: CreateEvent, scopes: Vec<String>) -> DispatchTemplateResult;

    /// Regenerate the cache for a tenant
    async fn regenerate_cache(&self, id: Id) -> Result<(), crate::Error>;

    /// The extra arguments needed to start the worker process
    fn start_args(&self) -> Vec<String>;

    /// The environment variables needed to start the worker process
    fn start_env(&self) -> Vec<(String, String)>;

    /// Wait for the worker process to be ready
    async fn wait_for_ready(&self) -> Result<(), crate::Error> {
        // Default implementation does nothing, can be overridden
        Ok(())
    }
}

/// Marker trait to signify that this is a client for the worker process communication
pub trait WorkerProcessCommClient {}

#[derive(serde::Serialize, serde::Deserialize)]
enum WorkerProcessCommTenantIdType {
    GuildId,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct WorkerProcessCommTenantId {
    id: u64,
    typ: WorkerProcessCommTenantIdType,
}

impl From<Id> for WorkerProcessCommTenantId {
    fn from(id: Id) -> Self {
        match id {
            Id::GuildId(guild_id) => Self { id: guild_id.get(), typ: WorkerProcessCommTenantIdType::GuildId },
        }
    }
}

impl From<WorkerProcessCommTenantId> for Id {
    fn from(tenant_id: WorkerProcessCommTenantId) -> Self {
        match tenant_id.typ {
            WorkerProcessCommTenantIdType::GuildId => Id::GuildId(tenant_id.id.into()),
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
enum WorkerProcessCommTemplateResult {
    Ok {
        result: KhronosValue
    },
    Error {
        error: String,
    },
}

impl From<WorkerProcessCommTemplateResult> for TemplateResult {
    fn from(result: WorkerProcessCommTemplateResult) -> Self {
        match result {
            WorkerProcessCommTemplateResult::Ok { result } => TemplateResult::Ok(result),
            WorkerProcessCommTemplateResult::Error { error } => TemplateResult::Err(error.into()),
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
enum WorkerProcessCommDispatchResult {
    Ok {
        result: Vec<(String, WorkerProcessCommTemplateResult)>,
    },
    Error {
        error: String,
    },
}

impl From<WorkerProcessCommDispatchResult> for DispatchTemplateResult {
    fn from(result: WorkerProcessCommDispatchResult) -> Self {
        match result {
            WorkerProcessCommDispatchResult::Ok { result } => DispatchTemplateResult::Ok(result.into_iter().map(|(key, value)| (key, value.into())).collect()),
            WorkerProcessCommDispatchResult::Error { error } => DispatchTemplateResult::Err(error.into()),
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
/// Stores the message data that is sent from master to the worker process
enum WorkerProcessCommWebsocketServerMessageData {
    DispatchEventToTemplates {
        id: WorkerProcessCommTenantId,
        event_json: String,
    },
    DispatchScopedEventToTemplates {
        id: WorkerProcessCommTenantId,
        event_json: String,
        scopes: Vec<String>,
    },
    CacheRegenerate {
        id: WorkerProcessCommTenantId,
    },
    IsReady {}
}

#[derive(serde::Serialize, serde::Deserialize)]
/// Stores the messages that can be sent from master to worker
enum WorkerProcessCommWebsocketServerMessage {
    Request {
        data: WorkerProcessCommWebsocketServerMessageData,
        id: String,
    },
}

#[derive(serde::Serialize, serde::Deserialize)]
/// Represents the message data that can be sent from worker to master
enum WorkerProcessCommWebsocketClientMessageData {
    DispatchEventToTemplates {
        resp: WorkerProcessCommDispatchResult,
    },
    DispatchScopedEventToTemplates {
        resp: WorkerProcessCommDispatchResult,
    },
    CacheRegenerate {},
    IsReady {}
}

#[derive(serde::Serialize, serde::Deserialize)]
/// Represents the messages that can be sent from worker to master
enum WorkerProcessCommWebsocketClientMessage {
    Response {
        data: WorkerProcessCommWebsocketClientMessageData,
        id: String,
    },
}

/// Messages that can be sent to the websocket task
enum WorkerProcessCommWebsocketMessage {
    MakeRequest {
        req: WorkerProcessCommWebsocketServerMessage,
    },
}

/// Worker Process Communication using a central websocket
#[derive(Clone)]
pub struct WorkerProcessCommWebsocketServer {
    token: String,
    port: u16,
    tx: UnboundedSender<WorkerProcessCommWebsocketMessage>,
    request_callbacks: Arc<Mutex<HashMap<String, OneShotSender<WorkerProcessCommWebsocketClientMessageData>>>>,
}

impl WorkerProcessCommWebsocketServer {
    /// Create a new instance of `WorkerProcessCommWebsocket` and starts up the websocket server
    pub async fn new() -> Result<Self, crate::Error> {
        let mut port = rand::rng().random_range(1030..=65535);
        
        let listener = loop {
            // Ensure the port is not already in use
            match tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port)).await {
                Ok(l) => {
                    break l;
                },
                Err(_) => {
                    port = rand::rng().random_range(1030..=65535); // Try a different port
                }
            }
        };

        // Generate random string for token
        let token = Alphanumeric.sample_string(&mut rand::rng(), 128);

        let (tx, rx) = unbounded_channel();
        let comm = Self {
            token,
            port,
            tx,
            request_callbacks: Arc::new(Mutex::default()),
        };

        // Start the websocket task in the background
        let comm_ref = comm.clone();
        tokio::spawn(async move {
            comm_ref.ws_task(listener, rx).await;
        });

        Ok(comm)
    }

    /// Background task to handle the websocket
    async fn ws_task(&self, listener: tokio::net::TcpListener, mut rx: UnboundedReceiver<WorkerProcessCommWebsocketMessage>) {
        while let Ok((stream, _)) = listener.accept().await {
            let ws_stream = match tokio_tungstenite::accept_hdr_async(stream, |req: &Request, res| {
                let token = req.headers().get("Worker-Token")
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s.to_string());

                if token.as_deref() != Some(&self.token) {
                    return Err(tokio_tungstenite::tungstenite::handshake::server::ErrorResponse::new(Some("Invalid token".to_string())));
                }

                Ok(res)
            })
                .await {
                Ok(ws) => ws,
                Err(e) => {
                    log::error!("Failed to accept websocket connection: {}", e);
                    continue;
                }
            };

            // NOTE: Binding to the websocket locks it so other clients will hang indefinitely if they try to connect
            self.ws_handler(ws_stream, &mut rx).await;
        }
    }

    /// Internal handler for the websocket connection
    async fn ws_handler(&self, stream: tokio_tungstenite::WebSocketStream<TcpStream>, rx: &mut UnboundedReceiver<WorkerProcessCommWebsocketMessage>) {
        // Begin recieving events
        let (mut write, mut read) = stream.split();
        loop {
            tokio::select! {
                Some(msg) = rx.recv() => {
                    match msg {
                        WorkerProcessCommWebsocketMessage::MakeRequest { req } => {
                            let msg = match serde_json::to_string(&req) {
                                Ok(msg) => msg,
                                Err(e) => {
                                    log::error!("Failed to serialize request message: {e}");
                                    // Close the websocket
                                    if let Err(e) = write.send(tokio_tungstenite::tungstenite::Message::Close(None)).await {
                                        log::error!("Failed to send close message: {}", e);
                                    }
                                    match write.close().await {
                                        Ok(_) => {},
                                        Err(e) => log::error!("Failed to close websocket: {}", e),
                                    }
                                    return;
                                }
                            };

                            if let Err(e) = write.send(tokio_tungstenite::tungstenite::Message::Text(msg.into())).await {
                                log::error!("Failed to send message: {}", e);
                            }
                        },
                    }
                },
                Some(msg) = read.next() => {
                    let msg = match msg {
                        Ok(msg) => msg,
                        Err(e) => {
                            log::error!("Error reading message from websocket: {}", e);
                            break;
                        }
                    };

                    let tokio_tungstenite::tungstenite::Message::Text(text) = msg else {
                        continue;
                    };

                    let msg: WorkerProcessCommWebsocketClientMessage = match serde_json::from_str(&text) {
                        Ok(msg) => msg,
                        Err(e) => {
                            // Close the websocket
                            log::error!("Failed to deserialize message: {e}");
                            if let Err(e) = write.send(tokio_tungstenite::tungstenite::Message::Close(None)).await {
                                log::error!("Failed to send close message: {}", e);
                            }
                            match write.close().await {
                                Ok(_) => {},
                                Err(e) => log::error!("Failed to close websocket: {}", e),
                            }
                            return;
                        }
                    };

                    match msg {
                        WorkerProcessCommWebsocketClientMessage::Response { data, id } => {
                            let callback = {
                                let mut guard = match self.request_callbacks.lock() {
                                    Ok(guard) => guard,
                                    Err(e) => {
                                        log::error!("Failed to lock request callbacks: {}", e);
                                        continue;
                                    }
                                };
                                guard.remove(&id)
                            }; // Mutex dropped here

                            if let Some(callback) = callback {
                                let _ = callback.send(data);
                            }
                        } 
                    }
                },
            }
        }
    }

    /// Send a message to the worker process and wait for a response
    async fn send(&self, data: WorkerProcessCommWebsocketServerMessageData) -> Result<WorkerProcessCommWebsocketClientMessageData, crate::Error> {
        struct SendDropGuard {
            id: String,
            request_callbacks: Arc<Mutex<HashMap<String, OneShotSender<WorkerProcessCommWebsocketClientMessageData>>>>,
        }

        impl Drop for SendDropGuard {
            fn drop(&mut self) {
                let mut guard = match self.request_callbacks.lock() {
                    Ok(guard) => guard,
                    Err(e) => {
                        log::error!("Failed to lock request callbacks: {}", e);
                        return;
                    }
                };
                guard.remove(&self.id);
            }
        }

        let id = Alphanumeric.sample_string(&mut rand::rng(), 16);
        let req = WorkerProcessCommWebsocketServerMessage::Request {
            data,
            id: id.clone(),
        };

        let (tx, rx) = oneshot_channel();
        {
            let mut guard = match self.request_callbacks.lock() {
                Ok(guard) => guard,
                Err(e) => {
                    return Err(format!("Failed to lock request callbacks: {e}").into());
                }
            };
            guard.insert(id.clone(), tx);
        }

        let _g = SendDropGuard { 
            id,
            request_callbacks: self.request_callbacks.clone()
        }; // Ensure the id is kept dropped when the request is done

        self.tx.send(WorkerProcessCommWebsocketMessage::MakeRequest { req }).map_err(|e| format!("Failed to send message: {}", e))?;

        let resp = rx.await?;
        Ok(resp)
    }
}

#[async_trait::async_trait]
impl WorkerProcessCommServer for WorkerProcessCommWebsocketServer {
    async fn dispatch_event_to_templates(&self, id: Id, event: CreateEvent) -> DispatchTemplateResult {
        let resp = self.send(WorkerProcessCommWebsocketServerMessageData::DispatchEventToTemplates {
            id: WorkerProcessCommTenantId::from(id),
            event_json: serde_json::to_string(&event)?,
        }).await?;

        match resp {
            WorkerProcessCommWebsocketClientMessageData::DispatchEventToTemplates { resp } => resp.into(),
            _ => Err(format!("Unexpected response type").into()),
        }
    }

    async fn dispatch_scoped_event_to_templates(&self, id: Id, event: CreateEvent, scopes: Vec<String>) -> DispatchTemplateResult {
        let resp = self.send(WorkerProcessCommWebsocketServerMessageData::DispatchScopedEventToTemplates {
            id: WorkerProcessCommTenantId::from(id),
            scopes,
            event_json: serde_json::to_string(&event)?,
        }).await?;

        match resp {
            WorkerProcessCommWebsocketClientMessageData::DispatchScopedEventToTemplates { resp } => resp.into(),
            _ => Err(format!("Unexpected response type").into()),
        }
    }

    async fn regenerate_cache(&self, id: Id) -> Result<(), crate::Error> {
        let resp = self.send(WorkerProcessCommWebsocketServerMessageData::CacheRegenerate {
            id: WorkerProcessCommTenantId::from(id),
        }).await?;

        match resp {
            WorkerProcessCommWebsocketClientMessageData::CacheRegenerate {} => Ok(()),
            _ => Err(format!("Unexpected response type").into()),
        }
    }

    fn start_args(&self) -> Vec<String> {
        vec![
            "--process-comm-type".to_string(),
            "websocket".to_string(),
        ]
    }

    fn start_env(&self) -> Vec<(String, String)> {
        vec![
            ("WORKER_COMM_WEBSOCKET_TOKEN".to_string(), self.token.clone()),
            ("WORKER_COMM_WEBSOCKET_PORT".to_string(), self.port.to_string()),
        ]
    }

    async fn wait_for_ready(&self) -> Result<(), crate::Error> {
        // Here there will implement logic to wait for the worker process to be ready
        // For now, we just return Ok
        let resp = self.send(WorkerProcessCommWebsocketServerMessageData::IsReady {}).await?;

        match resp {
            WorkerProcessCommWebsocketClientMessageData::IsReady {} => Ok(()),
            _ => Err(format!("Unexpected response type").into()),
        }
    }
}

/// A client for the worker process communication using a websocket
#[derive(Clone)]
pub struct WorkerProcessCommWebsocketClient {
    token: String,
    worker: Arc<dyn WorkerLike + Send + Sync>,
}

impl WorkerProcessCommWebsocketClient {
    /// Creates a new WorkerProcessCommWebsocketClient (worker process communication via websockets client)
    pub async fn new(worker: Arc<dyn WorkerLike + Send + Sync>) -> Result<Self, crate::Error> {
        let port = std::env::var("WORKER_COMM_WEBSOCKET_PORT")
            .map_err(|_| "WORKER_COMM_WEBSOCKET_PORT not set")?
            .parse::<u16>()
            .map_err(|_| "Invalid WORKER_COMM_WEBSOCKET_PORT")?;

        let token = std::env::var("WORKER_COMM_WEBSOCKET_TOKEN")
            .map_err(|_| "WORKER_COMM_WEBSOCKET_TOKEN not set")?;

        let url = format!("ws://127.0.1:{}", port);
        let mut request = url.into_client_request()?;
        request.headers_mut().insert("Worker-Token", token.clone().parse()?);
        let (conn, _) = tokio_tungstenite::connect_async(request)
            .await
            .map_err(|e| format!("Failed to connect to websocket: {}", e))?;

        let client = Self {
            token,
            worker,
        };

        let client_ref = client.clone();
        tokio::task::spawn(async move {
            client_ref.ws_task(conn).await;
        });

        Ok(client)
    }

    async fn ws_task(&self, mut conn: WebSocketStream<MaybeTlsStream<TcpStream>>) {
        loop {
        }
    }

    async fn send_to_ws(conn: &mut WebSocketStream<MaybeTlsStream<TcpStream>>, msg: WorkerProcessCommWebsocketClientMessage) -> Result<(), crate::Error> {
        let msg_str = serde_json::to_string(&msg)
            .map_err(|e| format!("Failed to serialize message: {}", e))?;
        
        conn.send(tokio_tungstenite::tungstenite::Message::Text(msg_str.into()))
            .await
            .map_err(|e| format!("Failed to send message: {}", e))?;
        Ok(())
    }
}

impl WorkerProcessCommClient for WorkerProcessCommWebsocketClient {}