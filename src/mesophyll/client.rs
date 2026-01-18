use std::{collections::HashMap, sync::Arc, time::Duration};

use futures::{SinkExt, StreamExt, stream::FuturesUnordered};
use khronos_runtime::utils::khronos_value::KhronosValue;
use tokio::time::interval;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::{mesophyll::{MESOPHYLL_DEFAULT_HEARTBEAT_MS, message::{ClientMessage, ServerMessage}, server::SerdeKvRecord}, worker::{workerlike::WorkerLike, workerstate::TenantState, workerthread::WorkerThread, workervmmanager::Id}};

/// Mesophyll client, NOT THREAD SAFE
#[derive(Clone)]
pub struct MesophyllClient {
    wt: Arc<WorkerThread>,
    addr: String,
}

#[allow(dead_code)]
impl MesophyllClient {
    /// Creates a new Mesophyll client
    pub fn new(addr: String, token: String, wt: Arc<WorkerThread>) -> Self {
        let worker_id = wt.id();
        let s = Self {
            wt,
            addr: format!("{}/ws?id={}?token={}", addr, worker_id, token),
        };

        let self_ref = s.clone();
        tokio::task::spawn_local(async move {
            loop {
                if let Err(e) = self_ref.handle_task().await {
                    log::error!("Mesophyll client task error: {}", e);
                }
                
                log::debug!("Mesophyll client reconnecting in 5 seconds...");
                tokio::time::sleep(Duration::from_secs(5)).await;
                log::debug!("Mesophyll client reconnecting now");
            }
        });

        s
    }

    async fn handle_task(&self) -> Result<(), crate::Error> {
        // Connect to the masters IP/port
        let (ws_stream, _) = connect_async(&self.addr).await.map_err(|e| format!("Failed to connect: {:?}", e))?;
        let (mut stream_tx, mut stream_rx) = ws_stream.split();
        let mut hb_timer = interval(Duration::from_millis(MESOPHYLL_DEFAULT_HEARTBEAT_MS));
        let mut dispatches = FuturesUnordered::new();
        let mut script_runs = FuturesUnordered::new();
        loop {
            tokio::select! {
                Some(Ok(msg)) = stream_rx.next() => {
                    if let Message::Close(_) = msg {
                        log::info!("Mesophyll client connection closed by server");
                        return Ok(());
                    }

                    let Ok(server_msg) = decode_message::<ServerMessage>(&msg) else {
                        continue;
                    };

                    match server_msg {
                        ServerMessage::Hello { heartbeat_interval_ms } => {
                            log::info!("Mesophyll client received Hello, heartbeat interval: {} ms", heartbeat_interval_ms);
                            hb_timer = interval(Duration::from_millis(heartbeat_interval_ms));
                        }
                        ServerMessage::DispatchEvent { id, event, req_id } => {
                            let fut = self.wt.dispatch_event(id, event);
                            dispatches.push(async move {
                                let resp = fut.await;
                                (req_id, resp)
                            });
                        },
                        ServerMessage::RunScript { id, name, code, event, req_id } => {
                            let fut = self.wt.run_script(id, name, code, event);
                            script_runs.push(async move {
                                let resp = fut.await;
                                (req_id, resp)
                            });
                        },
                        ServerMessage::DropWorker { id, req_id } => {
                            log::info!("Mesophyll client received DropWorker for ID {:?}", id);
                            let resp = self.wt.drop_tenant(id).await;
                            let response = encode_message(&ClientMessage::DispatchResponse {
                                req_id,
                                result: match resp {
                                    Ok(_) => Ok(KhronosValue::Null),
                                    Err(e) => Err(e.to_string()),
                                },
                            })?;
                            stream_tx.send(response).await
                                .map_err(|e| format!("Failed to send DropWorker response: {}", e))?;
                        }
                    }
                }
                Some((req_id, result)) = dispatches.next() => {
                    let Some(req_id) = req_id else {
                        continue;
                    };
                    let response = encode_message(&ClientMessage::DispatchResponse {
                        req_id,
                        result: result.map_err(|e| e.to_string()),
                    })?;
                    stream_tx.send(response).await
                        .map_err(|e| format!("Failed to send DispatchResponse: {}", e))?;
                }
                Some((req_id, result)) = script_runs.next() => {
                    let response = encode_message(&ClientMessage::DispatchResponse {
                        req_id,
                        result: result.map_err(|e| e.to_string()),
                    })?;
                    stream_tx.send(response).await
                        .map_err(|e| format!("Failed to send DispatchResponse for RunScript: {}", e))?;
                }
                _ = hb_timer.tick() => {
                    let heartbeat = encode_message(&ClientMessage::Heartbeat {})?;
                    stream_tx.send(heartbeat).await
                        .map_err(|e| format!("Failed to send Heartbeat: {}", e))?;
                }
            }
        }
    }
}

#[allow(dead_code)]
pub struct MesophyllDbClient {
    addr: String,
    worker_id: usize,
    token: String,
    client: reqwest::Client,
}

#[allow(dead_code)]
impl MesophyllDbClient {
    /// Creates a new MesophyllDbClient
    pub fn new(addr: String, worker_id: usize, token: String) -> Self {
        Self {
            addr,
            worker_id,
            token,
            client: reqwest::Client::builder()
            .http2_prior_knowledge()
            .build()
            .expect("Failed to create reqwest client"),
        }
    }

    fn url_for(&self, path: &str, id: Option<Id>) -> String {
        let mut base = format!("{}/db/{}?id={}&token={}", self.addr, self.worker_id, self.token, path);
        match id {
            Some(id) => {
                match id {
                    Id::GuildId(guild_id) => {
                        base.push_str(&format!("&tenant_id={}&tenant_type=guild", guild_id));
                    }
                }
            }
            None => {}
        }

        base
    }

    async fn decode_no_resp(resp: reqwest::Response) -> Result<(), crate::Error> {
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text()
                .await
                .unwrap_or_default();
            return Err(format!("request failed with status: {}: {}", status, text).into());
        }

        Ok(())
    }

    async fn decode_resp<T: for<'de> serde::Deserialize<'de>>(resp: reqwest::Response) -> Result<T, crate::Error> {
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("request failed with status: {}: {}", status, text).into());
        }

        let resp_bytes = resp.bytes()
            .await
            .map_err(|e| format!("Failed to parse list_tenant_states response: {}", e))?;

        rmp_serde::from_slice(&resp_bytes).map_err(|e| format!("Failed to decode response: {}", e).into())
    }

    fn encode_req<T: serde::Serialize>(&self, body: &T) -> Result<reqwest::Body, crate::Error> {
        let encoded = rmp_serde::to_vec(body)
            .map_err(|e| format!("Failed to encode request body: {}", e))?;
        Ok(reqwest::Body::from(encoded))
    }

    /// Returns a list of all tenant states from the Mesophyll server
    pub async fn list_tenant_states(&self) -> Result<HashMap<Id, TenantState>, crate::Error> {
        let url = self.url_for("tenant-states", None);
        let resp = self.client.get(&url)
            .send()
            .await
            .map_err(|e| format!("Failed to send list_tenant_states request: {}", e))?;

        Self::decode_resp(resp).await
    }

    /// Sets the tenant state for a given tenant ID
    pub async fn set_tenant_state_for(&self, id: Id, state: &TenantState) -> Result<(), crate::Error> {
        let url = self.url_for("tenant-state", Some(id));
        let body = self.encode_req(state)?;
        let resp = self.client.post(&url)
            .body(body)
            .send()
            .await
            .map_err(|e| format!("Failed to send set_tenant_state_for request: {}", e))?;

        Self::decode_no_resp(resp).await
    }

    pub async fn kv_get(&self, id: Id, scopes: Vec<String>, key: String) -> Result<Option<SerdeKvRecord>, crate::Error> {
        let url = self.url_for("kv", Some(id));
        let body = self.encode_req(&crate::mesophyll::message::KeyValueOp::Get { scopes, key })?;
        let resp = self.client.post(&url)
            .body(body)
            .send()
            .await
            .map_err(|e| format!("Failed to send kv_get request: {}", e))?;

        Self::decode_resp(resp).await
    }

    pub async fn kv_list_scopes(&self, id: Id) -> Result<Vec<String>, crate::Error> {
        let url = self.url_for("kv", Some(id));
        let body = self.encode_req(&crate::mesophyll::message::KeyValueOp::ListScopes {})?;
        let resp = self.client.post(&url)
            .body(body)
            .send()
            .await
            .map_err(|e| format!("Failed to send kv_list_scopes request: {}", e))?;

        Self::decode_resp(resp).await
    }

    pub async fn kv_set(&self, id: Id, scopes: Vec<String>, key: String, value: KhronosValue) -> Result<(), crate::Error> {
        let url = self.url_for("kv", Some(id));
        let body = self.encode_req(&crate::mesophyll::message::KeyValueOp::Set { scopes, key, value })?;
        let resp = self.client.post(&url)
            .body(body)
            .send()
            .await
            .map_err(|e| format!("Failed to send kv_set request: {}", e))?;

        Self::decode_no_resp(resp).await
    }

    pub async fn kv_delete(&self, id: Id, scopes: Vec<String>, key: String) -> Result<(), crate::Error> {
        let url = self.url_for("kv", Some(id));
        let body = self.encode_req(&crate::mesophyll::message::KeyValueOp::Delete { scopes, key })?;
        let resp = self.client.post(&url)
            .body(body)
            .send()
            .await
            .map_err(|e| format!("Failed to send kv_delete request: {}", e))?;

        Self::decode_no_resp(resp).await
    }

    pub async fn kv_find(&self, id: Id, scopes: Vec<String>, prefix: String) -> Result<Vec<SerdeKvRecord>, crate::Error> {
        let url = self.url_for("kv", Some(id));
        let body = self.encode_req(&crate::mesophyll::message::KeyValueOp::Find { scopes, prefix })?;
        let resp = self.client.post(&url)
            .body(body)
            .send()
            .await
            .map_err(|e| format!("Failed to send kv_find request: {}", e))?;

        Self::decode_resp(resp).await
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