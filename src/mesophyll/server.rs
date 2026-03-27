use futures::{Stream, StreamExt};
use serenity::all::{UserId, GuildId};
use tonic::Status;
use crate::{mesophyll::dbstate::DbState, worker::workervmmanager::Id as RealId};
use khronos_runtime::utils::khronos_value::KhronosValue as RealKhronosValue;
use khronos_runtime::primitives::event::CreateEvent as RealCreateEvent;
use dashmap::DashMap;
use std::net::ToSocketAddrs;
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
    fn validate(&self, server: &MesophyllServer) -> Result<(), Status> {
        let wid = self.worker_id_usize()?;
        if wid > server.db_state().num_workers() {
            return Err(Status::permission_denied(format!("Invalid worker ID: {}, exceeds number of workers in pool", wid)));
        }

        if self.token != crate::CONFIG.meta.mesophyll_token {
            return Err(Status::permission_denied("Invalid token"));
        }

        Ok(())
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
    conns: Arc<DashMap<usize, MesophyllServerConn>>,
    db_state: DbState,
}

impl MesophyllServer {
    pub async fn new(num_workers: usize, pool: sqlx::PgPool) -> Result<Self, crate::Error> {
        let s = Self {
            db_state: DbState::new(num_workers, pool).await?,
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
        self.conns.insert(wk.worker_id_usize()?, conn.clone());

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

    async fn exec_state_op(&self, request: tonic::Request<pb::WtmExecStateOp>) -> Result<tonic::Response<pb::AnyValue>, Status> {
        let req = request.into_inner();
        self.verify_worker(req.worker)?;
        let id = req.id.ok_or_else(|| Status::invalid_argument("Missing ID"))?.to_real_id();
        let state_op = req.state_op.ok_or_else(|| Status::invalid_argument("Missing state_op"))?.to_real()?;

        match self.db_state.state_db().do_op(id, state_op).await {
            Ok(result) => Ok(tonic::Response::new(pb::AnyValue::from_real(&result)?)),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn list_tenant_states(&self, request: tonic::Request<pb::WtmListTenantStates>) -> Result<tonic::Response<pb::AnyValue>, Status> {
        let req = request.into_inner();
        let wid = self.verify_worker(req.worker)?;
        match self.db_state.tenant_state_db().get_tenant_state(Some((wid as i64, self.db_state.num_workers() as i64))).await {
            Ok(ts) => Ok(tonic::Response::new(pb::AnyValue::from_real(&ts)?)),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn base_worker_info(&self, request: tonic::Request<pb::Worker>) -> Result<tonic::Response<pb::MtwBaseWorkerInfo>, Status> {
        let req = request.into_inner();
        req.validate(self)?;

        Ok(tonic::Response::new(pb::MtwBaseWorkerInfo {
            num_workers: self.db_state().num_workers().try_into().map_err(|_e| Status::internal("num_workers exceeds u32 max"))?,
        }))
    }
}

#[derive(Clone)]
pub struct MesophyllServerConn {
    id: u64,
    tx: mpsc::UnboundedSender<Result<pb::MtwMessage, Status>>,
    pub(super) dispatch_response_handlers: Arc<DashMap<u64, oneshot::Sender<pb::DispatchEventResponse>>>,
    pub(super) drop_tenant_ack_handlers: Arc<DashMap<u64, oneshot::Sender<()>>>,
}

// On drop, shutdown
impl Drop for MesophyllServerConn {
    fn drop(&mut self) {
        log::info!("Worker with ID: {} disconnected, cleaning up connection", self.id);
        // Send a shutdown message to the worker to clean up any resources on the worker side
        let _ = self.tx.send(Ok(pb::MtwMessage {
            payload: Some(pb::mtw_message::Payload::Shutdown("Worker disconnected".to_string())),
            id: None,
        }));
    }
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