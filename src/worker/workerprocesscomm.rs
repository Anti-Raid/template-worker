use khronos_runtime::{primitives::event::CreateEvent, utils::khronos_value::KhronosValue};
use serde::{de::DeserializeOwned, Serialize};

use crate::worker::{workerdispatch::{DispatchTemplateResult, TemplateResult}, workerlike::WorkerLike, workervmmanager::Id};
use rand::{distr::{Alphanumeric, SampleString}, Rng};

#[async_trait::async_trait]
pub trait WorkerProcessCommServer {
    /// Dispatch an event to the templates managed by this worker
    async fn dispatch_event_to_templates(&self, id: Id, event: CreateEvent) -> DispatchTemplateResult;

    /// Dispatch a scoped event to the templates managed by this worker
    async fn dispatch_scoped_event_to_templates(&self, id: Id, event: CreateEvent, scopes: Vec<String>) -> DispatchTemplateResult;

    /// Regenerate the cache for a tenant
    async fn regenerate_cache(&self, id: Id) -> Result<(), crate::Error>;

    /// The extra arguments needed to start the worker process
    fn start_args(&self) -> Vec<String>;

    /// The environment variables needed to start the worker process
    fn start_env(&self) -> Vec<(String, String)>;

    /// Wait for the worker process to be ready
    async fn wait_for_ready(&self) -> Result<(), crate::Error> {
        // Default implementation does nothing, can be overridden
        Ok(())
    }
}

/// Marker trait to signify that this is a client for the worker process communication
pub trait WorkerProcessCommClient {}

#[derive(serde::Serialize, serde::Deserialize)]
enum WorkerProcessCommTenantIdType {
    GuildId,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct WorkerProcessCommTenantId {
    id: u64,
    typ: WorkerProcessCommTenantIdType,
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
enum WorkerProcessCommTemplateResult {
    Ok {
        result: KhronosValue
    },
    Error {
        error: String,
    },
}

impl From<WorkerProcessCommTemplateResult> for TemplateResult {
    fn from(result: WorkerProcessCommTemplateResult) -> Self {
        match result {
            WorkerProcessCommTemplateResult::Ok { result } => TemplateResult::Ok(result),
            WorkerProcessCommTemplateResult::Error { error } => TemplateResult::Err(error.into()),
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
enum WorkerProcessCommDispatchResult {
    Ok {
        result: Vec<(String, WorkerProcessCommTemplateResult)>,
    },
    Error {
        error: String,
    },
}

impl From<WorkerProcessCommDispatchResult> for DispatchTemplateResult {
    fn from(result: WorkerProcessCommDispatchResult) -> Self {
        match result {
            WorkerProcessCommDispatchResult::Ok { result } => DispatchTemplateResult::Ok(result.into_iter().map(|(key, value)| (key, value.into())).collect()),
            WorkerProcessCommDispatchResult::Error { error } => DispatchTemplateResult::Err(error.into()),
        }
    }
}

/// Worker Process Communication using HTTP/2
/// 
/// Server here refers to the master side which sends data to/from the worker (client).
/// Most notably, the client is what exposes the HTTP/2 server to the master process.
#[derive(Clone)]
pub struct WorkerProcessCommHttp2Master {
    token: String,
    port: u16,
    reqwest: reqwest::Client,
}

impl WorkerProcessCommHttp2Master {
    const DISPATCH_TEMPLATES_PATH: &'static str = "/0";
    const REGENERATE_CACHE_PATH: &'static str = "/1";

    pub async fn new(reqwest: reqwest::Client) -> Result<Self, crate::Error> {
        let mut port = rand::rng().random_range(1030..=65535);
        
        let mut attempts = 0;
        loop {
            if attempts >= 20 {
                return Err("Failed to find an available port after 20 attempts".into());
            }
            // Ensure the port is not already in use
            match tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port)).await {
                Ok(_) => {
                    break;
                },
                Err(_) => {
                    port = rand::rng().random_range(1030..=65535); // Try a different port
                }
            }
        };

        Ok(Self {
            token: Alphanumeric.sample_string(&mut rand::rng(), 128),
            port,
            reqwest,
        })
    }

    async fn send<Request: Serialize, Response: DeserializeOwned>(
        &self,
        url: &str,
        request: Request,
    ) -> Result<Response, crate::Error> {
        let url = format!("http://127.0.1:{}{}", self.port, url);
        let request = self.reqwest.post(&url)
            .header("Token", &self.token)
            .json(&request)
            .send()
            .await
            .map_err(|e| format!("Failed to send request: {}", e))?;

        if !request.status().is_success() {
            let resp = request.text().await.map_err(|e| format!("Failed to read response text: {}", e))?;
            return Err(resp.into());
        }

        let response: Response = request.json().await.map_err(|e| format!("Failed to parse response: {}", e))?;

        Ok(response)
    }
}

#[async_trait::async_trait]
impl WorkerProcessCommServer for WorkerProcessCommHttp2Master {
    async fn dispatch_event_to_templates(&self, id: Id, event: CreateEvent) -> DispatchTemplateResult {
        let id = WorkerProcessCommTenantId::from(id);
        let event_json = serde_json::to_string(&event)
            .map_err(|e| format!("Failed to serialize event: {}", e))?;
        
        let request = WorkerProcessCommHttp2DispatchEventToTemplates {
            id,
            event_json,
            scopes: None,
        };

        let response: WorkerProcessCommHttp2DispatchEventToTemplatesResponse = self.send(Self::DISPATCH_TEMPLATES_PATH, request).await?;
        response.result.into()
    }

    async fn dispatch_scoped_event_to_templates(&self, id: Id, event: CreateEvent, scopes: Vec<String>) -> DispatchTemplateResult {
        let id = WorkerProcessCommTenantId::from(id);
        let event_json = serde_json::to_string(&event).map_err(|e| format!("Failed to serialize event: {}", e))?;
        
        let request = WorkerProcessCommHttp2DispatchEventToTemplates {
            id,
            event_json,
            scopes: Some(scopes),
        };

        let response: WorkerProcessCommHttp2DispatchEventToTemplatesResponse = self.send(Self::DISPATCH_TEMPLATES_PATH, request).await?;
        response.result.into()
    }

    async fn regenerate_cache(&self, id: Id) -> Result<(), crate::Error> {
        let id = WorkerProcessCommTenantId::from(id);
        
        let request = WorkerProcessCommHttp2RegenerateCache { id };
        
        let _: WorkerProcessCommHttp2RegenerateCacheResponse = self.send(Self::REGENERATE_CACHE_PATH, request).await?;

        Ok(())
    }

    fn start_args(&self) -> Vec<String> {
        vec![
            "--worker-comm-type".to_string(),
            "http2".to_string(),
        ]
    }

    fn start_env(&self) -> Vec<(String, String)> {
        vec![
            ("WORKER_PROCESS_COMM_TOKEN".to_string(), self.token.clone()),
            ("WORKER_PROCESS_COMM_PORT".to_string(), self.port.to_string()),
        ]
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
/// Stores the message data that is sent from master to the worker process
struct WorkerProcessCommHttp2DispatchEventToTemplates {
    id: WorkerProcessCommTenantId,
    event_json: String,
    scopes: Option<Vec<String>>,
}

#[derive(serde::Serialize, serde::Deserialize)]
/// Stores the message data that is sent from worker to master process
struct WorkerProcessCommHttp2DispatchEventToTemplatesResponse {
    result: WorkerProcessCommDispatchResult,
}

#[derive(serde::Serialize, serde::Deserialize)]
/// Stores the message data that is sent from master to the worker process
struct WorkerProcessCommHttp2RegenerateCache {
    id: WorkerProcessCommTenantId,
}

#[derive(serde::Serialize, serde::Deserialize)]
/// Stores the message data that is sent from worker to master process
struct WorkerProcessCommHttp2RegenerateCacheResponse {}