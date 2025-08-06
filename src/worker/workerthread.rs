use khronos_runtime::primitives::event::CreateEvent;
use tokio::sync::mpsc::{UnboundedSender, UnboundedReceiver};
use tokio::sync::oneshot::Sender as OneShotSender;
use rand::Rng;
use std::time::Duration;
use std::{panic::AssertUnwindSafe, thread::JoinHandle};

use crate::worker::limits::MAX_VM_THREAD_STACK_SIZE;
use super::{workercachedata::WorkerCacheData, workerstate::WorkerState, worker::Worker, workervmmanager::Id, workerdispatch::DispatchTemplateResult};

/// WorkerThreadMessage is the message type that is sent to the worker thread
enum WorkerThreadMessage {
    DispatchEvent {
        id: Id,
        event: CreateEvent,
        tx: Option<OneShotSender<DispatchTemplateResult>>,
    },
    DispatchScopedEvent {
        id: Id,
        event: CreateEvent,
        scopes: Vec<String>,
        tx: Option<OneShotSender<DispatchTemplateResult>>,
    }
}

/// CreatedWorkerThreadMessage is a wrapper around WorkerThreadMessage 
/// that is 'public'. Not usable outside of this file though
pub struct CreatedWorkerThreadMessage(pub(self) WorkerThreadMessage);

pub trait PushableMessage {
    type Response: Send + Sync + 'static;

    fn into_message(self, tx: Option<OneShotSender<Self::Response>>) -> CreatedWorkerThreadMessage;
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

    fn into_message(self, tx: Option<OneShotSender<Self::Response>>) -> CreatedWorkerThreadMessage {
        match self.scopes {
            Some(scopes) => CreatedWorkerThreadMessage(WorkerThreadMessage::DispatchScopedEvent {
                id: self.id,
                event: self.event,
                scopes,
                tx,
            }),
            None => CreatedWorkerThreadMessage(WorkerThreadMessage::DispatchEvent {
                id: self.id,
                event: self.event,
                tx,
            }),
        }
    }
}

/// WorkerThread provides a simple thread implementation in which a ``Worker`` runs in its own thread with messages
/// sent to it over a channel
pub struct WorkerThread {
    /// The tx channel for sending messages to the worker thread
    tx: UnboundedSender<WorkerThreadMessage>,
    /// The id of the worker thread, used for debugging and logging
    id: u64,
    /// Handle to the worker thread
    handle: JoinHandle<()>,
}

impl WorkerThread {
    /// Creates a new WorkerThread with the given cache data and worker state
    pub fn new(cache: WorkerCacheData, state: WorkerState) -> Result<Self, crate::Error> {
        let id = rand::rng().random();

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        
        let handle = Self::create_thread(id, cache, state, rx)?;
        
        let worker_thread = Self { tx, id, handle };

       Ok(worker_thread)
    }

    fn create_thread(id: u64, cache: WorkerCacheData, state: WorkerState, mut rx: UnboundedReceiver<WorkerThreadMessage>) -> Result<JoinHandle<()>, crate::Error> {
        std::thread::Builder::new()
            .name(format!("lua-vm-threadpool-{}", id))
            .stack_size(MAX_VM_THREAD_STACK_SIZE)
            .spawn(move || {
                let res = std::panic::catch_unwind(AssertUnwindSafe(|| {
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build_local(tokio::runtime::LocalOptions::default())
                        .expect("Failed to create tokio runtime");

                    rt.block_on(async move {
                        let worker = Worker::new(cache, state);

                        // Listen to messages and handle them
                        while let Some(msg) = rx.recv().await {
                            match msg {
                                WorkerThreadMessage::DispatchEvent { id, event, tx } => {
                                    let res = worker.dispatch.dispatch_event_to_templates(id, event).await;
                                    if let Some(tx) = tx {
                                        let _ = tx.send(res);
                                    }
                                }
                                WorkerThreadMessage::DispatchScopedEvent { id, event, scopes, tx } => {
                                    let res = worker.dispatch.dispatch_scoped_event_to_templates(id, event, &scopes).await;
                                    if let Some(tx) = tx {
                                        let _ = tx.send(res);
                                    }
                                }
                            }
                        }
                    });
                }));

                if let Err(e) = res {
                    eprintln!("Worker thread panicked: {:?}", e);
                    std::process::abort(); // TODO: Handle this more gracefully
                }
            })
            .map_err(|e| format!("Failed to spawn worker thread: {e}").into())
    }

    /// Sends a message to the worker thread
    /// and waits for a response
    pub async fn send<T: PushableMessage>(&self, msg: T) -> Result<T::Response, crate::Error> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let msg = msg.into_message(Some(tx));
        self.tx.send(msg.0)
            .map_err(|e| format!("Failed to send message to worker thread: {e}"))?;
        rx.await.map_err(|e| format!("Failed to receive response from worker thread: {e}").into())
    }

    /// Sends a message to the worker thread 
    /// and wait for a response with a timeout
    pub async fn send_timeout<T: PushableMessage>(&self, msg: T, duration: Duration) -> Result<T::Response, crate::Error> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let msg = msg.into_message(Some(tx));
        self.tx.send(msg.0)
            .map_err(|e| format!("Failed to send message to worker thread: {e}"))?;

        tokio::select! {
            res = rx => res.map_err(|e| format!("Failed to receive response from worker thread: {e}").into()),
            _ = tokio::time::sleep(duration) => Err("Timed out waiting for response from worker thread".into()),
        }
    }

    /// Sends a message to the worker thread
    pub async fn send_nowait<T: PushableMessage>(&self, msg: T) -> Result<(), crate::Error> {
        let (tx, _rx) = tokio::sync::oneshot::channel();
        let msg = msg.into_message(Some(tx));
        self.tx.send(msg.0)
            .map_err(|e| format!("Failed to send message to worker thread: {e}"))?;
        Ok(())
    }
}

// Assert that WorkerThread is Send + Sync
const _: () = {
    const fn assert_send_sync_clone<T: Send + Sync>() {}
    assert_send_sync_clone::<WorkerThread>();
};
