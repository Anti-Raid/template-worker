use crate::worker::workerstate::WorkerState;

use super::workervmmanager::WorkerVmManager;
use super::workerdispatch::WorkerDispatch;

/// Worker provides a full easy-to-develop-on structure including VM management, 
/// event dispatching, and caching for Luau templates in AntiRaid.
pub struct Worker {
    /// The VM manager
    pub vm_manager: WorkerVmManager,
    /// The event dispatcher
    pub dispatch: WorkerDispatch,
}

impl Worker {
    pub fn new(
        state: WorkerState,
    ) -> Self {        
        let vm_manager = WorkerVmManager::new(state.clone());
                
        // This will automatically fire key resumption tasks to all keys with resume flag upon creation
        // of this structure (in addition to providing dispatch services)
        let dispatch = WorkerDispatch::new(vm_manager.clone());

        Self {
            vm_manager,
            dispatch,
        }
    }
}