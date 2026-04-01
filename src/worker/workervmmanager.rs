use dapi::controller::DiscordProviderContext;
use khronos_runtime::rt::{KhronosRuntime, RuntimeCreateOpts};
use serde::{Deserialize, Serialize};
use serenity::all::{GuildId, UserId};
use std::cell::RefCell;
use std::sync::Arc;
use std::{collections::HashMap, rc::Rc};
use khronos_runtime::rt::mlua::prelude::*;
use crate::worker::builtins::{Builtins, TemplatingTypes};
use crate::worker::limits::TEMPLATE_GIVE_TIME;
use crate::worker::vmcontext::TemplateContextProvider;
use crate::worker::workertenantstate::WorkerTenantState;

use super::limits::{LuaKVConstraints, Ratelimits};

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
            Id::Guild(guild_id) => (guild_id.get() >> 22) as usize % num_workers,
            // TODO: Come up with a potentially better sharding formula for user IDs
            // or just use 0 always (what discord does for DMs)
            Id::User(user_id) => (user_id.get() >> 22) as usize % num_workers,
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

/// Represents the data associated with a VM, which includes the guild state and the Khronos runtime manager
#[derive(Clone)]
pub struct VmData {
    pub state: WorkerState,
    pub runtime: KhronosRuntime,
    pub kv_constraints: LuaKVConstraints,
    pub ratelimits: Arc<Ratelimits>,
}

/// Represents the vmdata and the dispatch function as well
#[derive(Clone)]
pub struct VmState {
    pub data: VmData,
    pub dispatch_func: LuaFunction
}

struct BaseTenantData<'a> {
    bot: &'a serenity::all::CurrentUser,
    id: Id
}

impl<'a> IntoLua for BaseTenantData<'a> {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        let table = lua.create_table_with_capacity(0, 2)?;
        table.set("bot", lua.to_value(&self.bot)?)?;
        table.set("id", self.id)?;
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
    /// The state all VMs in the WorkerVmManager share
    worker_state: WorkerState,
    /// The VMs managed by this WorkerVmManager, keyed by their tenant ID
    vms: Rc<RefCell<HashMap<Id, VmState>>>,
}

impl WorkerVmManager {
    /// Creates a new WorkerVmManager with the given worker state
    pub fn new(worker_state: WorkerState) -> Self {
        Self {
            worker_state,
            vms: RefCell::default().into()
        }
    }

    /// Returns the VM for the given tenant ID creating it if needed
    pub fn get_vm_for(&self, id: Id, wts: &WorkerTenantState) -> LuaResult<VmState> {
        let mut vms = self.vms.borrow_mut();
        if let Some(vm) = vms.get(&id) {
            return Ok(vm.clone());
        }

        let vm = self.create_vm(id, wts.clone())?;
        vms.insert(id, vm.clone());

        Ok(vm)
    }

    /// Creates a new VmData
    fn create_vm(&self, id: Id, wts: WorkerTenantState) -> LuaResult<VmState> {
        // If it doesn't exist, create a new VM
        let runtime = self.configure_runtime()
            .map_err(|e| LuaError::external(e))?;

        let vmd = VmData {
            state: self.worker_state.clone(),
            runtime,
            kv_constraints: LuaKVConstraints::default(),
            ratelimits: Ratelimits::new().map_err(|e| LuaError::external(e.to_string()))?.into(),
        };

        let func: LuaFunction = vmd
        .runtime
        .eval_script("./builtins.templateloop")?;

        let provider = TemplateContextProvider::new(
            id,
            vmd.clone(),
            wts.clone()
        );
        let context = vmd.runtime.create_context(provider)?;

        let tenant_state = wts.get_cached_tenant_state_for(id)
            .map_err(|e| LuaError::external(format!("Failed to get tenant state for ID {id:?}: {e}")))?;
        let lts = BaseTenantData { id, bot: &vmd.state.current_user };

        let dispatch_func = func.call::<LuaFunction>((context, tenant_state, lts))?;

        Ok(VmState {
            data: vmd,
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
            vm.data.runtime.mark_broken(true)?;
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
    fn configure_runtime(&self) -> LuaResult<KhronosRuntime> {
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

        if self.worker_state.worker_print {
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
