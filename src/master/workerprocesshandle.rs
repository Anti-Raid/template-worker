use std::time::Duration;

use khronos_runtime::utils::khronos_value::KhronosValue;
use tokio::process::Command;
use tokio::sync::mpsc::{UnboundedSender, UnboundedReceiver};
use crate::CONFIG;
use crate::geese::tenantstate::TenantState;
use crate::mesophyll::server::MesophyllServer;
use crate::worker::workerdispatch::SimpleEvent;
use crate::worker::workervmmanager::Id;

/// A WorkerProcessHandle is a handle to a worker process from the master process
/// that stores the process handle and provides methods to interact with the worker process.
#[derive(Clone)]
pub struct WorkerProcessHandle {
    /// Mesophyll server handle to communicate with the worker process
    mesophyll_server: MesophyllServer,

    /// The id of the worker process, used for routing
    id: usize,
    
    /// Whether to enable print
    worker_debug: bool,

    /// Kill message channel
    kill_msg_tx: UnboundedSender<()>,
}

#[allow(unused)]
impl WorkerProcessHandle {
    const MAX_CONSECUTIVE_FAILURES_BEFORE_CRASH: usize = 10;

    /// Creates a new WorkerProcessHandle given the worker ID and a mesophyll server
    pub fn new(id: usize, worker_debug: bool, mesophyll_server: MesophyllServer) -> Result<Self, crate::Error> {
        let (kill_msg_tx, mut kill_msg_rx) = tokio::sync::mpsc::unbounded_channel::<()>();
        let wps = Self {
            mesophyll_server,
            id,
            worker_debug,
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
            if consecutive_failures >= Self::MAX_CONSECUTIVE_FAILURES_BEFORE_CRASH {
                log::error!("Worker process with ID: {} has failed {} times in a row, crashing", self.id, consecutive_failures);

                // Abort the master process
                // TODO: Handle this more gracefully in the future
                std::thread::sleep(Duration::from_secs(1));
                std::process::abort(); 
            }

            let sleep_duration = Duration::from_secs(3 * std::cmp::min(failed_attempts, 5));

            let mut command = Command::new(&CONFIG.worker_path);
            
            command.arg("--worker-type");
            command.arg("processpoolworker");
            command.arg("--worker-id");
            command.arg(self.id.to_string());

            if self.worker_debug {
                command.arg("--worker-debug");
            }

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

impl WorkerProcessHandle {
    #[allow(dead_code)]
    pub fn id(&self) -> usize {
        self.id
    }

    pub async fn kill(&self) -> Result<(), crate::Error> {
        self.kill_msg_tx.send(())
        .map_err(|e| format!("Failed to send kill message to worker process with ID: {}: {}", self.id, e).into())
    }

    pub async fn dispatch_event(&self, id: Id, event: SimpleEvent) -> Result<KhronosValue, crate::Error> {
        let r = self.mesophyll_server.get_connection(self.id)
            .ok_or_else(|| format!("No Mesophyll connection found for worker process with ID: {}", self.id))?;
        r.dispatch_event(id, event).await
    }
    
    pub async fn drop_tenant(&self, id: Id) -> Result<(), crate::Error> {
        let r = self.mesophyll_server.get_connection(self.id)
            .ok_or_else(|| format!("No Mesophyll connection found for worker process with ID: {}", self.id))?;
        r.drop_tenant(id).await
    }

    pub async fn update_tenant_state(&self, id: Id, ts: TenantState) -> Result<(), crate::Error> {
        let r = self.mesophyll_server.get_connection(self.id)
            .ok_or_else(|| format!("No Mesophyll connection found for worker process with ID: {}", self.id))?;
        r.update_tenant_state(id, ts).await
    }
}

// Assert that WorkerProcessHandle is Send + Sync
const _: () = {
    const fn assert_send_sync_clone<T: Send + Sync>() {}
    assert_send_sync_clone::<WorkerProcessHandle>();
};
