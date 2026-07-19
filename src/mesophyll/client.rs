use std::time::Duration;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use crate::{geese::{state::{StateDbFlags, StateExecResponse, StateOp}, stream::LtcMessage, tenantstate::TenantState}, mesophyll::connman::{SockFile, new_sockfile_rooted}, worker::{workerthread::WorkerThread, workervmmanager::Id}};
use crate::mesophyll::server::pb;
use rand::distr::{Alphanumeric, SampleString};
use tokio::net::UnixListener;
use tokio_stream::wrappers::UnixListenerStream;
use tonic::Status;

/// Mesophyll client
#[derive(Clone)]
pub struct MesophyllClient {
    pub worker_id: u64,
    sock_file: Arc<SockFile>,
    client: pb::mesophyll_master_client::MesophyllMasterClient<tonic::transport::Channel>,
    wt: Arc<OnceLock<WorkerThread>>,
}

impl MesophyllClient {
    pub async fn new(worker_id: u64, master_sockfile: Arc<SockFile>) -> Result<Self, crate::Error> {
        let uri = tonic::transport::Endpoint::from_shared(format!("unix://{}", master_sockfile.sock.display()))?;
        let mut client = pb::mesophyll_master_client::MesophyllMasterClient::connect(uri).await?;

        let s = Self {
            sock_file: Arc::new(new_sockfile_rooted(master_sockfile.dir.clone(), Alphanumeric.sample_string(&mut rand::rng(), 16))?),
            worker_id,
            client: client.clone(),
            wt: OnceLock::new().into()
        };

        // Setup UDS stream
        let uds = UnixListener::bind(&s.sock_file.sock)?;
        let uds_stream = UnixListenerStream::new(uds);

        let s_ref = s.clone();
        tokio::spawn(async move {
            tonic::transport::Server::builder()
            .add_service(pb::mesophyll_worker_server::MesophyllWorkerServer::new(s_ref))
            .serve_with_incoming(uds_stream)
            .await
            .unwrap();
        });

        // Lastly, register the worker
        loop {
            let res = client.register_worker(pb::WorkerIdent { worker_id, endpoint: s.sock_file.sock.to_string_lossy().to_string() }).await;
            match res {
                Ok(_) => break,
                Err(e) => {
                    log::error!("Error registering worker: {e:?}");
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            }
        }

        Ok(s)
    }

    pub fn set_wt(&self, wt: WorkerThread) -> Result<(), crate::Error> {
        self.wt.set(wt).map_err(|_x| format!("wt already set"))?;
        Ok(())
    }

    fn try_wt(&self) -> Result<&WorkerThread, Status> {
        self.wt.get().ok_or_else(|| Status::internal("WorkerThread not up yet!"))
    }

    /// Returns a list of all tenant states from the Mesophyll server
    pub async fn list_tenant_states(&self) -> Result<HashMap<Id, TenantState>, crate::Error> {
        let mut cli = self.client.clone();
        cli.list_tenant_states(pb::WtmListTenantStates { worker_id: self.worker_id })
            .await
            .map_err(|e| e.to_string())?
            .into_inner()
            .to_real_exec()
    }

    /// Sets the tenant state for a given tenant ID
    pub async fn exec_state_op(&self, id: Id, state_op: Vec<StateOp>, flags: StateDbFlags) -> Result<StateExecResponse, crate::Error> {
        let mut cli = self.client.clone();
        Ok(cli.exec_state_op(pb::WtmExecStateOp { 
            worker_id: self.worker_id, 
            id: Some(pb::Id::from_real_id(&id)),
            state_op: Some(pb::AnyValue::from_real_exec(&state_op)?),
            flags: flags.bits()
        })
        .await
        .map_err(|e| e.to_string())?
        .into_inner()
        .to_real_exec()?)
    }

    /// Fetch common information for the worker from the Mesophyll server, such as number of workers in the pool
    pub async fn fetch_base_worker_info(&self) -> Result<pb::MtwBaseWorkerInfo, crate::Error> {
        let mut cli = self.client.clone();
        Ok(cli.base_worker_info(pb::Empty {})
            .await
            .map_err(|e| e.to_string())?
            .into_inner())
    }

    /// Send a stream message from worker to master
    pub async fn stream_message(&self, id: Id, payload: LtcMessage) -> Result<(), crate::Error> {
        let mut cli = self.client.clone();
        let pb_payload = pb::AnyValue::from_real(&payload)?;
        cli.send_stream(pb::StreamMessage {
            id: Some(pb::Id::from_real_id(&id)),
            payload: Some(pb_payload),
        })
        .await
        .map_err(|e| e.to_string())?
        .into_inner();
        Ok(())
    }

    /// Bulk send stream messages from worker to master
    pub async fn bulk_stream_message(&self, id: Id, conn_ids: Vec<u64>, msg: khronos_runtime::utils::khronos_value::KhronosValue) -> Result<(), crate::Error> {
        let mut cli = self.client.clone();
        let pb_payload = pb::AnyValue::from_real(&msg)?;
        let req = pb::BulkStreamMessage {
            id: Some(pb::Id::from_real_id(&id)),
            conn_ids,
            payload: Some(pb_payload),
        };
        cli.bulk_send_stream(req).await.map_err(|e| e.to_string())?;
        Ok(())
    }
}

#[tonic::async_trait]
impl pb::mesophyll_worker_server::MesophyllWorker for MesophyllClient {
    async fn send_stream(&self, request: tonic::Request<pb::StreamMessage>) -> Result<tonic::Response<pb::Empty>, Status> {
        let req = request.into_inner();
        let id = req.id.ok_or_else(|| Status::invalid_argument("Missing ID"))?.to_real_id();
        let evt = req.payload.ok_or_else(|| Status::invalid_argument("Missing payload"))?.to_real()?;
        let wt = self.try_wt()?;
        match wt.stream_msg(id, evt) {
            Ok(_) => Ok(tonic::Response::new(pb::Empty {})),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn dispatch_event(&self, request: tonic::Request<pb::DispatchEventReq>) -> Result<tonic::Response<pb::AnyValue>, Status> {
        let req = request.into_inner();
        let id = req.id.ok_or_else(|| Status::invalid_argument("Missing ID"))?.to_real_id();
        let evt = req.event_payload.ok_or_else(|| Status::invalid_argument("Missing event_payload"))?.to_real()?;
        let wt = self.try_wt()?;
        match wt.dispatch_event(id, evt).await {
            Ok(result) => Ok(tonic::Response::new(pb::AnyValue::from_real(&result)?)),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn drop_tenant(&self, request: tonic::Request<pb::Id>) -> Result<tonic::Response<pb::Empty>, Status> {
        let req = request.into_inner();
        let id = req.to_real_id();
        let wt = self.try_wt()?;
        match wt.drop_tenant(id).await {
            Ok(_) => Ok(tonic::Response::new(pb::Empty {})),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }

    async fn shutdown(&self, _request: tonic::Request<pb::Empty>) -> Result<tonic::Response<pb::Empty>, Status> {
        let wt = self.try_wt()?;
        log::info!("Mesophyll server requested shutdown");
        let _ = wt.kill().await;
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(5)).await;
            std::process::exit(0);
        });
        Ok(tonic::Response::new(pb::Empty {}))
    }

    async fn update_tenant_state(&self, request: tonic::Request<pb::UpdateTenantStateReq>) -> Result<tonic::Response<pb::Bool>, Status> {
        let req = request.into_inner();
        let id = req.id.ok_or_else(|| Status::invalid_argument("Missing ID"))?.to_real_id();
        let ts = req.new_tenant_state.ok_or_else(|| Status::invalid_argument("Missing new_tenant_state"))?.to_real()?;
        let wt = self.try_wt()?;
        match wt.update_tenant_state(id, ts).await {
            Ok(result) => Ok(tonic::Response::new(pb::Bool { b: result })),
            Err(e) => Err(Status::internal(e.to_string())),
        }
    }
}
