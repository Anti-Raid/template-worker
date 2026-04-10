use chrono::{DateTime, Utc};
use khronos_runtime::utils::khronos_value::KhronosValue;
use serde::Serialize;

#[derive(Debug, Serialize, sqlx::FromRow)]
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

    #[sqlx(skip)]
    pub data: Option<GlobalKvData>, // is only sent on GetGlobalKv
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct GlobalKvData {
    #[sqlx(json)]
    pub data: KhronosValue, // the actual value of the key-value, may be private
}
