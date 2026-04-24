use chrono::{DateTime, Utc};
use khronos_runtime::{primitives::opaque::Opaque, rt::mlua::prelude::*};
use khronos_runtime::utils::khronos_value::KhronosValue;
use khronos_runtime::core::datetime::DateTime as LuaDateTime;
use rand::distr::{Alphanumeric, SampleString};

use crate::geese::tenantstate::{DEFAULT_EVENTS, TenantState, TenantStateDb};
use crate::worker::limits::KV_MAX_KEY_LENGTH;
use crate::worker::workervmmanager::Id;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "op")]
pub enum StateOp {
    KvFind {
        query: String,
        scope: String
    },
    KvGet {
        key: String,
        scope: String
    },
    KvSet {
        key: String,
        scope: String,
        value: KhronosValue
    },
    KvDelete {
        key: String,
        scope: String
    },
    GlobalKvFind {
        query: String,
        scope: String
    },
    GlobalKvGet {
        key: String,
        version: i32,
        scope: String
    },
    GlobalKvCreate {
        key: String,
        version: i32,
        short: String, // short description for the key-value.
        public_metadata: KhronosValue, // public metadata about the key-value
        scope: String,
        public_data: bool,
        long: Option<String>, // long description for the key-value.
        data: KhronosValue, // the actual value of the key-value, may be private
    },
    GlobalKvDelete {
        key: String,
        version: i32,
        scope: String
    },
    GlobalKvGetData {
        key: String,
        version: i32,
        scope: String
    },
    SubscribeEvent {
        event: String,
        system: String,
    },
    UnsubscribeEvent {
        event: String,
        system: String,
    }
}

impl StateOp {
    /// Returns true if the operation may alter the tenant state
    fn alters_tenant_state(&self) -> bool {
        matches!(self, Self::SubscribeEvent { .. } | Self::UnsubscribeEvent { .. })
    }
}

impl FromLua for StateOp {
    fn from_lua(value: LuaValue, _lua: &Lua) -> LuaResult<Self> {
        let LuaValue::Table(tab) = value else {
            return Err(LuaError::FromLuaConversionError {
                from: value.type_name(),
                to: "LuauStateOp".to_string(),
                message: Some("expected a table".to_string()),
            })
        };

        let typ: LuaString = tab.get("op")?;
        match typ.as_bytes().as_ref() {
            b"KvFind" => {
                let query = tab.get("query")?;
                let scope = tab.get("scope")?;
                Ok(Self::KvFind { query, scope })
            },
            b"KvGet" => {
                let key = tab.get("key")?;
                let scope = tab.get("scope")?;
                Ok(Self::KvGet { key, scope })
            },
            b"KvSet" => {
                let key = tab.get("key")?;
                let scope = tab.get("scope")?;
                let value = tab.get("value")?;
                Ok(Self::KvSet { key, scope, value })
            },
            b"KvDelete" => {
                let key = tab.get("key")?;
                let scope = tab.get("scope")?;
                Ok(Self::KvDelete { key, scope })
            },
            b"SubscribeEvent" => {
                let event = tab.get("event")?;
                let system = tab.get("system")?;
                Ok(Self::SubscribeEvent { event, system })
            },
            b"UnsubscribeEvent" => {
                let event = tab.get("event")?;
                let system = tab.get("system")?;
                Ok(Self::UnsubscribeEvent { event, system })
            },
            b"GlobalKvFind" => {
                let query = tab.get("query")?;
                let scope = tab.get("scope")?;
                Ok(Self::GlobalKvFind { query, scope })
            },
            b"GlobalKvGet" => {
                let key = tab.get("key")?;
                let version = tab.get("version")?;
                let scope = tab.get("scope")?;
                Ok(Self::GlobalKvGet { key, version, scope })
            },
            b"GlobalKvCreate" => {
                let key = tab.get("key")?;
                let version = tab.get("version")?;
                let short = tab.get("short")?;
                let public_metadata = tab.get("public_metadata")?;
                let scope = tab.get("scope")?;
                let public_data = tab.get("public_data")?;
                let long = tab.get("long").ok();
                let data = tab.get("data")?;
                Ok(Self::GlobalKvCreate { key, version, short, public_metadata, scope, public_data, long, data })
            },
            b"GlobalKvDelete" => {
                let key = tab.get("key")?;
                let version = tab.get("version")?;
                let scope = tab.get("scope")?;
                Ok(Self::GlobalKvDelete { key, version, scope })
            },
            b"GlobalKvGetData" => {
                let key = tab.get("key")?;
                let version = tab.get("version")?;
                let scope = tab.get("scope")?;
                Ok(Self::GlobalKvGetData { key, version, scope })
            },
            _ => {
                Err(LuaError::FromLuaConversionError {
                    from: "table",
                    to: "LuauStateOp".to_string(),
                    message: Some("invalid op provided".to_string()),
                })
            }
        }
    }
}

#[derive(Clone)]
/// A simple wrapper around the database pool that provides luau state manipulation functionality
pub struct StateDb {
    pool: sqlx::PgPool,
    tsdb: TenantStateDb
}

impl StateDb {
    pub fn new(pool: sqlx::PgPool) -> Self {
        StateDb { pool: pool.clone(), tsdb: TenantStateDb::new(pool) }
    }

    /// Perform execution of an op
    pub async fn do_op(&self, tid: Id, op: Vec<StateOp>) -> Result<StateExecResponse, crate::Error> {
        let mut result = StateExecResponse { results: vec![], tenant_state_changed: false, new_tenant_state: None };
        // fast path of no explicit transaction can only be applied if none of the inner ops alter the tenant state
        let fastpath = op.len() <= 1 && op.iter().all(|x| !x.alters_tenant_state());

        if fastpath {
            for op in op {
                Self::apply_op(&self.pool, tid, op, &mut result).await?
            }
            if result.tenant_state_changed {
                return Err("internal error: tenant state changed in unsupported fastpath".into());
            }
        } else {
            // atomic
            let mut tx = self.pool.begin().await?;
            for op in op {
                Self::apply_op(&mut *tx, tid, op, &mut result).await?
            }

            if result.tenant_state_changed {
                result.new_tenant_state = self.tsdb.get_tenant_state_for(&mut tx, tid).await?;
            }

            tx.commit().await?;
        }

        return Ok(result)
    }

    async fn apply_op<'c, E>(
        executor: E, 
        tid: Id, 
        op: StateOp,
        state: &mut StateExecResponse
    ) -> Result<(), crate::Error> 
    where 
        E: sqlx::Executor<'c, Database = sqlx::Postgres>, 
    {
        match op {
            StateOp::KvFind { query, scope } => {
                let rec: Vec<KvLookup> = {
                    if query == "%%" {
                        // Fast path, omit ILIKE if '%%' is used
                        sqlx::query_as(
                        "SELECT key, value, scope, created_at, last_updated_at FROM tenant_kv WHERE owner_id = $1 AND owner_type = $2 AND scope = $3",
                        )
                        .bind(tid.tenant_id())
                        .bind(tid.tenant_type())
                        .bind(scope)
                        .fetch_all(executor)
                        .await?
                    } else {
                        // with query
                        sqlx::query_as(
                        "SELECT key, value, scope, created_at, last_updated_at FROM tenant_kv WHERE owner_id = $1 AND owner_type = $2 AND scope = $3 AND key LIKE $4",
                        )
                        .bind(tid.tenant_id())
                        .bind(tid.tenant_type())
                        .bind(scope)
                        .bind(query)
                        .fetch_all(executor)
                        .await?
                    }
                };

                KvLookup::apply(state, rec);
            }
            StateOp::KvGet { key, scope } => {
                if let Some(rec) = sqlx::query_as(
                    "SELECT key, value, scope, created_at, last_updated_at FROM tenant_kv WHERE owner_id = $1 AND owner_type = $2 AND key = $3 AND scope = $4",
                    )
                    .bind(tid.tenant_id())
                    .bind(tid.tenant_type())
                    .bind(&key)
                    .bind(scope)
                    .fetch_optional(executor)
                    .await? {
                        KvLookup::apply_one(state, rec);
                    }
            }
            StateOp::KvSet { key, scope, value } => {
                if key.len() > KV_MAX_KEY_LENGTH {
                    return Err(format!("key-value length exceeds {KV_MAX_KEY_LENGTH} chars").into())
                }
                let id = Alphanumeric.sample_string(&mut rand::rng(), 64);
                sqlx::query(
                    "INSERT INTO tenant_kv (id, owner_id, owner_type, key, value, scope) VALUES ($1, $2, $3, $4, $5, $6)
                    ON CONFLICT (owner_id, owner_type, key, scope) DO UPDATE SET value = EXCLUDED.value, last_updated_at = NOW()",
                )
                .bind(&id)
                .bind(tid.tenant_id())
                .bind(tid.tenant_type())
                .bind(key)
                .bind(serde_json::to_value(value)?)
                .bind(scope)
                .execute(executor)
                .await?;
            }
            StateOp::KvDelete { key, scope } => {
                sqlx::query(
                "DELETE FROM tenant_kv WHERE owner_id = $1 AND owner_type = $2 AND key = $3 AND scope = $4",
                )
                .bind(tid.tenant_id())
                .bind(tid.tenant_type())
                .bind(key)
                .bind(scope)
                .execute(executor)
                .await?;
            }
            StateOp::SubscribeEvent { event, system } => {
                if DEFAULT_EVENTS.contains(&event.as_str()) {
                    return Err("Cannot subscribe to default event".into())
                }

                let res = sqlx::query(
                    r#"
                    WITH ensure_tenant AS (
                        INSERT INTO tenant_state (owner_id, owner_type) 
                        VALUES ($1, $2) 
                        ON CONFLICT (owner_id, owner_type) DO NOTHING
                    )
                    INSERT INTO tenant_state_events (owner_id, owner_type, event, system)
                    VALUES ($1, $2, $3, $4)
                    ON CONFLICT (owner_id, owner_type, event, system) DO NOTHING
                    "#
                )
                .bind(tid.tenant_id())
                .bind(tid.tenant_type())
                .bind(&event)
                .bind(&system)
                .execute(executor)
                .await?;

                if res.rows_affected() > 0 {
                    state.tenant_state_changed = true;
                }
                
                //state.new_tenant_state = Some((events, flags));
            }
            StateOp::UnsubscribeEvent { event, system } => {
                if DEFAULT_EVENTS.contains(&event.as_str()) {
                    return Err("Cannot subscribe to default event".into())
                }

                let res = sqlx::query(
                    r#"
                    DELETE FROM tenant_state_events 
                    WHERE owner_id = $1 AND owner_type = $2 AND event = $3 AND system = $4
                    "#
                )
                .bind(tid.tenant_id())
                .bind(tid.tenant_type())
                .bind(&event)
                .bind(&system)
                .execute(executor)
                .await?;

                if res.rows_affected() > 0 {
                    state.tenant_state_changed = true;
                }
            }
            StateOp::GlobalKvFind { query, scope } => {
                let items: Vec<GlobalKv> = if query == "%%" {
                    sqlx::query_as(
                        "SELECT key, version, owner_id, owner_type, short, public_metadata, public_data, scope, created_at, last_updated_at, price, review_state FROM global_kv WHERE scope = $1 AND review_state = 'approved'"
                    )
                    .bind(scope)
                    .fetch_all(executor)
                    .await?
                } else {
                    sqlx::query_as(
                        "SELECT key, version, owner_id, owner_type, short, public_metadata, public_data, scope, created_at, last_updated_at, price, review_state FROM global_kv WHERE scope = $1 AND review_state = 'approved' AND key LIKE $2"
                    )
                    .bind(scope)
                    .bind(query)
                    .fetch_all(executor)
                    .await?
                };

                GlobalKv::apply(state, items);
            }
            StateOp::GlobalKvGet { key, version, scope } => {
                if let Some(rec) = sqlx::query_as(
                    "SELECT key, version, owner_id, owner_type, short, public_metadata, public_data, scope, created_at, last_updated_at, price, review_state FROM global_kv WHERE key = $1 AND version = $2 AND scope = $3 AND review_state = 'approved'",
                    )
                    .bind(&key)
                    .bind(version)
                    .bind(scope)
                    .fetch_optional(executor)
                    .await? {
                        GlobalKv::apply_one(state, rec);
                    }
            }
            StateOp::GlobalKvCreate { key, version, short, public_metadata, scope, public_data, long, data } => {
                // Validate key
                //
                // Rules:
                // 1. Between 3 and 64 characters long
                // 2. May not start or end with a dot (.)
                // 3. May only contain (ASCII) alphanumeric characters, dots (.), dashes (-), and underscores (_)
                if key.len() < 3 || key.len() > 64 {
                    return Err("keys must be between 3 and 64 characters long".into());
                }
                if key.starts_with('.') || key.ends_with('.') {
                    return Err("keys may not start or end with a dot".into());
                }
                if !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_') {
                    return Err("keys may only contain alphanumeric characters, dots, dashes, and underscores".into());
                }
                
                let id = Alphanumeric.sample_string(&mut rand::rng(), 64);
                
                // try to insert, if it fails then a record with the same key, version, and scope already exists
                // so we can return an error
                let inserted = sqlx::query(
                    "INSERT INTO global_kv (id, key, version, owner_id, owner_type, short, public_metadata, public_data, scope, long, data) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11) ON CONFLICT (key, version, scope) DO NOTHING"
                )
                .bind(&id)
                .bind(key)
                .bind(version)
                .bind(tid.tenant_id())
                .bind(tid.tenant_type())
                .bind(short)
                .bind(serde_json::to_value(public_metadata)?)
                .bind(public_data)
                .bind(scope)
                .bind(long)
                .bind(serde_json::to_value(data)?)
                .execute(executor)
                .await?;

                if inserted.rows_affected() == 0 {
                    return Err("Global KV with the same key, version, and scope already exists".into());
                }
            }
            StateOp::GlobalKvDelete { key, version, scope } => {
                let res = sqlx::query(
        "DELETE FROM global_kv WHERE key = $1 AND version = $2 AND scope = $3 AND owner_id = $4 AND owner_type = $5",
                )
                .bind(key)
                .bind(version)
                .bind(scope)
                .bind(tid.tenant_id())
                .bind(tid.tenant_type())
                .execute(executor)
                .await?;

                if res.rows_affected() == 0 {
                    return Err("No matching Global KV found to delete or insufficient permissions".into());
                }
            }
            StateOp::GlobalKvGetData { key, version, scope } => {
                if let Some(rec) = sqlx::query_as(
                "SELECT data, public_data, price FROM global_kv WHERE key = $1 AND version = $2 AND scope = $3 AND review_state = 'approved'",
                )
                .bind(&key)
                .bind(version)
                .bind(scope)
                .fetch_optional(executor)
                .await? {
                    GlobalKvData::apply_one(state, rec);
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "op")]
pub enum StateExecResult {
    Kv {
        l: KvLookup
    },
    GlobalKv {
        l: GlobalKv
    },
    GlobalKvData {
        data: KhronosValue,
    },
    GlobalKvDataOpaque {
        data: KhronosValue
    }
}

pub trait IntoStateExecResult {
    fn into_result(self) -> StateExecResult;

    fn apply_one(state: &mut StateExecResponse, l: Self) where Self: Sized {
        state.results.push(l.into_result())
    }
    fn apply(state: &mut StateExecResponse, lookups: Vec<Self>) where Self: Sized {
        for l in lookups {
            Self::apply_one(state, l);
        }
    }
}

impl IntoLua for StateExecResult {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        let table = lua.create_table()?;
        match self {
            Self::Kv { l } => {
                table.set("op", "Kv")?;
                table.set("key", l.key)?;
                table.set("scope", l.scope)?;
                table.set("value", l.value)?;
                table.set("created_at", LuaDateTime::from_utc(l.created_at))?;
                table.set("last_updated_at", LuaDateTime::from_utc(l.last_updated_at))?;
            }
            Self::GlobalKv { l } => {
                table.set("op", "GlobalKv")?;
                table.set("key", l.key)?;
                table.set("version", l.version)?;
                table.set("owner_id", l.owner_id)?;
                table.set("owner_type", l.owner_type)?;
                table.set("price", l.price)?;
                table.set("short", l.short)?;
                table.set("public_metadata", l.public_metadata)?;
                table.set("scope", l.scope)?;
                table.set("created_at", LuaDateTime::from_utc(l.created_at))?;
                table.set("last_updated_at", LuaDateTime::from_utc(l.last_updated_at))?;
                table.set("public_data", l.public_data)?;
                table.set("review_state", l.review_state)?;
                table.set("long", l.long)?;
            }
            Self::GlobalKvData { data } => {
                table.set("op", "GlobalKvData")?;
                table.set("data", data)?;
            }
            Self::GlobalKvDataOpaque { data } => {
                table.set("op", "GlobalKvDataOpaque")?;
                table.set("data", Opaque::new(data))?;
            }
        }
        table.set_readonly(true); // We want StateExecResult's to be immutable
        Ok(LuaValue::Table(table))
    }
}

/// The response from a state execution
#[derive(serde::Serialize, serde::Deserialize)]
pub struct StateExecResponse {
    pub results: Vec<StateExecResult>,
    #[serde(skip)]
    tenant_state_changed: bool,
    pub new_tenant_state: Option<TenantState>
}

#[derive(Debug, sqlx::FromRow, serde::Serialize, serde::Deserialize)]
pub struct KvLookup {
    key: String,
    #[sqlx(json)]
    value: KhronosValue,
    scope: String,
    created_at: chrono::DateTime<chrono::Utc>,
    last_updated_at: chrono::DateTime<chrono::Utc>,
}

impl IntoStateExecResult for KvLookup {
    fn into_result(self) -> StateExecResult {
        StateExecResult::Kv { l: self }
    }
}

#[derive(Debug, sqlx::FromRow, serde::Serialize, serde::Deserialize)]
pub struct GlobalKv {
    pub key: String,
    pub version: i32,
    pub owner_id: String,
    pub owner_type: String,
    pub price: Option<i64>, // will only be set for shop items, otherwise None
    pub short: String, // short description for the key-value.
    #[sqlx(json)]
    pub public_metadata: KhronosValue, // public metadata about the key-value
    pub scope: String,
    pub created_at: DateTime<Utc>,
    pub last_updated_at: DateTime<Utc>,
    pub public_data: bool,
    pub review_state: String,

    #[sqlx(default)]
    pub long: Option<String>, // long description for the key-value.
}

impl IntoStateExecResult for GlobalKv {
    fn into_result(self) -> StateExecResult {
        StateExecResult::GlobalKv { l: self }
    }
}

#[derive(Debug, sqlx::FromRow)]
pub struct GlobalKvData {
    #[sqlx(json)]
    pub data: KhronosValue,
    pub public_data: bool,
    pub price: Option<i64>, // will only be set for shop items, otherwise None
}

impl IntoStateExecResult for GlobalKvData {
    fn into_result(self) -> StateExecResult {
        if self.public_data && self.price.is_none() { 
            StateExecResult::GlobalKvData { data: self.data } 
        } else { 
            StateExecResult::GlobalKvDataOpaque { data: self.data }
        }
    }
}
