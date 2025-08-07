use khronos_runtime::primitives::event::CreateEvent;

use super::workerdispatch::DispatchTemplateResult;

use super::workervmmanager::Id;

/// WorkerLike defines a base trait for structures that can be used as Workers in template-worker
#[async_trait::async_trait]
pub trait WorkerLike {
    /// Dispatch an event to the templates managed by this worker
    async fn dispatch_event_to_templates(&self, id: Id, event: CreateEvent) -> DispatchTemplateResult;

    /// Dispatch a scoped event to the templates managed by this worker
    async fn dispatch_scoped_event_to_templates(&self, id: Id, event: CreateEvent, scopes: Vec<String>) -> DispatchTemplateResult;
}