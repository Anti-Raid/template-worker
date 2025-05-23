use khronos_runtime::rt::KhronosRuntimeManager;
use serenity::all::GuildId;
use std::rc::Rc;
use std::sync::atomic::AtomicUsize;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::sync::oneshot::{Sender, Receiver};
use std::sync::RwLock as StdRwLock;
use std::cell::{Cell, RefCell};
use crate::templatingrt::state::CreateGuildState;
use crate::config::VmDistributionStrategy;
use super::core::{
    configure_runtime_manager,
};
use super::{client::ArLua, ArLuaHandle, LuaVmAction, LuaVmResult};
use crate::templatingrt::MAX_VM_THREAD_STACK_SIZE;
use crate::templatingrt::state::GuildState;

pub const DEFAULT_MAX_THREADS: usize = 400; // Maximum number of threads in the pool

enum ThreadRequest {
    Dispatch {
        guild_id: GuildId,
        action: LuaVmAction,
        callback: Sender<Vec<(String, LuaVmResult)>>,
    },
    Ping {
        tx: Sender<()>,
    },
    ClearInactiveGuilds {
        tx: Sender< HashMap<GuildId, Option<String>> >,
    },
    RemoveIfUnused {
        tx: Sender<bool>
    },
    CloseThread {
        tx: Option<Sender<()>>
    }
}

/// A thread entry in the pool (worker thread)
struct ThreadEntry {
    /// The unique identifier for the thread entry
    id: u64,
    /// Number of guilds in the pool
    count: Arc<AtomicUsize>,
    /// A sender to create a new guild handle
    tx: UnboundedSender<ThreadRequest>,
}

impl ThreadEntry {
    /// Creates a new thread entry
    fn new(tx: UnboundedSender<ThreadRequest>) -> Self {
        Self {
            id: {
                // Generate a random id
                use rand::Rng;

                rand::thread_rng().gen()
            },
            count: Arc::new(AtomicUsize::new(0)),
            tx,
        }
    }

    /// Initializes a new thread entry, starting it after creation
    fn create(
        cgs: CreateGuildState,
    ) -> Result<Self, silverpelt::Error> {
        let (tx, rx) = unbounded_channel::<ThreadRequest>();

        let entry = Self::new(tx);

        entry.start(cgs, rx)?;

        Ok(entry)
    }

    /// Returns the number of servers in the pool
    fn thread_count(&self) -> usize {
        self.count.load(std::sync::atomic::Ordering::Acquire)
    }

    /// Start the thread up
    fn start(
        &self,
        cgs: CreateGuildState,
        rx: UnboundedReceiver<ThreadRequest>,
    ) -> Result<(), silverpelt::Error> {
        let mut rx = rx; // Take mutable ownership to receiver
        let count_ref = self.count.clone();
        let tid = self.id;
        std::thread::Builder::new()
            .name(format!("lua-vm-threadpool-{}", self.id))
            .stack_size(MAX_VM_THREAD_STACK_SIZE)
            .spawn(move || {
                super::perthreadpanichook::set_hook(Box::new(move |_| {
                    if let Err(e) = DEFAULT_THREAD_POOL.remove_thread(tid) {
                        log::error!("Error removing thread on panic: {:?}", e)
                    }
                }));

                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("Failed to create tokio runtime");

                let local = tokio::task::LocalSet::new();
                local.block_on(&rt, async {
                    // Keep waiting for new events
                    struct VmData {
                        guild_state: Rc<GuildState>,
                        tis_ref: KhronosRuntimeManager,
                        count: Rc<Cell<usize>>
                    }

                    let thread_vms: Rc<RefCell<HashMap<GuildId, Rc<VmData>>>> = Rc::new(HashMap::new().into());

                    while let Some(send) = rx.recv().await {
                        match send {
                            ThreadRequest::Ping { tx }=> {
                                // Send a pong
                                let _ = tx.send(());
                            }
                            ThreadRequest::Dispatch { guild_id, action, callback } => {
                                let vm = {
                                    let mut vms = thread_vms.borrow_mut();

                                    // Create server if not found, otherwise return existing
                                    match vms.get(&guild_id) {
                                        Some(vm) => vm.clone(),
                                        None => {
                                            // Create Lua VM
                                            let cgs_ref = cgs.clone();
                                            let gs = Rc::new(
                                                match cgs_ref.to_guild_state(guild_id) {
                                                    Ok(gs) => gs,
                                                    Err(e) => {
                                                        log::error!("Failed to create guild state: {}", e);
                                                        continue;
                                                    }
                                                }
                                            );
    
                                            let tis_ref = match configure_runtime_manager() {
                                                Ok(tis) => tis,
                                                Err(e) => {
                                                    log::error!("Failed to configure Lua VM: {}", e);
                                                    continue;
                                                }
                                            };
    
                                            count_ref.fetch_add(1, std::sync::atomic::Ordering::Release);
    
                                            let thread_vms_ref = thread_vms.clone();
                                            tis_ref.set_on_broken(Box::new(move || {
                                                super::remove_vm(guild_id);
                                                {
                                                    let mut vms = thread_vms_ref.borrow_mut();
                                                    vms.remove(&guild_id);
                                                }
                                            }));
    
                                            // Store into the thread
                                            let vmd = Rc::new(VmData {
                                                guild_state: gs,
                                                tis_ref: tis_ref,
                                                count: Cell::new(0).into(),
                                            });

                                            vms.insert(
                                                guild_id,
                                                vmd.clone(),
                                            );
    
                                            vmd
                                        }
                                    }    
                                };

                                let gcount_ref = vm.count.clone();
                                tokio::task::spawn_local(async move {
                                    gcount_ref.set(gcount_ref.get() + 1);
                                    let tis_ref = vm.tis_ref.clone();
                                    let gs = vm.guild_state.clone();
                                    action.handle(tis_ref, gs, callback).await;
                                    gcount_ref.set(gcount_ref.get() - 1);
                                });
                            },
                            ThreadRequest::ClearInactiveGuilds { tx } => {
                                let mut removed = vec![];

                                {
                                    let vms = thread_vms.borrow();
                                    for (guild_id, vm) in vms.iter() {
                                        if let Some(let_) = vm.tis_ref.runtime().last_execution_time() {
                                            if std::time::Instant::now() - let_ > crate::templatingrt::MAX_SERVER_INACTIVITY {
                                                removed.push((*guild_id, vm.tis_ref.clone()));
                                            }
                                        }
                                    }
                                }

                                let mut close_errors = HashMap::new();
                                for (guild_id, removed) in removed.into_iter() {
                                    match removed.runtime().mark_broken(true) {
                                        Ok(()) => close_errors.insert(guild_id, None),
                                        Err(e) => close_errors.insert(guild_id, Some(e.to_string()))
                                    };
                                }

                                let _ = tx.send(close_errors);
                            },
                            ThreadRequest::RemoveIfUnused { tx } => {
                                let vms = thread_vms.borrow();
                                if vms.is_empty() {
                                    log::debug!("Clearing unused thread: {}", tid);
                                    let _ = tx.send(true);
                                    return;
                                }

                                let _ = tx.send(false);
                            },
                            ThreadRequest::CloseThread { tx } => {
                                if let Some(tx) = tx {
                                    let _ = tx.send(());
                                }

                                log::debug!("Closing thread: {}", tid);
                                return;
                            }
                        }
                    }
                })
            })?;

        Ok(())
    }
}

pub struct ThreadPool {
    /// The worker threads in the pool
    ///
    /// We can't use a binary heap here due to interior mutability of ordering [count]
    threads: StdRwLock<Vec<ThreadEntry>>,

    /// A record mapping a guild id to the thread its currently on
    guilds: StdRwLock<HashMap<GuildId, u64>>,

    /// The maximum number of threads in the pool
    max_threads: usize,
}

impl ThreadPool {
    /// Creates a new thread pool
    pub fn new() -> Self {
        Self {
            threads: StdRwLock::new(Vec::new()),
            guilds: StdRwLock::new(HashMap::new()),
            max_threads: DEFAULT_MAX_THREADS,
        }
    }

    #[allow(dead_code)]
    pub async fn clear_inactive_guilds(
        &self
    ) -> Result<HashMap<u64, Receiver<HashMap<GuildId, Option<String>>>>, crate::Error> {
        let resp = {
            let threads = self.threads.try_read().map_err(|_| "Failed to read threads")?;
            let mut res = HashMap::new();
            for thread in threads.iter() {
                let (tx, rx) = tokio::sync::oneshot::channel();
                thread.tx.send(
                    ThreadRequest::ClearInactiveGuilds {
                        tx
                    }
                )?;
                res.insert(thread.id, rx);
            }

            res
        };

        self.remove_unused_threads().await?;

        Ok(resp)
    }

    /// Remove broken threads from the pool
    pub async fn remove_unused_threads(&self) -> Result<(), silverpelt::Error> {
        let (mut good_threads, old_threads) = {
            let mut threads = self.threads.try_write().map_err(|_| "Failed to write lock threads")?;

            let good_threads = Vec::with_capacity(threads.len());   
            let old_threads = std::mem::take(&mut *threads);

            (good_threads, old_threads)
        };

        for thread in old_threads {
            // Send Ping to thread
            let (tx, rx) = tokio::sync::oneshot::channel();
            let _ = thread.tx.send(ThreadRequest::RemoveIfUnused { tx });
            tokio::select! {
                resp = rx => {
                    // If we get a response, the thread is alive
                    if let Ok(res) = resp {
                        if res {
                            good_threads.push(thread);
                        }

                        // Not alive
                        continue;
                    }
                }
                _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {
                    // Timeout
                }
            };

            // Delete the thread by doing nothing
            log::warn!("Thread {} is unused, removing it from the pool", thread.id);
        }

        {
            let mut threads = self.threads.try_write().map_err(|_| "Failed to write lock threads")?; 
            *threads = good_threads;
        }

        Ok(())
    }

    /// Remove broken threads from the pool
    pub async fn remove_broken_threads(&self) -> Result<(), silverpelt::Error> {
        let (mut good_threads, old_threads) = {
            let mut threads = self.threads.try_write().map_err(|_| "Failed to write lock threads")?;

            let good_threads = Vec::with_capacity(threads.len());   
            let old_threads = std::mem::take(&mut *threads);

            (good_threads, old_threads)
        };

        for thread in old_threads {
            // Send Ping to thread
            let (tx, rx) = tokio::sync::oneshot::channel();
            let _ = thread.tx.send(ThreadRequest::Ping { tx });
            tokio::select! {
                resp = rx => {
                    // If we get a response, the thread is alive
                    if let Ok(_) = resp {
                        good_threads.push(thread);
                        continue;
                    }
                }
                _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {
                    // Timeout
                }
            };

            // Delete the thread
            log::warn!("Thread {} is broken, removing it from the pool", thread.id);
            let _ = thread.tx.send(ThreadRequest::CloseThread { tx: None });
        }

        {
            let mut threads = self.threads.try_write().map_err(|_| "Failed to write lock threads")?; 
            *threads = good_threads;
        }

        Ok(())
    }

    /// Adds a new thread to the pool
    pub fn add_thread(
        &self,
        cgs: CreateGuildState
    ) -> Result<(), silverpelt::Error> {
        let mut threads = self.threads.try_write().map_err(|_| "Failed to write lock threads")?;
        threads.push(ThreadEntry::create(cgs)?);
        Ok(())
    }

    /// Removes a thread from the pool. This also removes all guild vms attached to said thread as well
    pub fn remove_thread(
        &self,
        id: u64,
    ) -> Result<(), silverpelt::Error> {
        let idx = {
            let threads = self.threads.try_read().map_err(|_| "Failed to read lock threads")?;

            let mut idx = None;
            for (i, th) in threads.iter().enumerate() {
                if th.id == id {
                    idx = Some(i);
                }
            }

            idx
        };

        let Some(idx) = idx else {
            return Ok(());
        };

        {
            let mut threads = self.threads.try_write().map_err(|_| "Failed to write lock threads")?;
            threads.remove(idx);
        }

        {
            let guilds_guard = self.guilds.try_read().map_err(|_| "Failed to read lock guilds")?;
            for (guild_id, id_curr) in guilds_guard.iter() {
                if *id_curr == id {
                    super::remove_vm(*guild_id);
                }
            }
        }

        Ok(())
    }

    /// Returns the number of threads in the pool
    pub fn threads_len(&self) -> Result<usize, silverpelt::Error> {
        Ok(self.threads.try_read().map_err(|_| "Failed to read lock threads for threads_len")?.len())
    }

    /// Adds a guild to the pool
    pub async fn add_guild(
        &self,
        guild: GuildId,
        cgs: CreateGuildState
    ) -> Result<ThreadPoolLuaHandle, silverpelt::Error> {
        // Flush out broken threads
        self.remove_broken_threads().await?;

        if self.threads_len()? < self.max_threads || crate::CMD_ARGS.vm_distribution_strategy == VmDistributionStrategy::ThreadPerGuild {
            // Add a new thread to the pool
            self.add_thread(cgs)?;
        }

        // Find the thread with the least amount of guilds, then add guild to it
        //
        // This is a simple strategy to balance the load across threads
        let mut min_thread = None;
        let mut min_count = usize::MAX;

        let threads = self.threads.try_read().map_err(|_| "Could not lock threads")?;
        for thread in threads.iter() {
            let count = thread.thread_count();

            if count < min_count {
                min_count = count;
                min_thread = Some(thread);
            }
        }

        let Some(thread) = min_thread else {
            return Err("Failed to add guild to VM pool [no threads]".into());
        };

        {
            let mut guilds_guard = self.guilds.try_write().map_err(|_| "Could not write lock guilds")?;
            guilds_guard.insert(guild, thread.id);
        }

        return Ok(ThreadPoolLuaHandle {
            guild_id: guild,
            handle: thread.tx.clone(),
        });
    }
}

/// The default thread pool that ``create_lua_vm`` uses
pub static DEFAULT_THREAD_POOL: LazyLock<ThreadPool> = LazyLock::new(ThreadPool::new);

#[allow(dead_code)]
pub async fn create_lua_vm(
    guild_id: GuildId,
    cgs: CreateGuildState
) -> Result<ArLua, silverpelt::Error> {
    let thread_pool_handle = DEFAULT_THREAD_POOL
        .add_guild(guild_id, cgs)
        .await?;

    Ok(ArLua::ThreadPool(thread_pool_handle))
}

#[derive(Clone)]
pub struct ThreadPoolLuaHandle {
    /// The guild id
    guild_id: GuildId,
    /// The thread handle
    handle: tokio::sync::mpsc::UnboundedSender<ThreadRequest>,
}

impl ArLuaHandle for ThreadPoolLuaHandle {
    fn send_action(
        &self,
        action: LuaVmAction,
        callback: Sender<Vec<(String, LuaVmResult)>>,
    ) -> Result<(), khronos_runtime::Error> {
        self.handle.send(
            ThreadRequest::Dispatch {
                guild_id: self.guild_id,
                action,
                callback,
            }
        )?;
        Ok(())
    }
}
