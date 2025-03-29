use khronos_runtime::primitives::event::Event;
use serenity::all::GuildId;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::{Arc, LazyLock};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::sync::RwLock;

use super::client::VMS;
use super::core::{
    configure_runtime_manager, create_guild_state, dispatch_event_to_multiple_templates,
    dispatch_event_to_template,
};
use super::{client::ArLua, ArLuaHandle, LuaVmAction, LuaVmResult};
use crate::templatingrt::cache::get_all_guild_templates;
use crate::templatingrt::MAX_VM_THREAD_STACK_SIZE;

pub const DEFAULT_MAX_THREADS: usize = 100; // Maximum number of threads in the pool

struct ThreadEntrySend {
    guild_id: GuildId,
    tx: UnboundedSender<ThreadPoolLuaHandle>,
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

    /// Add a guild to the pool with a given shard messenger
    async fn add_guild(&self, guild: GuildId) -> Result<ThreadPoolLuaHandle, silverpelt::Error> {
        let (tx, mut rx) = unbounded_channel::<ThreadPoolLuaHandle>();

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
        std::thread::Builder::new()
            .name(format!("lua-vm-threadpool-{}", self.id))
            .stack_size(MAX_VM_THREAD_STACK_SIZE)
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("Failed to create tokio runtime");

                let local = tokio::task::LocalSet::new();
                local.block_on(&rt, async {
                    // Keep waiting for new guild creation requests
                    while let Some(send) = rx.recv().await {
                        let worker_broken = broken_ref.clone();

                        if worker_broken.load(std::sync::atomic::Ordering::Acquire) {
                            log::error!("Worker thread is broken, skipping guild creation");
                            continue;
                        }

                        // Create Lua VM
                        let gs = Rc::new(
                            create_guild_state(
                                send.guild_id,
                                pool.clone(),
                                serenity_context.clone(),
                                reqwest_client.clone(),
                            )
                            .expect("Failed to create Lua VM userdata"),
                        );

                        let tis_ref = match configure_runtime_manager() {
                            Ok(tis) => tis,
                            Err(e) => {
                                log::error!("Failed to configure Lua VM: {}", e);
                                panic!("Failed to configure Lua VM");
                            }
                        };

                        tis_ref.set_on_broken(Box::new(move |_lua| {
                            VMS.remove(&send.guild_id);
                        }));

                        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<(
                            LuaVmAction,
                            tokio::sync::oneshot::Sender<Vec<(String, LuaVmResult)>>,
                        )>();

                        tokio::task::spawn_local(async move {
                            while let Some((action, callback)) = rx.recv().await {
                                let tis_ref = tis_ref.clone();
                                let gs = gs.clone();
                                tokio::task::spawn_local(async move {
                                    match action {
                                        LuaVmAction::DispatchEvent { event } => {
                                            let Some(templates) =
                                                get_all_guild_templates(gs.guild_id).await
                                            else {
                                                return;
                                            };

                                            let _ = callback.send(
                                                dispatch_event_to_multiple_templates(
                                                    templates,
                                                    event,
                                                    &tis_ref,
                                                    gs.clone(),
                                                )
                                                .await,
                                            );
                                        }
                                        LuaVmAction::DispatchInlineEvent { event, template } => {
                                            let event = Event::from_create_event(&event);
                                            let name = template.name.clone();
                                            let result = dispatch_event_to_template(
                                                template, event, &tis_ref, gs,
                                            )
                                            .await;

                                            // Send back to the caller
                                            let _ = callback.send(vec![(name, result)]);
                                        }
                                        LuaVmAction::Stop {} => {
                                            // Mark VM as broken
                                            tis_ref.runtime().mark_broken(true);

                                            let _ = callback.send(vec![(
                                                "_".to_string(),
                                                LuaVmResult::Ok {
                                                    result_val: serde_json::Value::Null,
                                                },
                                            )]);
                                        }
                                        LuaVmAction::GetMemoryUsage {} => {
                                            let used = tis_ref.runtime().lua().used_memory();

                                            let _ = callback.send(vec![(
                                                "_".to_string(),
                                                LuaVmResult::Ok {
                                                    result_val: serde_json::Value::Number(
                                                        used.into(),
                                                    ),
                                                },
                                            )]);
                                        }
                                        LuaVmAction::SetMemoryLimit { limit } => {
                                            let result = match tis_ref
                                                .runtime()
                                                .lua()
                                                .set_memory_limit(limit)
                                            {
                                                Ok(limit) => LuaVmResult::Ok {
                                                    result_val: serde_json::Value::Number(
                                                        limit.into(),
                                                    ),
                                                },
                                                Err(e) => {
                                                    LuaVmResult::LuaError { err: e.to_string() }
                                                }
                                            };

                                            let _ = callback.send(vec![("_".to_string(), result)]);
                                        }
                                        LuaVmAction::ClearCache {} => {
                                            println!("Clearing cache in VM");
                                            tis_ref.clear_bytecode_cache();
                                            super::core::reset_vm_cache(gs.guild_id, &tis_ref)
                                                .await;
                                            let _ = callback.send(vec![(
                                                "_".to_string(),
                                                LuaVmResult::Ok {
                                                    result_val: serde_json::Value::Null,
                                                },
                                            )]);
                                        }
                                    };
                                });
                            }
                        });

                        if let Err(e) = send.tx.send(ThreadPoolLuaHandle { handle: tx }) {
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

    #[allow(dead_code)]
    /// Fills the thread pool where needed
    pub async fn fill(
        &self,
        pool: sqlx::PgPool,
        serenity_context: serenity::all::Context,
        reqwest_client: reqwest::Client,
    ) -> Result<(), silverpelt::Error> {
        let needed_threads = self.max_threads - self.threads_len().await;
        for _ in 0..needed_threads {
            self.add_thread(
                pool.clone(),
                serenity_context.clone(),
                reqwest_client.clone(),
            )
            .await?;
        }

        Ok(())
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
    ) -> Result<ThreadPoolLuaHandle, silverpelt::Error> {
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

#[allow(dead_code)]
pub async fn create_lua_vm(
    guild_id: GuildId,
    pool: sqlx::PgPool,
    serenity_context: serenity::all::Context,
    reqwest_client: reqwest::Client,
) -> Result<ArLua, silverpelt::Error> {
    let thread_pool_handle = DEFAULT_THREAD_POOL
        .add_guild(guild_id, pool, serenity_context, reqwest_client)
        .await?;

    Ok(ArLua::ThreadPool(thread_pool_handle))
}

#[derive(Clone)]
pub struct ThreadPoolLuaHandle {
    #[allow(clippy::type_complexity)]
    /// The thread handle for the Lua VM
    pub handle: tokio::sync::mpsc::UnboundedSender<(
        LuaVmAction,
        tokio::sync::oneshot::Sender<Vec<(String, LuaVmResult)>>,
    )>,
}

impl ArLuaHandle for ThreadPoolLuaHandle {
    fn send_action(
        &self,
        action: LuaVmAction,
        callback: tokio::sync::oneshot::Sender<Vec<(String, LuaVmResult)>>,
    ) -> Result<(), khronos_runtime::Error> {
        self.handle.send((action, callback))?;
        Ok(())
    }
}
