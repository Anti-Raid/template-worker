use serde::{Deserialize, Serialize};
use serenity::{all::GuildId, nonmax::NonMaxU16};
use sqlx::Row;
use crate::master::syscall::{MSyscallContext, MSyscallError, MSyscallHandler, internal::auth as iauth};
use super::types::discord::*;

/// While discord supports up to 1000, we limit to 20 for user experience purposes
pub const SEARCH_GUILD_MEMBERS_LIMIT: NonMaxU16 = match NonMaxU16::new(20) {
    Some(m) => m,
    None => unreachable!(),
};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "op")]
pub enum MDiscordSyscall {
    /// Get a list of all user guilds
    GetUserGuilds {
        refresh: bool,
    },
    /// Get guild info
    GetGuildInfo {
        guild_id: GuildId
    },
    /// Find all guild members beginning with given username/nickname
    SearchGuildMembers {
        guild_id: GuildId,
        name: String,
    }
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "op")]
pub enum MDiscordSyscallRet {
    /// List of all user guilds
    UserGuilds {
        data: DashboardGuildData
    },
    GuildInfo {
        data: BaseGuildUserInfo
    },
    GuildMembers {
        data: Vec<PartialMember>
    }
}

impl MDiscordSyscall {
    pub(super) async fn exec(self, handler: &MSyscallHandler, ctx: MSyscallContext) -> Result<MDiscordSyscallRet, MSyscallError> {
        let user_id = ctx.into_user_id()?;
        match self {
            Self::GetUserGuilds { refresh } => {
                handler.limit(&ctx, "GetUserGuilds")?;
                let mut guilds_cache = None;
                if !refresh {
                    // Check for guilds cache
                    let cached_guilds = sqlx::query("SELECT guilds_cache FROM users WHERE user_id = $1")
                        .bind(user_id.to_string())
                        .fetch_one(&handler.pool)
                        .await?;

                    if let Some(cached_guilds_data) = cached_guilds.try_get::<Option<serde_json::Value>, _>(0)? {
                        guilds_cache = Some(serde_json::from_value::<Vec<DashboardGuild>>(cached_guilds_data)?);
                    }
                }

                // Extra bucket limit for refresh ops
                if guilds_cache.is_none() {
                    handler.sub_limit(&ctx, "GetUserGuilds__Refresh")?;
                }

                let guilds = match guilds_cache {
                    Some(gc) => gc,
                    None => {
                        // Get the access token
                        let access_token = iauth::get_user_access_token(handler, &user_id.to_string()).await?;

                        let resp = handler.reqwest.get(format!("{}/api/v10/users/@me/guilds", crate::CONFIG.meta.proxy))
                        .header("Authorization", format!("Bearer {access_token}"))
                        .send()
                        .await?;

                        if resp.status() != reqwest::StatusCode::OK {
                            let error_text = resp.text().await?;
                            return Err(format!("Failed to get user guilds: {}", error_text).into());
                        }

                        #[derive(serde::Deserialize)]
                        pub struct OauthGuild {
                            id: serenity::all::GuildId,
                            name: String,
                            icon: Option<String>,
                            permissions: String,
                            owner: bool,
                        }

                        let guilds: Vec<OauthGuild> = resp.json().await?;

                        let mut dashboard_guilds = Vec::with_capacity(guilds.len());

                        for guild in guilds {
                            let dashboard_guild = DashboardGuild {
                                id: guild.id,
                                name: guild.name,
                                icon: guild.icon,
                                permissions: guild.permissions,
                                owner: guild.owner,
                            };

                            dashboard_guilds.push(dashboard_guild);
                        }

                        // Now update the database
                        sqlx::query("UPDATE users SET guilds_cache = $1 WHERE user_id = $2")
                            .bind(serde_json::to_value(&dashboard_guilds)?)
                            .bind(user_id.to_string())
                            .execute(&handler.pool)
                            .await?;

                        dashboard_guilds
                    }
                };

                let guild_ids = guilds.iter().map(|x| x.id).collect::<Vec<_>>();

                let guilds_exist = handler.has_bot(&guild_ids).await?;

                Ok(MDiscordSyscallRet::UserGuilds {
                    data: DashboardGuildData {
                        guilds,
                        guilds_exist,
                    }
                })
            }
            Self::GetGuildInfo { guild_id } => {
                handler.limit(&ctx, "GetGuildInfo")?;

                let bot_id = handler.current_user.id;
                let Some(guild_json) = handler.stratum.guild(guild_id).await? else {
                    return Err(MSyscallError::EntityNotFound { reason: "Failed to fetch guild data from stratum" });
                };

                let guild = serde_json::from_value::<serenity::all::PartialGuild>(guild_json)?;

                // Next fetch the member and bot_user
                let Some(member) = handler.guild_member(guild_id, user_id).await? else {
                    return Err(MSyscallError::EntityNotFound { reason: "Failed to find current member info. If you recently joined the server, you may need to wait up to 10-15 minutes." });
                };

                let Some(bot_user) = handler.guild_member(guild_id, bot_id).await? else {
                    return Err(MSyscallError::EntityNotFound { reason: "Failed to find bot user info" });
                };

                // Fetch the channels
                let Some(channels_json) = handler.stratum.guild_channels(guild_id).await? else {
                    return Err(MSyscallError::EntityNotFound { reason: "Failed to find guild channel info" });
                };

                let channels = serde_json::from_value::<Vec<serenity::all::GuildChannel>>(channels_json)?;

                let mut channels_with_permissions = Vec::with_capacity(channels.len());

                for channel in channels.iter() {
                    channels_with_permissions.push(GuildChannelWithPermissions {
                        user: guild.user_permissions_in(channel, &member),
                        bot: guild.user_permissions_in(channel, &bot_user),
                        channel: ApiPartialGuildChannel {
                            id: channel.id.widen(),
                            name: channel.base.name.to_string(),
                            position: channel.position,
                            parent_id: channel.parent_id.map(|id| id.widen()),
                            r#type: channel.base.kind.0,
                        },
                    });
                }

                Ok(MDiscordSyscallRet::GuildInfo { 
                    data: BaseGuildUserInfo {
                        name: guild.name.to_string(),
                        icon: guild.icon_url(),
                        owner_id: guild.owner_id.to_string(),
                        roles: guild.roles.into_iter().map(|role| {
                            ApiPartialRole {
                                id: role.id,
                                name: role.name.to_string(),
                                position: role.position,
                                permissions: role.permissions,
                            }
                        }).collect(),
                        user_roles: member.roles.to_vec(),
                        bot_roles: bot_user.roles.to_vec(),
                        channels: channels_with_permissions,
                    }
                })
            },
            Self::SearchGuildMembers { guild_id, name } => {
                handler.limit(&ctx, "SearchGuildMembers")?;

                // SAFETY: Ensure user is in the server they are trying to search in
                if handler.guild_member(guild_id, user_id).await?.is_none() {
                    return Err(MSyscallError::Unauthorized { reason: "You are potentially not a member of this server! If you recently joined the server, you may need to wait up to 5 minutes." });
                };

                let sgm = handler.stratum.discord_http().search_guild_members(guild_id, &name, Some(SEARCH_GUILD_MEMBERS_LIMIT)).await?;
                let mem_data = serde_json::from_value::<Vec<PartialMember>>(sgm)?;
                Ok(MDiscordSyscallRet::GuildMembers { data: mem_data })
            }
        }
    }
}
