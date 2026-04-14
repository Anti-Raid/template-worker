use chrono::{DateTime, Utc};
use khronos_runtime::utils::khronos_value::KhronosValue;
use serde::{Deserialize, Serialize};
use khronos_runtime::rt::mluau::prelude::*;

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
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
    #[sqlx(flatten)]
    pub data: Option<GlobalKvData>, // is only sent on GetGlobalKv
}

impl IntoLua for PartialGlobalKv {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        let table = lua.create_table()?;
        
        table.set("key", self.key)?;
        table.set("version", self.version)?;
        table.set("owner_id", self.owner_id)?;
        table.set("owner_type", self.owner_type)?;
        table.set("price", self.price)?;
        table.set("short", self.short)?;
        table.set("public_metadata", self.public_metadata)?;
        table.set("scope", self.scope)?;
        table.set("public_data", self.public_data)?;
        table.set("review_state", self.review_state)?;
        table.set("long", self.long)?;
        table.set("data", self.data)?;
        Ok(LuaValue::Table(table))
    }
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct GlobalKvData {
    #[sqlx(json)]
    pub data: KhronosValue, // the actual value of the key-value, may be private
}

impl IntoLua for GlobalKvData {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        self.data.into_lua(lua)
    }
}