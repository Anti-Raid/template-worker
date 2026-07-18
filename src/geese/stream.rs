use dapi::UserId;
use khronos_ext::mluau_ext::prelude::*;
use khronos_runtime::utils::khronos_value::KhronosValue;
use serde::{Deserialize, Serialize};

/// A (luau->client) message sent 
#[derive(Serialize, Deserialize)]
pub enum LtcMessage {
    Msg { msg: KhronosValue, id: usize },
    Close { id: usize }
}

impl LtcMessage {
    pub fn id(&self) -> usize {
        match self {
            Self::Msg { id, .. } => *id,
            Self::Close { id } => *id,
        }
    }
}

impl FromLua for LtcMessage {
    fn from_lua(value: LuaValue, _lua: &Lua) -> LuaResult<Self> {
        let LuaValue::Table(tab) = value else {
            return Err(LuaError::FromLuaConversionError {
                from: value.type_name(),
                to: "LtcMessage".to_string(),
                message: Some("expected a table".to_string()),
            })
        };

        let typ: LuaString = tab.get("op")?;
        match typ.as_bytes().as_ref() {
            b"Msg" => {
                let msg = tab.get("msg")?;
                let id = tab.get("id")?;
                Ok(Self::Msg { msg, id })
            },
            b"Close" => {
                let id = tab.get("id")?;
                Ok(Self::Close { id })
            },
            _ => {
                Err(LuaError::FromLuaConversionError {
                    from: "table",
                    to: "LtcMessage".to_string(),
                    message: Some("invalid op provided".to_string()),
                })
            }
        }
    }
}

/// A (client->luau) message sent 
#[derive(Serialize, Deserialize)]
pub enum CtlMessage {
    NewConn { id: usize, user_id: UserId },
    CloseConn { id: usize },
    Msg { msg: KhronosValue, id: usize }
}

impl IntoLua for CtlMessage {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        let table = lua.create_table_with_capacity(2, 0)?;
        
        match self {
            CtlMessage::NewConn { id, user_id } => {
                table.set("type", "NewConn")?;
                table.set("id", id)?;
                table.set("user_id", user_id.to_string())?; 
            }
            CtlMessage::Msg { id, msg } => {
                table.set("type", "Msg")?;
                table.set("id", id)?;
                table.set("msg", msg)?; 
            }
            CtlMessage::CloseConn { id } => {
                table.set("type", "CloseConn")?;
                table.set("id", id)?;
            }
        }
        
        Ok(LuaValue::Table(table))
    }
}
