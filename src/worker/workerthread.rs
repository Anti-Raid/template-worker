use serenity::async_trait;
use khronos_runtime::primitives::event::CreateEvent;
use khronos_runtime::utils::khronos_value::KhronosValue;
use tokio::sync::mpsc::{UnboundedSender, UnboundedReceiver};
use tokio::sync::oneshot::Sender as OneShotSender;
use std::sync::Arc;
use std::panic::AssertUnwindSafe;

use crate::worker::limits::MAX_VM_THREAD_STACK_SIZE;
use crate::worker::workerlike::WorkerLike;
use crate::worker::workerpool::Poolable;
use crate::worker::workerstate::CreateWorkerState;
use super::{workerstate::WorkerState, worker::Worker, workervmmanager::Id, workerfilter::WorkerFilter};

/// WorkerThreadMessage is the message type that is sent to the worker thread
enum WorkerThreadMessage {
    Kill {
        tx: OneShotSender<Result<(), crate::Error>>,
    },
    DropTenant {
        id: Id,
        tx: OneShotSender<Result<(), crate::Error>>,
    },
    RunScript {
        id: Id,
        name: String,
        code: String,
        event: CreateEvent,
        tx: OneShotSender<Result<KhronosValue, crate::Error>>,
    },
    DispatchEvent {
        id: Id,
        event: CreateEvent,
        tx: Option<OneShotSender<Result<KhronosValue, crate::Error>>>,
    },
}

/// WorkerThread provides a simple thread implementation in which a ``Worker`` runs in its own thread with messages
/// sent to it over a channel
#[derive(Clone)]
pub struct WorkerThread {
    /// The tx channel for sending messages to the worker thread
    tx: UnboundedSender<WorkerThreadMessage>,
    /// The id of the worker thread, used for routing
    id: usize,
}

impl WorkerThread {
    /// Creates a new WorkerThread with the given cache data and worker state
    pub fn new(state: CreateWorkerState, filter: WorkerFilter, id: usize) -> Result<Self, crate::Error> {

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        
        Self::create_thread(id, state, filter, rx)?;
        
        let worker_thread = Self { tx, id };

       Ok(worker_thread)
    }

    fn create_thread(id: usize, state: CreateWorkerState, filter: WorkerFilter, mut rx: UnboundedReceiver<WorkerThreadMessage>) -> Result<(), crate::Error> {
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
                        let worker = Worker::new(state, filter).expect("Failed to create Worker");

                        // Listen to messages and handle them
                        while let Some(msg) = rx.recv().await {
                            match msg {
                                WorkerThreadMessage::Kill { tx } => {
                                    log::info!("Killing worker thread with ID: {}", id);
                                    let _ = tx.send(Ok(()));
                                    return; // Exitting the loop will stop the thread automatically
                                }
                                WorkerThreadMessage::DispatchEvent { id, event, tx } => {
                                    let res = worker.dispatch.dispatch_event(id, event).await;
                                    if let Some(tx) = tx {
                                        let _ = tx.send(res.map_err(|e| e.to_string().into()));
                                    }
                                }
                                WorkerThreadMessage::RunScript { id, name, code, event, tx } => {
                                    let res = worker.dispatch.run_script(id, name, code, event).await;
                                    let _ = tx.send(res.map_err(|e| e.to_string().into()));
                                }
                                WorkerThreadMessage::DropTenant { id, tx } => {
                                    let res = worker.vm_manager.remove_vm_for(id);
                                    let _ = tx.send(res.map_err(|e| e.to_string().into()));
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
            .map_err(|e| format!("Failed to spawn worker thread: {e}"))?;

        Ok(())
    }
}

#[async_trait]
impl WorkerLike for WorkerThread {
    fn id(&self) -> usize {
        self.id
    }

    async fn kill(&self) -> Result<(), crate::Error> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.tx.send(WorkerThreadMessage::Kill { tx: tx })
            .map_err(|e| format!("Failed to send message to worker thread: {e}"))?;
        Ok(rx.await.map_err(|e| format!("Failed to receive response from worker thread: {e}"))??)
    }

    fn clone_to_arc(&self) -> Arc<dyn WorkerLike + Send + Sync> {
        Arc::new(self.clone())
    }

    async fn run_script(&self, id: Id, name: String, code: String, event: CreateEvent) -> Result<KhronosValue, crate::Error> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.tx.send(WorkerThreadMessage::RunScript { id, name, code, event, tx })
            .map_err(|e| format!("Failed to send message to worker thread: {e}"))?;
        Ok(rx.await.map_err(|e| format!("Failed to receive response from worker thread: {e}"))??)
    }

    async fn dispatch_event(&self, id: Id, event: CreateEvent) -> Result<KhronosValue, crate::Error> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.tx.send(WorkerThreadMessage::DispatchEvent { id, event, tx: Some(tx) })
            .map_err(|e| format!("Failed to send message to worker thread: {e}"))?;
        Ok(rx.await.map_err(|e| format!("Failed to receive response from worker thread: {e}"))??)
    }

    fn dispatch_event_nowait(&self, id: Id, event: CreateEvent) -> Result<(), crate::Error> {
        self.tx.send(WorkerThreadMessage::DispatchEvent { id, event, tx: None })
            .map_err(|e| format!("Failed to send message to worker thread: {e}"))?;
        Ok(())
    }

    async fn drop_tenant(&self, id: Id) -> Result<(), crate::Error> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.tx.send(WorkerThreadMessage::DropTenant { id, tx })
            .map_err(|e| format!("Failed to send message to worker thread: {e}"))?;
        Ok(rx.await.map_err(|e| format!("Failed to receive response from worker thread: {e}"))??)
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
