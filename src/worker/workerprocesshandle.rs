use tokio::process::Command;
use tokio::sync::broadcast::{Sender as BroadcastSender, channel as broadcast_channel};
use tokio::sync::oneshot::{channel as oneshot_channel, Sender as OneshotSender};
use tokio::sync::mpsc::{
    UnboundedSender, UnboundedReceiver,
    unbounded_channel
};

/// Message type for the worker process server monitor task
enum ProcessServerMessage {
    Kill {
        tx: OneshotSender<Result<(), crate::Error>>,
    }
}

/// A WorkerProcessServer is a handle to a worker process from the master process
/// that stores the process handle and provides methods to interact with the worker process.
/// 
/// WorkerProcessClient is the client side of the worker process server which runs on the worker process (not created on master)
#[derive(Clone)]
pub struct WorkerProcessHandle {
    /// The process handle for the worker process
    process_handle: UnboundedSender<ProcessServerMessage>,

    /// Start event channel
    start_events: BroadcastSender<Result<(), String>>,

    /// The id of the worker process, used for routing
    id: usize,
}

#[allow(unused)]
impl WorkerProcessHandle {
    /// Creates a new WorkerProcessHandle with the given process handle and communication ID
    pub async fn new(id: usize) -> Result<Self, crate::Error> {

        let (tx, rx) = unbounded_channel();

        let (stx, mut srx) = broadcast_channel(100);
        let wps = Self {
            start_events: stx,
            process_handle: tx,
            id,
        };

        let wps_ref = wps.clone();
        tokio::task::spawn(async move {
            wps_ref.run(rx).await;
        });

        // Wait for the worker process to start
        match srx.recv().await {
            Ok(Ok(())) => {
                Ok(wps)
            },
            Ok(Err(e)) => {
                Err(e.into())
            },
            Err(e) => {
                Err(e.into())
            }
        }
    }

    async fn run(
        &self, 
        mut rx: UnboundedReceiver<ProcessServerMessage>,
    ) {
        loop {
            // The path to the current executable
            let current_exe = match std::env::current_exe() {
                Ok(path) => path,
                Err(e) => {
                    let _ = self.start_events.send(Err(e.to_string()));
                    return;
                }
            };

            // ID used for routing messages to the correct worker process
            let communication_id = format!("worker-{}", uuid::Uuid::new_v4());

            let mut child = match Command::new(current_exe)
            .arg("--worker-type")
            .arg("processpoolworker")
            .arg("--worker-id")
            .arg(self.id.to_string())
            .arg("--communication-id")
            .arg(&communication_id)
            .kill_on_drop(true)
            .spawn() {
                Ok(process) => {
                    process
                },
                Err(e) => {
                    let _ = self.start_events.send(Err(e.to_string()));
                    return;
                }
            };
            log::info!("Spawned worker process with ID: {} and communication ID: {} and pid {:?}", self.id, communication_id, child.id());
            let mut is_killing = false;

            // Send the start signal to the caller
            let _ = self.start_events.send(Ok(()));

            loop {
                tokio::select! {
                    _ = child.wait() => {
                        if is_killing {
                            return; // Do not attempt to restart the process if it was killed
                        }

                        log::info!("Worker process with ID: {} exited, restarting...", self.id);
                        break; // Process exited, break out of inner loop to restart it
                    }
                    msg = rx.recv() => {
                        if let Some(msg) = msg {
                            // Handle the message
                            match msg {
                                ProcessServerMessage::Kill { tx } => {
                                    log::info!("Killing worker process with ID: {}", self.id);
                                    is_killing = true;
                                    let _ = tx.send(child.kill().await.map_err(|x| x.into()));
                                    return; // Exit the loop after killing the process
                                }
                            }
                        } else {
                            // Channel closed, exit the loop after killing the process
                            log::info!("Worker process server channel closed, exiting");
                            is_killing = true;
                            let _ = child.kill().await;
                            return;
                        }
                    }
                }
            }
        }
    }

    pub async fn error_on_startup_fail(
        &self,
        max_consecutive_failures: usize,
    ) -> Result<(), crate::Error> {
        let mut rx = self.start_events.subscribe();
        let mut consecutive_failures = 0;

        loop {
            match rx.recv().await {
                Ok(Ok(())) => {
                    consecutive_failures = 0; // Reset on success
                }
                Ok(Err(e)) => {
                    consecutive_failures += 1;
                    log::error!("Worker process startup failed: {}", e);
                    if consecutive_failures >= max_consecutive_failures {
                        return Err(format!(
                            "Worker process failed to start {} times consecutively: {}",
                            max_consecutive_failures, e
                        ).into());
                    }
                }
                Err(_) => {
                    return Ok(()); // Channel closed, exit gracefully
                }
            }
        }
    }

    /// Sends a kill signal to the worker process and waits for it to exit
    pub async fn kill(&self) -> Result<(), crate::Error> {
        let (tx, rx) = oneshot_channel();
        self.process_handle.send(ProcessServerMessage::Kill { tx })?;

        rx.await?
    }
}