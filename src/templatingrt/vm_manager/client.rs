//! Client side abstraction for the inner Lua VM

use crate::templatingrt::template::Template;
use khronos_runtime::primitives::event::CreateEvent;
use std::sync::Arc;

#[derive(serde::Serialize, serde::Deserialize)]
pub enum LuaVmAction {
    // tells what action the thread should apply to the guild
    /// Dispatch a template event to all templates
    /// template is a script that can be run on a server based on events
    DispatchEvent {
        event: CreateEvent,
        templates: Vec<Arc<Template>>,
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
    /// Clear the cache of all subisolates (isloate -> own environment/global state in same luau vm)
    /// Each server has a khronos runtime to manage luau vm; each runtime is
    /// split into multiple subisolates where every template gets it's own subisolate
    /// (isolated env -> can't access variables across vm's)
    ClearCache {},
    /// Panic. Only useful for testing/debugging
    Panic {},
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum LuaVmResult {
    Ok { result_val: serde_json::Value }, // any result can be a json enum
    LuaError { err: String },
    VmBroken {},
}
