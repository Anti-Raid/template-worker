//! Client side abstraction for the inner Lua VM

use std::{
    sync::{Arc, LazyLock},
    time::Instant,
};

use khronos_runtime::primitives::event::CreateEvent;
use serenity::all::GuildId;

use crate::{
    config::{VmDistributionStrategy, CMD_ARGS},
    templatingrt::template::Template,
};

use super::{
    threadperguild_strategy::{create_lua_vm as create_lua_vm_threadperguild, PerThreadLuaHandle},
    threadpool_strategy::{create_lua_vm as create_lua_vm_threadpool, ThreadPoolLuaHandle},
};

/// VM cache
pub(super) static VMS: LazyLock<scc::HashMap<GuildId, ArLua>> = LazyLock::new(scc::HashMap::new);

#[derive(serde::Serialize, serde::Deserialize)]
pub enum LuaVmAction {
    /// Dispatch a template event
    DispatchEvent { event: CreateEvent },
    /// Dispatch a template event to an inline template
    DispatchInlineEvent {
        event: CreateEvent,
        template: Arc<Template>,
    },
    /// Stop the Lua VM entirely
    Stop {},
    /// Returns the memory usage of the Lua VM
    GetMemoryUsage {},
    /// Set the memory limit of the Lua VM
    SetMemoryLimit { limit: usize },
}

#[derive(Debug)]
pub enum LuaVmResult {
    Ok { result_val: serde_json::Value },
    LuaError { err: String },
    VmBroken {},
}

#[derive(Clone)]
pub enum ArLua {
    ThreadPool(ThreadPoolLuaHandle),
    ThreadPerGuild(PerThreadLuaHandle),
}

impl ArLuaHandle for ArLua {
    fn last_execution_time(&self) -> Instant {
        match self {
            ArLua::ThreadPool(handle) => handle.last_execution_time(),
            ArLua::ThreadPerGuild(handle) => handle.last_execution_time(),
        }
    }

    fn send_action(
        &self,
        action: LuaVmAction,
        callback: tokio::sync::oneshot::Sender<Vec<(String, LuaVmResult)>>,
    ) -> Result<(), khronos_runtime::Error> {
        match self {
            ArLua::ThreadPool(handle) => handle.send_action(action, callback),
            ArLua::ThreadPerGuild(handle) => handle.send_action(action, callback),
        }
    }

    fn broken(&self) -> bool {
        match self {
            ArLua::ThreadPool(handle) => handle.broken(),
            ArLua::ThreadPerGuild(handle) => handle.broken(),
        }
    }

    fn set_broken(&self) {
        match self {
            ArLua::ThreadPool(handle) => handle.set_broken(),
            ArLua::ThreadPerGuild(handle) => handle.set_broken(),
        }
    }
}

/// ArLuaHandle provides a handle to a Lua VM
///
/// Note that the Lua VM is not directly exposed both due to thread safety issues
/// and to allow for multiple VM-thread allocation strategies in vm_manager
pub trait ArLuaHandle: Clone + Send + Sync {
    /// Returns the last execution time of the Lua VM
    fn last_execution_time(&self) -> Instant;

    /// Returns the thread handle for the Lua VM
    fn send_action(
        &self,
        action: LuaVmAction,
        callback: tokio::sync::oneshot::Sender<Vec<(String, LuaVmResult)>>,
    ) -> Result<(), khronos_runtime::Error>;

    /// Returns if the VM is broken
    fn broken(&self) -> bool;

    /// Sets the VM to be broken
    fn set_broken(&self);
}

/// Get a Lua VM for a guild
///
/// This function will either return an existing Lua VM for the guild or create a new one if it does not exist
pub async fn get_lua_vm(
    guild_id: GuildId,
    pool: sqlx::PgPool,
    serenity_context: serenity::all::Context,
    reqwest_client: reqwest::Client,
) -> Result<ArLua, silverpelt::Error> {
    let Some(mut vm) = VMS.get(&guild_id) else {
        let vm = match CMD_ARGS.vm_distribution_strategy {
            VmDistributionStrategy::ThreadPool => {
                create_lua_vm_threadpool(guild_id, pool, serenity_context, reqwest_client).await?
            }
            VmDistributionStrategy::ThreadPerGuild => {
                create_lua_vm_threadperguild(guild_id, pool, serenity_context, reqwest_client)
                    .await?
            }
        };
        if let Err((_, alt_vm)) = VMS.insert_async(guild_id, vm.clone()).await {
            return Ok(alt_vm);
        }
        return Ok(vm);
    };

    if vm.broken() {
        let new_vm = match CMD_ARGS.vm_distribution_strategy {
            VmDistributionStrategy::ThreadPool => {
                create_lua_vm_threadpool(guild_id, pool, serenity_context, reqwest_client).await?
            }
            VmDistributionStrategy::ThreadPerGuild => {
                create_lua_vm_threadperguild(guild_id, pool, serenity_context, reqwest_client)
                    .await?
            }
        };

        *vm = new_vm.clone();
        Ok(new_vm)
    } else {
        Ok(vm.clone())
    }
}
