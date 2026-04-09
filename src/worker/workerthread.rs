use khronos_runtime::primitives::event::CreateEvent;
use khronos_runtime::utils::khronos_value::KhronosValue;
use tokio::sync::mpsc::{UnboundedSender, UnboundedReceiver};
use tokio::sync::oneshot::Sender as OneShotSender;
use std::panic::AssertUnwindSafe;

use crate::worker::limits::MAX_VM_THREAD_STACK_SIZE;
use crate::worker::workerstate::WorkerState;
use super::{worker::Worker, workervmmanager::Id};

/// WorkerThreadMessage is the message type that is sent to the worker thread
enum WorkerThreadMessage {
    Kill {
        tx: OneShotSender<Result<(), crate::Error>>,
    },
    DropTenant {
        id: Id,
        tx: OneShotSender<Result<(), crate::Error>>,
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
    pub fn new(state: WorkerState, id: usize) -> Result<Self, crate::Error> {

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        
        Self::create_thread(id, state, rx)?;
        
        let worker_thread = Self { tx, id };

       Ok(worker_thread)
    }

    /// `id` is the worker thread ID, used for routing. `state` is the state to create the worker with. `rx` is the channel receiver for receiving messages from the worker thread.
    fn create_thread(id: usize, state: WorkerState, mut rx: UnboundedReceiver<WorkerThreadMessage>) -> Result<(), crate::Error> {
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
                        let worker = Worker::new(state).await.expect("Failed to setup worker");

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

impl WorkerThread {
    pub fn id(&self) -> usize {
        self.id
    }

    pub async fn kill(&self) -> Result<(), crate::Error> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.tx.send(WorkerThreadMessage::Kill { tx: tx })
            .map_err(|e| format!("Failed to send message to worker thread: {e}"))?;
        Ok(rx.await.map_err(|e| format!("Failed to receive response from worker thread: {e}"))??)
    }

    pub async fn dispatch_event(&self, id: Id, event: CreateEvent) -> Result<KhronosValue, crate::Error> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.tx.send(WorkerThreadMessage::DispatchEvent { id, event, tx: Some(tx) })
            .map_err(|e| format!("Failed to send message to worker thread: {e}"))?;
        Ok(rx.await.map_err(|e| format!("Failed to receive response from worker thread: {e}"))??)
    }

    pub fn dispatch_event_nowait(&self, id: Id, event: CreateEvent) -> Result<(), crate::Error> {
        self.tx.send(WorkerThreadMessage::DispatchEvent { id, event, tx: None })
            .map_err(|e| format!("Failed to send message to worker thread: {e}"))?;
        Ok(())
    }

    pub async fn drop_tenant(&self, id: Id) -> Result<(), crate::Error> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.tx.send(WorkerThreadMessage::DropTenant { id, tx })
            .map_err(|e| format!("Failed to send message to worker thread: {e}"))?;
        Ok(rx.await.map_err(|e| format!("Failed to receive response from worker thread: {e}"))??)
    }
}

// Assert that WorkerThread is Send + Sync
const _: () = {
    const fn assert_send_sync_clone<T: Send + Sync>() {}
    assert_send_sync_clone::<WorkerThread>();
};
