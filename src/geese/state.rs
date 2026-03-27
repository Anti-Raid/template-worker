use khronos_runtime::rt::mlua::prelude::*;
use khronos_runtime::utils::khronos_value::KhronosValue;
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
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_updated_at: Option<chrono::DateTime<chrono::Utc>>,
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
    #[sqlx(default)]
    created_at: Option<chrono::DateTime<chrono::Utc>>,
    #[sqlx(default)]
    last_updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl KvLookup {
    fn apply_one(state: &mut StateExecResponse, l: KvLookup) {
        state.results.push(StateExecResult { key: l.key, value: l.value, scope: l.scope, created_at: l.created_at, last_updated_at: l.last_updated_at })
    }
    fn apply(state: &mut StateExecResponse, lookups: Vec<KvLookup>) {
        for l in lookups {
            Self::apply_one(state, l);
        }
    }
}