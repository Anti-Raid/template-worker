use chrono::{DateTime, Utc};
use khronos_runtime::{primitives::opaque::Opaque, rt::mlua::prelude::*};
use khronos_runtime::utils::khronos_value::KhronosValue;
use khronos_runtime::core::datetime::DateTime as LuaDateTime;
use rand::distr::{Alphanumeric, SampleString};

use crate::worker::workervmmanager::Id;

#[derive(serde::Serialize, serde::Deserialize)]
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
    UpdateTenantState {
        events: Vec<String>,
        flags: i32
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

        let typ: String = tab.get("op")?;
        match typ.as_str() {
            "KvFind" => {
                let query = tab.get("query")?;
                let scope = tab.get("scope")?;
                Ok(Self::KvFind { query, scope })
            },
            "KvGet" => {
                let key = tab.get("key")?;
                let scope = tab.get("scope")?;
                Ok(Self::KvGet { key, scope })
            },
            "KvSet" => {
                let key = tab.get("key")?;
                let scope = tab.get("scope")?;
                let value = tab.get("value")?;
                Ok(Self::KvSet { key, scope, value })
            },
            "KvDelete" => {
                let key = tab.get("key")?;
                let scope = tab.get("scope")?;
                Ok(Self::KvDelete { key, scope })
            },
            "UpdateTenantState" => {
                let events = tab.get("events")?;
                let flags = tab.get("flag")?;
                Ok(Self::UpdateTenantState { events, flags })
            },
            "GlobalKvFind" => {
                let query = tab.get("query")?;
                let scope = tab.get("scope")?;
                Ok(Self::GlobalKvFind { query, scope })
            },
            "GlobalKvGet" => {
                let key = tab.get("key")?;
                let version = tab.get("version")?;
                let scope = tab.get("scope")?;
                Ok(Self::GlobalKvGet { key, version, scope })
            },
            "GlobalKvCreate" => {
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
            "GlobalKvDelete" => {
                let key = tab.get("key")?;
                let version = tab.get("version")?;
                let scope = tab.get("scope")?;
                Ok(Self::GlobalKvDelete { key, version, scope })
            },
            "GlobalKvGetData" => {
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
}

impl StateDb {
    pub fn new(pool: sqlx::PgPool) -> Self {
        StateDb { pool }
    }

    /// Perform execution of an op
    pub async fn do_op(&self, tid: Id, op: Vec<StateOp>) -> Result<StateExecResponse, crate::Error> {
        let mut result = StateExecResponse { results: vec![], new_tenant_state: None };
        match op.len() {
            0 => {},
            1 => {
                for op in op {
                    Self::apply_op(&self.pool, tid, op, &mut result).await?
                }
            },
            _ => {
                // atomic
                let mut tx = self.pool.begin().await?;
                for op in op {
                    Self::apply_op(&mut *tx, tid, op, &mut result).await?
                }
                tx.commit().await?;

            }
        };

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
            StateOp::UpdateTenantState { events, flags } => {
                sqlx::query(
                    "INSERT INTO tenant_state (owner_id, owner_type, events, flags) VALUES ($1, $2, $3, $4) ON CONFLICT (owner_id, owner_type) DO UPDATE SET events = EXCLUDED.events, flags = EXCLUDED.flags",
                )
                .bind(tid.tenant_id())
                .bind(tid.tenant_type())
                .bind(&events)
                .bind(&flags)
                .execute(executor)
                .await?;

                state.new_tenant_state = Some((events, flags));
            }
            StateOp::GlobalKvFind { query, scope } => {
                let items: Vec<PartialGlobalKv> = if query == "%%" {
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

                PartialGlobalKv::apply(state, items);
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
                        PartialGlobalKv::apply_one(state, rec);
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
                "SELECT data, key, scope, public_data, price, created_at, last_updated_at FROM global_kv WHERE key = $1 AND version = $2 AND scope = $3 AND review_state = 'approved'",
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

/// A single record from performing a state execution using the low-level state-exec API
#[derive(serde::Serialize, serde::Deserialize)]
pub struct StateExecResult {
    pub key: String,
    pub scope: String,
    pub value: KhronosValue,
    pub opaque: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_updated_at: chrono::DateTime<chrono::Utc>,
}

impl IntoLua for StateExecResult {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        let table = lua.create_table()?;
        table.set("key", self.key)?;
        table.set("scope", self.scope)?;
        if self.opaque {
            table.set("value", Opaque::new(self.value))?;
        } else {
            table.set("value", self.value)?;
        }
        table.set("created_at", LuaDateTime::from_utc(self.created_at))?;
        table.set("last_updated_at", LuaDateTime::from_utc(self.last_updated_at))?;
        table.set_readonly(true); // We want StateExecResult's to be immutable
        Ok(LuaValue::Table(table))
    }
}

/// The response from a state execution
#[derive(serde::Serialize, serde::Deserialize)]
pub struct StateExecResponse {
    pub results: Vec<StateExecResult>,
    pub new_tenant_state: Option<(Vec<String>, i32)>
}

#[derive(sqlx::FromRow)]
struct KvLookup {
    key: String,
    #[sqlx(json)]
    value: KhronosValue,
    scope: String,
    created_at: chrono::DateTime<chrono::Utc>,
    last_updated_at: chrono::DateTime<chrono::Utc>,
}

impl KvLookup {
    fn apply_one(state: &mut StateExecResponse, l: KvLookup) {
        state.results.push(StateExecResult { key: l.key, value: l.value, scope: l.scope, created_at: l.created_at, last_updated_at: l.last_updated_at, opaque: false })
    }
    fn apply(state: &mut StateExecResponse, lookups: Vec<KvLookup>) {
        for l in lookups {
            Self::apply_one(state, l);
        }
    }
}

#[derive(Debug, sqlx::FromRow)]
pub struct PartialGlobalKv {
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

impl PartialGlobalKv { 
    fn apply_one(state: &mut StateExecResponse, l: PartialGlobalKv) {
        state.results.push(StateExecResult {
            key: l.key,
            scope: l.scope,
            value: KhronosValue::Map(vec![
                (KhronosValue::Text("version".to_string()), KhronosValue::Integer(l.version as i64)),
                (KhronosValue::Text("owner_id".to_string()), KhronosValue::Text(l.owner_id)),
                (KhronosValue::Text("owner_type".to_string()), KhronosValue::Text(l.owner_type)),
                (KhronosValue::Text("short".to_string()), KhronosValue::Text(l.short)),
                (KhronosValue::Text("public_metadata".to_string()), l.public_metadata),
                (KhronosValue::Text("public_data".to_string()), KhronosValue::Boolean(l.public_data)),
                (KhronosValue::Text("review_state".to_string()), KhronosValue::Text(l.review_state)),
                (KhronosValue::Text("long".to_string()), l.long.map_or(KhronosValue::Null, |s| KhronosValue::Text(s))),
                (KhronosValue::Text("price".to_string()), l.price.map_or(KhronosValue::Null, |p| KhronosValue::Integer(p))),
            ]),
            created_at: l.created_at,
            last_updated_at: l.last_updated_at,
            opaque: false
        })
    }

    fn apply(state: &mut StateExecResponse, lookups: Vec<PartialGlobalKv>) {
        for l in lookups {
            Self::apply_one(state, l);
        }
    }
}  

#[derive(Debug, sqlx::FromRow)]
pub struct GlobalKvData {
    #[sqlx(json)]
    pub data: KhronosValue,
    pub key: String,
    pub scope: String,
    pub public_data: bool,
    pub price: Option<i64>, // will only be set for shop items, otherwise None
    pub created_at: DateTime<Utc>,
    pub last_updated_at: DateTime<Utc>,
}

impl GlobalKvData {
    fn apply_one(state: &mut StateExecResponse, l: GlobalKvData) {
        state.results.push(StateExecResult {
            key: l.key, // key and scope are not returned by GlobalKvGetData since it's only used to get the data field of a global kv, so we'll set them to empty strings
            scope: l.scope,
            value: l.data,
            created_at: l.created_at,
            last_updated_at: l.last_updated_at,
            opaque: !l.public_data || l.price.is_some(), // if the data is not public, we mark it as opaque so that it doesn't get exposed to user code
        })
    }
}