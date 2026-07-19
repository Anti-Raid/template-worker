use std::{sync::{Arc, RwLock}, time::{Duration, Instant}};

use tokio::{process::Command, time::sleep};
use tokio::sync::oneshot;
use crate::{CONFIG, mesophyll::connman::SockFile};
use std::sync::atomic::{AtomicU32, Ordering};

/// Simple exponential backoff struct for worker restarts. 
/// 
/// Not super important to be perfect as this is just to stop the worker pool from thrashing 
/// if there is a persistent error causing workers to immediately exit on spawn.
#[derive(Clone)]
pub struct ExpBackoff {
    current: Arc<AtomicU32>,
}

impl ExpBackoff {
    const INITIAL: u32 = 0;
    const STEP: u32 = 1000; // 1 second
    const MAX: u32 = 10_000; // 10 seconds

    pub fn new() -> Self {
        Self {
            current: Arc::new(AtomicU32::new(Self::INITIAL)),
        }
    }

    /// Resets the backoff to the initial value
    pub fn reset(&self) {
        self.current.store(Self::INITIAL, Ordering::Relaxed);
    }

    /// Returns the current backoff value and updates it for the next call
    pub fn next_backoff(&self) -> u32 {
        let current = self.current.load(Ordering::Relaxed);
        
        let next = if current == 0 {
            Self::STEP
        } else {
            (current * 3).min(Self::MAX)
        };     
           
        self.current.store(next, Ordering::Relaxed);
        current    }
}


#[derive(Clone, Debug)]
pub enum WorkerProcessStatus {
    Starting,
    Ready, // Only state at which the worker process can accept events
    Errored { err: Arc<std::io::Error>, on_spawn: bool },
    Exited { status: Option<std::process::ExitStatus> },
}

#[derive(Clone)]
struct StatusHolder(Arc<RwLock<WorkerProcessStatus>>);
impl StatusHolder {
    fn new() -> Self {
        Self(Arc::new(RwLock::new(WorkerProcessStatus::Starting)))
    }

    // Sets the status of the worker process
    fn set(&self, status: WorkerProcessStatus) {
        let mut s = self.0.write().unwrap();
        *s = status;
    }

    // Gets the status of the worker process
    fn get(&self) -> WorkerProcessStatus {
        let s = self.0.read().unwrap();
        s.clone()
    }
}

/// A lightweight, clonable handle to monitor and kill the worker
#[derive(Clone)]
pub struct WorkerProcessHandle {
    id: usize,
    worker_debug: bool,
    status: StatusHolder,
    backoff: ExpBackoff,
    master_sockfile: Arc<SockFile>
}

impl WorkerProcessHandle {
    /// Creates a new WorkerProcessHandle with the given ID and worker_debug flag
    pub fn new(id: usize, worker_debug: bool, backoff: ExpBackoff, master_sockfile: Arc<SockFile>) -> Self {
        Self {
            id,
            worker_debug,
            status: StatusHolder::new(),
            backoff,
            master_sockfile
        }
    }

    /// Returns the ID of the worker process
    pub fn id(&self) -> usize {
        self.id
    }

    /// Returns the current status of the worker process
    pub fn status(&self) -> WorkerProcessStatus {
        self.status.get()
    }

    /// Spawns the worker process and monitors it for exit or kill signals
    /// 
    /// Stores the status of the worker process in the `status`. Does not auto-restart
    /// (thats the responsibility of the caller i.e. the WorkerPool) 
    pub async fn run(&self, kill_signal: oneshot::Receiver<()>) {
        let delay_ms = self.backoff.next_backoff();
        if delay_ms > 0 {
            log::warn!("Crash loop detected! Worker {} backing off for {}ms", self.id, delay_ms);
            sleep(Duration::from_millis(delay_ms as u64)).await;
        }

        let mut command = Command::new(&CONFIG.worker_path);
        command.arg(self.id.to_string());
        if self.worker_debug {
            command.env("WORKER_DEBUG", "true");
        }
        // Set mesophyll params for dir + master sock
        command.env("MESO_DIR", &self.master_sockfile.dir);
        command.env("MESO_MSOCK", &self.master_sockfile.sock);

        command.kill_on_drop(true);

        let start_time = Instant::now();

        // spawn process
        let mut child = match command.spawn() {
            Ok(proc) => {
                self.status.set(WorkerProcessStatus::Ready);
                proc
            }
            Err(e) => {
                self.status.set(WorkerProcessStatus::Errored { err: e.into(), on_spawn: true });
                return;
            }
        };

        // Wait for the process to exit
        tokio::select! {
            resp = child.wait() => {
                match resp {
                    Ok(status) => self.status.set(WorkerProcessStatus::Exited { status: Some(status) }),
                    Err(e) => self.status.set(WorkerProcessStatus::Errored { err: e.into(), on_spawn: false }),
                }
            }
            Ok(_) = kill_signal => {
                log::info!("Received kill message for worker process with ID: {}, terminating process", self.id);
                match child.kill().await {
                    Ok(_) => self.status.set(WorkerProcessStatus::Exited { status: None }),
                    Err(e) => self.status.set(WorkerProcessStatus::Errored { err: e.into(), on_spawn: false }),
                }
                return; // Return early after killing the process to avoid resetting backoff on intentional kill
            }
        }

        // If the process lived for more than 5 seconds before dying, it wasn't a crash loop. 
        // Reset the backoff so the next restart is instant.
        if start_time.elapsed().as_secs() > 5 {
            log::info!("Worker {} was healthy before exiting. Resetting backoff.", self.id);
            self.backoff.reset();
        }
    }
}

// Assert that WorkerProcessHandle is Send + Sync
const _: () = {
    const fn assert_send_sync_clone<T: Send + Sync>() {}
    assert_send_sync_clone::<WorkerProcessHandle>();
};