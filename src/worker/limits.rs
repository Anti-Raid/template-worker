use governor::{clock::QuantaClock, DefaultKeyedRateLimiter};
use std::time::Duration;

use crate::geese::ratelimit::Ratelimiter;

pub const MAX_TEMPLATE_MEMORY_USAGE: usize = 1024 * 1024 * 25; // 25MB maximum memory
pub const MAX_ATTACHMENT_SIZE: usize = 1024 * 1024 * 5; // 5MB maximum memory
pub const MAX_VM_THREAD_STACK_SIZE: usize = 1024 * 1024 * 25; // 25MB maximum memory
pub const MAX_TEMPLATES_EXECUTION_TIME: Duration = Duration::from_secs(10); // 10 seconds maximum execution time before sched yield must happen
pub const TEMPLATE_GIVE_TIME: Duration = Duration::from_secs(1); // 1 second maximum time to give to a template to finish execution following a yield

pub const MAX_OBJ_STORAGE_PATH_LENGTH: usize = 2048;
pub const MAX_OBJ_STORAGE_BYTES: usize = 512 * 1024; // 512kb max per object

pub const KV_MAX_KEY_LENGTH: usize = 512;
pub const KV_SIGN_URL_EXPIRATION_SECONDS: u64 = 5 * 60; // 5 minutes

pub type LuaRatelimits = Ratelimiter<()>;
impl Ratelimits {
    pub const DISCORD_GLOBAL_IGNORE: [&'static str; 2] = [
        "CreateInteractionResponse", // intentionally not ratelimited to allow for proper error handling / load handling
        "CreateFollowupMessage",
    ];
    fn new_discord_rl() -> LuaRatelimits {
        // Create the global limit
        let global1 =
            LuaRatelimits::limit(20, Duration::from_secs(5));
        let global = vec![global1];

        // Create the per-bucket limits
        let ban_lim1 =
            LuaRatelimits::limit(5, Duration::from_secs(30));
        let ban_lim2 =
            LuaRatelimits::limit(10, Duration::from_secs(75));

        let kick_lim1 =
            LuaRatelimits::limit(5, Duration::from_secs(30));
        let kick_lim2 =
            LuaRatelimits::limit(10, Duration::from_secs(75));

        // Send message channel limits (are smaller to allow for more actions)
        let create_message_lim1 =
            LuaRatelimits::limit(30, Duration::from_secs(10));

        // get_original_interaction_response
        let get_original_interaction_response_lim1 =
            LuaRatelimits::limit(10, Duration::from_secs(3));

        // create_followup_message
        let create_followup_message_lim1 =
            LuaRatelimits::limit(10, Duration::from_secs(3));
        
        // get_followup_message
        let get_followup_message_lim1 =
            LuaRatelimits::limit(10, Duration::from_secs(3));

        // edit_followup_message
        let edit_followup_message_lim1 =
            LuaRatelimits::limit(10, Duration::from_secs(3));

        // delete_followup_message
        let delete_followup_message_lim1 =
            LuaRatelimits::limit(10, Duration::from_secs(3));

        // edit_original_interaction_response
        let edit_original_interaction_response_lim1 =
            LuaRatelimits::limit(10, Duration::from_secs(3));

        // delete_original_interaction_response
        let delete_original_interaction_response_lim1 =
            LuaRatelimits::limit(10, Duration::from_secs(3));

        // get_guild_commands
        let get_guild_commands_lim1 =
            LuaRatelimits::limit(1, Duration::from_secs(300));

        // create_guild_command
        let create_guild_command_lim1 =
            LuaRatelimits::limit(1, Duration::from_secs(300));

        // Create the clock
        let clock = QuantaClock::default();

        LuaRatelimits {
            global,
            per_bucket: indexmap::indexmap!(
                "CreateGuildBan" => vec![ban_lim1, ban_lim2] as Vec<DefaultKeyedRateLimiter<()>>,
                "RemoveGuildMember" => vec![kick_lim1, kick_lim2] as Vec<DefaultKeyedRateLimiter<()>>,
                "CreateMessage" => vec![create_message_lim1] as Vec<DefaultKeyedRateLimiter<()>>,
                "GetOriginalInteractionResponse" => vec![get_original_interaction_response_lim1] as Vec<DefaultKeyedRateLimiter<()>>,
                "EditOriginalInteractionResponse" => vec![edit_original_interaction_response_lim1] as Vec<DefaultKeyedRateLimiter<()>>,
                "DeleteOriginalInteractionResponse" => vec![delete_original_interaction_response_lim1] as Vec<DefaultKeyedRateLimiter<()>>,
                "CreateFollowupMessage" => vec![create_followup_message_lim1] as Vec<DefaultKeyedRateLimiter<()>>,
                "GetFollowupMessage" => vec![get_followup_message_lim1] as Vec<DefaultKeyedRateLimiter<()>>,
                "EditFollowupMessage" => vec![edit_followup_message_lim1] as Vec<DefaultKeyedRateLimiter<()>>,
                "DeleteFollowupMessage" => vec![delete_followup_message_lim1] as Vec<DefaultKeyedRateLimiter<()>>,
                "GetGuildCommands" => vec![get_guild_commands_lim1] as Vec<DefaultKeyedRateLimiter<()>>,
                "CreateGuildCommand" => vec![create_guild_command_lim1] as Vec<DefaultKeyedRateLimiter<()>>,
            ),
            clock,
        }
    }

    fn new_object_storage_rl() -> LuaRatelimits {
        // Create the global limit
        let global1 =
            LuaRatelimits::limit(75, Duration::from_secs(1));
        let global = vec![global1];

        // Create the clock
        let clock = QuantaClock::default();

        LuaRatelimits {
            global,
            per_bucket: indexmap::indexmap!(),
            clock,
        }
    }

    fn new_runtime_rl() -> LuaRatelimits {
        // Create the global limit
        let global1 =
            LuaRatelimits::limit(10, Duration::from_secs(1));
        let global = vec![global1];

        // Create the clock
        let clock = QuantaClock::default();

        LuaRatelimits {
            global,
            per_bucket: indexmap::indexmap!(),
            clock,
        }
    }

    fn new_cdn_rl() -> LuaRatelimits {
        // Create the global limit
        let global1 =
            LuaRatelimits::limit(10, Duration::from_secs(1));
        let global = vec![global1];

        // Create the clock
        let clock = QuantaClock::default();

        LuaRatelimits {
            global,
            per_bucket: indexmap::indexmap!(),
            clock,
        }
    }
}

/// Represents the limits for various operations in the worker
pub struct Ratelimits {
    /// Stores the lua discord ratelimiters
    pub discord: LuaRatelimits,

    /// Stores the object storage ratelimiters
    pub object_storage: LuaRatelimits,

    /// Stores the runtime ratelimiters
    pub runtime: LuaRatelimits,

    /// Stores the runtime ratelimiters
    pub cdn: LuaRatelimits,
}

impl Ratelimits {
    pub fn new() -> Self {
        Ratelimits {
            discord: Ratelimits::new_discord_rl(),
            object_storage: Ratelimits::new_object_storage_rl(),
            runtime: Ratelimits::new_runtime_rl(),
            cdn: Ratelimits::new_cdn_rl(),
        }
    }
}
