use governor::{clock::QuantaClock, DefaultKeyedRateLimiter};
use khronos_runtime::utils::ratelimits::LuaRatelimits;
pub use silverpelt::templates::LuaKVConstraints;
use std::num::NonZeroU32;
use std::rc::Rc;
use std::time::Duration;

pub fn create_nonmax_u32(value: u32) -> Result<NonZeroU32, silverpelt::Error> {
    Ok(NonZeroU32::new(value).ok_or("Value must be non-zero")?)
}

impl Ratelimits {
    fn new_discord_rl() -> Result<LuaRatelimits, silverpelt::Error> {
        // Create the global limit
        let global_quota =
            LuaRatelimits::create_quota(create_nonmax_u32(10)?, Duration::from_secs(10))?;
        let global1 = DefaultKeyedRateLimiter::keyed(global_quota);
        let global = vec![global1];

        // Create the per-bucket limits
        let ban_quota1 =
            LuaRatelimits::create_quota(create_nonmax_u32(5)?, Duration::from_secs(30))?;
        let ban_lim1 = DefaultKeyedRateLimiter::keyed(ban_quota1);
        let ban_quota2 =
            LuaRatelimits::create_quota(create_nonmax_u32(10)?, Duration::from_secs(75))?;
        let ban_lim2 = DefaultKeyedRateLimiter::keyed(ban_quota2);

        let kick_quota1 =
            LuaRatelimits::create_quota(create_nonmax_u32(5)?, Duration::from_secs(30))?;
        let kick_lim1 = DefaultKeyedRateLimiter::keyed(kick_quota1);
        let kick_quota2 =
            LuaRatelimits::create_quota(create_nonmax_u32(10)?, Duration::from_secs(75))?;
        let kick_lim2 = DefaultKeyedRateLimiter::keyed(kick_quota2);

        // Send message channel limits (are smaller to allow for more actions)
        let create_message_quota1 =
            LuaRatelimits::create_quota(create_nonmax_u32(15)?, Duration::from_secs(20))?;
        let create_message_lim1 = DefaultKeyedRateLimiter::keyed(create_message_quota1);

        // Create Interaction Response
        let create_interaction_response_quota1 =
            LuaRatelimits::create_quota(create_nonmax_u32(5)?, Duration::from_secs(10))?;

        let create_interaction_response_lim1 =
            DefaultKeyedRateLimiter::keyed(create_interaction_response_quota1);

        // get_original_interaction_response
        let get_original_interaction_response_quota1 =
            LuaRatelimits::create_quota(create_nonmax_u32(5)?, Duration::from_secs(10))?;

        let get_original_interaction_response_lim1 =
            DefaultKeyedRateLimiter::keyed(get_original_interaction_response_quota1);

        // get_guild_commands
        let get_guild_commands_quota1 =
            LuaRatelimits::create_quota(create_nonmax_u32(1)?, Duration::from_secs(300))?;
        let get_guild_commands_lim1 = DefaultKeyedRateLimiter::keyed(get_guild_commands_quota1);

        // create_guild_command
        let create_guild_command_quota1 =
            LuaRatelimits::create_quota(create_nonmax_u32(1)?, Duration::from_secs(300))?;
        let create_guild_command_lim1 = DefaultKeyedRateLimiter::keyed(create_guild_command_quota1);

        // Create the clock
        let clock = QuantaClock::default();

        Ok(LuaRatelimits {
            global,
            per_bucket: indexmap::indexmap!(
                "ban".to_string() => vec![ban_lim1, ban_lim2] as Vec<DefaultKeyedRateLimiter<()>>,
                "kick".to_string() => vec![kick_lim1, kick_lim2] as Vec<DefaultKeyedRateLimiter<()>>,
                "create_message".to_string() => vec![create_message_lim1] as Vec<DefaultKeyedRateLimiter<()>>,
                "create_interaction_response".to_string() => vec![create_interaction_response_lim1] as Vec<DefaultKeyedRateLimiter<()>>,
                "get_original_interaction_response".to_string() => vec![get_original_interaction_response_lim1] as Vec<DefaultKeyedRateLimiter<()>>,
                "get_guild_commands".to_string() => vec![get_guild_commands_lim1] as Vec<DefaultKeyedRateLimiter<()>>,
                "create_guild_command".to_string() => vec![create_guild_command_lim1] as Vec<DefaultKeyedRateLimiter<()>>,
            ),
            clock,
        })
    }

    fn new_kv_rl() -> Result<LuaRatelimits, silverpelt::Error> {
        // Create the global limit
        let global_quota =
            LuaRatelimits::create_quota(create_nonmax_u32(100)?, Duration::from_secs(1))?;
        let global1 = DefaultKeyedRateLimiter::keyed(global_quota);
        let global = vec![global1];

        // Create the clock
        let clock = QuantaClock::default();

        Ok(LuaRatelimits {
            global,
            per_bucket: indexmap::indexmap!(),
            clock,
        })
    }

    fn new_stings_rl() -> Result<LuaRatelimits, silverpelt::Error> {
        // Create the global limit
        let global_quota =
            LuaRatelimits::create_quota(create_nonmax_u32(100)?, Duration::from_secs(3))?;
        let global1 = DefaultKeyedRateLimiter::keyed(global_quota);
        let global = vec![global1];

        // Create the clock
        let clock = QuantaClock::default();

        Ok(LuaRatelimits {
            global,
            per_bucket: indexmap::indexmap!(),
            clock,
        })
    }

    fn new_lockdowns_rl() -> Result<LuaRatelimits, silverpelt::Error> {
        // Create the global limit
        let global_quota =
            LuaRatelimits::create_quota(create_nonmax_u32(3)?, Duration::from_secs(60))?;

        // TSL limit
        let tsl_quota =
            LuaRatelimits::create_quota(create_nonmax_u32(1)?, Duration::from_secs(60))?;

        let global1 = DefaultKeyedRateLimiter::keyed(global_quota);
        let global = vec![global1];

        // Create the per-bucket limits
        let tsl_lim1 = DefaultKeyedRateLimiter::keyed(tsl_quota);
        // Create the clock
        let clock = QuantaClock::default();

        Ok(LuaRatelimits {
            global,
            per_bucket: indexmap::indexmap!(
                "tsl".to_string() => vec![tsl_lim1] as Vec<DefaultKeyedRateLimiter<()>>,
            ),
            clock,
        })
    }

    fn new_userinfo_rl() -> Result<LuaRatelimits, silverpelt::Error> {
        // Create the global limit
        let global_quota =
            LuaRatelimits::create_quota(create_nonmax_u32(7)?, Duration::from_secs(60))?;
        let global1 = DefaultKeyedRateLimiter::keyed(global_quota);
        let global = vec![global1];

        // Create the clock
        let clock = QuantaClock::default();

        Ok(LuaRatelimits {
            global,
            per_bucket: indexmap::indexmap!(),
            clock,
        })
    }

    fn new_page_rl() -> Result<LuaRatelimits, silverpelt::Error> {
        // Create the global limit
        let global_quota =
            LuaRatelimits::create_quota(create_nonmax_u32(10)?, Duration::from_secs(1))?;
        let global1 = DefaultKeyedRateLimiter::keyed(global_quota);
        let global = vec![global1];

        // Create the clock
        let clock = QuantaClock::default();

        Ok(LuaRatelimits {
            global,
            per_bucket: indexmap::indexmap!(),
            clock,
        })
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

    /// Stores the lua page ratelimiters
    pub page: LuaRatelimits,
}

impl Ratelimits {
    pub fn new() -> Result<Self, silverpelt::Error> {
        Ok(Ratelimits {
            discord: Ratelimits::new_discord_rl()?,
            kv: Ratelimits::new_kv_rl()?,
            stings: Ratelimits::new_stings_rl()?,
            lockdowns: Ratelimits::new_lockdowns_rl()?,
            userinfo: Ratelimits::new_userinfo_rl()?,
            page: Ratelimits::new_page_rl()?,
        })
    }
}

#[allow(dead_code)]
pub struct GuildState {
    pub pool: sqlx::PgPool,
    pub guild_id: serenity::all::GuildId,
    pub serenity_context: serenity::all::Context,
    pub reqwest_client: reqwest::Client,
    pub kv_constraints: LuaKVConstraints,
    pub ratelimits: Rc<Ratelimits>,
}
