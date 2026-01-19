use crate::worker::workervmmanager::Id;
use khronos_runtime::rt::mluau::prelude::*;

pub struct LuaId(pub Id);

impl FromLua for LuaId {
    fn from_lua(value: LuaValue, _lua: &Lua) -> LuaResult<Self> {
        match value {
            LuaValue::Table(table) => {
                let tenant_type: String = table.get("tenant_type")?;
                let tenant_id: String = table.get("tenant_id")?;
                let Some(id) = Id::from_parts(&tenant_type, &tenant_id) else {
                    return Err(LuaError::external(format!("Failed to parse Id from tenant_type: {}, tenant_id: {}", tenant_type, tenant_id)));
                };
                Ok(LuaId(id))
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