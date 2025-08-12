use khronos_runtime::primitives::event::CreateEvent;

use crate::worker::workerdispatch::DispatchTemplateResult;
use crate::worker::workerlike::WorkerLike;

use super::workervmmanager::Id;

use super::workerstate::WorkerState;
use super::workerthread::WorkerThread;
use super::workerfilter::WorkerFilter;

/// A WorkerThreadPool stores a pool of worker threads in which servers are evenly distributed via
/// the Discord Id sharding formula:
/// 
/// shard_id = (guild_id >> 22) % num_shards
#[allow(dead_code)]
pub struct WorkerThreadPool {
    /// The threads in the pool
    threads: Vec<WorkerThread>,
}

impl WorkerThreadPool {
    /// Creates a new WorkerThread with the given cache data and worker state
    pub fn new(state: WorkerState, num_threads: usize) -> Result<Self, crate::Error> {
        let mut threads = Vec::with_capacity(num_threads);

        for id in 0..num_threads {
            let filter = Self::filter_for(id, num_threads);
            let thread = WorkerThread::new(state.clone(), filter, id)?;
            threads.push(thread);
        }

        Ok(WorkerThreadPool {
            threads,
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

    /// Returns a reference to the WorkerThread in the pool for a given tenant ID
    pub fn get_thread_for(&self, id: Id) -> &WorkerThread {
        let index = match id {
            // This is safe as AntiRaid workers does not currently support 32 bit platforms
            Id::GuildId(guild_id) => (guild_id.get() >> 22) as usize % self.threads.len(),
        };
        &self.threads[index]
    }

    /// Returns the number of threads in the pool
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.threads.len()
    }
}

#[async_trait::async_trait]
impl WorkerLike for WorkerThreadPool {
    fn id(&self) -> usize {
        0 // For a pool, return 0
    }

    async fn kill(&self) -> Result<(), crate::Error> {
        for thread in &self.threads {
            thread.kill().await?;
        }
        Ok(())
    }

    async fn dispatch_event_to_templates(&self, id: Id, event: CreateEvent) -> DispatchTemplateResult {
        self.get_thread_for(id).dispatch_event_to_templates(id, event).await
    }

    async fn dispatch_scoped_event_to_templates(&self, id: Id, event: CreateEvent, scopes: Vec<String>) -> DispatchTemplateResult {        
        self.get_thread_for(id).dispatch_scoped_event_to_templates(id, event, scopes).await
    }

    async fn dispatch_event_to_templates_nowait(&self, id: Id, event: CreateEvent) -> Result<(), crate::Error> {
        self.get_thread_for(id).dispatch_event_to_templates_nowait(id, event).await
    }

    async fn regenerate_cache(&self, id: Id) -> Result<(), crate::Error> {
        self.get_thread_for(id).regenerate_cache(id).await
    }

    fn len(&self) -> usize {
        self.threads.len()
    }
}

// Assert that WorkerThreadPool is Send + Sync
const _: () = {
    const fn assert_send_sync_clone<T: Send + Sync>() {}
    assert_send_sync_clone::<WorkerThreadPool>();
};
