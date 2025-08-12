use khronos_runtime::{primitives::event::CreateEvent, utils::khronos_value::KhronosValue};
use crate::worker::{workerdispatch::{DispatchTemplateResult, TemplateResult}, workervmmanager::Id};

#[async_trait::async_trait]
pub trait WorkerProcessCommServer: Send + Sync {
    /// Resets the state of the communication method for a restart
    /// of the worker process
    /// 
    /// For example, with http2, this would mean getting a new token and port
    async fn reset_state(&mut self) -> Result<(), crate::Error>;

    /// The extra arguments needed to start the worker process
    fn start_args(&self) -> Vec<String>;

    /// The environment variables needed to start the worker process
    fn start_env(&self) -> Vec<(String, String)>;

    /// Dispatch an event to the templates managed by this worker
    async fn dispatch_event_to_templates(&self, id: Id, event: CreateEvent) -> DispatchTemplateResult;

    /// Dispatch a scoped event to the templates managed by this worker
    async fn dispatch_scoped_event_to_templates(&self, id: Id, event: CreateEvent, scopes: Vec<String>) -> DispatchTemplateResult;

    /// Regenerate the cache for a tenant
    async fn regenerate_cache(&self, id: Id) -> Result<(), crate::Error>;
}

/// Marker trait to signify that this is a client for the worker process communication
pub trait WorkerProcessCommClient {}

/// Trait to create a worker process communication server
#[async_trait::async_trait]
pub trait WorkerProcessCommServerCreator {
    /// Creates a new worker process communication server
    async fn create(&self) -> Result<Box<dyn WorkerProcessCommServer>, crate::Error>;
}

#[derive(serde::Serialize, serde::Deserialize)]
/// Serializable representation of a tenant ID for the worker process communication
pub(super) enum WorkerProcessCommTenantIdType {
    GuildId,
}

#[derive(serde::Serialize, serde::Deserialize)]
/// Serializable representation of a tenant ID for the worker process communication
pub(super) struct WorkerProcessCommTenantId {
    pub(super) id: u64,
    pub(super) typ: WorkerProcessCommTenantIdType,
}

impl From<Id> for WorkerProcessCommTenantId {
    fn from(id: Id) -> Self {
        match id {
            Id::GuildId(guild_id) => Self { id: guild_id.get(), typ: WorkerProcessCommTenantIdType::GuildId },
        }
    }
}

impl From<WorkerProcessCommTenantId> for Id {
    fn from(tenant_id: WorkerProcessCommTenantId) -> Self {
        match tenant_id.typ {
            WorkerProcessCommTenantIdType::GuildId => Id::GuildId(tenant_id.id.into()),
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
/// Serializable representation of a template result for the worker process communication
pub(super) enum WorkerProcessCommTemplateResult {
    Ok {
        result: KhronosValue
    },
    Error {
        error: String,
    },
}

impl From<TemplateResult> for WorkerProcessCommTemplateResult {
    fn from(result: TemplateResult) -> Self {
        match result {
            TemplateResult::Ok(result) => WorkerProcessCommTemplateResult::Ok { result: result.into() },
            TemplateResult::Err(error) => WorkerProcessCommTemplateResult::Error { error: error.to_string() },
        }
    }
}

impl From<WorkerProcessCommTemplateResult> for TemplateResult {
    fn from(result: WorkerProcessCommTemplateResult) -> Self {
        match result {
            WorkerProcessCommTemplateResult::Ok { result } => TemplateResult::Ok(result.into()),
            WorkerProcessCommTemplateResult::Error { error } => TemplateResult::Err(error.into()),
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
/// Serializable representation of the result of dispatching a template in the worker process communication
pub(super) enum WorkerProcessCommDispatchResult {
    Ok {
        result: Vec<(String, WorkerProcessCommTemplateResult)>,
    },
    Error {
        error: String,
    },
}

impl From<DispatchTemplateResult> for WorkerProcessCommDispatchResult {
    fn from(result: DispatchTemplateResult) -> Self {
        match result {
            DispatchTemplateResult::Ok(results) => WorkerProcessCommDispatchResult::Ok {
                result: results.into_iter().map(|(name, res)| (name, res.into())).collect(),
            },
            DispatchTemplateResult::Err(error) => WorkerProcessCommDispatchResult::Error {
                error: error.to_string(),
            },
        }
    }
}

impl From<WorkerProcessCommDispatchResult> for DispatchTemplateResult {
    fn from(result: WorkerProcessCommDispatchResult) -> Self {
        match result {
            WorkerProcessCommDispatchResult::Ok { result } => DispatchTemplateResult::Ok(
                result.into_iter().map(|(name, res)| (name, res.into())).collect(),
            ),
            WorkerProcessCommDispatchResult::Error { error } => DispatchTemplateResult::Err(error.into()),
        }
    }
}
