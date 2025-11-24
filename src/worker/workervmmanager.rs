use khronos_runtime::rt::{KhronosRuntime, RuntimeCreateOpts};
use serde::{Deserialize, Serialize};
use serenity::all::GuildId;
use std::cell::RefCell;
use std::{collections::HashMap, rc::Rc};
use khronos_runtime::rt::mlua::prelude::*;
use crate::worker::limits::TEMPLATE_GIVE_TIME;
use crate::worker::vmisolatemanager::VmIsolateManager;

use super::limits::{LuaKVConstraints, Ratelimits};
use tokio::sync::broadcast::{channel as broadcast_channel, WeakSender as BroadcastWeakSender, Sender as BroadcastSender};

use super::workerstate::WorkerState;
use super::limits::{MAX_TEMPLATE_MEMORY_USAGE, MAX_TEMPLATES_EXECUTION_TIME};

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, Serialize, Deserialize)]
/// Represents the ID of a tenant, which can currently only be a GuildId
pub enum Id {
    GuildId(GuildId)
}

/// Represents the data associated with a VM, which includes the guild state and the Khronos runtime manager
#[derive(Clone)]
pub struct VmData {
    pub state: WorkerState,
    pub runtime_manager: VmIsolateManager,
    pub kv_constraints: LuaKVConstraints,
    pub ratelimits: Rc<Ratelimits>,
}

/// If multiple calls to get_vm_for happen at the same time, we want to ensure that only one VM is created
/// 
/// This is used to track the state of the VM creation process
#[derive(Clone)]
enum VmDataState {
    Created(VmData),
    Creating(BroadcastWeakSender<()>),
}

/// A handle to the VM creation process
/// This is used to notify waiters when the VM creation has finished
struct CreatingVmDataHandle {
    tx: BroadcastSender<()>,
}

impl Drop for CreatingVmDataHandle {
    fn drop(&mut self) {
        // When the handle is dropped, we notify all waiters that the VM creation has finished
        let _ = self.tx.send(());
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
    vms: Rc<RefCell<HashMap<Id, VmDataState>>>
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
        loop {
            let vm = {
                let vms = self.vms.borrow();
                vms.get(&id).cloned()
            }; // At this point, self.vm's is no longer borrowed

            match vm {
                Some(VmDataState::Created(vm_data)) => return Ok(vm_data),
                Some(VmDataState::Creating(tx)) => {
                    let mut rx = {
                        let Some(strong_tx) = tx.upgrade() else {
                            // If the channel has been dropped, we need to retry creating the VM
                            {
                                let mut vms = self.vms.borrow_mut();
                                vms.remove(&id); // Remove the VM if the channel is closed
                            }
                            continue;
                        };
                        let rx = strong_tx.subscribe();
                        drop(tx); // Drop the Sender handle
                        rx
                    };

                    if rx.is_closed() {
                        // Retry if the channel is closed (failed to create the VM)
                        {
                            let mut vms = self.vms.borrow_mut();
                            vms.remove(&id); // Remove the VM if the channel is closed
                        }
                        continue;
                    }

                    // If it's being created, we should wait for it to be created
                    let _ = rx.recv().await;

                    continue; // Retry to get the VM after it has been created
                }
                None => {
                    // If it doesn't exist, we need to create it
                    let (tx, _) = broadcast_channel(1);

                    {
                        let mut vms = self.vms.borrow_mut();
                        vms.insert(id, VmDataState::Creating(tx.downgrade()));
                    }

                    {
                        let _handle = CreatingVmDataHandle { tx };

                        match self.create_vm_for(id).await {
                            Ok(vmd) => {
                                {
                                    let mut vm_guard = self.vms.borrow_mut();
                                    vm_guard.insert(id, VmDataState::Created(vmd.clone()));
                                }
                                return Ok(vmd);
                            }
                            Err(e) => {
                                // If creation failed, remove the entry and notify waiters
                                {
                                    let mut vm_guard = self.vms.borrow_mut();
                                    vm_guard.remove(&id);
                                }
                                return Err(e);
                            }
                        } // _handle should be dropped here
                    }
                }
            }
        }
    }

    /// Creates a new VM for the given tenant ID
    /// 
    /// Note that this does not store the VM in the vms map, it only creates it
    async fn create_vm_for(&self, id: Id) -> LuaResult<VmData> {
        // If it doesn't exist, create a new VM
        let runtime_manager = self.configure_runtime_manager(id).await
            .map_err(|e| LuaError::external(e))?;

        let vmd = VmData {
            state: self.worker_state.clone(),
            runtime_manager,
            kv_constraints: LuaKVConstraints::default(),
            ratelimits: Ratelimits::new().map_err(|e| LuaError::external(e.to_string()))?.into(),
        };

        Ok(vmd)
    }

    /// Removes the VM for the given tenant ID and cleans up its resources
    pub fn remove_vm_for(&self, id: Id) -> Result<(), crate::Error> {
        let runtime_manager = {
            let mut vms = self.vms.borrow_mut();
            let removed = vms.remove(&id);

            match removed {
                Some(vm) => {
                    match vm {
                        VmDataState::Created(vmd) => vmd.runtime_manager,
                        VmDataState::Creating(_) => return Err("Cannot remove a VM that is being created".into()),
                    }
                },
                None => return Ok(()), // VM doesn't exist, nothing to do
            }
        }; // VM is no longer borrowed here 

        // If the VM was removed, we can also clean up the runtime manager
        //
        // This is safe as `self.vms` should not be borrowed or mutably borrowed at this point
        runtime_manager.runtime().mark_broken(true)?;

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

    /// Configures a new khronos runtime manager
    /// 
    /// Panics if `self.vms` is mutably borrowed
    async fn configure_runtime_manager(&self, id: Id) -> LuaResult<VmIsolateManager> {
        let mut rt = KhronosRuntime::new(
            RuntimeCreateOpts {
                disable_task_lib: false,
                time_limit: Some(MAX_TEMPLATES_EXECUTION_TIME),
                give_time: TEMPLATE_GIVE_TIME
            },
            None::<(fn(&Lua, LuaThread) -> Result<(), LuaError>, fn() -> ())>,
        )
        .await?;

        rt.set_memory_limit(MAX_TEMPLATE_MEMORY_USAGE)?;

        rt.sandbox()?;

        let manager = VmIsolateManager::new(rt);

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
