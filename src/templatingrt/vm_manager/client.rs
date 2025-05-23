//! Client side abstraction for the inner Lua VM

use std::sync::Arc;

use khronos_runtime::primitives::event::CreateEvent;

use crate::templatingrt::template::Template;

use super::threadpool::ThreadPoolLuaHandle;

#[derive(serde::Serialize, serde::Deserialize)]
pub enum LuaVmAction {
    /// Dispatch a template event
    DispatchEvent { event: CreateEvent },
    /// Dispatch a template event to a specific template
    DispatchTemplateEvent {
        event: CreateEvent,
        template_name: String,
    },
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
    /// Clear the cache of all subisolates
    ClearCache {},
    /// Panic. Only useful for testing/debugging
    Panic {},
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
}

impl ArLuaHandle for ArLua {
    fn send_action(
        &self,
        action: LuaVmAction,
        callback: tokio::sync::oneshot::Sender<Vec<(String, LuaVmResult)>>,
    ) -> Result<(), khronos_runtime::Error> {
        match self {
            ArLua::ThreadPool(handle) => handle.send_action(action, callback),
        }
    }
}

/// ArLuaHandle provides a handle to a Lua VM
///
/// Note that the Lua VM is not directly exposed both due to thread safety issues
/// and to allow for multiple VM-thread allocation strategies in vm_manager
pub trait ArLuaHandle: Clone + Send + Sync {
    /// Returns the thread handle for the Lua VM
    fn send_action(
        &self,
        action: LuaVmAction,
        callback: tokio::sync::oneshot::Sender<Vec<(String, LuaVmResult)>>,
    ) -> Result<(), khronos_runtime::Error>;
}
