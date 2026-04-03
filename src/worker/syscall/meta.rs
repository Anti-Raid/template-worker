use khronos_runtime::{core::datetime::DateTime, rt::mluau::prelude::*};

use crate::worker::{syscall::SyscallHandler, workervmmanager::Id};

/// The core underlying syscall
#[derive(Debug)]
pub enum MetaCall {
    GetStats {},
}

impl FromLua for MetaCall {
    fn from_lua(value: LuaValue, _lua: &Lua) -> LuaResult<Self> {
        let LuaValue::Table(tab) = value else {
            return Err(LuaError::FromLuaConversionError {
                from: value.type_name(),
                to: "MetaCall".to_string(),
                message: Some("expected a table".to_string()),
            })
        };

        let typ: LuaString = tab.get("op")?;
        match typ.as_bytes().as_ref() {
            b"GetStats" => {
                Ok(MetaCall::GetStats { })
            },
            _ => {
                Err(LuaError::FromLuaConversionError {
                    from: "table",
                    to: "CdnCall".to_string(),
                    message: Some("invalid op provided".to_string()),
                })
            }
        }
    }
}

pub enum MetaResult {
    Stats {
        total_guilds: u64,
        total_users: u64,
        last_started_at: chrono::DateTime<chrono::Utc>,
    }
}

impl IntoLua for MetaResult {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        let table = lua.create_table()?;
        match self {
            Self::Stats { total_guilds, total_users, last_started_at } => {
                table.set("op", "Stats")?;
                table.set("total_guilds", total_guilds)?;
                table.set("total_users", total_users)?;
                table.set("last_started_at", DateTime::from_utc(last_started_at))?;
            },
        }
        table.set_readonly(true); // We want StateExecResult's to be immutable
        Ok(LuaValue::Table(table))
    }
}

impl MetaCall {
    pub(super) async fn exec(self, _id: Id, handler: &SyscallHandler) -> Result<MetaResult, crate::Error> {
        match self {
            Self::GetStats {} => {
                handler.ratelimits.runtime.check("GetStats")?;
                let resp = handler.state.stratum.get_status().await?;

                Ok(MetaResult::Stats {
                    total_guilds: resp.guild_count,
                    total_users: resp.user_count,
                    //total_members: sandwich_resp.total_members.try_into()?,
                    last_started_at: crate::CONFIG.start_time,
                })
            }
        }
    }
}