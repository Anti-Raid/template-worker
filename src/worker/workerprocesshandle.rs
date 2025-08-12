use std::time::Duration;

use khronos_runtime::primitives::event::CreateEvent;
use tokio::process::Command;
use tokio::sync::broadcast::{Sender as BroadcastSender, channel as broadcast_channel};
use tokio::sync::oneshot::{channel as oneshot_channel, Sender as OneshotSender};
use tokio::sync::mpsc::{
    UnboundedSender, UnboundedReceiver,
    unbounded_channel
};

use crate::worker::workerdispatch::DispatchTemplateResult;
use crate::worker::workerlike::WorkerLike;
use crate::worker::workerprocesscomm::WorkerProcessCommServer;
use crate::worker::workervmmanager::Id;

/// Message type for the worker process server monitor task
enum ProcessServerMessage {
    Kill {
        tx: OneshotSender<Result<(), crate::Error>>,
    },
    DispatchEvent {
        id: Id,
        event: CreateEvent,
        tx: Option<OneshotSender<DispatchTemplateResult>>,
    },
    DispatchScopedEvent {
        id: Id,
        event: CreateEvent,
        scopes: Vec<String>,
        tx: Option<OneshotSender<DispatchTemplateResult>>,
    },
    RegenerateCache {
        id: Id,
        tx: Option<OneshotSender<Result<(), crate::Error>>>,
    },
}

trait PushableMessage {
    type Response: Send + Sync + 'static;

    fn into_message(self, tx: Option<OneshotSender<Self::Response>>) -> ProcessServerMessage;
}

pub struct DispatchEvent {
    /// The id of the template to dispatch the event to 
    pub id: Id,
    /// The event to dispatch
    pub event: CreateEvent,
    /// The scopes to dispatch the event to, if any
    pub scopes: Option<Vec<String>>,
}

impl PushableMessage for DispatchEvent {
    type Response = DispatchTemplateResult;

    fn into_message(self, tx: Option<OneshotSender<Self::Response>>) -> ProcessServerMessage {
        match self.scopes {
            Some(scopes) => ProcessServerMessage::DispatchScopedEvent {
                id: self.id,
                event: self.event,
                scopes,
                tx,
            },
            None => ProcessServerMessage::DispatchEvent {
                id: self.id,
                event: self.event,
                tx,
            },
        }
    }
}

pub struct RegenerateCache {
    /// The id of the template to regenerate the cache for
    pub id: Id,
}

impl PushableMessage for RegenerateCache {
    type Response = Result<(), crate::Error>;

    fn into_message(self, tx: Option<OneshotSender<Self::Response>>) -> ProcessServerMessage {
        ProcessServerMessage::RegenerateCache {
            id: self.id,
            tx,
        }
    }
}

/// A WorkerProcessHandle is a handle to a worker process from the master process
/// that stores the process handle and provides methods to interact with the worker process.
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
    /// Creates a new WorkerProcessHandle given the worker ID and a communication server backend
    pub async fn new(id: usize, process_comm: Box<dyn WorkerProcessCommServer + Send + Sync>) -> Result<Self, crate::Error> {
        let (tx, rx) = unbounded_channel();

        let (stx, mut srx) = broadcast_channel(100);
        let wps = Self {
            start_events: stx,
            process_handle: tx,
            id,
        };

        let wps_ref = wps.clone();
        tokio::task::spawn(async move {
            wps_ref.run(rx, process_comm).await;
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

    /// Runs the worker process server, spawning a new worker process and handling messages
    /// from the master process.
    async fn run(
        &self, 
        mut rx: UnboundedReceiver<ProcessServerMessage>,
        mut process_comm: Box<dyn WorkerProcessCommServer + Send + Sync>,
    ) {
        let mut failed_attempts = 0;
        loop {
            let sleep_duration = Duration::from_secs(3 * std::cmp::min(failed_attempts, 5));

            // A reset_state call is required to reset the communication state and make sure
            // the communication layer is ready for spinning up a new worker process.
            if let Err(e) = process_comm.reset_state().await {
                log::error!("Failed to reset worker process communication state: {}", e);
                let _ = self.start_events.send(Err(e.to_string()));
                failed_attempts += 1;
                tokio::time::sleep(sleep_duration).await;
                continue;
            }

            // The path to the current executable
            let current_exe = match std::env::current_exe() {
                Ok(path) => path,
                Err(e) => {
                    let _ = self.start_events.send(Err(e.to_string()));
                    failed_attempts += 1;
                    tokio::time::sleep(sleep_duration).await;
                    continue;
                }
            };

            let mut command = Command::new(current_exe);
            
            command.arg("--worker-type");
            command.arg("processpoolworker");
            command.arg("--worker-id");
            command.arg(self.id.to_string());

            for arg in process_comm.start_args() {
                command.arg(arg);
            }

            for (key, value) in process_comm.start_env() {
                command.env(key, value);
            }

            command.kill_on_drop(true);

            let mut child = match command.spawn() {
                Ok(process) => {
                    process
                },
                Err(e) => {
                    let _ = self.start_events.send(Err(e.to_string()));
                    failed_attempts += 1;
                    tokio::time::sleep(sleep_duration).await;
                    continue;
                }
            };
            log::info!("Spawned worker process with ID: {} and pid {:?}", self.id, child.id());
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
                                },
                                ProcessServerMessage::DispatchEvent { id, event, tx } => {
                                    let res = process_comm.dispatch_event_to_templates(id, event).await;
                                    if let Some(tx) = tx {
                                        let _ = tx.send(res);
                                    }
                                },
                                ProcessServerMessage::DispatchScopedEvent { id, event, scopes, tx } => {
                                    let res = process_comm.dispatch_scoped_event_to_templates(id, event, scopes).await;
                                    if let Some(tx) = tx {
                                        let _ = tx.send(res);
                                    }
                                },
                                ProcessServerMessage::RegenerateCache { id, tx } => {
                                    let res = process_comm.regenerate_cache(id).await;
                                    if let Some(tx) = tx {
                                        let _ = tx.send(res);
                                    }
                                },
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

    /// Sends a message to the worker thread
    /// and waits for a response
    async fn send<T: PushableMessage>(&self, msg: T) -> Result<T::Response, crate::Error> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let msg = msg.into_message(Some(tx));
        self.process_handle.send(msg)
            .map_err(|e| format!("Failed to send message to worker thread: {e}"))?;
        rx.await.map_err(|e| format!("Failed to receive response from worker thread: {e}").into())
    }

    /// Sends a message to the worker thread 
    /// and wait for a response with a timeout
    #[allow(dead_code)]
    async fn send_timeout<T: PushableMessage>(&self, msg: T, duration: Duration) -> Result<T::Response, crate::Error> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let msg = msg.into_message(Some(tx));
        self.process_handle.send(msg)
            .map_err(|e| format!("Failed to send message to worker thread: {e}"))?;

        tokio::select! {
            res = rx => res.map_err(|e| format!("Failed to receive response from worker thread: {e}").into()),
            _ = tokio::time::sleep(duration) => Err("Timed out waiting for response from worker thread".into()),
        }
    }

    /// Sends a message to the worker thread
    async fn send_nowait<T: PushableMessage>(&self, msg: T) -> Result<(), crate::Error> {
        let (tx, _rx) = tokio::sync::oneshot::channel();
        let msg = msg.into_message(Some(tx));
        self.process_handle.send(msg)
            .map_err(|e| format!("Failed to send message to worker thread: {e}"))?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl WorkerLike for WorkerProcessHandle {
    fn id(&self) -> usize {
        self.id
    }

    async fn dispatch_event_to_templates(&self, id: Id, event: CreateEvent) -> DispatchTemplateResult {
        self.send(DispatchEvent {
            id,
            event,
            scopes: None,
        }).await?
    }

    async fn dispatch_scoped_event_to_templates(&self, id: Id, event: CreateEvent, scopes: Vec<String>) -> DispatchTemplateResult {
        self.send(DispatchEvent {
            id,
            event,
            scopes: Some(scopes),
        }).await?
    }

    async fn dispatch_event_to_templates_nowait(&self, id: Id, event: CreateEvent) -> Result<(), crate::Error> {
        self.send_nowait(DispatchEvent {
            id,
            event,
            scopes: None,
        }).await
    }

    async fn regenerate_cache(&self, id: Id) -> Result<(), crate::Error> {
        self.send(RegenerateCache { id }).await?
    }
}