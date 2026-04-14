use std::{collections::HashMap, sync::Arc};

use futures::{StreamExt, stream::FuturesUnordered};
use crate::{geese::{state::{StateExecResponse, StateOp}, tenantstate::TenantState}, master::syscall::{MSyscallArgs, MSyscallError, MSyscallRet}, worker::{workerthread::WorkerThread, workervmmanager::Id}};
use crate::mesophyll::server::pb;
use khronos_runtime::utils::khronos_value::KhronosValue;

/// Mesophyll client
#[derive(Clone)]
pub struct MesophyllShellClient {
    client: pb::mesophyll_master_client::MesophyllMasterClient<tonic::transport::Channel>,
}

#[allow(dead_code)]
impl MesophyllShellClient {
    /// Creates a new Mesophyll client
    pub async fn new() -> Result<Self, crate::Error> {
        let uri = tonic::transport::Endpoint::from_shared(format!("http://{}", crate::CONFIG.addrs.mesophyll_server))?;
        let client = pb::mesophyll_master_client::MesophyllMasterClient::connect(uri).await?;

        Ok(Self { client })
    }

    /// Returns a list of all tenant states from the Mesophyll server
    pub async fn msyscall(&self, args: MSyscallArgs) -> Result<MSyscallRet, MSyscallError> {
        let mut cli = self.client.clone();
        Ok(cli.shell_msyscall(pb::ShellMSyscall { token: crate::CONFIG.meta.mesophyll_token.clone(), msyscall: Some(pb::AnyValue::from_real(&args)?) })
            .await
            .map_err(|e| e.to_string())?
            .into_inner()
            .to_real_exec()?)
    }
}

/// Mesophyll client
#[derive(Clone)]
pub struct MesophyllClient {
    worker: pb::Worker,
    client: pb::mesophyll_master_client::MesophyllMasterClient<tonic::transport::Channel>,
    client_stream_tx: tokio::sync::mpsc::UnboundedSender<pb::WtmMessage>,
}

pub struct MesophyllClientStream {
    server_stream: tonic::Streaming<pb::MtwMessage>,
}

#[allow(dead_code)]
impl MesophyllClient {
    /// Creates a new Mesophyll client
    pub async fn new(worker_id: usize) -> Result<(Self, MesophyllClientStream), crate::Error> {
        let worker = pb::Worker {
            worker_id: worker_id as u64,
            token: crate::CONFIG.meta.mesophyll_token.clone(),
        };
        let uri = tonic::transport::Endpoint::from_shared(format!("http://{}", crate::CONFIG.addrs.mesophyll_server))?;
        let mut client = pb::mesophyll_master_client::MesophyllMasterClient::connect(uri).await?;

        // Start worker_init and identify to the server
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        tx.send(pb::WtmMessage {
            payload: Some(pb::wtm_message::Payload::WorkerIdent(worker.clone())),
            resp_id: None,
        })?;
        let client_stream = tokio_stream::wrappers::UnboundedReceiverStream::new(rx);
        let server_stream = client.worker_init(client_stream).await?.into_inner();

        log::info!("Mesophyll client connected and initialized with worker ID {}", worker_id);

        let s = Self {
            worker,
            client,
            client_stream_tx: tx,
        };
        
        Ok((s, MesophyllClientStream { server_stream }))
    }

    pub fn listen(&self, stream: MesophyllClientStream, wt: Arc<WorkerThread>) {
        let self_ref = self.clone();
        let mut server_stream = stream.server_stream;
        tokio::task::spawn(async move {
            let mut dispatches = FuturesUnordered::new();
            let mut drop_tenants = FuturesUnordered::new();
            let mut update_tenant_state = FuturesUnordered::new();
            loop {
                tokio::select! {
                    Some(recv) = server_stream.next() => {
                        match recv {
                            Ok(pb::MtwMessage { payload: Some(p), id: req_id }) => {
                                match p {
                                    pb::mtw_message::Payload::Dispatch(de) => {
                                        let Some(id) = de.id else {
                                            log::error!("Mesophyll client received Dispatch message with no ID");
                                            continue;
                                        };
                                        let Some(event) = de.event_payload else {
                                            log::error!("Mesophyll client received Dispatch message with no event");
                                            continue;
                                        };
                                        let evt = match pb::AnyValue::to_real(&event) {
                                            Ok(ev) => ev,
                                            Err(e) => {
                                                log::error!("Mesophyll client failed to convert event payload to real value: {:?}", e);
                                                continue;
                                            }
                                        };
                                        let wt = wt.clone();
                                        dispatches.push(async move {
                                            let resp = wt.dispatch_event(id.to_real_id(), evt).await;
                                            (req_id, resp)
                                        });
                                    },
                                    pb::mtw_message::Payload::DropTenant(id) => {
                                        let wt = wt.clone();
                                        drop_tenants.push(async move {
                                            let resp = wt.drop_tenant(id.to_real_id()).await;
                                            (req_id, resp)
                                        });
                                    }
                                    pb::mtw_message::Payload::Shutdown(reason) => {
                                        log::info!("Mesophyll server requested shutdown: {}", reason);
                                        wt.kill().await.expect("Failed to kill worker thread");
                                        std::process::exit(0);
                                    }
                                    pb::mtw_message::Payload::UpdateTenantState(payload) => {
                                        let Some(id) = payload.id else {
                                            log::error!("Mesophyll client received UpdateTenantState message with no ID");
                                            continue;
                                        };
                                        let Some(new_tenant_state) = payload.new_tenant_state  else {
                                            log::error!("Mesophyll client received UpdateTenantState message with no event");
                                            continue;
                                        };
                                        let new_tenant_state: TenantState = match pb::AnyValue::to_real(&new_tenant_state) {
                                            Ok(ev) => ev,
                                            Err(e) => {
                                                log::error!("Mesophyll client failed to convert event payload to real value: {:?}", e);
                                                continue;
                                            }
                                        };
                                        let wt = wt.clone();
                                        update_tenant_state.push(async move {
                                            let resp = wt.update_tenant_state(id.to_real_id(), new_tenant_state).await;
                                            (req_id, resp)
                                        });
                                    }
                                }
                            }
                            Ok(_) => {
                                log::warn!("Mesophyll client received invalid message with no payload");
                            }
                            Err(e) => {
                                log::error!("Mesophyll client stream error, waiting 10 seconds to retry: {}", e);
                                tokio::time::sleep(std::time::Duration::from_secs(10)).await;
                                continue
                            }
                        }
                    }
                    Some((id, result)) = dispatches.next() => {
                        let Some(id) = id else { continue };
                        let response = pb::wtm_message::Payload::Resp(pb::WtmMessageResponse::from_real(result));
                        let _ = self_ref.client_stream_tx.send(pb::WtmMessage {
                            payload: Some(response),
                            resp_id: Some(id),
                        });
                    }
                    Some((id, result)) = drop_tenants.next() => {
                        let Some(id) = id else { continue };
                        let response = pb::wtm_message::Payload::Resp(pb::WtmMessageResponse::from_real(result.map(|_| KhronosValue::Null)));
                        let _ = self_ref.client_stream_tx.send(pb::WtmMessage {
                            payload: Some(response),
                            resp_id: Some(id),
                        });
                    }
                    Some((id, result)) = update_tenant_state.next() => {
                        let Some(id) = id else { continue };
                        let response = pb::wtm_message::Payload::Resp(pb::WtmMessageResponse::from_real(result.map(|_| KhronosValue::Null)));
                        let _ = self_ref.client_stream_tx.send(pb::WtmMessage {
                            payload: Some(response),
                            resp_id: Some(id),
                        });
                    }
                }
            }
        });
    }

    /// Returns a list of all tenant states from the Mesophyll server
    pub async fn list_tenant_states(&self) -> Result<HashMap<Id, TenantState>, crate::Error> {
        let mut cli = self.client.clone();
        cli.list_tenant_states(pb::WtmListTenantStates { worker: Some(self.worker.clone()) })
            .await
            .map_err(|e| e.to_string())?
            .into_inner()
            .to_real_exec()
    }

    /// Sets the tenant state for a given tenant ID
    pub async fn exec_state_op(&self, id: Id, state_op: Vec<StateOp>) -> Result<StateExecResponse, crate::Error> {
        let mut cli = self.client.clone();
        Ok(cli.exec_state_op(pb::WtmExecStateOp { 
            worker: Some(self.worker.clone()), 
            id: Some(pb::Id::from_real_id(&id)),
            state_op: Some(pb::AnyValue::from_real_exec(&state_op)?),
        })
        .await
        .map_err(|e| e.to_string())?
        .into_inner()
        .to_real_exec()?)
    }

    /// Fetch common information for the worker from the Mesophyll server, such as number of workers in the pool
    pub async fn fetch_base_worker_info(&self) -> Result<pb::MtwBaseWorkerInfo, crate::Error> {
        let mut cli = self.client.clone();
        Ok(cli.base_worker_info(tonic::Request::new(self.worker.clone()))
            .await
            .map_err(|e| e.to_string())?
            .into_inner())
    }
}