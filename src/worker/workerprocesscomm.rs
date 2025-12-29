use khronos_runtime::primitives::event::CreateEvent;
use crate::worker::workervmmanager::Id;

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
    async fn dispatch_event(&self, id: Id, event: CreateEvent) -> Result<serde_json::Value, crate::Error>;

    // Regenerate the cache for a tenant
    //async fn regenerate_cache(&self, id: Id) -> Result<(), crate::Error>;
}

/// Marker trait to signify that this is a client for the worker process communication
pub trait WorkerProcessCommClient: Send + Sync {}

/// Trait to create a worker process communication server
pub trait WorkerProcessCommServerCreator: Send + Sync {
    /// Creates a new worker process communication server
    fn create(&self) -> Result<Box<dyn WorkerProcessCommServer>, crate::Error>;
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
