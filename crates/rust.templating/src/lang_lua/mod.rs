pub mod ctx;
pub mod event;
pub mod primitives_docs;
pub(crate) mod state;

mod plugins;
use mlua_scheduler::taskmgr::SchedulerFeedback;
use mlua_scheduler::TaskManager;
use mlua_scheduler_ext::feedbacks::{MultipleSchedulerFeedback, ThreadTracker};
use mlua_scheduler_ext::Scheduler;
pub use plugins::PLUGINS;

mod handler;
pub use handler::handle_event;

use crate::atomicinstant;
use crate::{MAX_TEMPLATES_EXECUTION_TIME, MAX_TEMPLATE_MEMORY_USAGE};
use mlua::prelude::*;
use serenity::all::GuildId;
use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::Hash;
use std::hash::Hasher;
use std::ops::Deref;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::LazyLock;

#[cfg(feature = "send")]
pub type XRc<T> = Arc<T>;
#[cfg(not(feature = "send"))]
pub type XRc<T> = std::rc::Rc<T>;

mod vm_manager;

/// VM cache
static VMS: LazyLock<scc::HashMap<GuildId, ArLua>> = LazyLock::new(scc::HashMap::new);

#[derive(serde::Serialize, serde::Deserialize)]
#[allow(dead_code)]
pub enum LuaVmAction {
    /// Execute a template
    Exec {
        content: String,
        template: crate::Template,
        pragma: crate::TemplatePragma,
        event: event::Event,
    },
    ExecNoWait {
        content: String,
        template: crate::Template,
        pragma: crate::TemplatePragma,
        event: event::Event,
    },
    /// Stop the Lua VM entirely
    Stop {},
    /// Returns the memory usage of the Lua VM
    GetMemoryUsage {},
    /// Set the memory limit of the Lua VM
    SetMemoryLimit { limit: usize },
}

#[derive(Debug)]
pub enum LuaVmResult {
    Ok {
        result_val: serde_json::Value,
    },
    LuaError {
        err: String,
        template_name: Option<String>,
    },
    VmBroken {},
}

impl LuaVmResult {
    /// Convert the result to a response if possible, returning an error if the result is an error
    pub fn to_response<T: serde::de::DeserializeOwned>(self) -> Result<T, silverpelt::Error> {
        match self {
            LuaVmResult::Ok { result_val } => {
                let res = serde_json::from_value(result_val)?;
                Ok(res)
            }
            LuaVmResult::LuaError { err, template_name } => {
                Err(format!("Lua error: {:?}: {}", template_name, err).into())
            }
            LuaVmResult::VmBroken {} => Err("Lua VM is marked as broken".into()),
        }
    }
}

pub struct BytecodeCache(scc::HashMap<crate::Template, (Vec<u8>, u64)>);

impl BytecodeCache {
    /// Create a new bytecode cache
    pub fn new() -> Self {
        BytecodeCache(scc::HashMap::new())
    }
}

impl Deref for BytecodeCache {
    type Target = scc::HashMap<crate::Template, (Vec<u8>, u64)>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// ArLua provides a handle to a Lua VM
///
/// Note that the Lua VM is not directly exposed both due to thread safety issues
/// and to allow for multiple VM-thread allocation strategies in vm_manager
#[derive(Clone)]
struct ArLua {
    /// The last execution time of the Lua VM
    last_execution_time: Arc<atomicinstant::AtomicInstant>,

    /// The thread handle for the Lua VM
    handle: tokio::sync::mpsc::UnboundedSender<(
        LuaVmAction,
        tokio::sync::oneshot::Sender<LuaVmResult>,
    )>,

    /// Is the VM broken/needs to be remade
    broken: Arc<std::sync::atomic::AtomicBool>,
}

pub struct ArLuaThreadInnerState {
    /// The Lua VM
    lua: Lua,

    /// The bytecode cache maps template to (bytecode, source hash)
    bytecode_cache: BytecodeCache,

    /// Is the VM broken/needs to be remade
    broken: Arc<std::sync::atomic::AtomicBool>,

    /// Stores the servers global table
    pub global_table: mlua::Table,

    /// The scheduler
    pub scheduler: Scheduler,
}

pub(crate) fn create_lua_compiler() -> mlua::Compiler {
    mlua::Compiler::new()
        .set_optimization_level(2)
        .set_type_info_level(1)
}

/// Configures a raw Lua VM. Note that userdata is not set in this function
pub(crate) fn configure_lua_vm(
    broken: Arc<std::sync::atomic::AtomicBool>,
    last_execution_time: Arc<atomicinstant::AtomicInstant>,
    bytecode_cache: BytecodeCache,
) -> LuaResult<ArLuaThreadInnerState> {
    let lua = Lua::new_with(
        LuaStdLib::ALL_SAFE,
        LuaOptions::new().catch_rust_panics(true),
    )?;

    let compiler = mlua::Compiler::new()
        .set_optimization_level(2)
        .set_type_info_level(1);
    lua.set_compiler(compiler.clone());

    // Setup the global table using a metatable
    //
    // SAFETY: This works because the global table will not change in the VM
    let global_mt = lua.create_table()?;
    let global_tab = lua.create_table()?;

    // Proxy reads to globals if key is in globals, otherwise to the table
    global_mt.set("__index", lua.globals())?;
    global_tab.set("_G", global_tab.clone())?;
    global_tab.set("__stack", global_tab.clone())?;

    // Provies writes
    // Forward to _G if key is in globals, otherwise to the table
    let globals_ref = lua.globals();
    global_mt.set(
        "__newindex",
        lua.create_function(
            move |_lua, (tab, key, value): (LuaTable, LuaValue, LuaValue)| {
                let v = globals_ref.get::<LuaValue>(key.clone())?;

                if !v.is_nil() {
                    globals_ref.set(key, value)
                } else {
                    tab.raw_set(key, value)
                }
            },
        )?,
    )?;

    // Set __index on global_tab to point to _G
    global_tab.set_metatable(Some(global_mt));

    // Override require function for plugin support and increased security
    lua.globals()
        .set("require", lua.create_function(plugins::require)?)?;

    // Also create the mlua scheduler in the app data
    let thread_tracker = ThreadTracker::new();
    let ter = ThreadErrorTracker::new(thread_tracker.clone());

    let scheduler_feedback = MultipleSchedulerFeedback::new(vec![
        Box::new(thread_tracker.clone()),
        Box::new(ter.clone()),
    ]);

    lua.set_app_data(thread_tracker);
    lua.set_app_data(ter);

    let scheduler = Scheduler::new(TaskManager::new(lua.clone(), Rc::new(scheduler_feedback)));

    scheduler.attach(&lua);

    // Prelude code providing some basic functions directly to the Lua VM
    lua.load(
        r#"
        -- Override print function with function that appends to stdout table
        -- We do this by executing a lua script
        _G.print = function(...)
            local args = {...}

            if not _G.stdout then
                _G.stdout = {}
            end

            if #args == 0 then
                table.insert(_G.stdout, "nil")
            end

            local str = ""
            for i = 1, #args do
                str = str .. tostring(args[i])
            end
            table.insert(_G.stdout, str)
        end
    "#,
    )
    .set_name("prelude")
    .set_environment(global_tab.clone())
    .exec()?;

    // Patch coroutine and enable task
    let scheduler_lib = mlua_scheduler::userdata::scheduler_lib(&lua)?;

    lua.globals()
        .set("scheduler", scheduler_lib.clone())
        .expect("Failed to set scheduler global");

    mlua_scheduler::userdata::patch_coroutine_lib(&lua)?;
    lua.globals().set(
        "task",
        mlua_scheduler::userdata::task_lib(&lua, scheduler_lib)?,
    )?;

    lua.sandbox(true)?; // We explicitly want globals to be shared across all scripts in this VM
    lua.set_memory_limit(MAX_TEMPLATE_MEMORY_USAGE)?;

    lua.globals().set_readonly(true);

    // Create an interrupt to limit the execution time of a template
    lua.set_interrupt(move |_| {
        if last_execution_time
            .load(std::sync::atomic::Ordering::Acquire)
            .elapsed()
            >= MAX_TEMPLATES_EXECUTION_TIME
        {
            return Ok(LuaVmState::Yield);
        }
        Ok(LuaVmState::Continue)
    });

    Ok(ArLuaThreadInnerState {
        lua,
        bytecode_cache,
        broken,
        global_table: global_tab,
        scheduler,
    })
}

pub(crate) fn create_lua_vm_userdata(
    last_execution_time: Arc<atomicinstant::AtomicInstant>,
    guild_id: GuildId,
    pool: sqlx::PgPool,
    serenity_context: serenity::all::Context,
    reqwest_client: reqwest::Client,
) -> Result<state::LuaUserData, silverpelt::Error> {
    Ok(state::LuaUserData {
        pool,
        guild_id,
        serenity_context,
        shard_messenger: crate::shard_messenger_for_guild(guild_id)?,
        reqwest_client,
        kv_constraints: state::LuaKVConstraints::default(),
        kv_ratelimits: Rc::new(state::LuaRatelimits::new_kv_rl()?),
        actions_ratelimits: Rc::new(state::LuaRatelimits::new_action_rl()?),
        sting_ratelimits: Rc::new(state::LuaRatelimits::new_stings_rl()?),
        last_execution_time,
    })
}

/// Helper method to fetch a template from bytecode or compile it if it doesnt exist in bytecode cache
pub(crate) async fn resolve_template_to_bytecode(
    template_content: String,
    template: crate::Template,
    bytecode_cache_ref: &BytecodeCache,
) -> Result<Vec<u8>, LuaError> {
    // Check if the source hash matches the expected source hash
    let mut hasher = std::hash::DefaultHasher::new();
    template_content.hash(&mut hasher);
    let cur_hash = hasher.finish();

    let existing_bycode = bytecode_cache_ref.read(&template, |_, v| {
        if v.1 == cur_hash {
            Some(v.0.clone())
        } else {
            None
        }
    });

    if let Some(Some(bytecode)) = existing_bycode {
        return Ok(bytecode);
    }

    let bytecode = create_lua_compiler().compile(&template_content)?;

    let _ = bytecode_cache_ref
        .insert_async(template, (bytecode.clone(), cur_hash))
        .await;

    Ok(bytecode)
}

/// Get a Lua VM for a guild
///
/// This function will either return an existing Lua VM for the guild or create a new one if it does not exist
async fn get_lua_vm(
    guild_id: GuildId,
    pool: sqlx::PgPool,
    serenity_context: serenity::all::Context,
    reqwest_client: reqwest::Client,
) -> Result<ArLua, silverpelt::Error> {
    let Some(mut vm) = VMS.get(&guild_id) else {
        let vm =
            vm_manager::create_lua_vm(guild_id, pool, serenity_context, reqwest_client).await?;
        if let Err((_, alt_vm)) = VMS.insert_async(guild_id, vm.clone()).await {
            return Ok(alt_vm);
        }
        return Ok(vm);
    };

    if vm.broken.load(std::sync::atomic::Ordering::Acquire) {
        let new_vm =
            vm_manager::create_lua_vm(guild_id, pool, serenity_context, reqwest_client).await?;
        *vm = new_vm.clone();
        Ok(new_vm)
    } else {
        Ok(vm.clone())
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct FireBenchmark {
    pub hashmap_insert_time: u128,
    pub get_lua_vm: u128,
    pub exec_simple: u128,
    pub exec_no_wait: u128,
    pub exec_error: u128,
}

/// Benchmark the Lua VM
pub async fn benchmark_vm(
    guild_id: GuildId,
    pool: sqlx::PgPool,
    serenity_context: serenity::all::Context,
    reqwest_client: reqwest::Client,
) -> Result<FireBenchmark, silverpelt::Error> {
    // Get_lua_vm
    let pool_a = pool.clone();
    let guild_id_a = guild_id;
    let serenity_context_a = serenity_context.clone();
    let reqwest_client_a = reqwest_client.clone();

    let start = std::time::Instant::now();
    let _ = get_lua_vm(guild_id_a, pool_a, serenity_context_a, reqwest_client_a).await?;
    let get_lua_vm = start.elapsed().as_micros();

    let new_map = scc::HashMap::new();
    let start = std::time::Instant::now();
    let _ = new_map.insert_async(1, 1).await;
    let hashmap_insert_time = start.elapsed().as_micros();

    // Exec simple with wait
    let pt = crate::ParsedTemplate {
        template: crate::Template::Named("__benchmark".to_string()),
        pragma: crate::TemplatePragma::default(),
        template_content: "return 1".to_string(),
    };

    let pool_a = pool.clone();
    let guild_id_a = guild_id;
    let serenity_context_a = serenity_context.clone();
    let reqwest_client_a = reqwest_client.clone();

    let start = std::time::Instant::now();
    let n: i32 = execute(
        event::Event::new(
            "Benchmark".to_string(),
            "Benchmark".to_string(),
            "Benchmark".to_string(),
            event::ArcOrNormal::Normal(serde_json::Value::Null),
            None,
        ),
        ParseCompileState {
            serenity_context: serenity_context_a,
            reqwest_client: reqwest_client_a,
            guild_id: guild_id_a,
            pool: pool_a,
        },
        pt,
    )
    .await?
    .wait_timeout_then_response(std::time::Duration::from_secs(10))
    .await?;
    let exec_simple = start.elapsed().as_micros();

    if n != 1 {
        return Err(format!("Expected 1, got {}", n).into());
    }

    // Exec simple with no wait
    let pt = crate::ParsedTemplate {
        template: crate::Template::Named("__benchmark".to_string()),
        pragma: crate::TemplatePragma::default(),
        template_content: "return 1".to_string(),
    };

    let pool_a = pool.clone();
    let guild_id_a = guild_id;
    let serenity_context_a = serenity_context.clone();
    let reqwest_client_a = reqwest_client.clone();

    let start = std::time::Instant::now();
    execute_nowait(
        event::Event::new(
            "Benchmark".to_string(),
            "Benchmark".to_string(),
            "Benchmark".to_string(),
            event::ArcOrNormal::Normal(serde_json::Value::Null),
            None,
        ),
        ParseCompileState {
            serenity_context: serenity_context_a,
            reqwest_client: reqwest_client_a,
            guild_id: guild_id_a,
            pool: pool_a,
        },
        pt,
    )
    .await?
    .wait_timeout(std::time::Duration::from_secs(10))
    .await?;
    let exec_no_wait = start.elapsed().as_micros();

    // Exec simple with wait
    let pt = crate::ParsedTemplate {
        template: crate::Template::Named("__benchmark".to_string()),
        pragma: crate::TemplatePragma::default(),
        template_content: "error('MyError')\nreturn 1".to_string(),
    };

    let pool_a = pool.clone();
    let guild_id_a = guild_id;
    let serenity_context_a = serenity_context.clone();
    let reqwest_client_a = reqwest_client.clone();

    let start = std::time::Instant::now();
    let err = execute(
        event::Event::new(
            "Benchmark".to_string(),
            "Benchmark".to_string(),
            "Benchmark".to_string(),
            event::ArcOrNormal::Normal(serde_json::Value::Null),
            None,
        ),
        ParseCompileState {
            serenity_context: serenity_context_a,
            reqwest_client: reqwest_client_a,
            guild_id: guild_id_a,
            pool: pool_a,
        },
        pt,
    )
    .await?
    .wait_timeout_then_response::<i32>(std::time::Duration::from_secs(10))
    .await;
    let exec_error = start.elapsed().as_micros();

    match err {
        Ok(_) => return Err("Expected error, got success".into()),
        Err(e) => {
            if !e.to_string().contains("MyError") {
                return Err(format!("Expected MyError, got {}", e).into());
            }
        }
    }

    Ok(FireBenchmark {
        get_lua_vm,
        hashmap_insert_time,
        exec_simple,
        exec_no_wait,
        exec_error,
    })
}

#[derive(Clone)]
pub struct ParseCompileState {
    pub serenity_context: serenity::all::Context,
    pub reqwest_client: reqwest::Client,
    pub guild_id: GuildId,
    pub pool: sqlx::PgPool,
}

/// Render a template given an event, state and template
///
/// Pre-conditions: the serenity context's shard matches the guild itself
pub async fn execute(
    event: event::Event,
    state: ParseCompileState,
    template: crate::ParsedTemplate,
) -> Result<RenderTemplateHandle, silverpelt::Error> {
    let lua = get_lua_vm(
        state.guild_id,
        state.pool,
        state.serenity_context,
        state.reqwest_client,
    )
    .await?;

    // Update last execution time.
    lua.last_execution_time.store(
        std::time::Instant::now(),
        std::sync::atomic::Ordering::Release,
    );

    let (tx, rx) = tokio::sync::oneshot::channel();

    lua.handle
        .send((
            LuaVmAction::Exec {
                template: template.template,
                content: template.template_content,
                pragma: template.pragma,
                event,
            },
            tx,
        ))
        .map_err(|e| format!("Could not send data to Lua thread: {}", e))?;

    Ok(RenderTemplateHandle { rx })
}

/// Render a template given an event, state and template without waiting for the result
///
/// Pre-conditions: the serenity context's shard matches the guild itself
pub async fn execute_nowait(
    event: event::Event,
    state: ParseCompileState,
    template: crate::ParsedTemplate,
) -> Result<RenderTemplateHandle, silverpelt::Error> {
    let lua = get_lua_vm(
        state.guild_id,
        state.pool,
        state.serenity_context,
        state.reqwest_client,
    )
    .await?;

    // Update last execution time.
    lua.last_execution_time.store(
        std::time::Instant::now(),
        std::sync::atomic::Ordering::Release,
    );

    let (tx, rx) = tokio::sync::oneshot::channel();

    lua.handle
        .send((
            LuaVmAction::ExecNoWait {
                template: template.template,
                content: template.template_content,
                pragma: template.pragma,
                event,
            },
            tx,
        ))
        .map_err(|e| format!("Could not send data to Lua thread: {}", e))?;

    Ok(RenderTemplateHandle { rx })
}

/// A handle to allow waiting for a template to render
pub struct RenderTemplateHandle {
    rx: tokio::sync::oneshot::Receiver<LuaVmResult>,
}

impl RenderTemplateHandle {
    /// Wait for the template to render
    pub async fn wait(self) -> Result<LuaVmResult, silverpelt::Error> {
        self.rx
            .await
            .map_err(|e| format!("Could not receive data from Lua thread: {}", e).into())
    }

    /// Wait for the template to render with a timeout
    pub async fn wait_timeout(
        self,
        timeout: std::time::Duration,
    ) -> Result<Option<LuaVmResult>, silverpelt::Error> {
        match tokio::time::timeout(timeout, self.rx).await {
            Ok(Ok(res)) => Ok(Some(res)),
            Ok(Err(e)) => Err(format!("Could not receive data from Lua thread: {}", e).into()),
            Err(_) => Ok(None),
        }
    }

    /// Wait for the template to render with a timeout, returning an error if the timeout is reached
    pub async fn wait_timeout_or_err(
        self,
        timeout: std::time::Duration,
    ) -> Result<LuaVmResult, silverpelt::Error> {
        self.wait_timeout(timeout)
            .await?
            .ok_or("Lua VM timed out when rendering template".into())
    }

    /// Wait for the template to render with a timeout, returning an error if the timeout is reached
    pub async fn wait_timeout_then_response<T: serde::de::DeserializeOwned>(
        self,
        timeout: std::time::Duration,
    ) -> Result<T, silverpelt::Error> {
        self.wait_timeout_or_err(timeout).await?.to_response()
    }
}

pub fn log_error(lua: mlua::Lua, template_name: String, e: String) {
    tokio::task::spawn_local(async move {
        log::error!("Lua thread error: {}: {}", template_name, e);

        let tm = lua.app_data_ref::<mlua_scheduler::TaskManager>().unwrap();
        let inner = tm.inner.clone();
        let user_data = inner
            .lua
            .app_data_ref::<crate::lang_lua::state::LuaUserData>()
            .unwrap();

        let Ok(template) =
            crate::cache::get_guild_template(user_data.guild_id, &template_name, &user_data.pool)
                .await
        else {
            log::error!("Failed to get template data for error reporting");
            return;
        };

        if let Err(e) = dispatch_error(
            &user_data.serenity_context,
            &e,
            user_data.guild_id,
            &template,
        )
        .await
        {
            log::error!("Failed to dispatch error: {}", e);
        }
    });
}

/// Dispatches an error to a channel
pub async fn dispatch_error(
    ctx: &serenity::all::Context,
    error: &str,
    guild_id: serenity::all::GuildId,
    template: &crate::GuildTemplate,
) -> Result<(), silverpelt::Error> {
    let data = ctx.data::<silverpelt::data::Data>();

    match template.error_channel {
        Some(c) => {
            let Some(channel) =
                sandwich_driver::channel(&ctx.cache, &ctx.http, &data.reqwest, Some(guild_id), c)
                    .await?
            else {
                return Ok(());
            };

            let Some(guild_channel) = channel.guild() else {
                return Ok(());
            };

            if guild_channel.guild_id != guild_id {
                return Ok(());
            }

            c.send_message(
                &ctx.http,
                serenity::all::CreateMessage::new()
                    .embed(
                        serenity::all::CreateEmbed::new()
                            .title("Error executing template")
                            .field("Error", error, false)
                            .field("Template", template.name.clone(), false),
                    )
                    .components(vec![serenity::all::CreateActionRow::Buttons(
                        vec![serenity::all::CreateButton::new_link(
                            &config::CONFIG.meta.support_server_invite,
                        )
                        .label("Support Server")]
                        .into(),
                    )]),
            )
            .await?;
        }
        None => {
            // Try firing the error event
            execute(
                event::Event::new(
                    "Error".to_string(),
                    "Error".to_string(),
                    "Error".to_string(),
                    event::ArcOrNormal::Normal(error.into()),
                    None,
                ),
                ParseCompileState {
                    serenity_context: ctx.clone(),
                    reqwest_client: data.reqwest.clone(),
                    guild_id,
                    pool: data.pool.clone(),
                },
                template.to_parsed_template()?,
            )
            .await?;
        }
    }

    Ok(())
}

#[derive(Clone)]
pub struct ThreadErrorTracker {
    pub tracker: ThreadTracker,
    #[allow(clippy::type_complexity)]
    pub returns: Rc<
        RefCell<
            HashMap<
                String,
                tokio::sync::mpsc::UnboundedSender<Option<mlua::Result<mlua::MultiValue>>>,
            >,
        >,
    >,
}

impl ThreadErrorTracker {
    /// Creates a new thread error tracker
    pub fn new(tracker: ThreadTracker) -> Self {
        Self {
            tracker,
            returns: Rc::new(RefCell::new(HashMap::new())),
        }
    }

    pub fn thread_string(&self, th: &mlua::Thread) -> String {
        format!("{:?}", th.to_pointer())
    }

    /// Track a threads result
    pub fn track_thread(
        &self,
        th: &mlua::Thread,
    ) -> tokio::sync::mpsc::UnboundedReceiver<Option<mlua::Result<mlua::MultiValue>>> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        self.returns.borrow_mut().insert(self.thread_string(th), tx);

        rx
    }
}

impl SchedulerFeedback for ThreadErrorTracker {
    fn on_response(
        &self,
        _label: &str,
        tm: &TaskManager,
        th: &mlua::Thread,
        result: Option<&mlua::Result<mlua::MultiValue>>,
    ) {
        if let Some(Err(e)) = result {
            let initiator = self.tracker.get_initiator(th).unwrap_or_else(|| th.clone());

            let Some(template_name) = self.tracker.get_metadata(&initiator) else {
                return; // We can't do anything without metadata
            };

            let e = e.to_string();

            log_error(tm.inner.lua.clone(), template_name, e);
        }

        if let Some(tx) = self.returns.borrow_mut().remove(&self.thread_string(th)) {
            let _ = tx.send(match result {
                Some(Ok(mv)) => Some(Ok(mv.clone())),
                Some(Err(e)) => Some(Err(e.clone())),
                None => None,
            });
        }
    }
}
