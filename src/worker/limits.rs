use governor::{clock::QuantaClock, DefaultKeyedRateLimiter};
use khronos_runtime::utils::ratelimits::LuaRatelimits;
use std::num::NonZeroU32;
use std::time::Duration;

pub const MAX_TEMPLATE_MEMORY_USAGE: usize = 1024 * 1024 * 20; // 20MB maximum memory
pub const MAX_VM_THREAD_STACK_SIZE: usize = 1024 * 1024 * 20; // 20MB maximum memory
pub const MAX_TEMPLATES_EXECUTION_TIME: Duration = Duration::from_secs(10); // 10 seconds maximum execution time before sched yield must happen
pub const TEMPLATE_GIVE_TIME: Duration = Duration::from_secs(1); // 1 second maximum time to give to a template to finish execution following a yield

pub fn create_nonmax_u32(value: u32) -> Result<NonZeroU32, crate::Error> {
    Ok(NonZeroU32::new(value).ok_or("Value must be non-zero")?)
}

impl Ratelimits {
    fn new_discord_rl() -> Result<LuaRatelimits, crate::Error> {
        // Create the global limit
        let global_quota =
            LuaRatelimits::create_quota(create_nonmax_u32(20)?, Duration::from_secs(5))?;
        let global1 = DefaultKeyedRateLimiter::keyed(global_quota);
        let global = vec![global1];

        // Create bulk op limits
        let bulk_op_create_quota =
            LuaRatelimits::create_quota(create_nonmax_u32(3)?, Duration::from_secs(1))?;
        let bulk_op_create_lim = DefaultKeyedRateLimiter::keyed(bulk_op_create_quota);

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
            LuaRatelimits::create_quota(create_nonmax_u32(30)?, Duration::from_secs(10))?;
        let create_message_lim1 = DefaultKeyedRateLimiter::keyed(create_message_quota1);

        // get_original_interaction_response
        let get_original_interaction_response_quota1 =
            LuaRatelimits::create_quota(create_nonmax_u32(10)?, Duration::from_secs(3))?;
        let get_original_interaction_response_lim1 =
            DefaultKeyedRateLimiter::keyed(get_original_interaction_response_quota1);

        // create_followup_message
        let create_followup_message_quota1 =
            LuaRatelimits::create_quota(create_nonmax_u32(10)?, Duration::from_secs(3))?;
        let create_followup_message_lim1 =
            DefaultKeyedRateLimiter::keyed(create_followup_message_quota1);
        
        // get_followup_message
        let get_followup_message_quota1 =
            LuaRatelimits::create_quota(create_nonmax_u32(10)?, Duration::from_secs(3))?;
        let get_followup_message_lim1 =
            DefaultKeyedRateLimiter::keyed(get_followup_message_quota1);

        // edit_followup_message
        let edit_followup_message_quota1 =
            LuaRatelimits::create_quota(create_nonmax_u32(10)?, Duration::from_secs(3))?;
        let edit_followup_message_lim1 =
            DefaultKeyedRateLimiter::keyed(edit_followup_message_quota1);

        // delete_followup_message
        let delete_followup_message_quota1 =
            LuaRatelimits::create_quota(create_nonmax_u32(10)?, Duration::from_secs(3))?;
        let delete_followup_message_lim1 =
            DefaultKeyedRateLimiter::keyed(delete_followup_message_quota1);

        // edit_original_interaction_response
        let edit_original_interaction_response_quota1 =
            LuaRatelimits::create_quota(create_nonmax_u32(10)?, Duration::from_secs(3))?;
        let edit_original_interaction_response_lim1 =
            DefaultKeyedRateLimiter::keyed(edit_original_interaction_response_quota1);

        // delete_original_interaction_response
        let delete_original_interaction_response_quota1 =
            LuaRatelimits::create_quota(create_nonmax_u32(10)?, Duration::from_secs(3))?;
        let delete_original_interaction_response_lim1 =
            DefaultKeyedRateLimiter::keyed(delete_original_interaction_response_quota1);

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
            global_ignore: LuaRatelimits::create_global_ignore(
                vec![
                    "antiraid_bulk_op".to_string(),
                    "antiraid_bulk_op_wait".to_string(),
                    "create_interaction_response".to_string(), // intentionally not ratelimited to allow for proper error handling / load handling
                    "get_original_interaction_response".to_string(),
                    "edit_original_interaction_response".to_string(),
                    "delete_original_interaction_response".to_string(),
                    "create_followup_message".to_string(),
                    "get_followup_message".to_string(),
                    "edit_followup_message".to_string(),
                    "delete_followup_message".to_string(),
                ]
            )?,
            per_bucket: indexmap::indexmap!(
                "antiraid_bulk_op".to_string() => vec![bulk_op_create_lim] as Vec<DefaultKeyedRateLimiter<()>>,
                "ban".to_string() => vec![ban_lim1, ban_lim2] as Vec<DefaultKeyedRateLimiter<()>>,
                "kick".to_string() => vec![kick_lim1, kick_lim2] as Vec<DefaultKeyedRateLimiter<()>>,
                "create_message".to_string() => vec![create_message_lim1] as Vec<DefaultKeyedRateLimiter<()>>,
                "get_original_interaction_response".to_string() => vec![get_original_interaction_response_lim1] as Vec<DefaultKeyedRateLimiter<()>>,
                "edit_original_interaction_response".to_string() => vec![edit_original_interaction_response_lim1] as Vec<DefaultKeyedRateLimiter<()>>,
                "delete_original_interaction_response".to_string() => vec![delete_original_interaction_response_lim1] as Vec<DefaultKeyedRateLimiter<()>>,
                "create_followup_message".to_string() => vec![create_followup_message_lim1] as Vec<DefaultKeyedRateLimiter<()>>,
                "get_followup_message".to_string() => vec![get_followup_message_lim1] as Vec<DefaultKeyedRateLimiter<()>>,
                "edit_followup_message".to_string() => vec![edit_followup_message_lim1] as Vec<DefaultKeyedRateLimiter<()>>,
                "delete_followup_message".to_string() => vec![delete_followup_message_lim1] as Vec<DefaultKeyedRateLimiter<()>>,
                "get_guild_commands".to_string() => vec![get_guild_commands_lim1] as Vec<DefaultKeyedRateLimiter<()>>,
                "create_guild_command".to_string() => vec![create_guild_command_lim1] as Vec<DefaultKeyedRateLimiter<()>>,
            ),
            clock,
        })
    }

    fn new_kv_rl() -> Result<LuaRatelimits, crate::Error> {
        // Create the global limit
        let global_quota =
            LuaRatelimits::create_quota(create_nonmax_u32(500)?, Duration::from_millis(100))?;
        let global1 = DefaultKeyedRateLimiter::keyed(global_quota);
        let global = vec![global1];

        // Create the clock
        let clock = QuantaClock::default();

        Ok(LuaRatelimits {
            global,
            global_ignore: LuaRatelimits::create_empty_global_ignore()?,
            per_bucket: indexmap::indexmap!(),
            clock,
        })
    }

    fn new_object_storage_rl() -> Result<LuaRatelimits, crate::Error> {
        // Create the global limit
        let global_quota =
            LuaRatelimits::create_quota(create_nonmax_u32(75)?, Duration::from_secs(1))?;
        let global1 = DefaultKeyedRateLimiter::keyed(global_quota);
        let global = vec![global1];

        // Create the clock
        let clock = QuantaClock::default();

        Ok(LuaRatelimits {
            global,
            global_ignore: LuaRatelimits::create_empty_global_ignore()?,
            per_bucket: indexmap::indexmap!(),
            clock,
        })
    }

    fn new_http_rl() -> Result<LuaRatelimits, crate::Error> {
        // Create the global limit
        let global_quota =
            LuaRatelimits::create_quota(create_nonmax_u32(3)?, Duration::from_secs(5))?;
        let global1 = DefaultKeyedRateLimiter::keyed(global_quota);
        let global = vec![global1];

        // Create the clock
        let clock = QuantaClock::default();

        Ok(LuaRatelimits {
            global,
            global_ignore: LuaRatelimits::create_empty_global_ignore()?,
            per_bucket: indexmap::indexmap!(),
            clock,
        })
    }

    fn new_runtime_rl() -> Result<LuaRatelimits, crate::Error> {
        // Create the global limit
        let global_quota =
            LuaRatelimits::create_quota(create_nonmax_u32(10)?, Duration::from_secs(1))?;
        let global1 = DefaultKeyedRateLimiter::keyed(global_quota);
        let global = vec![global1];

        // Create the clock
        let clock = QuantaClock::default();

        Ok(LuaRatelimits {
            global,
            global_ignore: LuaRatelimits::create_empty_global_ignore()?,
            per_bucket: indexmap::indexmap!(),
            clock,
        })
    }
}

/// Represents the limits for various operations in the worker
pub struct Ratelimits {
    /// Stores the lua discord ratelimiters
    pub discord: LuaRatelimits,

    /// Stores the lua kv ratelimiters
    pub kv: LuaRatelimits,

    /// Stores the object storage ratelimiters
    pub object_storage: LuaRatelimits,

    /// Stores the http ratelimiters
    pub http: LuaRatelimits,

    /// Stores the runtime ratelimiters
    pub runtime: LuaRatelimits,
}

impl Ratelimits {
    pub fn new() -> Result<Self, crate::Error> {
        Ok(Ratelimits {
            discord: Ratelimits::new_discord_rl()?,
            kv: Ratelimits::new_kv_rl()?,
            object_storage: Ratelimits::new_object_storage_rl()?,
            http: Ratelimits::new_http_rl()?,
            runtime: Ratelimits::new_runtime_rl()?,
        })
    }
}

#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct LuaKVConstraints {
    /// Maximum length of a key
    pub max_key_length: usize,
    /// Maximum length of a value (in bytes)
    pub max_value_bytes: usize,
    /// Maximum length of a object storage path
    pub max_object_storage_path_length: usize,
    /// Maximum length of a object storage data
    pub max_object_storage_bytes: usize,
}

impl Default for LuaKVConstraints {
    fn default() -> Self {
        LuaKVConstraints {
            max_key_length: 512,
            // 256kb max per value
            max_value_bytes: 256 * 1024,
            max_object_storage_path_length: 2048,
            // 512kb max per value
            max_object_storage_bytes: 512 * 1024,
        }
    }
}
