use khronos_runtime::rt::{KhronosRuntime, RuntimeCreateOpts};
use serde::{Deserialize, Serialize};
use serenity::all::GuildId;
use std::cell::RefCell;
use std::{collections::HashMap, rc::Rc};
use khronos_runtime::rt::mlua::prelude::*;
use crate::worker::builtins::{Builtins, BuiltinsPatches, TemplatingTypes};
use crate::worker::limits::TEMPLATE_GIVE_TIME;

use super::limits::{LuaKVConstraints, Ratelimits};
use tokio::sync::OnceCell;

use super::workerstate::WorkerState;
use super::limits::{MAX_TEMPLATE_MEMORY_USAGE, MAX_TEMPLATES_EXECUTION_TIME};

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, Serialize, Deserialize)]
/// Represents the ID of a tenant, which can currently only be a GuildId
pub enum Id {
    GuildId(GuildId)
}

impl Id {
    pub fn tenant_type(&self) -> &'static str {
        match self {
            Id::GuildId(_) => "guild",
        }
    }

    pub fn tenant_id(&self) -> String {
        match self {
            Id::GuildId(gid) => gid.to_string(),
        }
    }
}

/// Represents the data associated with a VM, which includes the guild state and the Khronos runtime manager
#[derive(Clone)]
pub struct VmData {
    pub state: WorkerState,
    pub runtime: KhronosRuntime,
    pub kv_constraints: LuaKVConstraints,
    pub ratelimits: Rc<Ratelimits>,
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
    vms: Rc<RefCell<HashMap<Id, Rc<OnceCell<VmData>>>>>
}

impl WorkerVmManager {
    /// Creates a new WorkerVmManager with the given worker state
    pub fn new(worker_state: WorkerState) -> Self {
        Self {
            worker_state,
            vms: RefCell::default().into()
        }
    }

    /// Returns the underlying worker state
    pub fn worker_state(&self) -> &WorkerState {
        &self.worker_state
    }

    /// Returns the VM for the given tenant ID creating it if needed
    pub async fn get_vm_for(&self, id: Id) -> LuaResult<VmData> {
        let cell = {
            let mut vms = self.vms.borrow_mut();
            vms.entry(id)
                .or_insert_with(|| Rc::new(OnceCell::new()))
                .clone()
        };

        // Initialize if empty, or wait for the existing initialization to finish
        // get_or_try_init handles the concurrent locking automatically.
        let result = cell.get_or_try_init(|| async {
            self.create_vm().await
        }).await;

        match result {
            Ok(vm) => Ok(vm.clone()),
            Err(e) => {
                // If creation failed, remove the empty/failed cell so we can retry later
                self.vms.borrow_mut().remove(&id);
                Err(e)
            }
        }
    }

    /// Creates a new VmData
    async fn create_vm(&self) -> LuaResult<VmData> {
        // If it doesn't exist, create a new VM
        let runtime = Self::configure_runtime().await
            .map_err(|e| LuaError::external(e))?;

        let vmd = VmData {
            state: self.worker_state.clone(),
            runtime,
            kv_constraints: LuaKVConstraints::default(),
            ratelimits: Ratelimits::new().map_err(|e| LuaError::external(e.to_string()))?.into(),
        };

        Ok(vmd)
    }

    /// Removes the VM for the given tenant ID and cleans up its resources
    #[allow(dead_code)]
    pub fn remove_vm_for(&self, id: Id) -> Result<(), crate::Error> {
        // Remove from map
        let cell_opt = self.vms.borrow_mut().remove(&id);

        // If it existed and was initialized, mark it broken
        if let Some(cell) = cell_opt {
            if let Some(vmd) = cell.get() {
                vmd.runtime.mark_broken(true)?;
            }
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
    async fn configure_runtime() -> LuaResult<KhronosRuntime> {
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
                vfs::EmbeddedFS::<BuiltinsPatches>::new().into(),
                vfs::EmbeddedFS::<Builtins>::new().into(),
                vfs::EmbeddedFS::<TemplatingTypes>::new().into(),
            ])
        )
        .await?;

        rt.set_memory_limit(MAX_TEMPLATE_MEMORY_USAGE)?;
        Ok(rt)
    }
}
