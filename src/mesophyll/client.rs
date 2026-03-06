use std::{collections::HashMap, sync::Arc};

use futures::{StreamExt, stream::FuturesUnordered};
use khronos_runtime::utils::khronos_value::KhronosValue;
use crate::{mesophyll::{dbtypes::{CreateGlobalKv, GlobalKv, PartialGlobalKv, TenantState}}, worker::{workerlike::WorkerLike, workerthread::WorkerThread, workervmmanager::Id}};
use crate::geese::kv::SerdeKvRecord;
use crate::mesophyll::server::pb;

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
    pub async fn new(token: String, worker_id: usize) -> Result<(Self, MesophyllClientStream), crate::Error> {
        let worker = pb::Worker {
            worker_id: worker_id as u64,
            token: token.clone(),
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
            let mut run_scripts = FuturesUnordered::new();
            let mut drop_tenants = FuturesUnordered::new();
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
                                    pb::mtw_message::Payload::RunScript(de) => {
                                        let Some(id) = de.id else {
                                            log::error!("Mesophyll client received Dispatch message with no ID");
                                            continue;
                                        };
                                        let Some(event) = de.event else {
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
                                        run_scripts.push(async move {
                                            let resp = wt.run_script(id.to_real_id(), de.name, de.code, evt).await;
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
                        let response = pb::wtm_message::Payload::DispatchResponse(pb::DispatchEventResponse::from_real(result));
                        let _ = self_ref.client_stream_tx.send(pb::WtmMessage {
                            payload: Some(response),
                            resp_id: Some(id),
                        });
                    }
                    Some((id, result)) = run_scripts.next() => {
                        let Some(id) = id else { continue };
                        let response = pb::wtm_message::Payload::DispatchResponse(pb::DispatchEventResponse::from_real(result));
                        let _ = self_ref.client_stream_tx.send(pb::WtmMessage {
                            payload: Some(response),
                            resp_id: Some(id),
                        });
                    }
                    Some((id, result)) = drop_tenants.next() => {
                        let Some(id) = id else { continue };
                        let response = pb::wtm_message::Payload::DropTenantAck({
                            if let Err(err) = result { 
                                log::error!("Error dropping tenant: {err:?}"); 
                                1 
                            } else { 
                                0 
                            }
                        });
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
    pub async fn set_tenant_state_for(&self, id: Id, state: &TenantState) -> Result<(), crate::Error> {
        let mut cli = self.client.clone();
        cli.set_tenant_state_for(pb::WtmSetTenantStateFor { 
            worker: Some(self.worker.clone()), 
            id: Some(pb::Id::from_real_id(&id)),
            state: Some(pb::AnyValue::from_real_exec(state)?),
        })
        .await
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Fetch a key-value record given a tenant ID, list of scopes, and key
    pub async fn kv_get(&self, id: Id, scopes: Vec<String>, key: String) -> Result<Option<SerdeKvRecord>, crate::Error> {
        let mut cli = self.client.clone();
        log::info!("MesophyllClient: Fetching KV record for ID {:?}, scopes {:?}, key {:?}", id, scopes, key);
        cli.kv_get(pb::WtmKvGet { 
            worker: Some(self.worker.clone()), 
            id: Some(pb::Id::from_real_id(&id)),
            scopes,
            key,
        })
        .await
        .map_err(|e| e.to_string())?
        .into_inner()
        .to_real_exec()
    }

    /// List all scopes that have key-value records for a given tenant ID
    pub async fn kv_list_scopes(&self, id: Id) -> Result<Vec<String>, crate::Error> {
        let mut cli = self.client.clone();
        Ok(cli.kv_list_scopes(pb::WtmKvListScopes {
            worker: Some(self.worker.clone()),
            id: Some(pb::Id::from_real_id(&id)),
        })
        .await
        .map_err(|e| e.to_string())?
        .into_inner()
        .scopes)
    }

    /// Set a key-value record for a given tenant ID, list of scopes, and key
    pub async fn kv_set(&self, id: Id, scopes: Vec<String>, key: String, value: KhronosValue) -> Result<(), crate::Error> {
        let mut cli = self.client.clone();
        cli.kv_set(pb::WtmKvSet {
            worker: Some(self.worker.clone()),
            id: Some(pb::Id::from_real_id(&id)),
            scopes,
            key,
            value: Some(pb::AnyValue::from_real_exec(&value)?),
        })
        .await
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Delete a key-value record for a given tenant ID, list of scopes, and key
    pub async fn kv_delete(&self, id: Id, scopes: Vec<String>, key: String) -> Result<(), crate::Error> {
        let mut cli = self.client.clone();
        cli.kv_delete(pb::WtmKvDelete {
            worker: Some(self.worker.clone()),
            id: Some(pb::Id::from_real_id(&id)),
            scopes,
            key,
        })
        .await
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Find key-value records with keys that start with a given prefix for a given tenant ID and list of scopes
    pub async fn kv_find(&self, id: Id, scopes: Vec<String>, prefix: String) -> Result<Vec<SerdeKvRecord>, crate::Error> {
        let mut cli = self.client.clone();
        Ok(cli.kv_find(pb::WtmKvFind {
            worker: Some(self.worker.clone()),
            id: Some(pb::Id::from_real_id(&id)),
            scopes,
            prefix,
        })
        .await
        .map_err(|e| e.to_string())?
        .into_inner()
        .to_real_exec()?)
    }

    /// Find global key-value records with keys that start with a given prefix for a given scope and optional tenant ID
    pub async fn global_kv_find(&self, scope: String, query: String) -> Result<Vec<PartialGlobalKv>, crate::Error> {
        let mut cli = self.client.clone();
        Ok(cli.global_kv_find(pb::WtmGlobalKvFind {
            scope,
            query,
        })
        .await
        .map_err(|e| e.to_string())?
        .into_inner()
        .to_real_exec()?)
    }

    /// Get a global key-value record for a given key, version, scope, and optional tenant ID
    pub async fn global_kv_get(&self, key: String, version: i32, scope: String, id: Option<Id>) -> Result<Option<GlobalKv>, crate::Error> {
        let mut cli = self.client.clone();
        Ok(cli.global_kv_get(pb::WtmGlobalKvGet {
            key,
            version,
            scope,
            id: id.map(|id| pb::Id::from_real_id(&id)),
        })
        .await
        .map_err(|e| e.to_string())?
        .into_inner()
        .to_real_exec()?)
    }

    /// Create a global key-value record for a given tenant ID and CreateGlobalKv struct
    pub async fn global_kv_create(&self, id: Id, gkv: CreateGlobalKv) -> Result<(), crate::Error> {
        let mut cli = self.client.clone();
        cli.global_kv_create(pb::WtmGlobalKvCreate {
            worker: Some(self.worker.clone()),
            id: Some(pb::Id::from_real_id(&id)),
            data: Some(pb::AnyValue::from_real_exec(&gkv)?),
        })
        .await
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Delete a global key-value record for a given key, version, scope, and tenant ID
    pub async fn global_kv_delete(&self, id: Id, key: String, version: i32, scope: String) -> Result<(), crate::Error> {
        let mut cli = self.client.clone();
        cli.global_kv_delete(pb::WtmGlobalKvDelete {
            worker: Some(self.worker.clone()),
            key,
            version,
            scope,
            id: Some(pb::Id::from_real_id(&id)),
        })
        .await
        .map_err(|e| e.to_string())?;
        Ok(())
    }
}