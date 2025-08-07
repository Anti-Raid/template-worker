use khronos_runtime::primitives::event::CreateEvent;

use crate::worker::workerdispatch::DispatchTemplateResult;
use crate::worker::workerlike::WorkerLike;
use crate::worker::workerthread::DispatchEvent;

use super::workervmmanager::Id;

use super::workerstate::WorkerState;
use super::workercachedata::WorkerCacheData;
use super::workerthread::WorkerThread;
use super::workerfilter::WorkerFilter;

/// A WorkerThreadPool stores a pool of worker threads in which servers are evenly distributed via
/// the Discord Id sharding formula:
/// 
/// shard_id = (guild_id >> 22) % num_shards
pub struct WorkerThreadPool {
    /// The threads in the pool
    threads: Vec<WorkerThread>,
    /// The cache data shared between the threads
    cache: WorkerCacheData, 
    /// The worker state shared between the threads
    state: WorkerState
}

impl WorkerThreadPool {
    /// Creates a new WorkerThread with the given cache data and worker state
    pub fn new(cache: WorkerCacheData, state: WorkerState, num_threads: usize) -> Result<Self, crate::Error> {
        let mut threads = Vec::with_capacity(num_threads);

        for id in 0..num_threads {
            let filter = Self::filter_for(id, num_threads);
            let thread = WorkerThread::new(cache.clone(), state.clone(), filter, id)?;
            threads.push(thread);
        }

        Ok(WorkerThreadPool {
            threads,
            cache,
            state,
        })
    }

    /// Defines a filter for a worker thread
    fn filter_for(id: usize, num_threads: usize) -> WorkerFilter {
        let closure = move |tenant_id: Id| {
            match tenant_id {
                // This is safe as AntiRaid workers does not currently support 32 bit platforms
                Id::GuildId(guild_id) => ((guild_id.get() >> 22) as usize) % num_threads == id
            }
        };

        WorkerFilter::new(closure)
    }

    /// Returns a reference to the WorkerThread for a given tenant ID
    pub fn get_thread_for(&self, id: Id) -> &WorkerThread {
        let index = match id {
            // This is safe as AntiRaid workers does not currently support 32 bit platforms
            Id::GuildId(guild_id) => (guild_id.get() >> 22) as usize % self.threads.len(),
        };
        &self.threads[index]
    }
}

#[async_trait::async_trait]
impl WorkerLike for WorkerThreadPool {
    async fn dispatch_event_to_templates(&self, id: Id, event: CreateEvent) -> DispatchTemplateResult {
        self.get_thread_for(id).send(DispatchEvent {
            id,
            event,
            scopes: None,
        }).await?
    }

    async fn dispatch_scoped_event_to_templates(&self, id: Id, event: CreateEvent, scopes: Vec<String>) -> DispatchTemplateResult {
        self.get_thread_for(id).send(DispatchEvent {
            id,
            event,
            scopes: Some(scopes),
        }).await?
    }
}

// Assert that WorkerThreadPool is Send + Sync
const _: () = {
    const fn assert_send_sync_clone<T: Send + Sync>() {}
    assert_send_sync_clone::<WorkerThreadPool>();
};
