use super::core::configure_runtime_manager;
use super::sharedguild::SharedGuild;
use super::KhronosRuntimeManager;
use super::{LuaVmAction, LuaVmResult};
use crate::templatingrt::state::CreateGuildState;
use crate::templatingrt::state::GuildState;
use crate::templatingrt::MAX_VM_THREAD_STACK_SIZE;
use serenity::all::GuildId;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::sync::oneshot::Sender;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct ThreadGuildVmMetrics {
    /// Used memory
    pub used_memory: usize,
    /// Memory limit for the Luau VM
    pub memory_limit: usize,
    /// Number of luau threads
    pub num_threads: i64,
    /// Maximum luau threads
    pub max_threads: i64,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct ThreadMetrics {
    pub vm_metrics: HashMap<GuildId, ThreadGuildVmMetrics>,
    pub tid: u64,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct ThreadClearInactiveGuilds {
    pub tid: u64,
    pub cleared: HashMap<GuildId, Option<String>>,
}

pub enum ThreadRequest {
    /// Dispatch an 'action'
    Dispatch {
        guild_id: GuildId, // id of discord server
        action: LuaVmAction,
        callback: Sender<Vec<(String, LuaVmResult)>>,
    },
    /// Diagnostic message to check if a vm is alive or not
    Ping { tx: Sender<()> },
    /// Clear out inactive guilds
    ClearInactiveGuilds {
        tx: Sender<HashMap<GuildId, Option<String>>>,
    },
    /// Stop the underlying thread if it has no VM's currently on it
    RemoveIfUnused { tx: Sender<bool> },
    /// Get VM metrics across all guilds in the thread
    GetVmMetrics {
        tx: Sender<HashMap<GuildId, ThreadGuildVmMetrics>>,
    },
    /// Close the underlying thread unconditionally
    CloseThread { tx: Option<Sender<()>> },
}

/// A thread entry (worker thread)
///
/// Thread entries are the base primitive for handling guilds. All guild vms must be on a thread entry
#[derive(Clone)]
pub struct ThreadEntry {
    /// The unique identifier for the thread entry
    id: u64,
    /// Number of guilds in the pool
    count: Arc<AtomicUsize>,
    /// A sender to create a new guild handle
    tx: UnboundedSender<ThreadRequest>,
    /// shared guild
    sg: SharedGuild,
}

impl Hash for ThreadEntry {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl PartialEq for ThreadEntry {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for ThreadEntry {}

impl ThreadEntry {
    /// Creates, but does not spawn a new thread entry
    fn new(tx: UnboundedSender<ThreadRequest>, sg: SharedGuild) -> Self {
        // sending in and going out thread
        Self {
            id: {
                // Generate a random id for the thread entry
                use rand::Rng;

                rand::thread_rng().gen()
            },
            count: Arc::new(AtomicUsize::new(0)),
            tx,
            sg,
        }
    }

    /// Initializes a new thread entry, starting it after creation
    pub fn create(
        cgs: CreateGuildState, // all data needed by lua vm
        sg: SharedGuild,
    ) -> Result<Self, silverpelt::Error> {
        let (tx, rx) = unbounded_channel::<ThreadRequest>();

        let entry = Self::new(tx, sg);

        entry.start(cgs, rx)?;

        Ok(entry)
    }

    /// Returns the thread entry ID
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Returns the number of servers in the pool
    pub fn server_count(&self) -> usize {
        self.count.load(std::sync::atomic::Ordering::Acquire)
    }

    /// Returns the inner handle
    pub fn handle(&self) -> &UnboundedSender<ThreadRequest> {
        &self.tx
    }

    /// Start the thread up
    fn start(
        &self,
        cgs: CreateGuildState,
        rx: UnboundedReceiver<ThreadRequest>,
    ) -> Result<(), silverpelt::Error> {
        let mut rx = rx; // Take mutable ownership to receiver
        let count_ref = self.count.clone();
        let tid = self.id; // thread id
        let sg = self.sg.clone();
        let self_ref = self.clone();
        std::thread::Builder::new()
            .name(format!("lua-vm-threadpool-{}", self.id))
            .stack_size(MAX_VM_THREAD_STACK_SIZE)
            .spawn(move || {
                let sg_ref = sg.clone();
                let self_ref_a = self_ref.clone();
                super::perthreadpanichook::set_hook(Box::new(move |e| {
                    if let Err(e) = sg_ref.remove_thread_entry(&self_ref_a) {
                        log::error!("Error removing thread on panic: {:?}", e)
                    }

                    if let Some(e) = e.payload().downcast_ref::<String>() {
                        log::error!("Thread panicked: {}", e);
                    } else if let Some(e) = e.payload().downcast_ref::<&str>() {
                        log::error!("Thread panicked: {}", e);
                    }

                    log::error!("Thread panicked without representable panic!");
                }));

                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build_local(&tokio::runtime::LocalOptions::default())
                    .expect("Failed to create tokio runtime");

                rt.block_on(async move {
                    // Keep waiting for new events
                    struct VmData {
                        guild_state: Rc<GuildState>,
                        tis_ref: KhronosRuntimeManager,
                        count: Rc<Cell<usize>>,
                    }

                    // it' a hashmap that can be mutably borrowed and is also reference counted
                    let thread_vms: Rc<RefCell<HashMap<GuildId, Rc<VmData>>>> =
                        Rc::new(HashMap::new().into());

                    while let Some(send) = rx.recv().await {
                        match send {
                            ThreadRequest::Ping { tx } => {
                                // Send a pong
                                let _ = tx.send(());
                            }
                            ThreadRequest::Dispatch {
                                guild_id,
                                action,
                                callback,
                            } => {
                                let vm = {
                                    let mut vms = thread_vms.borrow_mut();

                                    // Create server if not found, otherwise return existing
                                    match vms.get(&guild_id) {
                                        Some(vm) => vm.clone(), // not costly cause of Rc
                                        None => {
                                            // Create Lua VM
                                            let cgs_ref = cgs.clone();
                                            let gs =
                                                Rc::new(match cgs_ref.to_guild_state(guild_id) {
                                                    Ok(gs) => gs,
                                                    Err(e) => {
                                                        log::error!(
                                                            "Failed to create guild state: {}",
                                                            e
                                                        );
                                                        continue;
                                                    }
                                                });

                                            let tis_ref = match configure_runtime_manager() {
                                                Ok(tis) => tis,
                                                Err(e) => {
                                                    log::error!(
                                                        "Failed to configure Lua VM: {}",
                                                        e
                                                    );
                                                    continue;
                                                }
                                            };

                                            count_ref
                                                .fetch_add(1, std::sync::atomic::Ordering::Release);

                                            let thread_vms_ref = thread_vms.clone();
                                            let sg_ref_a = sg.clone();
                                            tis_ref.set_on_broken(Box::new(move || {
                                                if let Err(e) = sg_ref_a.remove_guild(guild_id) {
                                                    log::error!("Error removing guild vm: {:?}", e);
                                                }

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

                                            vms.insert(guild_id, vmd.clone());

                                            if let Err(e) = sg.add_guild(guild_id, self_ref.clone())
                                            {
                                                log::error!(
                                                    "Error adding guild to shared guild: {:?}",
                                                    e
                                                );
                                            }

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
                            }
                            ThreadRequest::ClearInactiveGuilds { tx } => {
                                let mut removed = vec![];

                                {
                                    let vms = thread_vms.borrow();
                                    for (guild_id, vm) in vms.iter() {
                                        if let Some(let_) =
                                            vm.tis_ref.runtime().last_execution_time()
                                        {
                                            if std::time::Instant::now() - let_
                                                > crate::templatingrt::MAX_SERVER_INACTIVITY
                                            {
                                                removed.push((*guild_id, vm.tis_ref.clone()));
                                            }
                                        }
                                    }
                                }

                                let mut close_errors = HashMap::new();
                                for (guild_id, removed) in removed.into_iter() {
                                    match removed.runtime().mark_broken(true) {
                                        Ok(()) => close_errors.insert(guild_id, None),
                                        Err(e) => {
                                            close_errors.insert(guild_id, Some(e.to_string()))
                                        }
                                    };

                                    if let Err(e) = sg.remove_guild(guild_id) {
                                        log::error!("Error removing guild vm: {:?}", e);
                                    }
                                }

                                let _ = tx.send(close_errors);
                            }
                            ThreadRequest::RemoveIfUnused { tx } => {
                                // Borrow VMs for removal
                                let vms = thread_vms.borrow();
                                if vms.is_empty() {
                                    log::debug!("Clearing unused thread: {}", tid);
                                    if let Err(e) = sg.remove_thread_entry(&self_ref) {
                                        log::error!("Error removing thread entry: {:?}", e);
                                    }
                                    let _ = tx.send(true);
                                    return;
                                }

                                let _ = tx.send(false);
                            }
                            ThreadRequest::CloseThread { tx } => {
                                if let Some(tx) = tx {
                                    let _ = tx.send(());
                                }

                                log::debug!("Closing thread: {}", tid);
                                let _ = sg.remove_thread_entry(&self_ref);
                                return;
                            }
                            ThreadRequest::GetVmMetrics { tx } => {
                                let guard = thread_vms.borrow();

                                let mut metrics_map = HashMap::with_capacity(guard.len());

                                for (guild, vm) in guard.iter() {
                                    let metrics = ThreadGuildVmMetrics {
                                        used_memory: vm.tis_ref.runtime().memory_usage(),
                                        memory_limit:
                                            crate::templatingrt::MAX_TEMPLATE_MEMORY_USAGE,
                                        num_threads: vm.tis_ref.runtime().current_threads(),
                                        max_threads: vm.tis_ref.runtime().max_threads(),
                                    };

                                    metrics_map.insert(*guild, metrics);
                                }

                                let _ = tx.send(metrics_map);
                            }
                        }
                    }
                })
            })?;

        Ok(())
    }
}
