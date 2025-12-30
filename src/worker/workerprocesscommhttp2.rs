use std::sync::Arc;

use khronos_runtime::{primitives::event::CreateEvent, utils::khronos_value::KhronosValue};
use serde::{de::DeserializeOwned, Serialize};

use super::{workerlike::WorkerLike, workervmmanager::Id};
use rand::{distr::{Alphanumeric, SampleString}, Rng};
use super::workerprocesscomm::{WorkerProcessCommServer, WorkerProcessCommClient, WorkerProcessCommServerCreator, WorkerProcessCommTenantId};

/// Worker Process Communication Server Creator for HTTP/2
pub struct WorkerProcessCommHttp2ServerCreator {
    reqwest: reqwest::Client,
}

impl WorkerProcessCommHttp2ServerCreator {
    pub fn new(reqwest: reqwest::Client) -> Self {
        Self { reqwest }
    }
}

impl WorkerProcessCommServerCreator for WorkerProcessCommHttp2ServerCreator {
    fn create(&self) -> Result<Box<dyn WorkerProcessCommServer>, crate::Error> {
        Ok(Box::new(WorkerProcessCommHttp2Master::new(self.reqwest.clone())))
    }
}


/// Worker Process Communication using HTTP/2
/// 
/// Server here refers to the master side which sends data to/from the worker (client).
/// Most notably, the client is what exposes the HTTP/2 server to the master process.
pub struct WorkerProcessCommHttp2Master {
    token: Option<String>,
    port: Option<u16>,
    reqwest: reqwest::Client,
}

impl WorkerProcessCommHttp2Master {
    const DISPATCH_TEMPLATES_PATH: &'static str = "/0";
    const TOKEN_LENGTH: usize = 4096;

    pub fn new(reqwest: reqwest::Client) -> Self {
        Self {
            token: None,
            port: None,
            reqwest,
        }
    }

    async fn create_state() -> Result<(u16, String), crate::Error> {
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
                    attempts += 1;
                }
            }
        };

        Ok((port, Alphanumeric.sample_string(&mut rand::rng(), Self::TOKEN_LENGTH)))
    }

    /// Sends a request to the worker process and returns the response
    async fn send<Request: Serialize, Response: DeserializeOwned>(
        &self,
        url: &str,
        request: Request,
    ) -> Result<Response, crate::Error> {
        let Some(port) = self.port else {
            return Err("Worker process communication port not set".into());
        };
        let Some(token) = &self.token else {
            return Err("Worker process communication token not set".into());
        };

        let url = format!("http://127.0.1:{}{}", port, url);
        let request = self.reqwest.post(&url)
            .header("Token", token)
            .json(&request)
            .send()
            .await
            .map_err(|e| format!("Failed to send request: {}", e))?;

        if request.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err("Worker process communication unauthorized: invalid token".into());
        } 

        // All other errors
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
    async fn reset_state(&mut self) -> Result<(), crate::Error> {
        let (port, token) = Self::create_state().await?;
        self.port = Some(port);
        self.token = Some(token);

        Ok(())
    }

    async fn dispatch_event(&self, id: Id, event: CreateEvent) -> Result<KhronosValue, crate::Error> {
        let id = WorkerProcessCommTenantId::from(id);
        
        let request = WorkerProcessCommHttp2DispatchEventToTemplates {
            id,
            event,
        };

        self.send(Self::DISPATCH_TEMPLATES_PATH, request).await
    }

    fn start_args(&self) -> Vec<String> {
        vec![
            "--worker-comm-type".to_string(),
            "http2".to_string(),
        ]
    }

    fn start_env(&self) -> Vec<(String, String)> {
        let Some(token) = &self.token else {
            panic!("Worker process communication token not set");
        };
        let Some(port) = self.port else {
            panic!("Worker process communication port not set");
        };

        vec![
            ("WORKER_PROCESS_COMM_TOKEN".to_string(), token.clone()),
            ("WORKER_PROCESS_COMM_PORT".to_string(), port.to_string()),
        ]
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
/// Stores the message data that is sent from master to the worker process
struct WorkerProcessCommHttp2DispatchEventToTemplates {
    id: WorkerProcessCommTenantId,
    event: CreateEvent,
}

#[derive(Clone)]
pub struct WorkerProcessCommHttp2Worker {
    token: String,
    worker: Arc<dyn WorkerLike + Send + Sync>,
}

impl WorkerProcessCommHttp2Worker {
    pub async fn new(worker: Arc<dyn WorkerLike + Send + Sync>) -> Result<Self, crate::Error> {
        let token = std::env::var("WORKER_PROCESS_COMM_TOKEN")
            .map_err(|_| "WORKER_PROCESS_COMM_TOKEN environment variable not set")?;
        let port_str = std::env::var("WORKER_PROCESS_COMM_PORT")
            .map_err(|_| "WORKER_PROCESS_COMM_PORT environment variable not set")?
            .trim()
            .parse::<u16>()
            .map_err(|_| "Invalid WORKER_PROCESS_COMM_PORT value")?;

        Self::new_inner(token, port_str, worker).await
    }

    async fn new_inner(token: String, port: u16, worker: Arc<dyn WorkerLike + Send + Sync>) -> Result<Self, crate::Error> {
        // Ensure the port is not already in use
        let listener = tokio::net::TcpListener::bind(format!("127.0.1:{port}")).await?;

        let self_n = Self { token, worker };

        let router: axum::Router<()> = axum::Router::new()
        .route(
            WorkerProcessCommHttp2Master::DISPATCH_TEMPLATES_PATH,
            axum::routing::post(axum::routing::post(http2_endpoints::dispatch_template_endpoint)),
        )
        .with_state(self_n.clone());

        tokio::task::spawn(async move {
            axum::serve(listener, router.into_make_service()).await.unwrap();
        });

        Ok(self_n)
    }
}

impl WorkerProcessCommClient for WorkerProcessCommHttp2Worker {}

mod http2_endpoints {
    use axum::{
        extract::State,
        http::{HeaderMap, StatusCode},
        Json,
    };
    use khronos_runtime::utils::khronos_value::KhronosValue;

    fn verify_token(
        headers: &HeaderMap,
        token: &str,
    ) -> Result<(), (StatusCode, Json<String>)> {
        let token_header = headers.get("Token")
            .and_then(|h| h.to_str().ok())
            .ok_or((StatusCode::UNAUTHORIZED, Json("Missing or invalid Token header".to_string())))?;

        if token_header.len() != super::WorkerProcessCommHttp2Master::TOKEN_LENGTH || token_header != token {
            log::warn!("Worker call attempted with invalid token!");
            return Err((StatusCode::UNAUTHORIZED, Json("Invalid Token".to_string())));
        }

        Ok(())
    }

    pub(super) async fn dispatch_template_endpoint(
        State(data): State<super::WorkerProcessCommHttp2Worker>,
        headers: HeaderMap,
        Json(request): Json<super::WorkerProcessCommHttp2DispatchEventToTemplates>,
    ) -> Result<Json<KhronosValue>, (StatusCode, Json<String>)> {
        verify_token(&headers, &data.token)?;

        let result = data.worker.dispatch_event(request.id.into(), request.event).await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(e.to_string())))?;

        Ok(Json(result))
    }
}