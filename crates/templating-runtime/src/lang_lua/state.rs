use governor::clock::Clock;
use governor::{clock::QuantaClock, DefaultKeyedRateLimiter};
pub use silverpelt::templates::LuaKVConstraints;
use std::num::NonZeroU32;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use super::vm_manager::AtomicInstant;

#[allow(dead_code)]
pub struct LuaRatelimits {
    pub clock: QuantaClock,
    pub global: Vec<DefaultKeyedRateLimiter<()>>,
    pub per_bucket: indexmap::IndexMap<String, Vec<DefaultKeyedRateLimiter<()>>>,
}

impl LuaRatelimits {
    fn create_quota(
        limit_per: NonZeroU32,
        limit_time: Duration,
    ) -> Result<governor::Quota, crate::Error> {
        let quota = governor::Quota::with_period(limit_time)
            .ok_or("Failed to create quota")?
            .allow_burst(limit_per);

        Ok(quota)
    }

    fn new_discord_rl() -> Result<Self, crate::Error> {
        // Create the global limit
        let global_quota =
            Self::create_quota(NonZeroU32::new(10).unwrap(), Duration::from_secs(10))?;
        let global1 = DefaultKeyedRateLimiter::keyed(global_quota);
        let global = vec![global1];

        // Create the per-bucket limits
        let ban_quota1 = Self::create_quota(NonZeroU32::new(5).unwrap(), Duration::from_secs(30))?;
        let ban_lim1 = DefaultKeyedRateLimiter::keyed(ban_quota1);
        let ban_quota2 = Self::create_quota(NonZeroU32::new(10).unwrap(), Duration::from_secs(75))?;
        let ban_lim2 = DefaultKeyedRateLimiter::keyed(ban_quota2);

        let kick_quota1 = Self::create_quota(NonZeroU32::new(5).unwrap(), Duration::from_secs(30))?;
        let kick_lim1 = DefaultKeyedRateLimiter::keyed(kick_quota1);
        let kick_quota2 =
            Self::create_quota(NonZeroU32::new(10).unwrap(), Duration::from_secs(75))?;
        let kick_lim2 = DefaultKeyedRateLimiter::keyed(kick_quota2);

        // Send message channel limits (are smaller to allow for more actions)
        let create_message_quota1 =
            Self::create_quota(NonZeroU32::new(15).unwrap(), Duration::from_secs(20))?;
        let create_message_lim1 = DefaultKeyedRateLimiter::keyed(create_message_quota1);

        // Create Interaction Response
        let create_interaction_response_quota1 =
            Self::create_quota(NonZeroU32::new(5).unwrap(), Duration::from_secs(10))?;

        let create_interaction_response_lim1 =
            DefaultKeyedRateLimiter::keyed(create_interaction_response_quota1);

        // get_original_interaction_response
        let get_original_interaction_response_quota1 =
            Self::create_quota(NonZeroU32::new(5).unwrap(), Duration::from_secs(10))?;

        let get_original_interaction_response_lim1 =
            DefaultKeyedRateLimiter::keyed(get_original_interaction_response_quota1);

        // get_guild_commands
        let get_guild_commands_quota1 =
            Self::create_quota(NonZeroU32::new(1).unwrap(), Duration::from_secs(300))?;
        let get_guild_commands_lim1 = DefaultKeyedRateLimiter::keyed(get_guild_commands_quota1);

        // Create the clock
        let clock = QuantaClock::default();

        Ok(Self {
            global,
            per_bucket: indexmap::indexmap!(
                "ban".to_string() => vec![ban_lim1, ban_lim2] as Vec<DefaultKeyedRateLimiter<()>>,
                "kick".to_string() => vec![kick_lim1, kick_lim2] as Vec<DefaultKeyedRateLimiter<()>>,
                "create_message".to_string() => vec![create_message_lim1] as Vec<DefaultKeyedRateLimiter<()>>,
                "create_interaction_response".to_string() => vec![create_interaction_response_lim1] as Vec<DefaultKeyedRateLimiter<()>>,
                "get_original_interaction_response".to_string() => vec![get_original_interaction_response_lim1] as Vec<DefaultKeyedRateLimiter<()>>,
                "get_guild_commands".to_string() => vec![get_guild_commands_lim1] as Vec<DefaultKeyedRateLimiter<()>>,
            ),
            clock,
        })
    }

    fn new_kv_rl() -> Result<Self, crate::Error> {
        // Create the global limit
        let global_quota =
            Self::create_quota(NonZeroU32::new(10).unwrap(), Duration::from_secs(60))?;
        let global1 = DefaultKeyedRateLimiter::keyed(global_quota);
        let global = vec![global1];

        // Create the clock
        let clock = QuantaClock::default();

        Ok(Self {
            global,
            per_bucket: indexmap::indexmap!(),
            clock,
        })
    }

    fn new_stings_rl() -> Result<Self, crate::Error> {
        // Create the global limit
        let global_quota =
            Self::create_quota(NonZeroU32::new(10).unwrap(), Duration::from_secs(60))?;
        let global1 = DefaultKeyedRateLimiter::keyed(global_quota);
        let global = vec![global1];

        // Create the clock
        let clock = QuantaClock::default();

        Ok(Self {
            global,
            per_bucket: indexmap::indexmap!(),
            clock,
        })
    }

    fn new_lockdowns_rl() -> Result<Self, crate::Error> {
        // Create the global limit
        let global_quota =
            Self::create_quota(NonZeroU32::new(3).unwrap(), Duration::from_secs(60))?;

        // TSL limit
        let tsl_quota = Self::create_quota(NonZeroU32::new(1).unwrap(), Duration::from_secs(60))?;

        let global1 = DefaultKeyedRateLimiter::keyed(global_quota);
        let global = vec![global1];

        // Create the per-bucket limits
        let tsl_lim1 = DefaultKeyedRateLimiter::keyed(tsl_quota);
        // Create the clock
        let clock = QuantaClock::default();

        Ok(Self {
            global,
            per_bucket: indexmap::indexmap!(
                "tsl".to_string() => vec![tsl_lim1] as Vec<DefaultKeyedRateLimiter<()>>,
            ),
            clock,
        })
    }

    fn new_userinfo_rl() -> Result<Self, crate::Error> {
        // Create the global limit
        let global_quota =
            Self::create_quota(NonZeroU32::new(7).unwrap(), Duration::from_secs(60))?;
        let global1 = DefaultKeyedRateLimiter::keyed(global_quota);
        let global = vec![global1];

        // Create the clock
        let clock = QuantaClock::default();

        Ok(Self {
            global,
            per_bucket: indexmap::indexmap!(),
            clock,
        })
    }

    pub fn check(&self, bucket: &str) -> Result<(), crate::Error> {
        // Check global ratelimits
        for global_lim in self.global.iter() {
            match global_lim.check_key(&()) {
                Ok(()) => continue,
                Err(wait) => {
                    return Err(format!(
                        "Global ratelimit hit for bucket '{}', wait time: {:?}",
                        bucket,
                        wait.wait_time_from(self.clock.now())
                    )
                    .into());
                }
            };
        }

        // Check per bucket ratelimits
        if let Some(per_bucket) = self.per_bucket.get(bucket) {
            for lim in per_bucket.iter() {
                match lim.check_key(&()) {
                    Ok(()) => continue,
                    Err(wait) => {
                        return Err(format!(
                            "Per bucket ratelimit hit for '{}', wait time: {:?}",
                            bucket,
                            wait.wait_time_from(self.clock.now())
                        )
                        .into());
                    }
                };
            }
        }

        Ok(())
    }
}

pub struct Ratelimits {
    /// Stores the lua discord ratelimiters
    pub discord: LuaRatelimits,

    /// Stores the lua kv ratelimiters
    pub kv: LuaRatelimits,

    /// Stores the lua sting ratelimiters
    pub stings: LuaRatelimits,

    /// Stores the lua lockdown ratelimiters
    pub lockdowns: LuaRatelimits,

    /// Stores the lua userinfo ratelimiters
    pub userinfo: LuaRatelimits,
}

impl Ratelimits {
    pub fn new() -> Result<Self, crate::Error> {
        Ok(Self {
            discord: LuaRatelimits::new_discord_rl()?,
            kv: LuaRatelimits::new_kv_rl()?,
            stings: LuaRatelimits::new_stings_rl()?,
            lockdowns: LuaRatelimits::new_lockdowns_rl()?,
            userinfo: LuaRatelimits::new_userinfo_rl()?,
        })
    }
}

#[allow(dead_code)]
pub struct GuildState {
    pub last_execution_time: Arc<AtomicInstant>,
    pub pool: sqlx::PgPool,
    pub guild_id: serenity::all::GuildId,
    pub serenity_context: serenity::all::Context,
    pub reqwest_client: reqwest::Client,
    pub kv_constraints: LuaKVConstraints,
    pub ratelimits: Rc<Ratelimits>,
}
