use std::ops::Deref;

use super::workercachedata::WorkerCacheData;
use super::workervmmanager::{WorkerVmManager, Id};
use super::workerdispatch::WorkerDispatch;

/// WorkerCache extends WorkerCacheData with management of the VM during
/// cache regeneration etc. 
/// 
/// A WorkerCache stores both a WorkerCacheData and a WorkerVmManager
#[derive(Clone)]
pub struct WorkerCache {
    /// The cache data
    data: WorkerCacheData,
    /// The VM manager
    vm_manager: WorkerVmManager,
    /// Worker Dispatch (needed for dispatching resume keys)
    dispatch: WorkerDispatch,
}

impl Deref for WorkerCache {
    type Target = WorkerCacheData;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl WorkerCache {
    /// Creates a new WorkerCache with the given data and VM manager
    pub fn new(data: WorkerCacheData, vm_manager: WorkerVmManager, dispatch: WorkerDispatch) -> Self {
        Self { data, vm_manager, dispatch }
    }

    /// Clears the template cache for a guild. This refetches the templates
    /// into cache
    pub async fn regenerate_cache(&self, pool: &sqlx::PgPool, id: Id) -> Result<(), crate::Error> {
        self.data.regenerate_templates_for(pool, id).await?; // Regenerate templates
        self.data.regenerate_key_expiries_for(pool, id).await?; // Regenerate key expiries too
        self.vm_manager.remove_vm_for(id)?; // Remove the VM to force recreation 
        self.dispatch.dispatch_resume_keys(id).await?; // Dispatch resume keys after reload

        Ok(())
    }
}