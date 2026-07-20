use crate::worker::workerstate::WorkerState;
use crate::worker::workertenantstate::WorkerTenantState;

use super::workervmmanager::WorkerVmManager;
use super::workerdispatch::WorkerDispatch;

/// Internal worker struct
pub struct Worker {
    /// The VM manager
    pub vm_manager: WorkerVmManager,
    /// The event dispatcher
    pub dispatch: WorkerDispatch,
    /// Worker tenant state manager
    pub wts: WorkerTenantState,
}

#[derive(Default, Clone)]
pub struct WorkerFastPath {

}

impl Worker {
    pub async fn new(state: WorkerState) -> Result<Self, crate::Error> {        
        let vm_manager = WorkerVmManager::new();
        let wts = WorkerTenantState::new(state.mesophyll_client.clone(), vm_manager.clone()).await?;
                
        // This will automatically fire key resumption tasks to all keys with resume flag upon creation
        // of this structure (in addition to providing dispatch services)
        let dispatch = WorkerDispatch::new(vm_manager.clone(), state, wts.clone());

        Ok(Self {
            vm_manager,
            dispatch,
            wts
        })
    }
}