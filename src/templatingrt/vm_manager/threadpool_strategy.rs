use khronos_runtime::primitives::event::Event;
use khronos_runtime::rt::KhronosRuntimeManager;
use serenity::all::GuildId;
use std::rc::Rc;
use std::sync::atomic::AtomicUsize;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::sync::oneshot::Sender;
use tokio::sync::RwLock;
use std::cell::RefCell;

use super::client::VMS;
use super::core::{
    configure_runtime_manager, create_guild_state, dispatch_event_to_multiple_templates,
    dispatch_event_to_template,
};
use super::{client::ArLua, ArLuaHandle, LuaVmAction, LuaVmResult};
use crate::templatingrt::cache::{get_guild_template, get_all_guild_templates};
use crate::templatingrt::MAX_VM_THREAD_STACK_SIZE;
use crate::templatingrt::state::GuildState;

pub const DEFAULT_MAX_THREADS: usize = 100; // Maximum number of threads in the pool

enum ThreadRequest {
    Dispatch {
        guild_id: GuildId,
        action: LuaVmAction,
        callback: Sender<Vec<(String, LuaVmResult)>>,
    },
    Ping {
        tx: Sender<()>,
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
        pool: sqlx::PgPool,
        serenity_context: serenity::all::Context,
        reqwest_client: reqwest::Client,
    ) -> Result<Self, silverpelt::Error> {
        let (tx, rx) = unbounded_channel::<ThreadRequest>();

        let entry = Self::new(tx);

        entry.start(pool, serenity_context, reqwest_client, rx)?;

        Ok(entry)
    }

    /// Returns the number of servers in the pool
    fn thread_count(&self) -> usize {
        self.count.load(std::sync::atomic::Ordering::Acquire)
    }

    /// Start the thread up
    fn start(
        &self,
        pool: sqlx::PgPool,
        serenity_context: serenity::all::Context,
        reqwest_client: reqwest::Client,
        rx: UnboundedReceiver<ThreadRequest>,
    ) -> Result<(), silverpelt::Error> {
        let mut rx = rx; // Take mutable ownership to receiver
        let count_ref = self.count.clone();
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
                    // Keep waiting for new events
                    struct VmData {
                        guild_state: Rc<GuildState>,
                        tis_ref: KhronosRuntimeManager,
                    }

                    let thread_vms: RefCell<HashMap<GuildId, Rc<VmData>>> = HashMap::new().into();
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
                                            let gs = Rc::new(
                                                match create_guild_state(
                                                    guild_id,
                                                    pool.clone(),
                                                    serenity_context.clone(),
                                                    reqwest_client.clone(),
                                                ) {
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
    
                                            tis_ref.set_on_broken(Box::new(move |_lua| {
                                                VMS.remove(&guild_id);
                                            }));
    
                                            // Store into the thread
                                            let vmd = Rc::new(VmData {
                                                guild_state: gs,
                                                tis_ref: tis_ref,
                                            });
                                            vms.insert(
                                                guild_id,
                                                vmd.clone(),
                                            );
    
                                            vmd
                                        }
                                    }    
                                };

                                tokio::task::spawn_local(async move {
                                    let tis_ref = vm.tis_ref.clone();
                                    let gs = vm.guild_state.clone();
                                    match action {
                                        LuaVmAction::DispatchEvent { event } => {
                                            let Some(templates) =
                                                get_all_guild_templates(gs.guild_id).await
                                            else {
                                                if event.name() == "INTERACTION_CREATE" {
                                                    log::info!("No templates for event: {}", event.name());
                                                }    
                                                return;
                                            };

                                            if event.name() == "INTERACTION_CREATE" {
                                                log::info!("Found templates: {} {}", event.name(), templates.len());
                                            }

                                            let _ = callback.send(
                                                dispatch_event_to_multiple_templates(
                                                    templates,
                                                    event,
                                                    &tis_ref,
                                                    gs
                                                )
                                                .await,
                                            );
                                        }
                                        LuaVmAction::DispatchTemplateEvent { event, template_name } => {
                                            let event = Event::from_create_event(&event);
                                            let Some(template) = get_guild_template(gs.guild_id, &template_name).await else {
                                                let _ = callback.send(vec![(
                                                    template_name.clone(),
                                                    LuaVmResult::LuaError {
                                                        err: format!("Template {} not found", template_name),
                                                    },
                                                )]);
                                                return;
                                            };
            
                                            let result =
                                                dispatch_event_to_template(template, event, tis_ref, gs).await;
            
                                            // Send back to the caller
                                            let _ = callback.send(vec![(template_name, result)]);
                                        }            
                                        LuaVmAction::DispatchInlineEvent { event, template } => {
                                            let event = Event::from_create_event(&event);
                                            let name = template.name.clone();
                                            let result = dispatch_event_to_template(
                                                template, event, tis_ref, gs,
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

        let mut good_threads = Vec::with_capacity(threads.len());   
        let old_threads = std::mem::take(&mut *threads);
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

            // Delete the thread by doing nothing
            log::warn!("Thread {} is broken, removing it from the pool", thread.id);
        }

        *threads = good_threads;
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

        let Some(thread) = min_thread else {
            return Err("Failed to add guild to VM pool [no threads]".into());
        };

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
