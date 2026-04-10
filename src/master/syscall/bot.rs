use std::sync::Arc;

use dapi::types::CreateCommand;
use serde::Serialize;
use serenity::all::{GuildId, UserId};
use crate::master::syscall::{MSyscallContext, MSyscallError, MSyscallHandler, types::bot::{BotStatus, ShardConn}};

#[derive(Serialize)]
#[serde(tag = "op")]
pub enum MBotSyscall {
    /// Returns the commands registered on the bot
    GetBotCommands {},
    /// Returns the bots base config
    GetBotConfig {},
    /// Returns the bots status
    GetBotStatus {},
    /// Returns the uncached bot status (works in secure contexts only)
    GetUncachedBotStatus {}
}

#[derive(Serialize)]
#[serde(tag = "op")]
pub enum MBotSyscallRet {
    /// A list of bot commands
    CommandList {
        /// The ID of the user who created the session
        commands: Arc<Vec<CreateCommand>>
    },
    BotConfig {
        /// The ID of the main AntiRaid support server
        main_server: GuildId,
        /// Discord Support Server Link
        support_server_invite: String,
        /// The ID of the AntiRaid bot client
        client_id: UserId,
    },
    BotStatus {
        status: BotStatus
    }
}

impl MBotSyscall {
    pub(super) async fn exec(self, handler: &MSyscallHandler, ctx: MSyscallContext) -> Result<MBotSyscallRet, MSyscallError> {
        match self {
            Self::GetBotCommands {  } => {
                Ok(MBotSyscallRet::CommandList { commands: crate::register::REGISTER.commands.clone() })
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
        }
    }
}
