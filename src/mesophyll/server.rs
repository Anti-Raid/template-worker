use dapi::{UserId, GuildId};
use rand::distr::{Alphanumeric, SampleString};
use tokio_stream::wrappers::UnixListenerStream;
use tonic::Status;
use crate::{geese::{state::{StateDb, StateDbFlags}, tenantstate::{TenantState, TenantStateDb}}, mesophyll::connman::{SockFile, new_sockfile}, worker::{workerdispatch::SimpleEvent, workervmmanager::Id as RealId}};
use khronos_runtime::utils::khronos_value::KhronosValue as RealKhronosValue;
use dashmap::DashMap;
use std::sync::Arc;
use tokio::{net::UnixListener, sync::broadcast};

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

#[derive(Clone)]
pub struct MesophyllServer {
    conns: Arc<DashMap<usize, WorkerConnGuard>>,
    tenant_state_db: TenantStateDb,
    state_db: StateDb,
    num_workers: usize,
    sock_file: Arc<SockFile>,
    attached_streams: AttachedStreams,
}

impl MesophyllServer {
    pub async fn new(num_workers: usize, pool: sqlx::PgPool) -> Result<Self, crate::Error> {
        let s = Self {
            conns: Arc::new(DashMap::new()),
            tenant_state_db: TenantStateDb::new(pool.clone()),
            state_db: StateDb::new(pool),
            num_workers,
            sock_file: Arc::new(new_sockfile(Alphanumeric.sample_string(&mut rand::rng(), 16), Alphanumeric.sample_string(&mut rand::rng(), 16))?),
            attached_streams: Arc::new(DashMap::new()),
        };

        // Setup UDS stream
        let uds = UnixListener::bind(&s.sock_file.sock)?;
        let uds_stream = UnixListenerStream::new(uds);

        let s_ref = s.clone();
        tokio::spawn(async move {
            tonic::transport::Server::builder()
            .add_service(pb::mesophyll_master_server::MesophyllMasterServer::new(s_ref))
            .serve_with_incoming(uds_stream)
            .await
            .unwrap();
        });

        Ok(s)
    }

    pub fn sock_file(&self) -> &Arc<SockFile> {
        &self.sock_file
    }

    pub fn get_connection(&self, worker_id: usize) -> Option<WorkerConn> {
        self.conns.get(&worker_id).map(|r| r.value().conn.clone())
    }

    fn verify_worker(&self, worker: u64) -> Result<usize, Status> {
        let id = worker.try_into().map_err(|_e| tonic::Status::internal("WID not a u64"))?;
        if id > self.num_workers {
            return Err(Status::permission_denied(format!("Invalid worker ID: {}, exceeds number of workers in pool", id)));
        }
        Ok(id)
    }
}

#[tonic::async_trait]
impl pb::mesophyll_master_server::MesophyllMaster for MesophyllServer {
    async fn exec_state_op(&self, request: tonic::Request<pb::WtmExecStateOp>) -> Result<tonic::Response<pb::AnyValue>, Status> {
        let req = request.into_inner();
        let wid = self.verify_worker(req.worker_id)?;
        let id = req.id.ok_or_else(|| Status::invalid_argument("Missing ID"))?.to_real_id();
        if id.worker_id(self.num_workers) != wid {
            return Err(Status::internal("ID expected worker_id and actual worker_id mismatched"));
        }
        let state_op = req.state_op.ok_or_else(|| Status::invalid_argument("Missing state_op"))?.to_real()?;
        let sdb_flags = StateDbFlags::from_bits(req.flags).ok_or_else(|| Status::invalid_argument("Invalid flags"))?;
        match self.state_db.do_op(id, state_op, sdb_flags).await {
            Ok(result) => Ok(tonic::Response::new(pb::AnyValue::from_real(&result)?)),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn list_tenant_states(&self, request: tonic::Request<pb::WtmListTenantStates>) -> Result<tonic::Response<pb::AnyValue>, Status> {
        let req = request.into_inner();
        let wid = self.verify_worker(req.worker_id)?;
        match self.tenant_state_db.get_tenant_state(wid as i64, self.num_workers as i64).await {
            Ok(ts) => Ok(tonic::Response::new(pb::AnyValue::from_real(&ts)?)),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn base_worker_info(&self, _request: tonic::Request<pb::Empty>) -> Result<tonic::Response<pb::MtwBaseWorkerInfo>, Status> {
        Ok(tonic::Response::new(pb::MtwBaseWorkerInfo {
            num_workers: self.num_workers.try_into().map_err(|_e| Status::internal("num_workers exceeds u32 max"))?,
        }))
    }

    async fn register_worker(&self, request: tonic::Request<pb::WorkerIdent>) -> Result<tonic::Response<pb::Empty>, Status> {
        let req = request.into_inner();
        let wid = self.verify_worker(req.worker_id)?;

        // We have an endpoint now to connect to the worker conn
        let uri = tonic::transport::Endpoint::from_shared(format!("unix://{}", req.endpoint)).map_err(|x| Status::internal(x.to_string()))?;
        let client = pb::mesophyll_worker_client::MesophyllWorkerClient::connect(uri).await.map_err(|x| Status::internal(x.to_string()))?;
        let conn = WorkerConn::new(req.worker_id, client, self.attached_streams.clone());

        self.conns.insert(wid, WorkerConnGuard { conn });

        Ok(tonic::Response::new(pb::Empty {}))
    }

    async fn publish_feed(&self, request: tonic::Request<pb::PublishFeedMessage>) -> Result<tonic::Response<pb::Empty>, Status> {
        let req = request.into_inner();
        let id = req.id.ok_or_else(|| Status::invalid_argument("Missing ID"))?.to_real_id();

        if let Some(tenant_streams) = self.attached_streams.get(&id) {
            if let Some(tx) = tenant_streams.get(&req.topic) {
                let payload: RealKhronosValue = req.payload.ok_or_else(|| Status::invalid_argument("Missing payload"))?.to_real()?;
                let _ = tx.send(payload);
            }
        }
        Ok(tonic::Response::new(pb::Empty {}))
    }
}

type AttachedStreams = Arc<DashMap<RealId, DashMap<String, broadcast::Sender<RealKhronosValue>>>>;

/// Internal struct to send a shutdown to workers if a new worker gets spinned in its place
struct WorkerConnGuard {
    conn: WorkerConn
}

// On drop, shutdown
impl Drop for WorkerConnGuard {
    fn drop(&mut self) {
        log::info!("Worker with ID: {} disconnected, cleaning up connection", self.conn.id);
        let mut cli = self.conn.client.clone();
        tokio::spawn(async move {
            let _ = cli.shutdown(pb::Empty {}).await;
        });
    }
}

#[derive(Clone)]
pub struct WorkerConn {
    id: u64,
    client: pb::mesophyll_worker_client::MesophyllWorkerClient<tonic::transport::Channel>,
    attached_streams: AttachedStreams
}

impl WorkerConn {
    fn new(id: u64, client: pb::mesophyll_worker_client::MesophyllWorkerClient<tonic::transport::Channel>, attached_streams: AttachedStreams) -> Self {
        Self { id, client, attached_streams }
    }

    pub async fn dispatch_event(&self, id: RealId, event: SimpleEvent) -> Result<RealKhronosValue, crate::Error> {
        let pb_event = pb::AnyValue::from_real(&event)?;

        let msg = pb::DispatchEventReq {
            id: Some(pb::Id::from_real_id(&id)),
            event_payload: Some(pb_event),
        };

        let mut cli = self.client.clone();
        let resp = cli.dispatch_event(tonic::Request::new(msg))
            .await
            .map_err(|e| e.to_string())?
            .into_inner();
        resp.to_real_exec()
    }
    
    pub async fn drop_tenant(&self, id: RealId) -> Result<(), crate::Error> {
        let mut cli = self.client.clone();
        cli.drop_tenant(pb::Id::from_real_id(&id))
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn update_tenant_state(&self, id: RealId, tenant_state: TenantState) -> Result<bool, crate::Error> {
        let msg = pb::UpdateTenantStateReq {
            id: Some(pb::Id::from_real_id(&id)),
            new_tenant_state: Some(pb::AnyValue::from_real(&tenant_state)?)
        };
        let mut cli = self.client.clone();
        let resp = cli.update_tenant_state(msg)
            .await
            .map_err(|e| e.to_string())?
            .into_inner();
        Ok(resp.b)
    }

    pub async fn shutdown(&self) -> Result<(), crate::Error> {
        let mut cli = self.client.clone();
        cli.shutdown(pb::Empty {})
            .await
            .map_err(|e| e.to_string())?
            .into_inner();
        Ok(())
    }

    pub async fn subscribe_topics(&self, id: RealId, topics: &[String]) -> Result<(TopicGuard, Vec<(String, broadcast::Receiver<RealKhronosValue>)>), crate::Error> {
        let tenant_streams = self.attached_streams.entry(id).or_default();
        let mut receivers = Vec::new();

        for topic in topics {
            let rx = {
                let tx = tenant_streams.entry(topic.clone()).or_insert_with(|| broadcast::channel(256).0);
                tx.subscribe()
            };
            receivers.push((topic.clone(), rx));
        }

        Ok((TopicGuard { msc: self.clone(), id, topics: topics.to_vec() }, receivers))
    }
}

pub struct TopicGuard {
    msc: WorkerConn,
    pub id: RealId,
    pub topics: Vec<String>
}

impl Drop for TopicGuard {
    fn drop(&mut self) {
        if let Some(tenant) = self.msc.attached_streams.get(&self.id) {
            for topic in &self.topics {
                tenant.remove_if(topic, |_, tx| tx.receiver_count() == 0);
            }
            if tenant.is_empty() {
                drop(tenant);
                self.msc.attached_streams.remove_if(&self.id, |_, tenant| tenant.is_empty());
            }
        }
    }
}
