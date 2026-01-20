use std::{collections::{HashMap, HashSet}, sync::{Arc, Weak}, time::Instant};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use rand::{distr::{Alphanumeric, SampleString}};
use dashmap::DashMap;
use futures::{SinkExt, StreamExt};
use khronos_runtime::{primitives::event::CreateEvent, traits::ir::KvRecord, traits::ir::globalkv as gkv_ir, utils::khronos_value::KhronosValue};
use tokio::{select, spawn, sync::{mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel}, oneshot::{Receiver, Sender, channel}}};
use tokio_util::sync::CancellationToken;
use ts_rs::TS;

use crate::{mesophyll::message::{ClientMessage, GlobalKeyValueOp, KeyValueOp, PublicGlobalKeyValueOp, ServerMessage}, worker::{workerstate::TenantState, workervmmanager::Id}};

use axum::{
    Router, body::Bytes, extract::{Query, State, WebSocketUpgrade, ws::{Message, WebSocket}}, http::StatusCode, response::{IntoResponse, Response}, routing::{get, post}
};
use sqlx::Row;

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct SerdeKvRecord {
    pub id: String,
    pub key: String,
    pub value: KhronosValue,
    pub scopes: Vec<String>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl Into<KvRecord> for SerdeKvRecord {
    fn into(self) -> KvRecord {
        KvRecord {
            id: self.id,
            key: self.key,
            value: self.value,
            scopes: self.scopes,
            created_at: self.created_at,
            last_updated_at: self.last_updated_at,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, TS, utoipa::ToSchema, sqlx::FromRow)]
#[ts(export)]
/// A global key-value entry that can be viewed by all guilds
/// 
/// Unlike normal key-values, these are not scoped to a specific guild or tenant,
/// are immutable (new versions must be created, updates not allowed) and have both
/// a public metadata and potentially private value. Only staff may create global kv's that
/// have a price attached to them.
/// 
/// These are primarily used for things like the template shop but may be used for other
/// things as well in the future beyond template shop as well such as global lists.
pub struct GlobalKv {
    pub key: String,
    pub version: i32,
    pub owner_id: String,
    pub owner_type: String,
    pub price: Option<i64>, // will only be set for shop items, otherwise None
    pub short: String, // short description for the key-value.
    pub public_metadata: serde_json::Value, // public metadata about the key-value
    pub scope: String,
    pub created_at: DateTime<Utc>,
    pub last_updated_at: DateTime<Utc>,
    pub public_data: bool,
    pub review_state: String,

    #[sqlx(default)]
    pub long: Option<String>, // long description for the key-value.
    #[sqlx(default)]
    pub data: serde_json::Value, // the actual value of the key-value, may be private
}

impl Into<gkv_ir::GlobalKv> for GlobalKv {
    fn into(self) -> gkv_ir::GlobalKv {
        gkv_ir::GlobalKv {
            key: self.key,
            version: self.version,
            owner_id: self.owner_id,
            owner_type: self.owner_type,
            price: self.price,
            short: self.short,
            public_metadata: self.public_metadata,
            scope: self.scope,
            created_at: self.created_at,
            last_updated_at: self.last_updated_at,
            public_data: self.public_data,
            review_state: self.review_state,
            long: self.long,
            data: self.data,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum AttachResult {
    PurchaseRequired { url: String },
    Ok(()),
}

impl Into<gkv_ir::AttachResult> for AttachResult {
    fn into(self) -> gkv_ir::AttachResult {
        match self {
            AttachResult::PurchaseRequired { url } => gkv_ir::AttachResult::PurchaseRequired { url },
            AttachResult::Ok(()) => gkv_ir::AttachResult::Ok(()),
        }
    }
}

/// NOTE: Global KV's created publicly cannot have a price associated to them for legal reasons.
/// Only staff may create priced global KV's.
/// NOTE 2: All Global KV's undergo staff review before being made available. When this occurs,
/// review state will be updated accordingly from 'pending' to 'approved' or otherwise if rejected.
#[derive(Debug, Serialize, Deserialize, TS, utoipa::ToSchema, sqlx::FromRow)]
#[ts(export)]
pub struct CreateGlobalKv {
    pub key: String,
    pub version: i32,
    pub short: String, // short description for the key-value.
    pub public_metadata: serde_json::Value, // public metadata about the key-value
    pub scope: String,
    pub public_data: bool,
    pub long: Option<String>, // long description for the key-value.
    pub data: serde_json::Value, // the actual value of the key-value, may be private
}

impl From<gkv_ir::CreateGlobalKv> for CreateGlobalKv {
    fn from(g: gkv_ir::CreateGlobalKv) -> Self {
        Self {
            key: g.key,
            version: g.version,
            short: g.short,
            public_metadata: g.public_metadata,
            scope: g.scope,
            public_data: g.public_data,
            long: g.long,
            data: g.data,
        }
    }
}

#[derive(Clone)]
pub struct DbState {
    pool: sqlx::PgPool,
    tenant_state_cache: Arc<RwLock<HashMap<Id, TenantState>>> // server side tenant state cache
}

impl DbState {
    pub async fn new(pool: sqlx::PgPool) -> Result<Self, crate::Error> {
        let mut s = Self {
            pool,
            tenant_state_cache: Arc::new(RwLock::new(HashMap::new())),
        };

        s.tenant_state_cache = Arc::new(RwLock::new(s.get_tenant_state().await?));

        Ok(s)
    }

    /// Returns the underlying SQLx Postgres pool
    pub fn get_pool(&self) -> &sqlx::PgPool {
        &self.pool
    }

    /// Returns the tenant state(s) for all guilds in the database as well as a set of guild IDs that have startup events enabled
    /// 
    /// Should only be called once, on startup, to initialize the tenant state cache
    async fn get_tenant_state(&self) -> Result<HashMap<Id, TenantState>, crate::Error> {
        #[derive(sqlx::FromRow)]
        struct TenantStatePartial {
            events: Vec<String>,
            data: serde_json::Value,
            owner_id: String,
            owner_type: String,
        }

        let partials: Vec<TenantStatePartial> =
            sqlx::query_as("SELECT owner_id, owner_type, events, data FROM tenant_state")
            .fetch_all(&self.pool)
            .await?;

        let mut states = HashMap::new();  
        for partial in partials {
            let Some(id) = Id::from_parts(&partial.owner_type, &partial.owner_id) else {
                continue;
            };
            let state = TenantState {
                events: HashSet::from_iter(partial.events),
                data: partial.data,
            };

            states.insert(id, state);
        }

        Ok(states)
    }

    /// Helper method to return all tenant states from the internal cache
    pub async fn list_tenant_states(&self) -> Result<HashMap<Id, TenantState>, crate::Error> {
        let cache = self.tenant_state_cache.read().await;
        Ok(cache.clone())
    }

    /// Sets the tenant state for a specific tenant and updates the internal cache
    pub async fn set_tenant_state_for(&self, id: Id, state: TenantState) -> Result<(), crate::Error> {
        let events = state.events.iter().collect::<Vec<_>>();
        sqlx::query(
            "INSERT INTO tenant_state (owner_id, owner_type, events, data) VALUES ($1, $2, $3, $4) ON CONFLICT (owner_id, owner_type) DO UPDATE SET events = EXCLUDED.events, data = EXCLUDED.data",
        )
        .bind(id.tenant_id())
        .bind(id.tenant_type())
        .bind(&events)
        .bind(&state.data)
        .execute(&self.pool)
        .await?;

        let mut cache = self.tenant_state_cache.write().await;
        cache.insert(id, state);

        Ok(())
    }

    /// Gets a key-value record for a given tenant ID, scopes, and key
    pub async fn kv_get(&self, tid: Id, mut scopes: Vec<String>, key: String) -> Result<Option<SerdeKvRecord>, crate::Error> {
        scopes.sort();
        
        // Shouldn't happen but scopes must be non-empty
        if scopes.is_empty() {
            return Err("Scopes cannot be empty".into());
        }

        let rec = sqlx::query(
            "SELECT id, scopes, value, created_at, last_updated_at FROM tenant_kv WHERE owner_id = $1 AND owner_type = $2 AND key = $3 AND scopes @> $4",
            )
            .bind(tid.tenant_id())
            .bind(tid.tenant_type())
            .bind(&key)
            .bind(scopes)
            .fetch_optional(&self.pool)
            .await?;

        let Some(rec) = rec else {
            return Ok(None);
        };

        Ok(Some(SerdeKvRecord {
            id: rec.try_get::<String, _>("id")?,
            key,
            scopes: rec.try_get::<Vec<String>, _>("scopes")?,
            value: {
                let value = rec
                    .try_get::<Option<serde_json::Value>, _>("value")?
                    .unwrap_or(serde_json::Value::Null);

                serde_json::from_value(value)
                    .map_err(|e| format!("Failed to deserialize value: {}", e))?
            },
            created_at: Some(rec.try_get("created_at")?),
            last_updated_at: Some(rec.try_get("last_updated_at")?),
        }))
    }

    pub async fn kv_list_scopes(&self, id: Id) -> Result<Vec<String>, crate::Error> {
        let rec = sqlx::query(
            "SELECT DISTINCT unnest_scope AS scope
FROM tenant_kv, unnest(scopes) AS unnest_scope
WHERE owner_id = $1
AND owner_type = $2
ORDER BY scope",
        )
        .bind(id.tenant_id())
        .bind(id.tenant_type())
        .fetch_all(&self.pool)
        .await?;

        let mut scopes = vec![];

        for rec in rec {
            scopes.push(rec.try_get("scope")?);
        }

        Ok(scopes)
    }

    pub async fn kv_set(
        &self,
        tid: Id,
        mut scopes: Vec<String>,
        key: String,
        data: KhronosValue,
    ) -> Result<(), crate::Error> {
        scopes.sort();

        // Shouldn't happen but scopes must be non-empty
        if scopes.is_empty() {
            return Err("Scopes cannot be empty".into());
        }

        let id = gen_random(64);
        sqlx::query(
            "INSERT INTO tenant_kv (id, owner_id, owner_type, key, value, scopes) VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (owner_id, owner_type, key, scopes) DO UPDATE value = EXCLUDED.value, last_updated = NOW()",
        )
        .bind(&id)
        .bind(tid.tenant_id())
        .bind(tid.tenant_type())
        .bind(key)
        .bind(serde_json::to_value(data)?)
        .bind(scopes)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn kv_delete(
        &self,
        tid: Id,
        mut scopes: Vec<String>,
        key: String,
    ) -> Result<(), crate::Error> {
        scopes.sort();

        // Shouldn't happen but scopes must be non-empty
        if scopes.is_empty() {
            return Err("Scopes cannot be empty".into());
        }

        sqlx::query(
        "DELETE FROM tenant_kv WHERE owner_id = $1 AND owner_type = $2 AND key = $3 AND scopes @> $4",
        )
        .bind(tid.tenant_id())
        .bind(tid.tenant_type())
        .bind(key)
        .bind(scopes)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn kv_find(
        &self,
        tid: Id,
        mut scopes: Vec<String>,
        query: String,
    ) -> Result<Vec<SerdeKvRecord>, crate::Error> {
        scopes.sort();

        // Shouldn't happen but scopes must be non-empty
        if scopes.is_empty() {
            return Err("Scopes cannot be empty".into());
        }

        let rec = {
            if query == "%%" {
                // Fast path, omit ILIKE if '%%' is used
                sqlx::query(
                "SELECT id, key, value, created_at, last_updated_at, scopes, resume FROM tenant_kv WHERE owner_id = $1 AND owner_type = $2 AND scopes @> $3",
                )
                .bind(tid.tenant_id())
                .bind(tid.tenant_type())
                .bind(scopes)
                .fetch_all(&self.pool)
                .await?
            } else {
                // with query
                sqlx::query(
                "SELECT id, key, value, created_at, last_updated_at, scopes, resume FROM tenant_kv WHERE owner_id = $1 AND owner_type = $2 AND scopes @> $3 AND key ILIKE $4",
                )
                .bind(tid.tenant_id())
                .bind(tid.tenant_type())
                .bind(scopes)
                .bind(query)
                .fetch_all(&self.pool)
                .await?
            }
        };

        let mut records = vec![];

        for rec in rec {
            let record = SerdeKvRecord {
                id: rec.try_get::<String, _>("id")?,
                scopes: rec.try_get::<Vec<String>, _>("scopes")?,
                key: rec.try_get("key")?,
                value: {
                    let rec = rec
                        .try_get::<Option<serde_json::Value>, _>("value")?
                        .unwrap_or(serde_json::Value::Null);

                    serde_json::from_value(rec)
                        .map_err(|e| format!("Failed to deserialize value: {}", e))?
                },
                created_at: Some(rec.try_get("created_at")?),
                last_updated_at: Some(rec.try_get("last_updated_at")?),
            };

            records.push(record);
        }

        Ok(records)
    }

    pub async fn global_kv_find(&self, scope: String, query: String) -> Result<Vec<GlobalKv>, crate::Error> {
        let items: Vec<GlobalKv> = if query == "%%" {
            sqlx::query_as(
                "SELECT key, version, owner_id, owner_type, short, public_metadata, public_data, scope, created_at, last_updated_at, price, review_state FROM global_kv WHERE scope = $1 AND review_state = 'approved'"
            )
            .bind(scope)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as(
                "SELECT key, version, owner_id, owner_type, short, public_metadata, public_data, scope, created_at, last_updated_at, price, review_state FROM global_kv WHERE scope = $1 AND review_state = 'approved' AND key ILIKE $2"
            )
            .bind(scope)
            .bind(query)
            .fetch_all(&self.pool)
            .await?
        };

        Ok(items)
    }
    
    pub async fn global_kv_get(&self, key: String, version: i32, scope: String) -> Result<Option<GlobalKv>, crate::Error> {
        let item: Option<GlobalKv> = sqlx::query_as(
            "SELECT key, version, owner_id, owner_type, short, long, public_metadata, public_data, scope, created_at, last_updated_at, price, review_state FROM global_kv WHERE review_state = 'approved' AND key = $1 AND version = $2 AND scope = $3",
        )
        .bind(&key)
        .bind(version)
        .bind(scope)
        .fetch_optional(&self.pool)
        .await?;

        Ok(item)
    }

    pub async fn global_kv_create(&self, id: Id, gkv: CreateGlobalKv) -> Result<(), crate::Error> {
        let inserted = sqlx::query(
            "INSERT INTO global_kv (key, version, owner_id, owner_type, short, long, public_metadata, public_data, scope, data) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) ON CONFLICT (key, version, scope) DO NOTHING",
        )
        .bind(&gkv.key)
        .bind(gkv.version)
        .bind(id.tenant_id())
        .bind(id.tenant_type())
        .bind(&gkv.short)
        .bind(&gkv.long)
        .bind(&gkv.public_metadata)
        .bind(gkv.public_data)
        .bind(&gkv.scope)
        .bind(&gkv.data)
        .execute(&self.pool)
        .await?;

        if inserted.rows_affected() == 0 {
            return Err("Global KV with the same key, version, and scope already exists".into());
        }

        Ok(())
    }

    pub async fn global_kv_delete(&self, id: Id, key: String, version: i32, scope: String) -> Result<(), crate::Error> {
        let res = sqlx::query(
        "DELETE FROM global_kv WHERE key = $1 AND version = $2 AND scope = $3 AND owner_id = $4 AND owner_type = $5",
        )
        .bind(key)
        .bind(version)
        .bind(scope)
        .bind(id.tenant_id())
        .bind(id.tenant_type())
        .execute(&self.pool)
        .await?;

        if res.rows_affected() == 0 {
            return Err("No matching Global KV found to delete or insufficient permissions".into());
        }

        Ok(())
    }

    pub async fn global_kv_attach(&self, id: Id, key: String, version: i32, scope: String) -> Result<AttachResult, crate::Error> {
        let mut tx = self.pool.begin().await?;

        // First, check if the attachment already exists
        #[derive(sqlx::FromRow)]
        struct AttachmentCheck {
            count: i64,
        }

        let atc: AttachmentCheck = sqlx::query_as(
            "SELECT COUNT(*) AS count FROM global_kv_attachments WHERE owner_id = $1 AND owner_type = $2 AND key = $3 AND version = $4 AND scope = $5",
        )
        .bind(id.tenant_id())
        .bind(id.tenant_type())
        .bind(&key)
        .bind(version)
        .bind(&scope)
        .fetch_one(&mut *tx)
        .await?;
        
        if atc.count > 0 {
            return Err("Global KV attachment already exists".into());
        }

        // Then check if this global KV has a price associated to it
        // required for attachment
        #[derive(sqlx::FromRow)]
        struct MdCheck {
            price: Option<i64>,
        }

        let data: Option<MdCheck> = sqlx::query_as(
            "SELECT price FROM global_kv WHERE key = $1 AND version = $2 AND scope = $3",
        )
        .bind(&key)
        .bind(version)
        .bind(&scope)
        .fetch_optional(&mut *tx)
        .await?;

        let Some(data) = data else {
            return Err("Global KV entry does not exist".into());
        };
        
        if data.price.is_some() {
            // TODO: Get a proper URL here
            return Ok(AttachResult::PurchaseRequired { url: "todo url".to_string() })
        }

        let inserted = sqlx::query(
            "INSERT INTO global_kv_attachments (owner_id, owner_type, key, version, scope) VALUES ($1, $2, $3, $4, $5) ON CONFLICT (owner_id, owner_type, key, version, scope) DO NOTHING",
        )
        .bind(id.tenant_id())
        .bind(id.tenant_type())
        .bind(&key)
        .bind(version)
        .bind(&scope)
        .execute(&mut *tx)
        .await?;

        if inserted.rows_affected() == 0 {
            return Err("Global KV attachment already exists".into());
        }

        tx.commit().await?;

        Ok(AttachResult::Ok(()))
    }
}

fn gen_random(length: usize) -> String {
    Alphanumeric.sample_string(&mut rand::rng(), length)
}

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
            let ident = gen_random(Self::TOKEN_LENGTH);
            idents.insert(i, ident);
        }
        Self::new_with(addr, idents, pool).await
    }

    pub async fn new_with(addr: String, idents: HashMap<usize, String>, pool: sqlx::PgPool) -> Result<Self, crate::Error> {
        let s = Self {
            idents: Arc::new(idents),
            conns: Arc::new(DashMap::new()),
            db_state: DbState::new(pool).await?,
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

        let listener = tokio::net::TcpListener::bind(addr).await?;
        
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

/// DB API to fetch all tenant states
async fn list_tenant_states(
    Query(worker_query): Query<WorkerQuery>,
    State(state): State<MesophyllServer>,
) -> impl IntoResponse {
    if let Some(resp) = worker_query.validate(&state) {
        return resp;
    }

    let cache = state.db_state.tenant_state_cache.read().await;
    encode_db_resp(&*cache)
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
        PublicGlobalKeyValueOp::Get { key, version, scope } => {
            match state.db_state.global_kv_get(key, version, scope).await {
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
        GlobalKeyValueOp::Attach { key, version, scope } => {
            match state.db_state.global_kv_attach(id, key, version, scope).await {
                Ok(result) => encode_db_resp(&result),
                Err(e) => {
                    log::error!("Failed to attach global KV record: {}", e);
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
    let worker_id = worker_query.id;
    if let Some(resp) = worker_query.validate(&state) {
        return resp;
    }
    
    ws.on_upgrade(move |socket| handle_socket(socket, worker_id, state))
}

/// Handles a new Mesophyll server WebSocket connection
async fn handle_socket(socket: WebSocket, id: usize, state: MesophyllServer) {
    if state.conns.contains_key(&id) {
        log::warn!("Worker {id} reconnection - overwriting old connection.");
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