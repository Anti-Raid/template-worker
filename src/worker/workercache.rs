use super::workercachedata::WorkerCacheData;
use super::workervmmanager::{WorkerVmManager, Id};

/// WorkerCache extends WorkerCacheData with management of the VM during
/// cache regeneration etc. 
/// 
/// A WorkerCache stores both a WorkerCacheData and a WorkerVmManager
pub struct WorkerCache {
    /// The cache data
    data: WorkerCacheData,
    /// The VM manager
    vm_manager: WorkerVmManager,
}

impl WorkerCache {
    /// Creates a new WorkerCache with the given data and VM manager
    fn new(data: WorkerCacheData, vm_manager: WorkerVmManager) -> Self {
        Self { data, vm_manager }
    }

    /// Clears the template cache for a guild. This refetches the templates
    /// into cache
    async fn regenerate_cache(&self, pool: &sqlx::PgPool, id: Id) -> Result<(), crate::Error> {
        self.data.regenerate_templates_for(pool, id).await?;
        self.data.regenerate_key_expiries_for(pool, id).await?;
        self.vm_manager.remove_vm_for(id)?;

        // TODO: Once workers supports resumption keys, dispatch resume keys here
        //super::resume_dispatch::dispatch_resume_keys(context, data, guild_id).await?;

        Ok(())
    }
}