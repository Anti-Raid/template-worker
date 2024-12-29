use serenity::all::GuildId;
use std::panic::PanicHookInfo;
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::{Arc, LazyLock};
use std::time::Duration;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::sync::RwLock;

use crate::lang_lua::{BytecodeCache, XRc};
use crate::{handle_event, LuaVmAction, LuaVmResult};
use crate::{lang_lua::ArLua, MAX_VM_THREAD_STACK_SIZE};

pub const DEFAULT_MAX_THREADS: usize = 100; // Maximum number of threads in the pool

struct ThreadEntrySend {
    guild_id: GuildId,
    tx: UnboundedSender<crate::lang_lua::ArLua>,
}

/// A thread entry in the pool (worker thread)
struct ThreadEntry {
    /// The unique identifier for the thread entry
    id: u64,
    /// Number of guilds in the pool
    count: Arc<AtomicUsize>,
    /// A sender to create a new guild handle
    tx: UnboundedSender<ThreadEntrySend>,
    /// Is the thread pool itself broken
    broken: Arc<AtomicBool>,
}

impl ThreadEntry {
    /// Creates a new thread entry
    fn new(tx: UnboundedSender<ThreadEntrySend>) -> Self {
        Self {
            id: {
                // Generate a random id
                use rand::Rng;

                rand::thread_rng().gen()
            },
            count: Arc::new(AtomicUsize::new(0)),
            tx,
            broken: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Initializes a new thread entry, starting it after creation
    fn create(
        pool: sqlx::PgPool,
        serenity_context: serenity::all::Context,
        reqwest_client: reqwest::Client,
    ) -> Result<Self, silverpelt::Error> {
        let (tx, rx) = unbounded_channel::<ThreadEntrySend>();

        let entry = Self::new(tx);

        entry.start(pool, serenity_context, reqwest_client, rx)?;

        Ok(entry)
    }

    /// Returns the number of threads in the pool
    fn thread_count(&self) -> usize {
        self.count.load(std::sync::atomic::Ordering::Acquire)
    }

    /// Add a guild to the pool
    async fn add_guild(&self, guild: GuildId) -> Result<crate::lang_lua::ArLua, silverpelt::Error> {
        let (tx, mut rx) = unbounded_channel::<crate::lang_lua::ArLua>();

        self.tx
            .send(ThreadEntrySend {
                guild_id: guild,
                tx,
            })
            .map_err(|_| "Failed to add guild to VM pool [send fail]")?;

        if let Some(lua) = rx.recv().await {
            Ok(lua)
        } else {
            Err("Failed to add guild to VM pool [no response]".into())
        }
    }

    /// Start the thread up
    fn start(
        &self,
        pool: sqlx::PgPool,
        serenity_context: serenity::all::Context,
        reqwest_client: reqwest::Client,
        rx: UnboundedReceiver<ThreadEntrySend>,
    ) -> Result<(), silverpelt::Error> {
        let mut rx = rx; // Take mutable ownership to receiver
        let broken_ref = self.broken.clone();
        let id = self.id;
        std::thread::Builder::new()
            .name(format!("lua-vm-threadpool-{}", self.id))
            .stack_size(MAX_VM_THREAD_STACK_SIZE)
            .spawn(move || {
                // TODO: Implement handling code here
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap();

                let local = tokio::task::LocalSet::new();

                local.block_on(&rt, async {
                    // Catch panics
                    fn panic_catcher(
                        id: u64,
                        broken_ref: Arc<std::sync::atomic::AtomicBool>,
                    ) -> Box<dyn Fn(&PanicHookInfo<'_>) + 'static + Sync + Send>
                    {
                        Box::new(move |_| {
                            log::error!("Lua thread panicked: {}", id);
                            broken_ref.store(true, std::sync::atomic::Ordering::Release);
                        })
                    }

                    super::perthreadpanichook::set_hook(panic_catcher(id, broken_ref.clone()));

                    // Keep waiting for new guild creation requests
                    while let Some(send) = rx.recv().await {
                        let worker_broken = broken_ref.clone();

                        if worker_broken.load(std::sync::atomic::Ordering::Acquire) {
                            log::error!("Worker thread is broken, skipping guild creation");
                            continue;
                        }

                        let broken = Arc::new(AtomicBool::new(false));
                        let last_execution_time = Arc::new(
                            crate::atomicinstant::AtomicInstant::new(std::time::Instant::now()),
                        );

                        // Create Lua VM
                        let userdata = crate::lang_lua::create_lua_vm_userdata(
                            last_execution_time.clone(),
                            send.guild_id,
                            pool.clone(),
                            serenity_context.clone(),
                            reqwest_client.clone(),
                        )
                        .expect("Failed to create Lua VM userdata");

                        let bytecode_cache = BytecodeCache::new();

                        let tis_ref = XRc::new(
                            match crate::lang_lua::configure_lua_vm(
                                broken.clone(),
                                last_execution_time.clone(),
                                bytecode_cache,
                            ) {
                                Ok(tis) => tis,
                                Err(e) => {
                                    log::error!("Failed to configure Lua VM: {}", e);
                                    panic!("Failed to configure Lua VM");
                                }
                            },
                        );

                        tis_ref.lua.set_app_data(userdata);

                        // Start the scheduler in a tokio task
                        let broken_sched_ref = tis_ref.broken.clone();
                        let scheduler = tis_ref.scheduler.clone();
                        tokio::task::spawn_local(async move {
                            log::info!("Starting Lua scheduler");
                            match scheduler.run(Duration::from_millis(1)).await {
                                Ok(_) => {
                                    log::info!("Lua scheduler exited. This should not happen.");

                                    // If the scheduler exited, the Lua VM is broken
                                    broken_sched_ref
                                        .store(true, std::sync::atomic::Ordering::Release);
                                }
                                Err(e) => {
                                    log::error!("Lua scheduler exited with error: {}", e);

                                    // If the scheduler exited, the Lua VM is broken
                                    broken_sched_ref
                                        .store(true, std::sync::atomic::Ordering::Release);
                                }
                            }
                        });

                        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<(
                            LuaVmAction,
                            tokio::sync::oneshot::Sender<LuaVmResult>,
                        )>();

                        tokio::task::spawn_local(async move {
                            while let Some((action, callback)) = rx.recv().await {
                                let tis_ref = tis_ref.clone();

                                // Mark VM as broken if worker thread is broken
                                if worker_broken.load(std::sync::atomic::Ordering::Acquire) {
                                    tis_ref
                                        .broken
                                        .store(true, std::sync::atomic::Ordering::Release);

                                    // Send error back
                                    let _ = callback.send(LuaVmResult::LuaError {
                                        err: "Worker thread is broken".to_string(),
                                        template_name: None,
                                    });

                                    continue;
                                }

                                tokio::task::spawn_local(async move {
                                    let result = handle_event(action, &tis_ref).await;

                                    #[allow(clippy::single_match)]
                                    match result {
                                        LuaVmResult::LuaError {
                                            ref err,
                                            ref template_name,
                                        } => {
                                            let template_name = template_name.clone();
                                            log::error!(
                                                "Lua error in template {}: {}",
                                                template_name
                                                    .clone()
                                                    .unwrap_or_else(|| "Unknown".to_string()),
                                                err
                                            );

                                            if let Some(template_name) = template_name.as_ref() {
                                                crate::lang_lua::log_error(
                                                    tis_ref.lua.clone(),
                                                    template_name.clone(),
                                                    format!(
                                                        "Lua error in template {}: {}",
                                                        template_name, err
                                                    ),
                                                )
                                            }
                                        }
                                        _ => {}
                                    }

                                    let _ = callback.send(result);
                                });
                            }
                        });

                        if let Err(e) = send.tx.send(ArLua {
                            last_execution_time,
                            handle: tx,
                            broken,
                        }) {
                            log::error!("Failed to send new guild handle: {}", e);
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
    threads: RwLock<Vec<ThreadEntry>>,

    /// The maximum number of threads in the pool
    max_threads: usize,
}

impl ThreadPool {
    /// Creates a new thread pool
    pub fn new() -> Self {
        Self {
            threads: RwLock::new(Vec::new()),
            max_threads: DEFAULT_MAX_THREADS,
        }
    }

    /// Remove broken threads from the pool
    pub async fn remove_broken_threads(&self) {
        let mut threads = self.threads.write().await;

        threads.retain(|thread| !thread.broken.load(std::sync::atomic::Ordering::Acquire));
    }

    /// Adds a new thread to the pool
    pub async fn add_thread(
        &self,
        pool: sqlx::PgPool,
        serenity_context: serenity::all::Context,
        reqwest_client: reqwest::Client,
    ) -> Result<(), silverpelt::Error> {
        let mut threads = self.threads.write().await;
        threads.push(ThreadEntry::create(pool, serenity_context, reqwest_client)?);
        Ok(())
    }

    /// Returns the number of threads in the pool
    pub async fn threads_len(&self) -> usize {
        self.threads.read().await.len()
    }

    /// Adds a guild to the pool
    pub async fn add_guild(
        &self,
        guild: GuildId,
        pool: sqlx::PgPool,
        serenity_context: serenity::all::Context,
        reqwest_client: reqwest::Client,
    ) -> Result<crate::lang_lua::ArLua, silverpelt::Error> {
        // Flush out broken threads
        self.remove_broken_threads().await;

        if self.threads_len().await < self.max_threads {
            // Add a new thread to the pool
            self.add_thread(pool, serenity_context, reqwest_client)
                .await?;
        }

        // Find the thread with the least amount of guilds, then add guild to it
        //
        // This is a simple strategy to balance the load across threads
        let mut min_thread = None;
        let mut min_count = usize::MAX;

        let threads = self.threads.read().await;
        for thread in threads.iter() {
            let count = thread.thread_count();

            if count < min_count {
                min_count = count;
                min_thread = Some(thread);
            }
        }

        if let Some(thread) = min_thread {
            thread.add_guild(guild).await
        } else {
            Err("Failed to add guild to VM pool [no threads]".into())
        }
    }
}

/// The default thread pool that ``create_lua_vm`` uses
static DEFAULT_THREAD_POOL: LazyLock<ThreadPool> = LazyLock::new(ThreadPool::new);

pub async fn create_lua_vm(
    guild_id: GuildId,
    pool: sqlx::PgPool,
    serenity_context: serenity::all::Context,
    reqwest_client: reqwest::Client,
) -> Result<ArLua, silverpelt::Error> {
    DEFAULT_THREAD_POOL
        .add_guild(guild_id, pool, serenity_context, reqwest_client)
        .await
}
