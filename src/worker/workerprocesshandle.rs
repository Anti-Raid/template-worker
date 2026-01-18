use serenity::async_trait;
use std::sync::Arc;
use std::time::Duration;

use khronos_runtime::primitives::event::CreateEvent;
use khronos_runtime::utils::khronos_value::KhronosValue;
use tokio::process::Command;
use tokio::sync::mpsc::{UnboundedSender, UnboundedReceiver};
use crate::mesophyll::server::MesophyllServer;
use crate::worker::workerfilter::WorkerFilter;
use crate::worker::workerlike::WorkerLike;
use crate::worker::workerpool::Poolable;
use crate::worker::workervmmanager::Id;

/// A WorkerProcessHandle is a handle to a worker process from the master process
/// that stores the process handle and provides methods to interact with the worker process.
#[derive(Clone)]
pub struct WorkerProcessHandle {
    /// Mesophyll server handle to communicate with the worker process
    mesophyll_server: MesophyllServer,

    /// The id of the worker process, used for routing
    id: usize,
    
    /// The total number of processes in the pool
    total: usize,

    /// Kill message channel
    kill_msg_tx: UnboundedSender<()>,
}

#[allow(unused)]
impl WorkerProcessHandle {
    const MAX_CONSECUTIVE_FAILURES_BEFORE_CRASH: usize = 10;

    /// Creates a new WorkerProcessHandle given the worker ID and a mesophyll server
    pub fn new(id: usize, total: usize, mesophyll_server: MesophyllServer) -> Result<Self, crate::Error> {
        let (kill_msg_tx, mut kill_msg_rx) = tokio::sync::mpsc::unbounded_channel::<()>();
        let wps = Self {
            mesophyll_server,
            id,
            total,
            kill_msg_tx
        };

        let wps_ref = wps.clone();
        tokio::task::spawn(async move { wps_ref.run(kill_msg_rx).await });

        Ok(wps)
    }

    /// Runs the worker process server, spawning a new worker process and handling messages
    /// from the master process.
    async fn run(&self, mut kill_msg_rx: UnboundedReceiver<()>) {
        let mut failed_attempts = 0;
        let mut consecutive_failures = 0;

        loop {
            let Some(meso_token) = self.mesophyll_server.get_token_for_worker(self.id) else {
                log::error!("No ident found for worker process with ID: {}", self.id);
                return;
            };

            if consecutive_failures >= Self::MAX_CONSECUTIVE_FAILURES_BEFORE_CRASH {
                log::error!("Worker process with ID: {} has failed {} times in a row, crashing", self.id, consecutive_failures);

                // Abort the master process
                // TODO: Handle this more gracefully in the future
                std::process::abort(); 
            }

            let sleep_duration = Duration::from_secs(3 * std::cmp::min(failed_attempts, 5));

            // The path to the current executable
            let current_exe = match std::env::current_exe() {
                Ok(path) => path,
                Err(e) => {
                    log::error!("Failed to get current executable path: {}", e);
                    failed_attempts += 1;
                    consecutive_failures += 1;
                    tokio::time::sleep(sleep_duration).await;
                    continue;
                }
            };

            let mut command = Command::new(current_exe);
            
            command.arg("--worker-type");
            command.arg("processpoolworker");
            command.arg("--worker-id");
            command.arg(self.id.to_string());
            command.arg("--worker-threads");
            command.arg(self.total.to_string());
            command.env("MESOPHYLL_CLIENT_TOKEN", meso_token);
            command.kill_on_drop(true);

            let mut child = match command.spawn() {
                Ok(process) => {
                    process
                },
                Err(e) => {
                    log::error!("Failed to spawn worker process: {}", e);
                    failed_attempts += 1;
                    consecutive_failures += 1;
                    tokio::time::sleep(sleep_duration).await;
                    continue;
                }
            };
            log::info!("Spawned worker process with ID: {} and pid {:?}", self.id, child.id());

            failed_attempts = 0; // Reset failed attempts on successful start
            consecutive_failures = 0; // Reset consecutive failures on successful start

            tokio::select! {
                resp = child.wait() => {
                    match resp {
                        Ok(status) => {
                            log::warn!("Worker process with ID: {} exited with status: {}", self.id, status);
                        },
                        Err(e) => {
                            log::error!("Failed to wait for worker process with ID: {}: {}", self.id, e);
                        }
                    }
                }
                _ = kill_msg_rx.recv() => {
                    log::info!("Received kill message for worker process with ID: {}, terminating process", self.id);
                    if let Err(e) = child.kill().await {
                        log::error!("Failed to kill worker process with ID: {}: {}", self.id, e);
                    } else {
                        log::info!("Successfully killed worker process with ID: {}", self.id);
                    }
                    return;
                }
            }
        }
    }
}

#[async_trait]
impl WorkerLike for WorkerProcessHandle {
    fn id(&self) -> usize {
        self.id
    }

    async fn kill(&self) -> Result<(), crate::Error> {
        self.kill_msg_tx.send(())
        .map_err(|e| format!("Failed to send kill message to worker process with ID: {}: {}", self.id, e).into())
    }

    fn clone_to_arc(&self) -> Arc<dyn WorkerLike + Send + Sync> {
        Arc::new(self.clone())
    }

    async fn run_script(&self, id: Id, name: String, code: String, event: CreateEvent) -> Result<KhronosValue, crate::Error> {
        let r = self.mesophyll_server.get_connection(self.id)
            .ok_or_else(|| format!("No Mesophyll connection found for worker process with ID: {}", self.id))?;
        r.run_script(id, name, code, event).await
    }

    async fn dispatch_event(&self, id: Id, event: CreateEvent) -> Result<KhronosValue, crate::Error> {
        let r = self.mesophyll_server.get_connection(self.id)
            .ok_or_else(|| format!("No Mesophyll connection found for worker process with ID: {}", self.id))?;
        r.dispatch_event(id, event).await
    }
    
    fn dispatch_event_nowait(&self, id: Id, event: CreateEvent) -> Result<(), crate::Error> {
        let r = self.mesophyll_server.get_connection(self.id)
            .ok_or_else(|| format!("No Mesophyll connection found for worker process with ID: {}", self.id))?;
        r.dispatch_event_nowait(id, event)
    }

    async fn drop_tenant(&self, id: Id) -> Result<(), crate::Error> {
        let r = self.mesophyll_server.get_connection(self.id)
            .ok_or_else(|| format!("No Mesophyll connection found for worker process with ID: {}", self.id))?;
        r.drop_tenant(id).await
    }
}

pub struct WorkerProcessHandleCreateOpts {
    pub(super) mesophyll_server: MesophyllServer,
}

impl WorkerProcessHandleCreateOpts {
    /// Creates a new WorkerProcessHandleCreateOpts with the given communication layer
    pub fn new(mesophyll_server: MesophyllServer) -> Self {
        Self {
            mesophyll_server,
        }
    }
}

// WorkerProcessHandle's can be pooled via WorkerPool!
impl Poolable for WorkerProcessHandle {
    type ExtState = WorkerProcessHandleCreateOpts;

    fn new(_filter: WorkerFilter, id: usize, total: usize, ext_state: &Self::ExtState) -> Result<Self, crate::Error>
    where Self: Sized 
    {
        Self::new(id, total, ext_state.mesophyll_server.clone())
    }
}

// Assert that WorkerProcessHandle is Send + Sync
const _: () = {
    const fn assert_send_sync_clone<T: Send + Sync>() {}
    assert_send_sync_clone::<WorkerProcessHandle>();
};
