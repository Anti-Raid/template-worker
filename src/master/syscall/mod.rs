pub mod auth;
pub mod discord;
pub mod types;
pub mod bot;
pub mod gkv;
pub mod webapi;
pub(super) mod internal;

use std::{sync::Arc, time::Duration};
use std::fmt::{Display, Debug};
use moka::future::Cache;
use serde::{Deserialize, Serialize};
use serenity::all::{GuildId, UserId};
use crate::geese::tenantstate::TenantStateDb;
use crate::{geese::stratum::Stratum, master::{syscall::{auth::{AuthError, MAuthSyscall, MAuthSyscallRet}, bot::{MBotSyscall, MBotSyscallRet}, discord::{MDiscordSyscall, MDiscordSyscallRet}, gkv::{MGkvSyscall, MGkvSyscallRet}, types::bot::BotStatus}, workerpool::WorkerPool}};
use khronos_ext::mluau_ext::prelude::*;

/// The context in which the syscall is executing in
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
#[allow(dead_code)]
pub enum MSyscallContext {
    /// API context (anonymous/logged out)
    ApiAnon,
    /// API context (normal)
    Api(UserId),
    //// A 'secure' API context (w/ 2fa etc used)
    ApiSecure(UserId),
    /// The tw shell (anonymous)
    ShellAnon,
    /// The tw shell (mocking as a specific user)
    ShellWithUser(UserId)
}

#[allow(dead_code)]
impl MSyscallContext {
    /// Returns if the given context is secure (admin/root access only)
    /// 
    /// A context is considered secure iff it originates from a user (with admin permissions)
    /// running under the secure msyscall API endpoint (which verifies that the user has admin)
    /// or if the request comes from the tw shell (which is assumed to have admin permissions)
    #[inline(always)]
    pub const fn is_secure(self) -> bool {
        matches!(self, Self::ApiSecure(_) | Self::ShellAnon | Self::ShellWithUser(_))
    }

    /// Returns if the given context is a shell
    #[inline(always)]
    pub const fn is_shell(self) -> bool {
        matches!(self, Self::ShellAnon | Self::ShellWithUser(_))
    }

    /// Gets the user id, erroring if not found in the context
    pub const fn into_user_id(self) -> Result<UserId, MSyscallError> {
        match self {
            Self::Api(u) | Self::ApiSecure(u) | Self::ShellWithUser(u) => Ok(u),
            _ => Err(MSyscallError::ContextRequiresUser)
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "op")]
pub enum MSyscallArgs {
    /// A bot-specific system calls
    Bot {
        req: MBotSyscall
    },
    /// A discord-specific syscall
    Discord {
        req: MDiscordSyscall
    },
    /// A auth-specific syscall
    Auth {
        req: MAuthSyscall
    },
    /// A global-kv specific syscall
    Gkv {
        req: MGkvSyscall
    }
}

impl FromLua for MSyscallArgs {
    fn from_lua(value: LuaValue, _lua: &Lua) -> LuaResult<Self> {
        let LuaValue::Table(tab) = value else {
            return Err(LuaError::FromLuaConversionError {
                from: value.type_name(),
                to: "MSyscallArgs".to_string(),
                message: Some("expected a table".to_string()),
            })
        };

        let typ: LuaString = tab.get("op")?;
        match typ.as_bytes().as_ref() {
            b"Bot" => {
                let req = tab.get("data")?;
                Ok(Self::Bot { req })
            },
            b"Discord" => {
                let req = tab.get("data")?;
                Ok(Self::Discord { req })
            },
            b"Auth" => {
                let req = tab.get("data")?;
                Ok(Self::Auth { req })
            },
            b"Gkv" => {
                let req = tab.get("data")?;
                Ok(Self::Gkv { req })
            },
            _ => {
                Err(LuaError::FromLuaConversionError {
                    from: "table",
                    to: "MSyscallArgs".to_string(),
                    message: Some("invalid op provided".to_string()),
                })
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "op")]
pub enum MSyscallRet {
    Bot {
        data: MBotSyscallRet
    },
    Discord {
        data: MDiscordSyscallRet
    },
    Auth {
        data: MAuthSyscallRet
    },
    Gkv {
        data: MGkvSyscallRet
    }
}

impl MSyscallRet {
    const fn name(&self) -> &'static str {
        match self {
            Self::Bot { .. } => "Bot",
            Self::Discord { .. } => "Discord",
            Self::Auth { .. } => "Auth",
            Self::Gkv { .. } => "Gkv",
        }
    }
}

impl IntoLua for MSyscallRet {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        let table = lua.create_table_with_capacity(0, 2)?;
        table.set("op", self.name())?;
        match self {
            Self::Bot { data } => table.set("data", data)?,
            Self::Discord { data } => table.set("data", data)?,
            Self::Auth { data } => table.set("data", data)?,
            Self::Gkv { data } => table.set("data", data)?,
        }
        table.set_readonly(true);
        Ok(LuaValue::Table(table))
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "op")]
pub enum MSyscallError {
    /// Generic error response
    Generic(String),
    /// Invalid event name/data
    InvalidEvent { reason: &'static str },
    /// Context is too insecure to perform this operation
    ContextInsecure,
    /// Context requires a user to actually perform this operation on/with
    ContextRequiresUser,
    /// Guild does not have bot added to it
    BotNotOnGuild,
    /// User needs to login via OAuth2 once first *before* using this API
    UserOauth2Needed,
    /// An authentication error has occurred
    AuthError { reason: AuthError },
    /// Unauthorized
    Unauthorized { reason: &'static str },
    /// Entity not found
    EntityNotFound { reason: &'static str }
}

impl<T: Debug + Display + 'static> From<T> for MSyscallError {
    fn from(value: T) -> Self {
        Self::Generic(value.to_string())
    }
}

impl IntoLua for MSyscallError {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        lua.to_value(&self)
    }
}

#[derive(Clone)]
pub struct MSyscallHandler {
    pub(super) current_user: Arc<serenity::all::CurrentUser>,
    pub(super) reqwest: reqwest::Client,
    pub(super) worker_pool: Arc<WorkerPool>,
    pub(super) stratum: Stratum,
    pub(super) pool: sqlx::PgPool,
    pub(super) bot_has_guild_cache: Cache<GuildId, bool>,
    pub(super) oauth2_code_cache: Cache<String, ()>,
    pub(super) status_cache: Cache<(), BotStatus>,
    pub(super) tsdb: TenantStateDb
}

impl MSyscallHandler {
    /// Creates a new MSyscallHandler
    pub fn new(
        current_user: Arc<serenity::all::CurrentUser>, 
        worker_pool: Arc<WorkerPool>,
        stratum: Stratum,
        reqwest: reqwest::Client,
        pool: sqlx::PgPool,
    ) -> Self {
        Self { 
            pool: pool.clone(), 
            current_user,
            reqwest,
            stratum,
            worker_pool,
            bot_has_guild_cache: Cache::builder().time_to_live(Duration::from_secs(60)).build(),
            oauth2_code_cache: Cache::builder().time_to_live(Duration::from_secs(60 * 10)).build(),
            status_cache: Cache::builder().time_to_live(Duration::from_secs(100)).build(),
            tsdb: TenantStateDb::new(pool)
        }
    }

    /// Helper function to check if the bot is in a guild
    async fn has_bot(&self, guilds: &[serenity::all::GuildId]) -> Result<Vec<bool>, MSyscallError> {
        if guilds.len() == 1 {
            let hb = self.bot_has_guild_cache.try_get_with::<_, crate::Error>(guilds[0], async move {
                let guild_exists = self.stratum.has_guilds(guilds).await?;
                if guild_exists.is_empty() {
                    Err("internal error: guild_exists is empty when it shouldnt be".into())
                } else {
                    Ok(guild_exists[0])
                }
            })
            .await?;

            return Ok(vec![hb])
        };
        let guild_exists = self.stratum.has_guilds(guilds).await?;
        if guild_exists.len() != guilds.len() {
            return Err("internal error: guild_exists is empty when it shouldnt be".into())
        }

        Ok(guild_exists)
    }

    /// Handles a syscall
    pub async fn handle_syscall(&self, args: MSyscallArgs, ctx: MSyscallContext) -> Result<MSyscallRet, MSyscallError> {
        match args {
            MSyscallArgs::Bot { req } => {
                Ok(MSyscallRet::Bot { data: req.exec(self, ctx).await? })
            }
            MSyscallArgs::Discord { req } => {
                Ok(MSyscallRet::Discord { data: req.exec(ctx.into_user_id()?, self).await? })
            }
            MSyscallArgs::Auth { req } => {
                Ok(MSyscallRet::Auth { data: req.exec(self, ctx).await? })
            }
            MSyscallArgs::Gkv { req } => {
                Ok(MSyscallRet::Gkv { data: req.exec(self, ctx).await? })
            }
        }
    }
}