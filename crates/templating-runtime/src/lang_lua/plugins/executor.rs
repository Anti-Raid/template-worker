use mlua::prelude::*;
use serenity::all::GuildId;

use crate::TemplateContextRef;

#[derive(Default, Clone, Copy)]
pub enum ExecutorScope {
    #[default]
    /// The originating guild
    ThisGuild,
    /// The guild that owns the template on the shop (only available in shop templates, on non-shop templates this will be the same as Guild)
    OwnerGuild,
}

impl std::str::FromStr for ExecutorScope {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "this_guild" => Ok(ExecutorScope::ThisGuild),
            "owner_guild" => Ok(ExecutorScope::OwnerGuild),
            _ => Err("invalid scope, must be one of 'this_guild' or 'owner_guild'".to_string()),
        }
    }
}

impl std::fmt::Display for ExecutorScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecutorScope::ThisGuild => write!(f, "this_guild"),
            ExecutorScope::OwnerGuild => write!(f, "owner_guild"),
        }
    }
}

impl ExecutorScope {
    pub fn scope_str(scope: Option<String>) -> LuaResult<Self> {
        match scope {
            Some(scope) => scope.parse().map_err(LuaError::external),
            None => Ok(ExecutorScope::ThisGuild),
        }
    }

    pub fn guild(&self, token: &TemplateContextRef) -> GuildId {
        match self {
            ExecutorScope::ThisGuild => token.guild_state.guild_id,
            ExecutorScope::OwnerGuild => token
                .template_data
                .shop_owner
                .unwrap_or(token.guild_state.guild_id),
        }
    }
}
