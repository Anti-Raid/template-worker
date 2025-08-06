use crate::worker::workerstate::WorkerState;

use super::workervmmanager::WorkerVmManager;
use super::workerdispatch::WorkerDispatch;
use super::workercachedata::WorkerCacheData;
use super::workerdb::WorkerDB;
use super::workerfilter::WorkerFilter;
use super::keyexpirychannel::KeyExpiryChannel;
use super::keyexpiry::WorkerKeyExpiry;

/// Worker provides a full easy-to-develop-on structure including VM management, 
/// event dispatching, and caching for Luau templates in AntiRaid.
pub struct Worker {
    /// The VM manager
    pub vm_manager: WorkerVmManager,
    /// The event dispatcher
    pub dispatch: WorkerDispatch,
    /// Worker DB
    pub db: WorkerDB,
    /// Worker filter
    pub filter: WorkerFilter,
    /// Worker key expiry channel
    pub key_expiry_chan: KeyExpiryChannel,
    /// Key expiry event dispatch task
    pub key_expiry_task: WorkerKeyExpiry,
}

impl Worker {
    pub fn new(
        cache: WorkerCacheData, // The cache data, this can be shared across workers if needed (e.g. threadpool worker)
        state: WorkerState,
        filter: WorkerFilter, // The worker filter, used to filter automatically dispatched events based on tenant ID and worker ID
    ) -> Self {
        let db = cache.db().clone();
        let vm_manager = WorkerVmManager::new(state.clone());
        
        // This will automatically start a channel that will dispatch out key expiry notices to subscribed
        // tasks when a key expires 
        let key_expiry_chan = KeyExpiryChannel::new(cache.clone(), filter.clone());
        
        // This will automatically fire key resumption tasks to all keys with resume flag upon creation
        // of this structure
        let dispatch = WorkerDispatch::new(vm_manager.clone(), state, cache.clone(), db.clone(), key_expiry_chan.clone(), filter.clone());

        // This will automatically start a task to handle expiring keys
        let key_expiry_task = WorkerKeyExpiry::new(cache, dispatch.clone(), key_expiry_chan.clone());

        Self {
            vm_manager,
            dispatch,
            db,
            filter,
            key_expiry_chan,
            key_expiry_task
        }
    }
}