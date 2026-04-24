use dapi::controller::DiscordProviderContext;
use khronos_runtime::core::typesext::Vfs;
use khronos_runtime::primitives::syscall::RawSyscall;
use khronos_runtime::rt::{KhronosRuntime, RuntimeCreateOpts};
use serde::{Deserialize, Serialize};
use serenity::all::{GuildId, UserId};
use stratum_common::worker_id_for_tenant;
use std::cell::RefCell;
use std::sync::Arc;
use std::{collections::HashMap, rc::Rc};
use khronos_runtime::rt::mlua::prelude::*;
use crate::worker::builtins::{Builtins, TemplatingTypes};
use crate::worker::limits::TEMPLATE_GIVE_TIME;
use crate::worker::syscall::SyscallHandler;
use crate::worker::workertenantstate::WorkerTenantState;

use super::limits::Ratelimits;

use super::workerstate::WorkerState;
use super::limits::{MAX_TEMPLATE_MEMORY_USAGE, MAX_TEMPLATES_EXECUTION_TIME};

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, Serialize, Deserialize)]
/// Represents the ID of a tenant, which can currently only be a GuildId
#[serde(tag = "type", content = "id")]
pub enum Id {
    Guild(GuildId),
    User(UserId), // User-owned VMs (user-installed apps etc.)
}

impl Id {
    /// Returns the tenant id
    pub fn tenant_id(&self) -> String {
        match self {
            Id::Guild(guild_id) => guild_id.to_string(),
            Id::User(user_id) => user_id.to_string(),
        }
    }

    /// Returns the tenant type
    pub fn tenant_type(&self) -> String {
        match self {
            Id::Guild(_) => "guild".to_string(),
            Id::User(_) => "user".to_string(),
        }
    }

    /// Create a new Id from type/id pair
    pub fn from_parts(tenant_type: &str, tenant_id: &str) -> Option<Self> {
        match tenant_type {
            "guild" => {
                let Some(gid) = tenant_id.parse::<GuildId>().ok() else {
                    return None;
                };
                Some(Id::Guild(gid))
            },
            "user" => {
                let Some(uid) = tenant_id.parse::<UserId>().ok() else {
                    return None;
                };
                Some(Id::User(uid))
            },
            _ => None
        }
    }

    /// Converts an Id into a khronos DiscordProviderContext
    pub fn to_provider_context(self) -> DiscordProviderContext {
        match self {
            Id::Guild(guild_id) => DiscordProviderContext::Guild(guild_id),
            Id::User(user_id) => DiscordProviderContext::User(user_id),
        }
    }

    /// Returns a the worker ID given tenant ID
    pub fn worker_id(&self, num_workers: usize) -> usize {
        match self {
            // This is safe as AntiRaid workers does not currently support 32 bit platforms
            Id::Guild(guild_id) =>  worker_id_for_tenant(guild_id.get(), num_workers),
            Id::User(user_id) => worker_id_for_tenant(user_id.get(), num_workers),
        }
    }
}

impl FromLua for Id {
    fn from_lua(value: LuaValue, _lua: &Lua) -> LuaResult<Self> {
        match value {
            LuaValue::Table(table) => {
                let tenant_type: String = table.get("tenant_type")?;
                let tenant_id: String = table.get("tenant_id")?;
                let Some(id) = Id::from_parts(&tenant_type, &tenant_id) else {
                    return Err(LuaError::external(format!("Failed to parse Id from tenant_type: {}, tenant_id: {}", tenant_type, tenant_id)));
                };
                Ok(id)
            }
            _ => {
                Err(LuaError::FromLuaConversionError {
                    from: value.type_name(),
                    to: "Id".to_string(),
                    message: Some("Expected a table representing an Id".to_string()),
                })
            }
        }
    }
}

impl IntoLua for Id {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        let table = lua.create_table_with_capacity(0, 2)?;
        table.set("tenant_type", self.tenant_type())?;
        table.set("tenant_id", self.tenant_id())?;
        table.set_readonly(true);
        Ok(LuaValue::Table(table))
    }
}

/// Represents the vmdata and the dispatch function as well
#[derive(Clone)]
pub struct VmState {
    pub runtime: KhronosRuntime,
    pub dispatch_func: LuaFunction
}

struct BaseTenantData<'a> {
    bot: Arc<serenity::all::CurrentUser>,
    id: Id,
    dispatchable_events: &'a [&'static str],
    base_vfs: &'a HashMap<String, Vfs>,
    support_server: &'a str,
    website: &'a str
}

impl<'a> IntoLua for BaseTenantData<'a> {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        let table = lua.create_table_with_capacity(0, 2)?;
        table.set("bot", lua.to_value(&self.bot)?)?;
        table.set("id", self.id)?;
        table.set("dispatchable_events", self.dispatchable_events)?;
        
        let base_vfs_tab = lua.create_table_with_capacity(0, self.base_vfs.len())?;
        for (s, vfs) in self.base_vfs {
            base_vfs_tab.set(s.as_str(), vfs.clone())?;
        }
        base_vfs_tab.set_readonly(true);
        table.set("base_vfs", base_vfs_tab)?;
        table.set("support_server", self.support_server)?;
        table.set("website", self.website)?;
        table.set_readonly(true);
        Ok(LuaValue::Table(table))
    }
}

/// A WorkerVmManager manages the state and VMs for a worker
/// 
/// # Notes
/// 
/// 1. A WorkerVmManager is *not* thread safe
/// 2. A WorkerVmManager only manages the VMs for a single worker and nothing more 
#[derive(Clone)]
pub struct WorkerVmManager {
    /// The VMs managed by this WorkerVmManager, keyed by their tenant ID
    vms: Rc<RefCell<HashMap<Id, VmState>>>,
}

impl WorkerVmManager {
    /// Creates a new WorkerVmManager with the given worker state
    pub fn new() -> Self {
        Self {
            vms: RefCell::default().into()
        }
    }

    /// Returns the VM for the given tenant ID creating it if needed
    pub fn get_vm_for(&self, id: Id, worker_state: &WorkerState, wts: &WorkerTenantState) -> LuaResult<VmState> {
        let mut vms = self.vms.borrow_mut();
        if let Some(vm) = vms.get(&id) {
            return Ok(vm.clone());
        }

        let vm = self.create_vm(id, worker_state.clone(), wts.clone())?;
        vms.insert(id, vm.clone());

        Ok(vm)
    }

    /// Creates a new VmData
    fn create_vm(&self, id: Id, worker_state: WorkerState, wts: WorkerTenantState) -> LuaResult<VmState> {
        // If it doesn't exist, create a new VM
        let runtime = self.configure_runtime(&worker_state)
            .map_err(|e| LuaError::external(e))?;

        let func: LuaFunction = runtime
        .eval_script("./builtins.templateloop")?;

        let tenant_state = wts.get_cached_tenant_state_for(id)
            .map_err(|e| LuaError::external(format!("Failed to get tenant state for ID {id:?}: {e}")))?;
        let lts = BaseTenantData { 
            id, 
            bot: worker_state.current_user.clone(), 
            dispatchable_events: &dapi::EVENT_LIST, 
            base_vfs: &super::builtins::EXPOSED_VFS,
            support_server: &crate::CONFIG.meta.support_server_invite,
            website: &crate::CONFIG.sites.frontend
        };

        let syscall_h = SyscallHandler::new(
            worker_state,
            wts,
            Ratelimits::new().map_err(|e| LuaError::external(e.to_string()))?.into(),
            id
        );

        let dispatch_func = func.call::<LuaFunction>((RawSyscall::new(syscall_h), tenant_state, lts))?;

        Ok(VmState {
            runtime,
            dispatch_func
        })
    }

    /// Removes the VM for the given tenant ID and cleans up its resources
    #[allow(dead_code)]
    pub fn remove_vm_for(&self, id: Id) -> Result<(), crate::Error> {
        // Remove from map
        let cell_opt = self.vms.borrow_mut().remove(&id);

        // If it existed and was initialized, mark it broken
        if let Some(vm) = cell_opt {
            vm.runtime.mark_broken(true)?;
        }
        
        Ok(())
    }
    
    /// Returns the number of VMs managed by this WorkerVmManager
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.vms.borrow().len()
    }

    /// Returns true if there are no VMs managed by this WorkerVmManager
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.vms.borrow().is_empty()
    }

    /// Returns a list of all tenant IDs for which VMs are managed by this WorkerVmManager
    #[allow(dead_code)]
    pub fn keys(&self) -> Vec<Id> {
        self.vms.borrow().keys().cloned().collect()
    }

    /// Configures a new khronos runtime
    fn configure_runtime(&self, worker_state: &WorkerState) -> LuaResult<KhronosRuntime> {
        let rt = KhronosRuntime::new(
            RuntimeCreateOpts {
                disable_task_lib: false,
                time_limit: Some(MAX_TEMPLATES_EXECUTION_TIME),
                give_time: TEMPLATE_GIVE_TIME
            },
            None::<(fn(&Lua, LuaThread) -> Result<(), LuaError>, fn(LuaLightUserData) -> ())>,
            // We start with builtins *always* as the root template, the builtins root template then spawns in all other templates to dispatch
            // automatically from within luau (which is a lot easier + maintainable and allows for custom events etc.)
            vfs::OverlayFS::new(&vec![
                vfs::EmbeddedFS::<Builtins>::new().into(),
                vfs::EmbeddedFS::<TemplatingTypes>::new().into(),
            ]),
            "antiraid"
        )?;

        rt.set_memory_limit(MAX_TEMPLATE_MEMORY_USAGE)?;

        if worker_state.worker_print {
            let gtab = rt.global_table().clone();
            rt.with_lua(|lua| {
                gtab.set("_debug", lua.create_function(|_, values: LuaMultiValue| {
                    khronos_runtime::utils::pp::pretty_print(values);

                    Ok(())
                })?)?;
                
                Ok(())
            })?;
        }

        Ok(rt)
    }
}
