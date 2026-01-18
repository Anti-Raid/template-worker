use std::sync::Arc;

use serenity::async_trait;
use khronos_runtime::primitives::event::CreateEvent;
use khronos_runtime::utils::khronos_value::KhronosValue;

use crate::worker::workerlike::WorkerLike;
use crate::worker::workerthread::WorkerThread;

use super::workervmmanager::Id;

use super::workerfilter::WorkerFilter;

/// The Poolable trait provides the needed operations a WorkerLike needs to additionally
/// implement to be used in a worker thread pool
pub trait Poolable: WorkerLike + Send + Sync {
    type ExtState: Send + Sync;

    /// Returns a new `Poolable` object given `state`, `filters` and `id`
    fn new(filter: WorkerFilter, id: usize, total: usize, ext_state: &Self::ExtState) -> Result<Self, crate::Error>
    where
        Self: Sized;
}

/// A WorkerPool stores a pool of workers in which servers are evenly distributed via
/// the Discord Id sharding formula:
/// 
/// shard_id = (guild_id >> 22) % num_shards
#[allow(dead_code)]
pub struct WorkerPool<T: WorkerLike> {
    /// The workers in the pool
    workers: Vec<T>,
}

impl<T: Poolable> WorkerPool<T> {
    /// Creates a new WorkerPool with the given cache data and worker state
    pub fn new(num_threads: usize, ext_state: &T::ExtState) -> Result<Self, crate::Error> {
        let mut workers = Vec::with_capacity(num_threads);

        for id in 0..num_threads {
            let filter = Self::filter_for(id, num_threads);
            let thread = T::new(filter, id, num_threads, ext_state)?;
            workers.push(thread);
        }

        Ok(WorkerPool {
            workers,
        })
    }
}

impl<T: WorkerLike> WorkerPool<T> {
    /// Defines a filter for a worker in the pool
    pub fn filter_for(id: usize, num_threads: usize) -> WorkerFilter {
        let closure = move |tenant_id: Id| {
            match tenant_id {
                // This is safe as AntiRaid workers does not currently support 32 bit platforms
                Id::GuildId(guild_id) => ((guild_id.get() >> 22) as usize) % num_threads == id
            }
        };

        WorkerFilter::new(closure)
    }

    /// Returns a reference to the WorkerThread in the pool for a given tenant ID
    pub fn get_worker_for(&self, id: Id) -> &T {
        let index = match id {
            // This is safe as AntiRaid workers does not currently support 32 bit platforms
            Id::GuildId(guild_id) => (guild_id.get() >> 22) as usize % self.workers.len(),
        };
        &self.workers[index]
    }

    /// Returns the number of workers in the pool
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.workers.len()
    }
}

#[async_trait]
impl<T: WorkerLike> WorkerLike for WorkerPool<T> {
    fn id(&self) -> usize {
        0 // For a pool, return 0
    }

    async fn kill(&self) -> Result<(), crate::Error> {
        for worker in &self.workers {
            worker.kill().await?;
        }
        Ok(())
    }

    fn clone_to_arc(&self) -> Arc<dyn WorkerLike + Send + Sync> {
        Arc::new(
            WorkerPool {
                workers: self.workers.iter().map(|w| w.clone_to_arc()).collect(),
            }
        )
    }

    async fn run_script(&self, id: Id, name: String, code: String, event: CreateEvent) -> Result<KhronosValue, crate::Error> {
        self.get_worker_for(id).run_script(id, name, code, event).await
    }

    async fn dispatch_event(&self, id: Id, event: CreateEvent) -> Result<KhronosValue, crate::Error> {
        self.get_worker_for(id).dispatch_event(id, event).await
    }

    fn dispatch_event_nowait(&self, id: Id, event: CreateEvent) -> Result<(), crate::Error> {
        self.get_worker_for(id).dispatch_event_nowait(id, event)
    }

    async fn drop_tenant(&self, id: Id) -> Result<(), crate::Error> {
        self.get_worker_for(id).drop_tenant(id).await
    }

    fn len(&self) -> usize {
        self.workers.len()
    }
}

// Assert that WorkerPool is Send + Sync
const _: () = {
    const fn assert_send_sync_clone<T: Send + Sync>() {}
    assert_send_sync_clone::<WorkerPool<WorkerThread>>();
};
