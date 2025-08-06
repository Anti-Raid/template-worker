use khronos_runtime::primitives::event::CreateEvent;

use super::workerdispatch::TemplateResult;

use super::workervmmanager::Id;

/// WorkerLike defines a base trait for structures that can be used as Workers in template-worker
pub trait WorkerLike {
    /// Dispatch an event to the templates managed by this worker
    async fn dispatch_event_to_templates(&self, id: Id, event: CreateEvent) -> Result<Vec<(String, TemplateResult)>, crate::Error>;

    /// Dispatch a scoped event to the templates managed by this worker
    async fn dispatch_scoped_event_to_templates(&self, id: Id, event: CreateEvent, scopes: &[String]) -> Result<Vec<(String, TemplateResult)>, crate::Error>;
}