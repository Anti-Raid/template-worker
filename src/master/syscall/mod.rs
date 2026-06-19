pub mod auth;
pub mod discord;
pub mod types;
pub mod bot;
pub mod gkv;
pub mod webapi;
pub(super) mod internal;

use std::num::NonZeroU32;
use std::{sync::Arc, time::Duration};
use std::fmt::{Display, Debug};
use governor::DefaultKeyedRateLimiter;
use governor::clock::{Clock, QuantaClock};
use moka::future::Cache;
use serde::{Deserialize, Serialize};
use serenity::all::{GuildId, UserId};
use crate::geese::state::StateDb;
use crate::geese::tenantstate::TenantStateDb;
use crate::{geese::stratum::Stratum, master::{syscall::{auth::{AuthError, MAuthSyscall, MAuthSyscallRet}, bot::{MBotSyscall, MBotSyscallRet}, discord::{MDiscordSyscall, MDiscordSyscallRet}, gkv::{MGkvSyscall, MGkvSyscallRet}, types::bot::BotStatus}, workerpool::WorkerPool}};

/// The context in which the syscall is executing in
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
#[allow(dead_code)]
pub enum MSyscallContext {
    /// API context (anonymous/logged out)
    ApiAnon,
    /// API 'getter' context
    ApiAnonGetter,
    /// API context (login session/login token)
    ApiOauth(UserId),
    /// API context (api token)
    ApiToken(UserId),
    //// A 'secure' API context (mesoproxy etc.)
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

    /// Returns if the given context is an anonymous API getter
    #[inline(always)]    
    pub const fn is_anon_getter(self) -> bool {
        matches!(self, Self::ApiAnonGetter)
    }

    /// Returns if the given context comes from an oauth authorization
    /// 
    /// Required for oauth2-related APIs like GetUserGuilds (with refresh true) to work
    pub const fn is_oauth(self) -> bool {
        matches!(self, Self::ApiOauth(_))
    }

    /// Returns if the given context is a shell
    #[inline(always)]
    pub const fn is_shell(self) -> bool {
        matches!(self, Self::ShellAnon | Self::ShellWithUser(_))
    }

    /// Gets the user id, erroring if not found in the context
    pub const fn into_user_id(self) -> Result<UserId, MSyscallError> {
        match self {
            Self::ApiOauth(u) | Self::ApiToken(u) | Self::ApiSecure(u) | Self::ShellWithUser(u) => Ok(u),
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

#[derive(Debug, Serialize)]
#[serde(tag = "op")]
pub enum MSyscallError {
    /// Generic error response
    Generic { message: String },
    /// Invalid event name/data
    InvalidEvent { reason: &'static str },
    /// Context is too insecure to perform this operation
    ContextInsecure,
    /// Context requires a user to actually perform this operation on/with
    ContextRequiresUser,
    /// Context requires oauth2 login token to work
    ContextRequiresOauth,
    /// Guild does not have bot added to it
    BotNotOnGuild,
    /// User needs to login via OAuth2 once first *before* using this API
    UserOauth2Needed,
    /// An authentication error has occurred
    AuthError { reason: AuthError },
    /// Unauthorized
    Unauthorized { reason: &'static str },
    /// Entity not found
    EntityNotFound { reason: &'static str },
    /// Ratelimited
    Ratelimited {
        retry_after: f32,
        bucket: &'static str,
        req_bucket: &'static str
    }
}

impl<T: Debug + Display + 'static> From<T> for MSyscallError {
    fn from(value: T) -> Self {
        Self::Generic { message: value.to_string() }
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
    pub(super) guild_members_cache: Cache<(GuildId, UserId), Option<serenity::all::Member>>,
    pub(super) oauth2_code_cache: Cache<String, ()>,
    pub(super) user_rl: Arc<Ratelimiter>,
    pub(super) status_cache: Cache<(), BotStatus>,
    pub(super) tsdb: TenantStateDb,
    pub(super) statedb: StateDb,
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
            bot_has_guild_cache: Cache::builder().time_to_idle(Duration::from_secs(60)).build(),
            guild_members_cache: Cache::builder().time_to_idle(Duration::from_mins(5)).build(),
            oauth2_code_cache: Cache::builder().time_to_live(Duration::from_secs(60 * 10)).build(),
            user_rl: Self::user_limits().expect("Failed to build user limits").into(),
            status_cache: Cache::builder().time_to_live(Duration::from_secs(100)).build(),
            tsdb: TenantStateDb::new(pool.clone()),
            statedb: StateDb::new(pool),
        }
    }

    /// Helper method to return msyscall ratelimits
    fn user_limits() -> Result<Ratelimiter, crate::Error> {
        fn new(limit_per: u32, limit_time: Duration) -> DefaultKeyedRateLimiter<UserId> {
            let quota =
                Ratelimiter::create_quota(NonZeroU32::new(limit_per).unwrap(), limit_time).expect("Failed to create quota");
            let lim = DefaultKeyedRateLimiter::keyed(quota);
            lim
        }

        // Create the global limit
        let global1 = new(10, Duration::from_secs(1));

        // GetUserGuilds (with refresh)
        let gug_refresh1 = new(3, Duration::from_secs(5));

        // GetGuildInfo
        let ggi1 = new(2, Duration::from_secs(5));

        // SearchGuildMembers
        let sgm1 = new(1, Duration::from_secs(4));
        let sgm2 = new(5, Duration::from_mins(1));

        // Create the clock
        let clock = QuantaClock::default();

        Ok(Ratelimiter {
            global: vec![global1],
            per_bucket: indexmap::indexmap!(
                "GetUserGuilds__Refresh" => vec![gug_refresh1],
                "GetGuildInfo" => vec![ggi1],
                "SearchGuildMembers" => vec![sgm1, sgm2]
            ),
            clock,
        })
    }

    /// Helper function to check if the bot is in a single guild
    async fn has_bot_single(&self, guild: serenity::all::GuildId) -> Result<bool, MSyscallError> {
        let hb = self.bot_has_guild_cache.try_get_with::<_, crate::Error>(guild, async move {
            self.stratum.has_guild(guild).await
        })
        .await?;
        Ok(hb)
    }

    /// Helper function to check if the bot is in a list of guilds
    async fn has_bot(&self, guilds: &[serenity::all::GuildId]) -> Result<Vec<bool>, MSyscallError> {
        if guilds.len() == 1 {
            let hb = self.bot_has_guild_cache.try_get_with::<_, crate::Error>(guilds[0], async move {
                self.stratum.has_guild(guilds[0]).await
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

    /// Helper method to get user guild member with caching
    async fn guild_member(&self, guild_id: serenity::all::GuildId, user_id: serenity::all::UserId) -> Result<Option<serenity::all::Member>, MSyscallError> {
        let hb = self.guild_members_cache.try_get_with::<_, crate::Error>((guild_id, user_id), async move {
            let mem_v = self.stratum.guild_member(guild_id, user_id).await?;
            if let Some(mem_v) = mem_v {
                let v = serde_json::from_value(mem_v)?;
                Ok(Some(v))
            } else {
                Ok(None)
            }
        })
        .await?;
        Ok(hb)
    }

    pub(super) fn limit(&self, ctx: &MSyscallContext, op: &'static str) -> Result<(), MSyscallError> {
        match ctx {
            MSyscallContext::ApiOauth(u) | MSyscallContext::ApiToken(u) => {
                self.user_rl.check(op, *u).map_err(|e| MSyscallError::Ratelimited {
                    retry_after: e.dur.as_secs_f32(),
                    bucket: e.bucket,
                    req_bucket: e.req_bucket
                })
            },
            _ => Ok(())
        }
    }

    pub(super) fn sub_limit(&self, ctx: &MSyscallContext, op: &'static str) -> Result<(), MSyscallError> {
        match ctx {
            MSyscallContext::ApiOauth(u) => {
                self.user_rl.sub_check(op, *u).map_err(|e| MSyscallError::Ratelimited {
                    retry_after: e.dur.as_secs_f32(),
                    bucket: e.bucket,
                    req_bucket: e.req_bucket
                })
            },
            _ => Ok(())
        }
    }

    /// Handles a syscall
    pub async fn handle_syscall(&self, args: MSyscallArgs, ctx: MSyscallContext) -> Result<MSyscallRet, MSyscallError> {
        match args {
            MSyscallArgs::Bot { req } => {
                Ok(MSyscallRet::Bot { data: req.exec(self, ctx).await? })
            }
            MSyscallArgs::Discord { req } => {
                Ok(MSyscallRet::Discord { data: req.exec(self, ctx).await? })
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


#[allow(dead_code)]
pub struct Ratelimiter {
    pub clock: QuantaClock,
    pub global: Vec<DefaultKeyedRateLimiter<UserId>>,
    pub per_bucket: indexmap::IndexMap<&'static str, Vec<DefaultKeyedRateLimiter<UserId>>>,
}

struct RlExceeded {
    dur: Duration,
    bucket: &'static str,
    req_bucket: &'static str
}

impl Ratelimiter {
    fn create_quota(
        limit_per: NonZeroU32,
        limit_time: Duration,
    ) -> Result<governor::Quota, crate::Error> {
        let quota = governor::Quota::with_period(limit_time)
            .ok_or("Failed to create quota")?
            .allow_burst(limit_per);

        Ok(quota)
    }

    fn check(&self, bucket: &'static str, user: UserId) -> Result<(), RlExceeded> {
        for global_lim in self.global.iter() {
            match global_lim.check_key(&user) {
                Ok(()) => continue,
                Err(wait) => {
                    return Err(RlExceeded { dur: wait.wait_time_from(self.clock.now()), bucket: "global", req_bucket: bucket });
                }
            };
        }

        // Check per bucket ratelimits
        if let Some(per_bucket) = self.per_bucket.get(bucket) {
            for lim in per_bucket.iter() {
                match lim.check_key(&user) {
                    Ok(()) => continue,
                    Err(wait) => {
                        return Err(RlExceeded { dur: wait.wait_time_from(self.clock.now()), bucket, req_bucket: bucket });
                    }
                };
            }
        }

        Ok(())
    }

    /// Same as check, but only checks bucket
    fn sub_check(&self, bucket: &'static str, user: UserId) -> Result<(), RlExceeded> {
        // Check per bucket ratelimits
        if let Some(per_bucket) = self.per_bucket.get(bucket) {
            for lim in per_bucket.iter() {
                match lim.check_key(&user) {
                    Ok(()) => continue,
                    Err(wait) => {
                        return Err(RlExceeded { dur: wait.wait_time_from(self.clock.now()), bucket, req_bucket: bucket });
                    }
                };
            }
        }

        Ok(())
    }
}
