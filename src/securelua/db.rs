use khronos_runtime::{chrono_tz, core::{datetime::DateTime as LuaDateTime, typesext::I64}};
use chrono::{DateTime, Utc};
use serde_json::Value as JsonValue;
use sqlx::types::Uuid;
use khronos_ext::db_plugin;

db_plugin! {
    i32 => { I32, I32Opt, I32List, "i32", |lua, value| { lua.from_value(value) }, |lua, opaque| { lua.to_value(opaque) } },
    i64 => { I64, I64Opt, I64List, "i64", |lua, value| { 
        match value {
            LuaValue::UserData(ud) => {
                if let Ok(i64) = ud.borrow::<I64>() {
                    Ok(i64.0)
                } else {
                    Err(LuaError::external("Expected I64 userdata"))
                }
            }
            LuaValue::Integer(i) => Ok(i as i64),
            LuaValue::Number(n) => Ok(n as i64),
            _ => lua.from_value(value)
        }
    }, |lua, opaque| { lua.to_value(&opaque) } },
    String => { String, StringOpt, StringList, "string", |lua, value| { lua.from_value(value) }, |lua, opaque| { lua.to_value(opaque) } },
    bool => { Bool, BoolOpt, BoolList, "boolean", |lua, value| { lua.from_value(value) }, |_lua, opaque| { Ok::<_, LuaError>(LuaValue::Boolean(*opaque)) } },
    f64 => { F64, F64Opt, F64List, "f64", |lua, value| { lua.from_value(value) }, |lua, opaque| { lua.to_value(opaque) } },
    DateTime<Utc> => { DateTime, DateTimeOpt, DateTimeList, "datetime", |lua, value| { 
        match value {
            LuaValue::UserData(s) => {
                if let Ok(dt) = s.borrow::<LuaDateTime<chrono_tz::Tz>>() {
                    Ok(dt.dt.with_timezone(&chrono::Utc))
                } else {
                    Err(LuaError::external("Expected DateTime<Utc> userdata"))
                }
            }
            _ => lua.from_value(value)
        }    
    }, |lua, opaque| { lua.to_value(opaque) } },
    JsonValue => { Json, JsonOpt, JsonList, "json", |lua, value| { lua.from_value(value) }, |lua, opaque| { lua.to_value(opaque) } },
    Uuid => { Uuid, UuidOpt, UuidList, "uuid", |lua, value| { lua.from_value(value) }, |lua, opaque| { lua.to_value(opaque) } },
}

