use crate::worker::workervmmanager::Id;
use khronos_runtime::rt::mluau::prelude::*;
use serenity::all::GuildId;

pub struct LuaId(pub Id);

impl FromLua for LuaId {
    fn from_lua(value: LuaValue, _lua: &Lua) -> LuaResult<Self> {
        match value {
            LuaValue::Table(table) => {
                let tenant_type: String = table.get("tenant_type")?;
                match tenant_type.as_str() {
                    "guild" => {
                        let guild_id: u64 = table.get("guild_id")?;
                        Ok(LuaId(Id::GuildId(GuildId::new(guild_id))))
                    }
                    _ => Err(LuaError::external(format!("Unknown tenant_type: {}", tenant_type))),
                }
            }
            _ => {
                Err(LuaError::FromLuaConversionError {
                    from: value.type_name(),
                    to: "Id".to_string(),
                    message: Some("Expected a table representing an Id".to_string()),
                })
            }
        }
    }
}