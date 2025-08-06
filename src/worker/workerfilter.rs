use std::sync::Arc;

use super::workervmmanager::Id;

#[derive(Clone)]
pub struct WorkerFilter {
    filter: Arc<dyn Fn(Id) -> bool + Send + Sync>,
}

impl WorkerFilter {
    /// Creates a new WorkerFilter with the given filter function
    pub fn new<F>(filter: F) -> Self
    where
        F: Fn(Id) -> bool + Send + Sync + 'static,
    {
        Self {
            filter: Arc::new(filter),
        }
    }

    /// Checks if the worker ID is allowed to dispatch events for the given tenant ID
    pub fn is_allowed(&self, tenant_id: Id) -> bool {
        (self.filter)(tenant_id)
    }
}