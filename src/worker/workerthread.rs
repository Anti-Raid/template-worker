use khronos_runtime::primitives::event::CreateEvent;
use khronos_runtime::utils::khronos_value::KhronosValue;
use tokio::sync::mpsc::{UnboundedSender, UnboundedReceiver};
use tokio::sync::oneshot::Sender as OneShotSender;
use std::time::Duration;
use std::{panic::AssertUnwindSafe, thread::JoinHandle};

use crate::worker::limits::MAX_VM_THREAD_STACK_SIZE;
use crate::worker::workerlike::WorkerLike;
use crate::worker::workerpool::Poolable;
use crate::worker::workerstate::CreateWorkerState;
use super::{workerstate::WorkerState, worker::Worker, workervmmanager::Id, workerfilter::WorkerFilter};

/// WorkerThreadMessage is the message type that is sent to the worker thread
enum WorkerThreadMessage {
    Kill {
        tx: Option<OneShotSender<Result<(), crate::Error>>>,
    },
    DispatchEvent {
        id: Id,
        event: CreateEvent,
        tx: Option<OneShotSender<Result<KhronosValue, crate::Error>>>,
    },
}

trait PushableMessage {
    type Response: Send + Sync + 'static;

    fn into_message(self, tx: Option<OneShotSender<Self::Response>>) -> WorkerThreadMessage;
}

pub struct Kill {}

impl PushableMessage for Kill {
    type Response = Result<(), crate::Error>;

    fn into_message(self, tx: Option<OneShotSender<Self::Response>>) -> WorkerThreadMessage {
        WorkerThreadMessage::Kill { tx }
    }
}

pub struct DispatchEvent {
    /// The id of the template to dispatch the event to 
    pub id: Id,
    /// The event to dispatch
    pub event: CreateEvent,
}

impl PushableMessage for DispatchEvent {
    type Response = Result<KhronosValue, crate::Error>;

    fn into_message(self, tx: Option<OneShotSender<Self::Response>>) -> WorkerThreadMessage {
        WorkerThreadMessage::DispatchEvent {
            id: self.id,
            event: self.event,
            tx,
        }
    }
}

/// WorkerThread provides a simple thread implementation in which a ``Worker`` runs in its own thread with messages
/// sent to it over a channel
#[allow(unused)]
pub struct WorkerThread {
    /// The tx channel for sending messages to the worker thread
    tx: UnboundedSender<WorkerThreadMessage>,
    /// The id of the worker thread, used for routing
    id: usize,
    /// Handle to the worker thread
    handle: JoinHandle<()>,
}

impl WorkerThread {
    /// Creates a new WorkerThread with the given cache data and worker state
    pub fn new(state: CreateWorkerState, filter: WorkerFilter, id: usize) -> Result<Self, crate::Error> {

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        
        let handle = Self::create_thread(id, state, filter, rx)?;
        
        let worker_thread = Self { tx, id, handle };

       Ok(worker_thread)
    }

    fn create_thread(id: usize, state: CreateWorkerState, filter: WorkerFilter, mut rx: UnboundedReceiver<WorkerThreadMessage>) -> Result<JoinHandle<()>, crate::Error> {
        std::thread::Builder::new()
            .name(format!("lua-vm-threadpool-{id}"))
            .stack_size(MAX_VM_THREAD_STACK_SIZE)
            .spawn(move || {
                let res = std::panic::catch_unwind(AssertUnwindSafe(|| {
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build_local(tokio::runtime::LocalOptions::default())
                        .expect("Failed to create tokio runtime");

                    rt.block_on(async move {
                        let state = WorkerState::new(state).await.expect("Failed to create WorkerState");
                        let worker = Worker::new(state, filter).await.expect("Failed to create Worker");

                        // Listen to messages and handle them
                        while let Some(msg) = rx.recv().await {
                            match msg {
                                WorkerThreadMessage::Kill { tx } => {
                                    log::info!("Killing worker thread with ID: {}", id);
                                    if let Some(tx) = tx {
                                        let _ = tx.send(Ok(()));
                                    }
                                    return; // Exitting the loop will stop the thread automatically
                                }
                                WorkerThreadMessage::DispatchEvent { id, event, tx } => {
                                    let res = worker.dispatch.dispatch_event(id, event).await;
                                    if let Some(tx) = tx {
                                        let _ = tx.send(res.map_err(|e| e.to_string().into()));
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
    async fn send<T: PushableMessage>(&self, msg: T) -> Result<T::Response, crate::Error> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let msg = msg.into_message(Some(tx));
        self.tx.send(msg)
            .map_err(|e| format!("Failed to send message to worker thread: {e}"))?;
        rx.await.map_err(|e| format!("Failed to receive response from worker thread: {e}").into())
    }

    /// Sends a message to the worker thread 
    /// and wait for a response with a timeout
    #[allow(dead_code)]
    async fn send_timeout<T: PushableMessage>(&self, msg: T, duration: Duration) -> Result<T::Response, crate::Error> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let msg = msg.into_message(Some(tx));
        self.tx.send(msg)
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
        self.tx.send(msg)
            .map_err(|e| format!("Failed to send message to worker thread: {e}"))?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl WorkerLike for WorkerThread {
    fn id(&self) -> usize {
        self.id
    }

    async fn kill(&self) -> Result<(), crate::Error> {
        self.send(Kill {}).await?
    }

    async fn dispatch_event(&self, id: Id, event: CreateEvent) -> Result<KhronosValue, crate::Error> {
        self.send(DispatchEvent {
            id,
            event,
        }).await?
    }

    async fn dispatch_event_nowait(&self, id: Id, event: CreateEvent) -> Result<(), crate::Error> {
        self.send_nowait(DispatchEvent {
            id,
            event,
        }).await
    }
}

// WorkerThread's can be pooled via WorkerPool!
impl Poolable for WorkerThread {
    type ExtState = CreateWorkerState;
    fn new(filter: WorkerFilter, id: usize, _num_threads: usize, ext_state: &Self::ExtState) -> Result<Self, crate::Error>
        where
            Self: Sized {
        Self::new(ext_state.clone(), filter, id)
    }
}

// Assert that WorkerThread is Send + Sync
const _: () = {
    const fn assert_send_sync_clone<T: Send + Sync>() {}
    assert_send_sync_clone::<WorkerThread>();
};
