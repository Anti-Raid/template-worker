use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use khronos_runtime::traits::ir::globalkv as gkv_ir;
use ts_rs::TS;

#[derive(Debug, Serialize, Deserialize, TS, utoipa::ToSchema, sqlx::FromRow)]
#[ts(export)]
pub struct PartialGlobalKv {
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
}

impl Into<gkv_ir::PartialGlobalKv> for PartialGlobalKv {
    fn into(self) -> gkv_ir::PartialGlobalKv {
        gkv_ir::PartialGlobalKv {
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
        }
    }
}

#[derive(Debug, Serialize, Deserialize, TS, utoipa::ToSchema, sqlx::FromRow)]
pub struct GlobalKv {
    #[sqlx(flatten)]
    pub partial: PartialGlobalKv,
    #[serde(skip)]
    #[sqlx(rename = "data")]
    #[ts(skip)]
    pub(super) raw_data: serde_json::Value, // the actual value of the key-value, may be private
    #[sqlx(skip)]
    pub data: GlobalKvData,
}

impl GlobalKv {
    /// Drop sensitive data from the GlobalKv, replacing it with null if it's opaque
    pub fn drop_sensitive(mut self) -> Self {
        match self.data {
            GlobalKvData::Value { data: _, opaque: true } => {
                self.data = GlobalKvData::Value {
                    data: serde_json::Value::Null,
                    opaque: true,
                };
            }
            _ => { /* do nothing */ }
        }
        self
    }
}

impl Into<gkv_ir::GlobalKv> for GlobalKv {
    fn into(self) -> gkv_ir::GlobalKv {
        gkv_ir::GlobalKv {
            partial: self.partial.into(),
            data: self.data.into(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, TS, utoipa::ToSchema)]
#[ts(export)]
#[serde(tag = "type")]
pub enum GlobalKvData {
    Value {
        data: serde_json::Value,
        opaque: bool,
    },
    PurchaseRequired {
        purchase_url: String,
    },
}

impl Default for GlobalKvData {
    fn default() -> Self {
        GlobalKvData::Value {
            data: serde_json::Value::Null,
            opaque: true,
        }
    }
}

impl Into<gkv_ir::GlobalKvData> for GlobalKvData {
    fn into(self) -> gkv_ir::GlobalKvData {
        match self {
            GlobalKvData::Value { data, opaque } => gkv_ir::GlobalKvData::Value { data, opaque },
            GlobalKvData::PurchaseRequired { purchase_url } => gkv_ir::GlobalKvData::PurchaseRequired { purchase_url },
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
