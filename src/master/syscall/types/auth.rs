use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use khronos_ext::mluau_ext::prelude::*;
use khronos_runtime::core::datetime::DateTime as LuaDateTime;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSession {
    /// The ID of the session
    pub id: String,
    /// The name of the session
    pub name: Option<String>,
    /// The ID of the user who created the session
    pub user_id: String,
    /// The time the session was created
    pub created_at: DateTime<Utc>,
    /// The type of session (e.g., "login", "api")
    pub r#type: String,
    /// The time the session expires
    pub expiry: DateTime<Utc>,
}

impl IntoLua for UserSession {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        let table = lua.create_table_with_capacity(0, 5)?;
        table.set("id", self.id)?;
        table.set("name", self.name)?;
        table.set("user_id", self.user_id)?;
        table.set("created_at", LuaDateTime::from_utc(self.created_at))?;
        table.set("expiry", LuaDateTime::from_utc(self.expiry))?;
        Ok(LuaValue::Table(table))
    }
}