use std::sync::Arc;

use dapi::types::CreateCommand;
use khronos_runtime::{primitives::event::CreateEvent, utils::khronos_value::KhronosValue};
use serde::{Deserialize, Serialize};
use serenity::all::{GuildId, UserId};
use crate::{geese::tenantstate::ModFlags, master::syscall::{MSyscallContext, MSyscallError, MSyscallHandler, types::bot::{BotStatus, ShardConn}}, worker::workervmmanager::Id};
use khronos_ext::mluau_ext::prelude::*;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "op")]
pub enum MBotSyscall {
    /// Returns the commands registered on the bot
    GetBotCommands {},
    /// Returns the bots base config
    GetBotConfig {},
    /// Returns the bots status
    GetBotStatus {},
    /// Dispatch an event to a worker process
    DispatchEvent {
        /// Tenant ID to dispatch the event to
        id: Id,
        /// Name of the event
        name: String,
        /// Data to send
        data: KhronosValue
    },
    /// Returns the uncached bot status (works in secure contexts only)
    GetUncachedBotStatus {},
    /// Admin API to drop a tenant (works in secure contexts only)
    AdminDropTenant { id: Id },
    /// Admin API to set tenant state moderation flags (ban them etc.) (works in secure contexts only)
    AdminSetTenantStateModFlags { id: Id, modflags: ModFlags },
}

impl FromLua for MBotSyscall {
    fn from_lua(value: LuaValue, _lua: &Lua) -> LuaResult<Self> {
        let LuaValue::Table(tab) = value else {
            return Err(LuaError::FromLuaConversionError {
                from: value.type_name(),
                to: "MBotSyscall".to_string(),
                message: Some("expected a table".to_string()),
            })
        };

        let typ: LuaString = tab.get("op")?;
        match typ.as_bytes().as_ref() {
            b"GetBotCommands" => Ok(Self::GetBotCommands {}),
            b"GetBotConfig" => Ok(Self::GetBotConfig {}),
            b"GetBotStatus" => Ok(Self::GetBotStatus {}),
            b"DispatchEvent" => {
                let id = tab.get("id")?;
                let name = tab.get("name")?;
                let data = tab.get("data")?;
                Ok(Self::DispatchEvent { id, name, data })
            },
            b"GetUncachedBotStatus" => Ok(Self::GetUncachedBotStatus {}),
            b"AdminDropTenant" => {
                let id = tab.get("id")?;
                Ok(Self::AdminDropTenant { id })
            },
            b"AdminSetTenantStateModFlags" => {
                let id = tab.get("id")?;
                let modflags: u8 = tab.get("modflags")?;
                Ok(Self::AdminSetTenantStateModFlags { id, modflags: ModFlags::from_bits_retain(modflags) })
            },
            _ => {
                Err(LuaError::FromLuaConversionError {
                    from: "table",
                    to: "MBotSyscall".to_string(),
                    message: Some("invalid op provided".to_string()),
                })
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "op")]
pub enum MBotSyscallRet {
    /// A list of bot commands
    CommandList {
        /// The ID of the user who created the session
        commands: Arc<Vec<CreateCommand>>
    },
    /// Bot config
    BotConfig {
        /// The ID of the main AntiRaid support server
        main_server: GuildId,
        /// Discord Support Server Link
        support_server_invite: String,
        /// The ID of the AntiRaid bot client
        client_id: UserId,
    },
    /// Bot status
    BotStatus {
        status: BotStatus
    },
    /// Khronos value response
    KhronosValue {
        data: KhronosValue
    },
    Ack,
}

impl IntoLua for MBotSyscallRet {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        match self {
            Self::KhronosValue { data } => {
                let table = lua.create_table_with_capacity(0, 2)?;
                table.set("op", "KhronosValue")?;
                table.set("data", data)?;
                Ok(LuaValue::Table(table))
            }
            _ => lua.to_value(&self) // hack to speed up dev
        }
    }
}

impl MBotSyscall {
    pub(super) async fn exec(self, handler: &MSyscallHandler, ctx: MSyscallContext) -> Result<MBotSyscallRet, MSyscallError> {
        match self {
            Self::GetBotCommands {  } => {
                Ok(MBotSyscallRet::CommandList { commands: crate::master::register::REGISTER.commands.clone() })
            }
            Self::GetBotConfig {  } => {
                Ok(MBotSyscallRet::BotConfig { 
                    main_server: crate::CONFIG.servers.main,
                    client_id: crate::CONFIG.discord_auth.client_id,
                    support_server_invite: crate::CONFIG.meta.support_server_invite.clone(),
                })
            }
            Self::GetBotStatus {  } => {
                let status = handler.status_cache.try_get_with::<_, crate::Error>((), async move {
                    let raw_stats = handler.stratum.get_status().await?;

                    let stats = BotStatus {
                        shard_conns: raw_stats.shards.into_iter().map(|shard| {
                            (shard.shard_id, ShardConn {
                                status: shard.state().as_str_name().to_string(),
                                latency: shard.latency,
                            })
                        }).collect(),
                        total_guilds: raw_stats.guild_count,
                        total_users: raw_stats.user_count,
                    };

                    Ok(stats)
                }).await?;

                Ok(MBotSyscallRet::BotStatus { status })
            }
            Self::DispatchEvent { id, name, data } => {
                if !ctx.is_secure() && !name.starts_with("Web") {
                    return Err(MSyscallError::InvalidEvent { reason: "Event name must start with Web in insecure contexts"});
                }
                let user_id = ctx.into_user_id()?;
                match id {
                    Id::Guild(id) => {
                        // Ensure the bot is in the guild
                        let hb = handler.has_bot(&[id]).await?;    
                        if !hb[0] {
                            return Err(MSyscallError::BotNotOnGuild);
                        }
                        // Ensure guild is in server
                    }
                    Id::User(id) => {
                        if user_id != id {
                            return Err(MSyscallError::InvalidEvent { reason: "Cannot send events to users who are not yourself" });
                        }
                    }
                }

                let event = CreateEvent::new_khronos_value(name, Some(user_id.to_string()), data);

                Ok(MBotSyscallRet::KhronosValue { data: handler.worker_pool.dispatch_event(id, event).await? })
            }
            Self::GetUncachedBotStatus {  } => {
                if !ctx.is_secure() {
                    return Err(MSyscallError::ContextInsecure);
                }

                let raw_stats = handler.stratum.get_status().await?;

                let status = BotStatus {
                    shard_conns: raw_stats.shards.into_iter().map(|shard| {
                        (shard.shard_id, ShardConn {
                            status: shard.state().as_str_name().to_string(),
                            latency: shard.latency,
                        })
                    }).collect(),
                    total_guilds: raw_stats.guild_count,
                    total_users: raw_stats.user_count,
                };

                Ok(MBotSyscallRet::BotStatus { status })
            }
            Self::AdminDropTenant { id } => {
                if !ctx.is_secure() {
                    return Err(MSyscallError::ContextInsecure);
                }

                handler.worker_pool.drop_tenant(id).await?;
                Ok(MBotSyscallRet::Ack)
            }
            Self::AdminSetTenantStateModFlags { id, modflags } => {
                if !ctx.is_secure() {
                    return Err(MSyscallError::ContextInsecure);
                }
                let mut tx = handler.pool.begin().await?;
                sqlx::query("INSERT INTO tenant_state (owner_id, owner_type, modflags) VALUES ($1, $2, $3) ON CONFLICT (owner_id, owner_type) DO UPDATE SET modflags = EXCLUDED.modflags")
                    .bind(id.tenant_id())
                    .bind(id.tenant_type())
                    .bind(modflags.bits() as i32)
                    .execute(&mut *tx)
                    .await?;

                // Refresh tenant state now
                let Some(ts) = handler.tsdb.get_tenant_state_for(&mut tx, id).await? else {
                    return Err("failed to find tenant state after update".into())
                };
                
                let conn = handler.mesophyll_server.get_connection_for(id)
                .ok_or_else(|| format!("No Mesophyll connection found for worker process for ID: {id:?}"))?;
                conn.update_tenant_state(id, ts).await?;
                Ok(MBotSyscallRet::Ack)
            }
        }
    }
}
