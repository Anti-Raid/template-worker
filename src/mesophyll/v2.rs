use futures::{Stream, StreamExt, stream::Any};
use serenity::all::{UserId, GuildId};
use tonic::Status;
use crate::{mesophyll::dbstate::DbState, worker::workervmmanager::Id as RealId};
use khronos_runtime::utils::khronos_value::KhronosValue as RealKhronosValue;
use khronos_runtime::primitives::event::CreateEvent as RealCreateEvent;
use dashmap::DashMap;
use rand::distr::{SampleString, Alphanumeric};
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::{oneshot, mpsc};

/// Internal transport layer
mod pb {
    tonic::include_proto!("mesophyll");
}

fn encode_any<T: serde::Serialize>(msg: &T) -> Result<Vec<u8>, crate::Error> {
    let bytes = rmp_serde::encode::to_vec(msg)
        .map_err(|e| format!("Failed to serialize Mesophyll any: {}", e))?;
    Ok(bytes)
}

fn decode_any<T: for<'de> serde::Deserialize<'de>>(msg: &[u8]) -> Result<T, crate::Error> {
    let decoded: T = rmp_serde::from_slice(msg)
        .map_err(|e| format!("Failed to deserialize Mesophyll any: {}", e))?;
    Ok(decoded)
}

impl pb::AnyValue {
    pub fn from_real<T: serde::Serialize>(value: &T) -> Result<Self, Status> {
        let data = encode_any(value)
            .map_err(|e| Status::internal(format!("Failed to encode response value: {}", e)))?;
        Ok(Self { value: data })
    }

    pub fn to_real<T: for<'de> serde::Deserialize<'de>>(&self) -> Result<T, Status> {
        let val = decode_any(&self.value)
            .map_err(|e| Status::internal(format!("Failed to decode request value: {}", e)))?;
        Ok(val)
    }

    pub fn to_real_exec<T: for<'de> serde::Deserialize<'de>>(&self) -> Result<T, crate::Error> {
        let val = decode_any(&self.value)
            .map_err(|e| format!("Failed to decode request value: {}", e))?;
        Ok(val)
    }
}

impl pb::Id {
    pub fn to_real_id(&self) -> RealId {        
        // Ensure we don't panic
        let tenant_id = if self.tenant_id == u64::MAX {
            0
        } else {
            self.tenant_id
        };
        match self.tenant_type() {
            pb::TenantType::User => RealId::User(UserId::new(tenant_id)),
            pb::TenantType::Guild => RealId::Guild(GuildId::new(tenant_id)),
        }
    }

    pub fn from_real_id(id: &RealId) -> Self {
        match id {
            RealId::User(uid) => Self {
                tenant_id: uid.get(),
                tenant_type: pb::TenantType::User as i32,
            },
            RealId::Guild(gid) => Self {
                tenant_id: gid.get(),
                tenant_type: pb::TenantType::Guild as i32,
            },
        }
    }
} 

impl pb::Worker {
    /// Validates the workers session against the Mesophyll server state
    fn validate(&self, state: &MesophyllServer) -> Result<(), Status> {
        let Some(expected_key) = state.get_token_for_worker(self.worker_id) else {
            return Err(Status::unauthenticated(format!("Invalid worker ID: {}", self.worker_id)));
        };
        if self.token != *expected_key {
            return Err(Status::permission_denied("Invalid token"));
        }

        Ok(())
    }
}

#[derive(Clone)]
pub struct MesophyllServer {
    idents: Arc<HashMap<u64, String>>,
    conns: Arc<DashMap<u64, MesophyllServerConn>>,
    db_state: DbState,
}

impl MesophyllServer {
    const TOKEN_LENGTH: usize = 64;

    pub async fn new(num_idents: u64, pool: sqlx::PgPool) -> Result<Self, crate::Error> {
        let mut idents = HashMap::new();
        for i in 0..num_idents {
            let ident = Alphanumeric.sample_string(&mut rand::rng(), Self::TOKEN_LENGTH);
            idents.insert(i, ident);
        }
        Self::new_with(idents, pool).await
    }

    async fn new_with(idents: HashMap<u64, String>, pool: sqlx::PgPool) -> Result<Self, crate::Error> {
        let s = Self {
            db_state: DbState::new(idents.len(), pool).await?,
            idents: Arc::new(idents),
            conns: Arc::new(DashMap::new()),
        };

        Ok(s)
    }

    pub fn get_token_for_worker(&self, worker_id: u64) -> Option<&String> {
        self.idents.get(&worker_id)
    }

    pub fn get_connection(&self, worker_id: u64) -> Option<MesophyllServerConn> {
        self.conns.get(&worker_id).map(|r| r.value().clone())
    }

    pub fn db_state(&self) -> &DbState { &self.db_state }

    fn verify_worker(&self, worker: Option<pb::Worker>) -> Result<u64, Status> {
        let worker = worker.ok_or_else(|| Status::invalid_argument("Missing worker info in request"))?;
        worker.validate(self)?;
        Ok(worker.worker_id)
    }
}

#[tonic::async_trait]
impl pb::mesophyll_master_server::MesophyllMaster for MesophyllServer {
    type WorkerInitStream = Pin<Box<dyn Stream<Item = Result<pb::MtwMessage, Status>> + Send>>;

    async fn worker_init(&self, request: tonic::Request<tonic::Streaming<pb::WtmMessage>>) -> Result<tonic::Response<Self::WorkerInitStream>, Status> {
        let mut stream = request.into_inner();

        let Some(Ok(pb::WtmMessage { payload: Some(p), resp_id: _ })) = stream.next().await else {
            return Err(Status::invalid_argument("Failed to deserialize initial message"));
        };

        let wk = match p {
            pb::wtm_message::Payload::WorkerIdent(worker) => {
                worker.validate(self)?;
                worker
            },
            _ => return Err(Status::invalid_argument("Expected first message to be Init")),
        };

        // Create a new connection for this worker
        let (tx, rx) = mpsc::unbounded_channel();
        let conn = MesophyllServerConn::new(wk.worker_id, tx);
        self.conns.insert(wk.worker_id, conn.clone());

        tokio::spawn(async move {
            while let Some(item) = stream.next().await {
                match item {
                    Ok(pb::WtmMessage { payload: Some(p), resp_id }) => {
                        // Handle incoming messages here, potentially using resp_id to correlate responses
                        let Some(resp_id) = resp_id else { continue };
                        match p {
                            pb::wtm_message::Payload::DispatchResponse(der) => {
                                let Some((_, handler)) = conn.dispatch_response_handlers.remove(&resp_id) else {
                                    continue;
                                };
                                let _ = handler.send(der);
                            },
                            pb::wtm_message::Payload::WorkerIdent(_) => {
                                log::error!("Received unexpected WorkerIdent message from worker {}, ignoring", wk.worker_id);
                                break;
                            },
                            pb::wtm_message::Payload::DropTenantAck(_) => {
                                let Some((_, handler)) = conn.drop_tenant_ack_handlers.remove(&resp_id) else {
                                    continue;
                                };
                                let _ = handler.send(());
                            }
                        }
                    },
                    Ok(_) => {
                        log::warn!("Received message with no payload from worker {}", wk.worker_id);
                    },
                    Err(e) => {
                        log::warn!("Error receiving message from worker {}: {}, waiting", wk.worker_id, e);
                        break;
                    }
                }
            }
        });

        let resp_stream = tokio_stream::wrappers::UnboundedReceiverStream::new(rx);
        Ok(tonic::Response::new(Box::pin(resp_stream) as Self::WorkerInitStream))
    }

    async fn kv_get(&self, request: tonic::Request<pb::WtmKvGet>) -> Result<tonic::Response<pb::AnyValue>, Status> {
        let req = request.into_inner();
        self.verify_worker(req.worker)?;
        let id = req.id.ok_or_else(|| Status::invalid_argument("Missing ID"))?.to_real_id();
        let scopes = req.scopes;
        let key = req.key;

        match self.db_state.key_value_db().kv_get(id, scopes, key).await {
            Ok(record) => Ok(tonic::Response::new(pb::AnyValue::from_real(&record)?)),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn kv_list_scopes(&self, request: tonic::Request<pb::WtmKvListScopes>) -> Result<tonic::Response<pb::KvListScopesResponse>, Status> {
        let req = request.into_inner();
        self.verify_worker(req.worker)?;
        let id = req.id.ok_or_else(|| Status::invalid_argument("Missing ID"))?.to_real_id();

        match self.db_state.key_value_db().kv_list_scopes(id).await {
            Ok(scopes) => Ok(tonic::Response::new(pb::KvListScopesResponse { scopes })),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn kv_set(&self, request: tonic::Request<pb::WtmKvSet>) -> Result<tonic::Response<pb::WtmBool>, Status> {
        let req = request.into_inner();
        self.verify_worker(req.worker)?;
        let id = req.id.ok_or_else(|| Status::invalid_argument("Missing ID"))?.to_real_id();
        let scopes = req.scopes;
        let key = req.key;
        let Some(value) = req.value else {
            return Err(Status::invalid_argument("Missing value"));
        
        };

        let value = value.to_real()?;
        match self.db_state.key_value_db().kv_set(id, scopes, key, value).await {
            Ok(_) => Ok(tonic::Response::new(pb::WtmBool { value: true })),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn kv_delete(&self, request: tonic::Request<pb::WtmKvDelete>) -> Result<tonic::Response<pb::WtmBool>, Status> {
        let req = request.into_inner();
        self.verify_worker(req.worker)?;
        let id = req.id.ok_or_else(|| Status::invalid_argument("Missing ID"))?.to_real_id();
        let scopes = req.scopes;
        let key = req.key;

        match self.db_state.key_value_db().kv_delete(id, scopes, key).await {
            Ok(_) => Ok(tonic::Response::new(pb::WtmBool { value: true })),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn kv_find(&self, request: tonic::Request<pb::WtmKvFind>) -> Result<tonic::Response<pb::AnyValue>, Status> {
        let req = request.into_inner();
        self.verify_worker(req.worker)?;
        let id = req.id.ok_or_else(|| Status::invalid_argument("Missing ID"))?.to_real_id();
        let scopes = req.scopes;
        let prefix = req.prefix;
        match self.db_state.key_value_db().kv_find(id, scopes, prefix).await {
            Ok(records) => Ok(tonic::Response::new(pb::AnyValue::from_real(&records)?)),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn global_kv_find(&self, request: tonic::Request<pb::WtmGlobalKvFind>) -> Result<tonic::Response<pb::AnyValue>, Status> {
        let req = request.into_inner();
        let scope = req.scope;
        let query = req.query;

        match self.db_state.global_kv_find(scope, query).await {
            Ok(records) => Ok(tonic::Response::new(pb::AnyValue::from_real(&records)?)),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn global_kv_get(&self, request: tonic::Request<pb::WtmGlobalKvGet>) -> Result<tonic::Response<pb::AnyValue>, Status> {
        let req = request.into_inner();
        let key = req.key;
        let version = req.version;
        let scope = req.scope;
        let id = req.id.map(|id| id.to_real_id());

        match self.db_state.global_kv_get(key, version, scope, id).await {
            Ok(record) => Ok(tonic::Response::new(pb::AnyValue::from_real(&record)?)),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn global_kv_create(&self, request: tonic::Request<pb::WtmGlobalKvCreate>) -> Result<tonic::Response<pb::WtmBool>, Status> {
        let req = request.into_inner();
        self.verify_worker(req.worker)?;
        let id = req.id.ok_or_else(|| Status::invalid_argument("Missing ID"))?.to_real_id();
        let value = req.data.ok_or_else(|| Status::invalid_argument("Missing data"))?.to_real()?;

        match self.db_state.global_kv_create(id, value).await {
            Ok(_) => Ok(tonic::Response::new(pb::WtmBool { value: true })),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn global_kv_delete(&self, request: tonic::Request<pb::WtmGlobalKvDelete>) -> Result<tonic::Response<pb::WtmBool>, Status> {
        let req = request.into_inner();
        self.verify_worker(req.worker)?;
        let id = req.id.ok_or_else(|| Status::invalid_argument("Missing ID"))?.to_real_id();
        let key = req.key;
        let version = req.version;
        let scope = req.scope;

        match self.db_state.global_kv_delete(id, key, version, scope).await {
            Ok(_) => Ok(tonic::Response::new(pb::WtmBool { value: true })),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }
}

#[derive(Clone)]
pub struct MesophyllServerConn {
    id: u64,
    tx: mpsc::UnboundedSender<Result<pb::MtwMessage, Status>>,
    pub(super) dispatch_response_handlers: Arc<DashMap<u64, oneshot::Sender<pb::DispatchEventResponse>>>,
    pub(super) drop_tenant_ack_handlers: Arc<DashMap<u64, oneshot::Sender<()>>>,
}

impl MesophyllServerConn {
    fn new(
        id: u64, 
        tx: mpsc::UnboundedSender<Result<pb::MtwMessage, Status>>,
    ) -> Self {
        let dispatch_response_handlers = Arc::new(DashMap::new());
        let drop_tenant_ack_handlers = Arc::new(DashMap::new());
        Self { id, tx, dispatch_response_handlers, drop_tenant_ack_handlers }
    }

    pub async fn dispatch_event(&self, id: RealId, event: RealCreateEvent) -> Result<RealKhronosValue, crate::Error> {
        let (name, author, data) = event.extract();
        let pb_event = pb::CreateEvent {
            name,
            author,
            value: Some(pb::AnyValue::from_real(&data)?),
        };

        let (resp_tx, resp_rx) = oneshot::channel();
        let resp_id = rand::random::<u64>();
        self.dispatch_response_handlers.insert(resp_id, resp_tx);

        let msg = pb::MtwMessage {
            payload: Some(pb::mtw_message::Payload::Dispatch(pb::DispatchEvent {
                id: Some(pb::Id::from_real_id(&id)),
                event_payload: Some(pb_event),
            })),
            id: Some(resp_id),
        };
        
        self.tx.send(Ok(msg)).map_err(|e| Status::internal(format!("Failed to send dispatch message to worker {}: {}", self.id, e)))?;
        let resp = resp_rx.await?;
        match resp.resp {
            Some(pb::dispatch_event_response::Resp::Value(v)) => v.to_real_exec(),
            Some(pb::dispatch_event_response::Resp::Error(e)) => Err(e.into()),
            None => Err("Received dispatch response with no payload".into()),
        }
    }

    pub async fn run_script(&self, id: RealId, name: String, code: String, event: RealCreateEvent) -> Result<RealKhronosValue, crate::Error> {
        let (event_name, event_author, event_data) = event.extract();
        let pb_event = pb::CreateEvent {
            name: event_name,
            author: event_author,
            value: Some(pb::AnyValue::from_real(&event_data)?),
        };

        let (resp_tx, resp_rx) = oneshot::channel();
        let resp_id = rand::random::<u64>();
        self.dispatch_response_handlers.insert(resp_id, resp_tx);

        let msg = pb::MtwMessage {
            payload: Some(pb::mtw_message::Payload::RunScript(pb::RunScript {
                id: Some(pb::Id::from_real_id(&id)),
                name,
                code,
                event: Some(pb_event),
            })),
            id: Some(resp_id),
        };

        self.tx.send(Ok(msg)).map_err(|e| Status::internal(format!("Failed to send run script message to worker {}: {}", self.id, e)))?;
        let resp = resp_rx.await?;
        match resp.resp {
            Some(pb::dispatch_event_response::Resp::Value(v)) => v.to_real_exec(),
            Some(pb::dispatch_event_response::Resp::Error(e)) => Err(e.into()),
            None => Err("Received run script response with no payload".into()),
        }
    }

    pub async fn drop_tenant(&self, id: RealId) -> Result<(), crate::Error> {
        let (ack_tx, ack_rx) = oneshot::channel();
        let resp_id = rand::random::<u64>();
        self.drop_tenant_ack_handlers.insert(resp_id, ack_tx);

        let msg = pb::MtwMessage {
            payload: Some(pb::mtw_message::Payload::DropTenant(pb::Id::from_real_id(&id))),
            id: Some(resp_id),
        };

        self.tx.send(Ok(msg)).map_err(|e| Status::internal(format!("Failed to send drop tenant message to worker {}: {}", self.id, e)))?;
        ack_rx.await.map_err(|e| format!("Failed to receive drop tenant ack from worker {}: {}", self.id, e).into())
    }
}