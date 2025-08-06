use crate::worker::workerstate::WorkerState;

use super::workervmmanager::WorkerVmManager;
use super::workerdispatch::WorkerDispatch;
use super::workercachedata::WorkerCacheData;
use super::workerdb::WorkerDB;

/// Worker provides a full easy-to-develop-on structure including VM management, 
/// event dispatching, and caching for Luau templates in AntiRaid.
pub struct Worker {
    /// The VM manager
    pub vm_manager: WorkerVmManager,
    /// The event dispatcher
    pub dispatch: WorkerDispatch,
    /// Worker DB
    pub db: WorkerDB,
}

impl Worker {
    pub fn new(
        cache: WorkerCacheData, // The cache data, this can be shared across workers if needed (e.g. threadpool worker)
        state: WorkerState,
    ) -> Self {
        let db = cache.db().clone();
        let vm_manager = WorkerVmManager::new(state.clone());
        let dispatch = WorkerDispatch::new(vm_manager.clone(), state, cache, db.clone());

        Self {
            vm_manager,
            dispatch,
            db,
        }
    }
}