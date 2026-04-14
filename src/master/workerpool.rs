use khronos_runtime::primitives::event::CreateEvent;
use khronos_runtime::utils::khronos_value::KhronosValue;

use crate::geese::tenantstate::TenantState;
use crate::master::workerprocesshandle::WorkerProcessHandle;
use crate::mesophyll::server::MesophyllServer;
use crate::worker::workervmmanager::Id;

/// A WorkerPool stores a pool of workers in which servers are evenly distributed via
/// the Discord Id sharding formula:
#[allow(dead_code)]
pub struct WorkerPool {
    /// The workers in the pool
    workers: Vec<WorkerProcessHandle>,
}

impl WorkerPool {
    /// Creates a new WorkerPool with the given cache data and worker state
    pub fn new(num_threads: usize, worker_debug: bool, server: &MesophyllServer) -> Result<Self, crate::Error> {
        let mut workers = Vec::with_capacity(num_threads);

        for id in 0..num_threads {
            let thread = WorkerProcessHandle::new(id, worker_debug, server.clone())?;
            workers.push(thread);
        }

        Ok(WorkerPool {
            workers,
        })
    }

    /// Returns a reference to the WorkerThread in the pool for a given tenant ID
    pub fn get_worker_for(&self, id: Id) -> &WorkerProcessHandle {
        &self.workers[id.worker_id(self.workers.len())]
    }
}

impl WorkerPool {
    pub async fn kill(&self) -> Result<(), crate::Error> {
        for worker in &self.workers {
            worker.kill().await?;
        }
        Ok(())
    }

    pub async fn dispatch_event(&self, id: Id, event: CreateEvent) -> Result<KhronosValue, crate::Error> {
        self.get_worker_for(id).dispatch_event(id, event).await
    }

    pub async fn drop_tenant(&self, id: Id) -> Result<(), crate::Error> {
        self.get_worker_for(id).drop_tenant(id).await
    }

    pub async fn update_tenant_state(&self, id: Id, ts: TenantState) -> Result<(), crate::Error> {
        self.get_worker_for(id).update_tenant_state(id, ts).await
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.workers.len()
    }
}

// Assert that WorkerPool is Send + Sync
const _: () = {
    const fn assert_send_sync_clone<T: Send + Sync>() {}
    assert_send_sync_clone::<WorkerPool>();
};
