pub mod auth;
pub mod discord;
pub mod types;
pub mod bot;
pub mod gkv;
pub mod webapi;
pub(super) mod internal;

use std::{error::Error, time::Duration};
use khronos_runtime::{primitives::event::CreateEvent, utils::khronos_value::KhronosValue};
use moka::future::Cache;
use serde::{Deserialize, Serialize};
use serenity::all::UserId;
use crate::{geese::stratum::Stratum, master::{syscall::{auth::{AuthError, MAuthSyscall, MAuthSyscallRet}, bot::{MBotSyscall, MBotSyscallRet}, discord::{MDiscordSyscall, MDiscordSyscallRet}, gkv::{MGkvSyscall, MGkvSyscallRet}, types::bot::BotStatus}, workerpool::WorkerPool}, worker::workervmmanager::Id};

/// The context in which the syscall is executing in
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum MSyscallContext {
    /// API context (normal)
    Api(UserId),
    //// A 'secure' API context (w/ 2fa etc used)
    ApiSecure(UserId),
    /// The tw shell (anonymous)
    ShellAnon,
    /// The tw shell (logged in)
    ShellWithUser(UserId)
}

impl MSyscallContext {
    /// Returns if the given context is secure
    pub const fn is_secure(self) -> bool {
        matches!(self, Self::ApiSecure(_) | Self::ShellAnon | Self::ShellWithUser(_))
    }

    /// Returns if the given context is a shell
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
    /// Dispatch an event to a worker process
    DispatchEvent {
        /// Tenant ID to dispatch the event to
        id: Id,
        /// Name of the event
        name: String,
        /// Data to send
        data: KhronosValue
    },
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

#[derive(Serialize, Deserialize)]
#[serde(tag = "op")]
pub enum MSyscallRet {
    KhronosValue {
        data: KhronosValue
    },
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

#[derive(Serialize)]
#[serde(tag = "op")]
pub enum MSyscallError {
    /// Generic error response
    Generic(crate::Error),
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

impl<T: Error + Send + Sync + 'static> From<T> for MSyscallError {
    fn from(value: T) -> Self {
        Self::Generic(value.into())
    }
}

pub struct MSyscallHandler {
    pub(super) current_user: serenity::all::CurrentUser,
    pub(super) reqwest: reqwest::Client,
    pub(super) worker_pool: WorkerPool,
    pub(super) stratum: Stratum,
    pub(super) pool: sqlx::PgPool,
    pub(super) bot_has_guild_cache: Cache<serenity::all::GuildId, bool>,
    pub(super) oauth2_code_cache: Cache<String, ()>,
    pub(super) status_cache: Cache<(), BotStatus>
}

impl MSyscallHandler {
    /// Creates a new MSyscallHandler
    pub fn new(
        current_user: serenity::all::CurrentUser, 
        worker_pool: WorkerPool,
        stratum: Stratum,
        reqwest: reqwest::Client,
        pool: sqlx::PgPool
    ) -> Self {
        Self { 
            pool, 
            current_user,
            reqwest,
            stratum,
            worker_pool,
            bot_has_guild_cache: Cache::builder().time_to_live(Duration::from_secs(60)).build(),
            oauth2_code_cache: Cache::builder().time_to_live(Duration::from_secs(60 * 10)).build(),
            status_cache: Cache::builder().time_to_live(Duration::from_secs(100)).build()
        }
    }

    /// Helper function to check if the bot is in a guild
    async fn has_bot(
        &self,
        guilds: &[serenity::all::GuildId],
    ) -> Result<Vec<bool>, MSyscallError> {
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
            return Err("internal error: guild_exists is empty when it shouldnt be")
        }

        Ok(guild_exists)
    }


    /// Handles a syscall
    pub async fn handle_syscall(&self, args: MSyscallArgs, ctx: MSyscallContext) -> Result<MSyscallRet, MSyscallError> {
        match args {
            MSyscallArgs::DispatchEvent { id, name, data } => {
                if !ctx.is_secure() && !name.starts_with("Web") {
                    return Err(MSyscallError::InvalidEvent { reason: "Event name must start with Web in insecure contexts"});
                }
                let user_id = ctx.into_user_id()?;
                match id {
                    Id::Guild(id) => {
                        // Ensure the bot is in the guild
                        let hb = self.has_bot(&[id]).await?;    
                        if !hb[0] {
                            return Err(MSyscallError::BotNotOnGuild);
                        }    
                    }
                    Id::User(id) => {
                        if user_id != id {
                            return Err(MSyscallError::InvalidEvent { reason: "Cannot send events to users who are not yourself" });
                        }
                    }
                }

                let event = CreateEvent::new_khronos_value(name, Some(user_id.to_string()), data);

                Ok(MSyscallRet::KhronosValue { data: self.worker_pool.dispatch_event(id, event).await? })
            }
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