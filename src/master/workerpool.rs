use dapi::UserId;
use khronos_runtime::utils::khronos_value::KhronosValue;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::sleep;

use crate::geese::stream::{CtlMessage, LtcMessage};
use crate::geese::tenantstate::TenantState;
use crate::master::workerprocesshandle::{ExpBackoff, WorkerProcessHandle};
use crate::mesophyll::connman::SockFile;
use crate::mesophyll::server::{AttachedStreamGuard, MesophyllServer};
use crate::worker::workerdispatch::SimpleEvent;
use crate::worker::workervmmanager::Id;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use parking_lot::{Mutex, RwLock};
use tokio::sync::{mpsc, oneshot};

#[derive(Clone)]
/// A WorkerPool stores a pool of workers in which servers are evenly distributed via
/// the Discord Id sharding formula
pub struct WorkerPool {
    /// Pool size
    pool_size: usize,

    /// The fast-read collection of worker handles in the pool
    workers: Arc<RwLock<Vec<WorkerProcessHandle>>>,
    
    /// The kill switches, held by the pool so the pool can trigger clean shutdowns
    kill_switches: Arc<Mutex<Vec<Option<oneshot::Sender<()>>>>>,
    
    /// The underlying mesophyll server thats used for communication between master->worker
    mesophyll: MesophyllServer,

    /// Whether the worker pool is in the process of shutting down. 
    is_shutting_down: Arc<AtomicBool>
}

impl WorkerPool {
    /// Initializes the worker process pool and starts the supervisor task.
    /// 
    /// The supervisor task monitors the worker processes for unexpected exits and respawns them, 
    /// and also listens for shutdown signals to gracefully kill the workers.
    pub fn new(pool_size: usize, worker_debug: bool, mesophyll: MesophyllServer) -> Self {
        let sock_file = mesophyll.sock_file().clone();
        let mut workers = Vec::with_capacity(pool_size);
        let mut kill_switches = Vec::with_capacity(pool_size);
        let mut backoffs = Vec::with_capacity(pool_size);
        let is_shutting_down = Arc::new(AtomicBool::new(false));
        for _ in 0..pool_size {
            backoffs.push(ExpBackoff::new());
        }

        // Each worker process takes the worker_ctrl_tx side of the channel, and the supervisor takes the worker_ctrl_rx side
        // to ensure that the supervisor is notified when a worker process exits for whatever reason
        let (worker_ctrl_tx, worker_ctrl_rx) = mpsc::channel(pool_size);

        for id in 0..pool_size {
            let handle = WorkerProcessHandle::new(id, worker_debug, backoffs[id].clone(), sock_file.clone());
            let (kill_tx, kill_rx) = oneshot::channel();

            workers.push(handle.clone());
            kill_switches.push(Some(kill_tx));

            // Background task to run the worker process and notify the supervisor on exit
            let worker_ctrl_tx_ref = worker_ctrl_tx.clone();
            tokio::spawn(async move {
                handle.run(kill_rx).await;
                let _ = worker_ctrl_tx_ref.send(id).await;
            });
        }

        let pool = Self {
            pool_size,
            workers: Arc::new(RwLock::new(workers)),
            kill_switches: Arc::new(Mutex::new(kill_switches)),
            mesophyll,
            is_shutting_down: is_shutting_down.clone()
        };

        let workers_ref = pool.workers.clone();
        let kill_switches_ref = pool.kill_switches.clone();
        
        tokio::spawn(async move {
            Self::supervisor(
                workers_ref, 
                kill_switches_ref, 
                worker_ctrl_rx, 
                worker_ctrl_tx, 
                backoffs,
                is_shutting_down,
                worker_debug,
                sock_file
            ).await;
        });

        pool
    }

    /// The supervisor loop. See `new` for details.
    async fn supervisor(
        workers: Arc<RwLock<Vec<WorkerProcessHandle>>>, 
        kill_switches: Arc<Mutex<Vec<Option<oneshot::Sender<()>>>>>,
        mut worker_ctrl_rx: mpsc::Receiver<usize>,
        worker_ctrl_tx: mpsc::Sender<usize>, 
        backoffs: Vec<ExpBackoff>, // held permamently to track backoffs now
        is_shutting_down: Arc<AtomicBool>,
        debug: bool,
        sock_file: Arc<SockFile>
    ) {
        while let Some(dead_id) = worker_ctrl_rx.recv().await {
            if is_shutting_down.load(Ordering::Relaxed) {
                break;
            }

            log::error!("Worker {} died. Trying to restart...", dead_id);
                
            let new_handle = WorkerProcessHandle::new(dead_id, debug, backoffs[dead_id].clone(), sock_file.clone());
            let (new_kill_tx, new_kill_rx) = oneshot::channel();

            // Update the worker handle in the pool
            {
                let mut write_guard = workers.write();
                write_guard[dead_id] = new_handle.clone();
            }

            // Swap the kill switch 
            {
                let mut kills_guard = kill_switches.lock();
                kills_guard[dead_id] = Some(new_kill_tx);
            }

            // Background task to run the worker process and notify the supervisor on exit
            let worker_ctrl_tx_ref = worker_ctrl_tx.clone();
            tokio::spawn(async move {
                new_handle.run(new_kill_rx).await;
                let _ = worker_ctrl_tx_ref.send(dead_id).await;
            });
        }
    }

    /// Explicitly kills all workers with open kill switches
    pub async fn shutdown_all(&self) -> Result<(), crate::Error> {
        self.is_shutting_down.store(true, Ordering::SeqCst);
        let mut kills_guard = self.kill_switches.lock();
        for kill_opt in kills_guard.iter_mut() {
            if let Some(tx) = kill_opt.take() {
                let _ = tx.send(());
            }
        }

        sleep(std::time::Duration::from_secs(5)).await; // wait for workers to shut down. TODO: make this more robust by tracking worker shutdowns in the supervisor loop

        Ok(())
    }

    pub fn mesophyll(&self) -> &MesophyllServer {
        &self.mesophyll
    }

    pub async fn dispatch_event(&self, id: Id, event: SimpleEvent) -> Result<KhronosValue, crate::Error> {
        let worker_id = id.worker_id(self.pool_size);
        let r = self.mesophyll.get_connection(worker_id)
            .ok_or_else(|| format!("No Mesophyll connection found for worker process with ID: {}", worker_id))?;
        r.dispatch_event(id, event).await
    }

    pub async fn drop_tenant(&self, id: Id) -> Result<(), crate::Error> {
        let worker_id = id.worker_id(self.pool_size);
        let r = self.mesophyll.get_connection(worker_id)
            .ok_or_else(|| format!("No Mesophyll connection found for worker process with ID: {}", worker_id))?;
        r.drop_tenant(id).await
    }

    pub async fn update_tenant_state(&self, id: Id, ts: TenantState) -> Result<bool, crate::Error> {
        let worker_id = id.worker_id(self.pool_size);
        let r = self.mesophyll.get_connection(worker_id)
            .ok_or_else(|| format!("No Mesophyll connection found for worker process with ID: {}", worker_id))?;
        r.update_tenant_state(id, ts).await
    }

    pub async fn attach_stream(&self, id: Id, user_id: UserId) -> Result<(AttachedStreamGuard, UnboundedReceiver<LtcMessage>), crate::Error> {
        let r = self.mesophyll.get_connection(id.worker_id(self.pool_size)).ok_or("Failed to get worker")?;
        r.attach_stream(id, user_id).await
    }

    pub async fn stream_message(&self, id: Id, payload: CtlMessage) -> Result<(), crate::Error> {
        let r = self.mesophyll.get_connection(id.worker_id(self.pool_size))
        .ok_or_else(|| format!("No Mesophyll connection found for worker process with ID: {}", id.worker_id(self.pool_size)))?;
        r.stream_message(id, payload).await
    }
}

// Assert that WorkerPool is Send + Sync
const _: () = {
    const fn assert_send_sync_clone<T: Send + Sync>() {}
    assert_send_sync_clone::<WorkerPool>();
};
