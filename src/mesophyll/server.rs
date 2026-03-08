use chrono::TimeDelta;
use futures::{Stream, StreamExt};
use khronos_runtime::chrono_tz;
use serenity::all::{UserId, GuildId};
use tonic::Status;
use crate::{mesophyll::dbstate::DbState, worker::workervmmanager::Id as RealId};
use khronos_runtime::utils::khronos_value::KhronosValue as RealKhronosValue;
use khronos_runtime::primitives::event::CreateEvent as RealCreateEvent;
use dashmap::DashMap;
use rand::distr::{SampleString, Alphanumeric};
use std::{collections::HashMap, net::ToSocketAddrs};
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::{oneshot, mpsc};

/// Internal transport layer
pub(super) mod pb {
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
        Self::from_real_exec(value).map_err(|e| Status::internal(e.to_string()))
    }

    pub fn from_real_exec<T: serde::Serialize>(value: &T) -> Result<Self, crate::Error> {
        let data = encode_any(value)
            .map_err(|e| format!("Failed to encode response value: {}", e))?;
        Ok(Self { value: data })
    }

    pub fn to_real<T: for<'de> serde::Deserialize<'de>>(&self) -> Result<T, Status> {
        self.to_real_exec().map_err(|e| Status::internal(e.to_string()))
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
    fn worker_id_usize(&self) -> Result<usize, Status> {
        Ok(self.worker_id.try_into().map_err(|_e| tonic::Status::internal("WID not a u64"))?)
    }

    /// Validates the workers session against the Mesophyll server state
    fn validate(&self, state: &MesophyllServer) -> Result<(), Status> {
        let wid = self.worker_id_usize()?;
        let Some(expected_key) = state.get_token_for_worker(wid) else {
            return Err(Status::unauthenticated(format!("Invalid worker ID: {}", wid)));
        };
        if self.token != *expected_key {
            return Err(Status::permission_denied("Invalid token"));
        }

        Ok(())
    }
}

impl pb::KhronosValue {
    // No &self to give master ownership
    pub fn into_real(self) -> Result<RealKhronosValue, crate::Error> {
        use pb::khronos_value::Value;

        match self.value {
            None => Err("KhronosValue missing value".into()),
            Some(v) => match v {
                Value::Text(s) => Ok(RealKhronosValue::Text(s)),
                Value::Integer(i) => Ok(RealKhronosValue::Integer(i)),
                Value::UnsignedInteger(u) => Ok(RealKhronosValue::UnsignedInteger(u)),
                Value::Float(f) => Ok(RealKhronosValue::Float(f)),
                Value::Boolean(b) => Ok(RealKhronosValue::Boolean(b)),
                Value::Buffer(b) => Ok(RealKhronosValue::Buffer(b)),
                Value::Vector(v) => Ok(RealKhronosValue::Vector((v.x, v.y, v.z))),
                Value::Map(m) => {
                    let entries = m.entries.into_iter().map(|e| {
                        let k = e.key.ok_or("Map entry missing key")?.into_real()?;
                        let v = e.value.ok_or("Map entry missing value")?.into_real()?;
                        Ok((k, v))
                    }).collect::<Result<Vec<_>, crate::Error>>()?;
                    Ok(RealKhronosValue::Map(entries))
                },
                Value::List(l) => {
                    let items = l.values.into_iter().map(|v| v.into_real()).collect::<Result<Vec<_>, _>>()?;
                    Ok(RealKhronosValue::List(items))
                },
                Value::Timestamptz(s) => {
                    let dt = chrono::DateTime::parse_from_rfc3339(&s)
                        .map_err(|e| format!("Invalid timestamptz: {}", e))?
                        .with_timezone(&chrono::Utc);
                    Ok(RealKhronosValue::Timestamptz(dt))
                },
                Value::Interval(ms) => {
                    let new_time = TimeDelta::new(ms.secs, ms.nanos as u32).ok_or(format!("duration is out of bounds"))?;

                    Ok(RealKhronosValue::Interval(new_time))
                },
                Value::TimeZone(tz) => {
                    let tz: chrono_tz::Tz = tz.parse()
                        .map_err(|e| format!("Invalid timezone: {}", e))?;
                    Ok(RealKhronosValue::TimeZone(tz))
                },
                Value::MemoryVfs(vfs) => {
                    Ok(RealKhronosValue::MemoryVfs(vfs.entries))
                },
                Value::NullValue(_) => Ok(RealKhronosValue::Null),
            },
        }
    }

    pub fn from_real(value: RealKhronosValue) -> Self {
        use pb::khronos_value::Value;

        let v = match value {
            RealKhronosValue::Text(s) => Value::Text(s),
            RealKhronosValue::Integer(i) => Value::Integer(i),
            RealKhronosValue::UnsignedInteger(u) => Value::UnsignedInteger(u),
            RealKhronosValue::Float(f) => Value::Float(f),
            RealKhronosValue::Boolean(b) => Value::Boolean(b),
            RealKhronosValue::Buffer(b) => Value::Buffer(b),
            RealKhronosValue::Vector((x, y, z)) => Value::Vector(pb::KhronosVector { x, y, z }),
            RealKhronosValue::Map(m) => {
                let entries = m.into_iter().map(|(k, v)| {
                    let key = Some(Self::from_real(k));
                    let value = Some(Self::from_real(v));
                    pb::KhronosMapEntry { key, value }
                }).collect::<Vec<_>>();
                Value::Map(pb::KhronosMap { entries })
            },
            RealKhronosValue::List(l) => {
                let values = l.into_iter().map(Self::from_real).collect::<Vec<_>>();
                Value::List(pb::KhronosList { values })
            },
            RealKhronosValue::Timestamptz(dt) => {
                Value::Timestamptz(dt.to_rfc3339())
            },
            RealKhronosValue::Interval(dur) => {
                Value::Interval(pb::KhronosInterval { secs: dur.num_seconds(), nanos: dur.subsec_nanos() })
            },
            RealKhronosValue::TimeZone(tz) => {
                Value::TimeZone(tz.name().to_string())
            },
            RealKhronosValue::MemoryVfs(entries) => {
                Value::MemoryVfs(pb::KhronosMemoryVfs { entries })
            },
            RealKhronosValue::Null => Value::NullValue(true),
        };

        Self { value: Some(v) }
    }
}



impl pb::DispatchEventResponse {
    pub fn from_real(result: Result<RealKhronosValue, crate::Error>) -> Self {
        Self {
            resp: Some(match result {
                Ok(v) => {
                    match pb::AnyValue::from_real_exec(&v) {
                        Ok(v) => pb::dispatch_event_response::Resp::Value(v),
                        Err(e) => pb::dispatch_event_response::Resp::Error(format!("Failed to convert dispatch result to AnyValue: {:?}", e)),
                    }
                },
                Err(e) => pb::dispatch_event_response::Resp::Error(e.to_string()),
            })
        }
    }

    pub fn to_real(self) -> Result<RealKhronosValue, crate::Error> {
        match self.resp {
            Some(pb::dispatch_event_response::Resp::Value(v)) => v.to_real_exec(),
            Some(pb::dispatch_event_response::Resp::Error(e)) => Err(e.into()),
            None => Err("DispatchEventResponse missing payload".into()),
        }
    }
}

#[derive(Clone)]
pub struct MesophyllServer {
    idents: Arc<HashMap<usize, String>>,
    conns: Arc<DashMap<usize, MesophyllServerConn>>,
    db_state: DbState,
}

impl MesophyllServer {
    const TOKEN_LENGTH: usize = 64;

    pub async fn new(num_idents: usize, pool: sqlx::PgPool) -> Result<Self, crate::Error> {
        let mut idents = HashMap::new();
        for i in 0..num_idents {
            let ident = Alphanumeric.sample_string(&mut rand::rng(), Self::TOKEN_LENGTH);
            idents.insert(i, ident);
        }
        Self::new_with(idents, pool).await
    }

    async fn new_with(idents: HashMap<usize, String>, pool: sqlx::PgPool) -> Result<Self, crate::Error> {
        let s = Self {
            db_state: DbState::new(idents.len(), pool).await?,
            idents: Arc::new(idents),
            conns: Arc::new(DashMap::new()),
        };

        let meso_addr = crate::CONFIG.addrs.mesophyll_server.to_socket_addrs()?.next().ok_or("Invalid Mesophyll server address")?;

        let s_ref = s.clone();
        tokio::spawn(async move {
            tonic::transport::Server::builder()
            .add_service(pb::mesophyll_master_server::MesophyllMasterServer::new(s_ref))
            .serve(meso_addr)
            .await
            .unwrap();
        });

        Ok(s)
    }

    pub fn get_token_for_worker(&self, worker_id: usize) -> Option<&String> {
        self.idents.get(&worker_id)
    }

    pub fn get_connection(&self, worker_id: usize) -> Option<MesophyllServerConn> {
        self.conns.get(&worker_id).map(|r| r.value().clone())
    }

    pub fn db_state(&self) -> &DbState { &self.db_state }

    fn verify_worker(&self, worker: Option<pb::Worker>) -> Result<usize, Status> {
        let worker = worker.ok_or_else(|| Status::invalid_argument("Missing worker info in request"))?;
        worker.validate(self)?;
        Ok(worker.worker_id_usize()?)
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
        if let Some(old) = self.conns.insert(wk.worker_id_usize()?, conn.clone()) {
            old.tx.send(Ok(pb::MtwMessage {
                payload: Some(pb::mtw_message::Payload::Shutdown("Another worker with the same ID has connected".to_string())),
                id: None,
            })).ok();
        }

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
                                continue;
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

        match self.db_state.global_key_value_db().global_kv_find(scope, query).await {
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

        match self.db_state.global_key_value_db().global_kv_get(key, version, scope, id).await {
            Ok(record) => Ok(tonic::Response::new(pb::AnyValue::from_real(&record)?)),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn global_kv_create(&self, request: tonic::Request<pb::WtmGlobalKvCreate>) -> Result<tonic::Response<pb::WtmBool>, Status> {
        let req = request.into_inner();
        self.verify_worker(req.worker)?;
        let id = req.id.ok_or_else(|| Status::invalid_argument("Missing ID"))?.to_real_id();
        let value = req.data.ok_or_else(|| Status::invalid_argument("Missing data"))?.to_real()?;

        match self.db_state.global_key_value_db().global_kv_create(id, value).await {
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

        match self.db_state.global_key_value_db().global_kv_delete(id, key, version, scope).await {
            Ok(_) => Ok(tonic::Response::new(pb::WtmBool { value: true })),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn list_tenant_states(&self, request: tonic::Request<pb::WtmListTenantStates>) -> Result<tonic::Response<pb::AnyValue>, Status> {
        let req = request.into_inner();
        let wid = self.verify_worker(req.worker)?;
        let val = self.db_state.tenant_state_cache_for(wid).await;
        Ok(tonic::Response::new(pb::AnyValue::from_real(&val)?))
    }

    async fn set_tenant_state_for(&self, request: tonic::Request<pb::WtmSetTenantStateFor>) -> Result<tonic::Response<pb::WtmBool>, Status> {
        let req = request.into_inner();
        self.verify_worker(req.worker)?;
        let id = req.id.ok_or_else(|| Status::invalid_argument("Missing ID"))?.to_real_id();
        let state = req.state.ok_or_else(|| Status::invalid_argument("Missing state"))?.to_real()?;

        match self.db_state.set_tenant_state_for(id, state).await {
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
        let pb_event = pb::AnyValue::from_real(&event)?;

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
        resp.to_real()
    }
    
    pub fn dispatch_event_nowait(&self, id: RealId, event: RealCreateEvent) -> Result<(), crate::Error> {
        let pb_event = pb::AnyValue::from_real(&event)?;

        let msg = pb::MtwMessage {
            payload: Some(pb::mtw_message::Payload::Dispatch(pb::DispatchEvent {
                id: Some(pb::Id::from_real_id(&id)),
                event_payload: Some(pb_event),
            })),
            id: None,
        };
        
        self.tx.send(Ok(msg)).map_err(|e| Status::internal(format!("Failed to send dispatch message to worker {}: {}", self.id, e)))?;
        Ok(())
    }

    pub async fn run_script(&self, id: RealId, name: String, code: String, event: RealCreateEvent) -> Result<RealKhronosValue, crate::Error> {
        let pb_event = pb::AnyValue::from_real(&event)?;

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
        resp.to_real()
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