use khronos_runtime::primitives::event::CreateEvent;

use super::workervmmanager::Id;

/// WorkerLike defines a base trait for structures that can be used as Workers in template-worker
#[async_trait::async_trait]
#[allow(unused)]
pub trait WorkerLike {
    /// Returns the worker's ID, if present
    /// 
    /// May return 0 for worker pools etc where a worker ID is not applicable
    fn id(&self) -> usize {
        0
    }

    /// Kill the worker like
    async fn kill(&self) -> Result<(), crate::Error>;

    /// Dispatch an event to the templates managed by this worker
    async fn dispatch_event(&self, id: Id, event: CreateEvent) -> Result<serde_json::Value, crate::Error>;

    /// Dispatch an event to the templates managed by this worker without waiting for the result
    async fn dispatch_event_nowait(&self, id: Id, event: CreateEvent) -> Result<(), crate::Error>;

    /// For a pool, returns the length of the pool
    /// 
    /// Returns 0 for non-pool workers
    fn len(&self) -> usize {
        0
    }
}