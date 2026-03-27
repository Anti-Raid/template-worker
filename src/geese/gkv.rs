use chrono::{DateTime, Utc};
use khronos_runtime::utils::khronos_value::KhronosValue;
use serde::{Serialize, Deserialize};
use ts_rs::TS;
use crate::{api::types::KhronosValueApi, worker::workervmmanager::Id};

#[derive(Clone)]
/// A simple wrapper around the database pool that provides just the global key-value storage functionality
pub struct GlobalKeyValueDb {
    pool: sqlx::PgPool,
}

impl GlobalKeyValueDb {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }

    // TODO: Actually implement this
    async fn global_kv_is_purchased(&self, _key: String, _tid: Id) -> Result<bool, crate::Error> {
        Ok(false)
    }

    // TODO: Actually implement this
    async fn global_kv_to_url(&self, key: &str) -> String {
        // TODO: Replace with actual purchase URL generation logic
        format!("{}/shop/{key}", crate::CONFIG.sites.frontend)
    }

    pub async fn global_kv_find(&self, scope: String, query: String) -> Result<Vec<PartialGlobalKv>, crate::Error> {
        let items: Vec<PartialGlobalKv> = if query == "%%" {
            sqlx::query_as(
                "SELECT key, version, owner_id, owner_type, short, public_metadata, public_data, scope, created_at, last_updated_at, price, review_state FROM global_kv WHERE scope = $1 AND review_state = 'approved'"
            )
            .bind(scope)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as(
                "SELECT key, version, owner_id, owner_type, short, public_metadata, public_data, scope, created_at, last_updated_at, price, review_state FROM global_kv WHERE scope = $1 AND review_state = 'approved' AND key LIKE $2"
            )
            .bind(scope)
            .bind(query)
            .fetch_all(&self.pool)
            .await?
        };

        Ok(items)
    }
    
    pub async fn global_kv_get(&self, key: String, version: i32, scope: String, id: Option<Id>) -> Result<Option<GlobalKv>, crate::Error> {
        let item: Option<GlobalKv> = sqlx::query_as(
            "SELECT key, version, owner_id, owner_type, short, long, data, public_metadata, public_data, scope, created_at, last_updated_at, price, review_state FROM global_kv WHERE review_state = 'approved' AND key = $1 AND version = $2 AND scope = $3",
        )
        .bind(&key)
        .bind(version)
        .bind(scope)
        .fetch_optional(&self.pool)
        .await?;

        let Some(mut gkv) = item else {
            return Ok(None);
        };

        // Drop data immediately here to ensure it is not leaked
        let data = std::mem::replace(&mut gkv.raw_data, KhronosValue::Null);

        if gkv.partial.price.is_some() {
            match id {
                Some(tid) => {
                    // Check if purchased
                    let is_purchased = self.global_kv_is_purchased(key, tid).await?;
                    if !is_purchased {
                        gkv.data = GlobalKvData::PurchaseRequired {
                            purchase_url: self.global_kv_to_url(&gkv.partial.key).await,
                        };
                        return Ok(Some(gkv));
                    }
                }
                None => {
                    // No tenant ID provided, cannot verify purchase
                    gkv.data = GlobalKvData::PurchaseRequired {
                        purchase_url: self.global_kv_to_url(&gkv.partial.key).await,
                    };
                    return Ok(Some(gkv));
                }
            }
        }

        let opaque = gkv.partial.price.is_some() || !gkv.partial.public_data;
        gkv.data = GlobalKvData::Value { data, opaque };

        Ok(Some(gkv))
    }

    pub async fn global_kv_create(&self, id: Id, gkv: CreateGlobalKv) -> Result<(), crate::Error> {
        // Validate key
        //
        // Rules:
        // 1. Between 3 and 64 characters long
        // 2. May not start or end with a dot (.)
        // 3. May only contain (ASCII) alphanumeric characters, dots (.), dashes (-), and underscores (_)
        if gkv.key.len() < 3 || gkv.key.len() > 64 {
            return Err("keys must be between 3 and 64 characters long".into());
        }
        if gkv.key.starts_with('.') || gkv.key.ends_with('.') {
            return Err("keys may not start or end with a dot".into());
        }
        if !gkv.key.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_') {
            return Err("keys may only contain alphanumeric characters, dots, dashes, and underscores".into());
        }

        let inserted = sqlx::query(
            "INSERT INTO global_kv (key, version, owner_id, owner_type, short, long, public_metadata, public_data, scope, data) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) ON CONFLICT (key, version, scope) DO NOTHING",
        )
        .bind(&gkv.key)
        .bind(gkv.version)
        .bind(id.tenant_id())
        .bind(id.tenant_type())
        .bind(&gkv.short)
        .bind(&gkv.long)
        .bind(serde_json::to_value(gkv.public_metadata)?)
        .bind(gkv.public_data)
        .bind(&gkv.scope)
        .bind(serde_json::to_value(gkv.data)?)
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
}

#[derive(Debug, Serialize, Deserialize, TS, utoipa::ToSchema, sqlx::FromRow)]
#[ts(export)]
pub struct PartialGlobalKv {
    pub key: String,
    pub version: i32,
    pub owner_id: String,
    pub owner_type: String,
    pub price: Option<i64>, // will only be set for shop items, otherwise None
    pub short: String, // short description for the key-value.
    #[ts(as = "KhronosValueApi")]
    #[schema(value_type = KhronosValueApi)]
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

#[derive(Debug, Serialize, Deserialize, TS, utoipa::ToSchema, sqlx::FromRow)]
pub struct GlobalKv {
    #[sqlx(flatten)]
    pub partial: PartialGlobalKv,
    #[serde(skip)]
    #[sqlx(rename = "data")]
    #[ts(skip)]
    #[sqlx(json)]
    pub(super) raw_data: KhronosValue, // the actual value of the key-value, may be private
    #[sqlx(skip)]
    pub data: GlobalKvData,
}

impl GlobalKv {
    /// Drop sensitive data from the GlobalKv, replacing it with null if it's opaque
    pub fn drop_sensitive(mut self) -> Self {
        match self.data {
            GlobalKvData::Value { data: _, opaque: true } => {
                self.data = GlobalKvData::Value {
                    data: KhronosValue::Null,
                    opaque: true,
                };
            }
            _ => { /* do nothing */ }
        }
        self
    }
}

#[derive(Debug, Serialize, Deserialize, TS, utoipa::ToSchema)]
#[ts(export)]
#[serde(tag = "type")]
pub enum GlobalKvData {
    Value {
        #[ts(as = "KhronosValueApi")]
        #[schema(value_type = KhronosValueApi)]
        data: KhronosValue,
        opaque: bool,
    },
    PurchaseRequired {
        purchase_url: String,
    },
}

impl Default for GlobalKvData {
    fn default() -> Self {
        GlobalKvData::Value {
            data: KhronosValue::Null,
            opaque: true,
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
    #[ts(as = "KhronosValueApi")]
    #[schema(value_type = KhronosValueApi)]
    pub public_metadata: KhronosValue, // public metadata about the key-value
    pub scope: String,
    pub public_data: bool,
    pub long: Option<String>, // long description for the key-value.
    #[ts(as = "KhronosValueApi")]
    #[schema(value_type = KhronosValueApi)]
    pub data: KhronosValue, // the actual value of the key-value, may be private
}
