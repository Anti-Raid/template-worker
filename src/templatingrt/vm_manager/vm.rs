use std::sync::LazyLock;
use serenity::all::GuildId;
use super::client::ArLua;
use crate::config::{VmDistributionStrategy, CMD_ARGS};
use crate::templatingrt::state::CreateGuildState;
use super::threadpool::create_lua_vm as create_lua_vm_threadpool;

/// VM cache
static VMS: LazyLock<scc::HashMap<GuildId, ArLua>> = LazyLock::new(scc::HashMap::new);

/// Get a Lua VM for a guild
///
/// This function will either return an existing Lua VM for the guild or create a new one if it does not exist
pub async fn get_lua_vm(
    guild_id: GuildId,
    cgs: CreateGuildState
) -> Result<ArLua, silverpelt::Error> {
    let Some(vm) = VMS.get(&guild_id) else {
        let vm = match CMD_ARGS.vm_distribution_strategy {
            VmDistributionStrategy::ThreadPool | VmDistributionStrategy::ThreadPerGuild => {
                create_lua_vm_threadpool(guild_id, cgs).await?
            }
        };
        if let Err((_, alt_vm)) = VMS.insert_async(guild_id, vm.clone()).await {
            return Ok(alt_vm);
        }
        return Ok(vm);
    };

    Ok(vm.clone())
}

pub fn get_lua_vm_if_exists(guild_id: GuildId) -> Option<ArLua> {
    let vm = VMS.get(&guild_id)?;

    Some(vm.clone())
}

/// Removes a vm from the global vm cache
pub fn remove_vm(guild_id: GuildId) {
    VMS.remove(&guild_id);
}