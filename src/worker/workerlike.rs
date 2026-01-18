use std::sync::Arc;

use serenity::async_trait;
use khronos_runtime::{primitives::event::CreateEvent, utils::khronos_value::KhronosValue};

use super::workervmmanager::Id;

/// WorkerLike defines a base trait for structures that can be used as Workers in template-worker
#[async_trait]
#[allow(unused)]
pub trait WorkerLike: Send + Sync + 'static {
    /// Returns the worker's ID, if present
    /// 
    /// May return 0 for worker pools etc where a worker ID is not applicable
    fn id(&self) -> usize {
        0
    }

    fn clone_to_arc(&self) -> Arc<dyn WorkerLike + Send + Sync>;

    /// Runs a script with the given chunk name, code and event
    /// 
    /// This is the special version of dispatch event that directly enables for running arbitrary scripts
    /// (which is useful for the fauxpas staff API and other future internal tooling etc.)
    async fn run_script(&self, id: Id, name: String, code: String, event: CreateEvent) -> Result<KhronosValue, crate::Error>;

    /// Kill the worker like
    async fn kill(&self) -> Result<(), crate::Error>;

    /// Dispatch an event to the templates managed by this worker
    async fn dispatch_event(&self, id: Id, event: CreateEvent) -> Result<KhronosValue, crate::Error>;

    /// Dispatch an event to the templates managed by this worker without waiting for the result
    fn dispatch_event_nowait(&self, id: Id, event: CreateEvent) -> Result<(), crate::Error>;

    /// Drop a tenant from the worker
    async fn drop_tenant(&self, id: Id) -> Result<(), crate::Error>;

    /// For a pool, returns the length of the pool
    /// 
    /// Returns 0 for non-pool workers
    fn len(&self) -> usize {
        0
    }
}

#[async_trait]
impl WorkerLike for Arc<dyn WorkerLike + Send + Sync> {
    fn id(&self) -> usize {
        self.as_ref().id()
    }

    async fn kill(&self) -> Result<(), crate::Error> {
        self.as_ref().kill().await
    }

    fn clone_to_arc(&self) -> Arc<dyn WorkerLike + Send + Sync> {
        self.as_ref().clone_to_arc()
    }

    async fn run_script(&self, id: Id, name: String, code: String, event: CreateEvent) -> Result<KhronosValue, crate::Error> {
        self.as_ref().run_script(id, name, code, event).await
    }

    async fn dispatch_event(&self, id: Id, event: CreateEvent) -> Result<KhronosValue, crate::Error> {
        self.as_ref().dispatch_event(id, event).await
    }

    fn dispatch_event_nowait(&self, id: Id, event: CreateEvent) -> Result<(), crate::Error> {
        self.as_ref().dispatch_event_nowait(id, event)
    }

    async fn drop_tenant(&self, id: Id) -> Result<(), crate::Error> {
        self.as_ref().drop_tenant(id).await
    }

    fn len(&self) -> usize {
        self.as_ref().len()
    }
}