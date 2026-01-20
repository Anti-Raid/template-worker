use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::migrations::Migration;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct KhronosBuffer(pub Vec<u8>);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(from = "KhronosProxy", into = "KhronosProxy")]
pub enum KhronosValue {
    Text(String),
    Integer(i64),
    UnsignedInteger(u64),
    Float(f64),
    Boolean(bool),
    Buffer(KhronosBuffer),   // Binary data
    Vector((f32, f32, f32)), // Luau vector
    Map(indexmap::IndexMap<String, KhronosValue>),
    List(Vec<KhronosValue>),
    Timestamptz(chrono::DateTime<chrono::Utc>),
    Interval(chrono::Duration),
    TimeZone(khronos_runtime::chrono_tz::Tz),
    Null,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "___khronosValType___", content = "value")]
#[serde(rename_all = "lowercase")] 
enum KhronosSpecial {
    Buffer(KhronosBuffer),
    Vector((f32, f32, f32)),
    Timestamptz(chrono::DateTime<chrono::Utc>),
    Interval(chrono::Duration),
    TimeZone(khronos_runtime::chrono_tz::Tz),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum KhronosProxy {
    // Note that order matters here as serde(untagged) will try each variant in order.

    // First, check special types
    Special(KhronosSpecial),

    // Primitives
    Boolean(bool),
    Integer(i64),
    UnsignedInteger(u64),
    Float(f64),
    Text(String),
    List(Vec<KhronosValue>),
    
    // Map (as this can overlap with other types, it must be last)
    Map(indexmap::IndexMap<String, KhronosValue>),
    
    Null,
}

impl From<KhronosProxy> for KhronosValue {
    fn from(proxy: KhronosProxy) -> Self {
        match proxy {
            KhronosProxy::Special(s) => match s {
                KhronosSpecial::Buffer(b) => KhronosValue::Buffer(b),
                KhronosSpecial::Vector(v) => KhronosValue::Vector(v),
                KhronosSpecial::Timestamptz(t) => KhronosValue::Timestamptz(t),
                KhronosSpecial::Interval(i) => KhronosValue::Interval(i),
                KhronosSpecial::TimeZone(t) => KhronosValue::TimeZone(t),
            },
            KhronosProxy::Boolean(b) => KhronosValue::Boolean(b),
            KhronosProxy::Integer(i) => KhronosValue::Integer(i),
            KhronosProxy::UnsignedInteger(u) => KhronosValue::UnsignedInteger(u),
            KhronosProxy::Float(f) => KhronosValue::Float(f),
            KhronosProxy::Text(t) => KhronosValue::Text(t),
            KhronosProxy::List(l) => KhronosValue::List(l),
            KhronosProxy::Map(m) => KhronosValue::Map(m),
            KhronosProxy::Null => KhronosValue::Null,
        }
    }
}

impl From<KhronosValue> for KhronosProxy {
    fn from(val: KhronosValue) -> Self {
        match val {
            KhronosValue::Buffer(b) => KhronosProxy::Special(KhronosSpecial::Buffer(b)),
            KhronosValue::Vector(v) => KhronosProxy::Special(KhronosSpecial::Vector(v)),
            KhronosValue::Timestamptz(t) => KhronosProxy::Special(KhronosSpecial::Timestamptz(t)),
            KhronosValue::Interval(i) => KhronosProxy::Special(KhronosSpecial::Interval(i)),
            KhronosValue::TimeZone(t) => KhronosProxy::Special(KhronosSpecial::TimeZone(t)),
            KhronosValue::Boolean(b) => KhronosProxy::Boolean(b),
            KhronosValue::Integer(i) => KhronosProxy::Integer(i),
            KhronosValue::UnsignedInteger(u) => KhronosProxy::UnsignedInteger(u),
            KhronosValue::Float(f) => KhronosProxy::Float(f),
            KhronosValue::Text(t) => KhronosProxy::Text(t),
            KhronosValue::List(l) => KhronosProxy::List(l),
            KhronosValue::Map(m) => KhronosProxy::Map(m),
            KhronosValue::Null => KhronosProxy::Null,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum KhronosValueV2 {
    Text(String),
    Integer(i64),
    UnsignedInteger(u64),
    Float(f64),
    Boolean(bool),
    Buffer(Vec<u8>),   // Binary data
    Vector((f32, f32, f32)), // Luau vector
    Map(Vec<(KhronosValueV2, KhronosValueV2)>),
    List(Vec<KhronosValueV2>),
    Timestamptz(chrono::DateTime<chrono::Utc>),
    Interval(chrono::Duration),
    TimeZone(khronos_runtime::chrono_tz::Tz),
    LazyStringMap(HashMap<String, String>), // For lazy string maps
    Null,
}

impl KhronosValueV2 {
    fn from_v1(v: KhronosValue) -> Self {
        match v {
            KhronosValue::Text(t) => KhronosValueV2::Text(t),
            KhronosValue::Integer(i) => KhronosValueV2::Integer(i),
            KhronosValue::UnsignedInteger(u) => KhronosValueV2::UnsignedInteger(u),
            KhronosValue::Float(f) => KhronosValueV2::Float(f),
            KhronosValue::Boolean(b) => KhronosValueV2::Boolean(b),
            KhronosValue::Buffer(b) => KhronosValueV2::Buffer(b.0),
            KhronosValue::Vector(v) => KhronosValueV2::Vector(v),
            KhronosValue::Map(m) => {
                let mut arr = Vec::new();
                for (k, v) in m {
                    arr.push((KhronosValueV2::Text(k), KhronosValueV2::from_v1(v)));
                }

                KhronosValueV2::Map(arr)
            },
            KhronosValue::List(l) => {
                let new_list = l.into_iter().map(KhronosValueV2::from_v1).collect();
                KhronosValueV2::List(new_list)
            },
            KhronosValue::Timestamptz(t) => KhronosValueV2::Timestamptz(t),
            KhronosValue::Interval(i) => KhronosValueV2::Interval(i),
            KhronosValue::TimeZone(t) => KhronosValueV2::TimeZone(t),
            KhronosValue::Null => KhronosValueV2::Null,
        }
    }
}

pub static MIGRATION: Migration = Migration {
    id: "khronosvalue_v2",
    description: "Migrate from KhronosValue v1 to KhronosValue v2 format (which is slightly larger but more robust)",
    up: |pool| {
        Box::pin(async move {
            use sqlx::Row;
            let rows = sqlx::query(
                r#"
                SELECT id, value FROM guild_templates_kv
                "#
            )
            .fetch_all(&pool)
            .await?;

            for row in rows {
                let id: String = row.get("id");
                let value: serde_json::Value = row.get("value");

                // Deserialize the old KhronosValue
                let old_value: KhronosValue = serde_json::from_value(value)
                    .map_err(|e| format!("Failed to deserialize old KhronosValue for id {}: {}", id, e))?;

                // Convert to new KhronosValueV2
                let new_value = KhronosValueV2::from_v1(old_value);

                // Serialize the new value to JSON
                let json_value = serde_json::to_value(&new_value)
                    .map_err(|e| format!("Failed to serialize new KhronosValueV2 for id {}: {}", id, e))?;

                // Update the database
                sqlx::query(
                    r#"
                    UPDATE guild_templates_kv
                    SET value = $1
                    WHERE id = $2
                    "#
                )
                .bind(json_value)
                .bind(&id)
                .execute(&pool)
                .await?;
            }

            Ok(())
        })
    },
};
