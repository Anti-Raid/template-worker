use khronos_runtime::rt::{CreatedKhronosContext, KhronosRuntime, KhronosRuntimeInterruptData, KhronosRuntimeManager, RuntimeCreateOpts};
use serenity::all::GuildId;
use std::cell::RefCell;
use std::{cell::Cell, collections::HashMap, rc::Rc};
use khronos_runtime::rt::mlua::prelude::*;

use super::workerstate::WorkerState;
use super::limits::{
    MAX_TEMPLATE_MEMORY_USAGE, MAX_TEMPLATES_EXECUTION_TIME,
};

pub type RuntimeManager = KhronosRuntimeManager<CreatedKhronosContext>;

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
/// Represents the ID of a tenant, which can currently only be a GuildId
pub enum Id {
    GuildId(GuildId)
}

/// Represents the data associated with a VM, which includes the guild state and the Khronos runtime manager
#[derive(Clone)]
pub struct VmData {
    pub state: WorkerState,
    pub runtime_manager: RuntimeManager,
    pub thread_count: Rc<Cell<usize>>,
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
    vms: Rc<RefCell<HashMap<Id, VmData>>>
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
    pub async fn get_vm_for(&self, id: Id) -> LuaResult<VmData> {
        // Check if the VM already exists
        {
            let vms = self.vms.borrow();
            if let Some(vm) = vms.get(&id) {
                return Ok(vm.clone());
            }
        }

        // If it doesn't exist, create a new VM
        //
        // This is safe as `self.vms` should not be borrowed or mutably borrowed
        let runtime_manager = self.configure_runtime_manager(id).await
            .map_err(|e| LuaError::external(e))?;

        let vmd = VmData {
            state: self.worker_state.clone(),
            runtime_manager,
            thread_count: Cell::new(0).into(),
        };

        let mut vm_guard = self.vms.borrow_mut();
        vm_guard.insert(id, vmd.clone());

        Ok(vmd)
    }

    /// Removes the VM for the given tenant ID and cleans up its resources
    pub fn remove_vm_for(&self, id: Id) -> Result<(), crate::Error> {
        let mut vms = self.vms.borrow_mut();
        if let Some(vm) = vms.remove(&id) {
            // If the VM was removed, we can also clean up the runtime manager
            vm.runtime_manager.runtime().mark_broken(true)?;
        }

        Ok(())
    }
    

    /// Returns the number of VMs managed by this WorkerVmManager
    pub fn len(&self) -> usize {
        self.vms.borrow().len()
    }

    /// Returns true if there are no VMs managed by this WorkerVmManager
    pub fn is_empty(&self) -> bool {
        self.vms.borrow().is_empty()
    }

    /// Returns a list of all tenant IDs for which VMs are managed by this WorkerVmManager
    pub fn keys(&self) -> Vec<Id> {
        self.vms.borrow().keys().cloned().collect()
    }

    /// Configures a new khronos runtime manager
    /// 
    /// Panics if `self.vms` is mutably borrowed
    async fn configure_runtime_manager(&self, id: Id) -> LuaResult<RuntimeManager> {
        let mut rt = KhronosRuntime::new(
            RuntimeCreateOpts {
                disable_task_lib: false,
            },
            Some(|_a: &Lua, b: &KhronosRuntimeInterruptData| {
                let Some(last_execution_time) = b.last_execution_time else {
                    return Ok(LuaVmState::Continue);
                };

                if last_execution_time.elapsed() >= MAX_TEMPLATES_EXECUTION_TIME {
                    return Ok(LuaVmState::Yield);
                }

                Ok(LuaVmState::Continue)
            }),
            None::<(fn(&Lua, LuaThread) -> Result<(), LuaError>, fn() -> ())>,
        )
        .await?;

        rt.set_memory_limit(MAX_TEMPLATE_MEMORY_USAGE)?;

        rt.sandbox()?;

        let manager = KhronosRuntimeManager::new(rt);

        {
            let vms_ref = self.vms.clone();
            manager.set_on_broken(Box::new(move || {
                let mut vms = vms_ref.borrow_mut();
                vms.remove(&id);
            }));
        }

        Ok(manager)
    }
}
