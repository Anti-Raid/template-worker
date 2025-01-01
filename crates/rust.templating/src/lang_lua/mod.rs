pub mod ctx;
pub mod event;
pub mod primitives_docs;
pub(crate) mod state;

mod plugins;
use mlua_scheduler::TaskManager;
use mlua_scheduler_ext::feedbacks::ThreadTracker;
use mlua_scheduler_ext::Scheduler;
pub use plugins::PLUGINS;

mod handler;
pub use handler::handle_event;

use crate::atomicinstant;
use crate::{MAX_TEMPLATES_EXECUTION_TIME, MAX_TEMPLATE_MEMORY_USAGE};
use mlua::prelude::*;
use serenity::all::GuildId;
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
        template: Arc<crate::Template>,
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
    Ok { result_val: serde_json::Value },
    LuaError { err: String },
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
            LuaVmResult::LuaError { err } => Err(format!("Lua error: {}", err).into()),
            LuaVmResult::VmBroken {} => Err("Lua VM is marked as broken".into()),
        }
    }

    /// Returns ``true`` if the result is an LuaError or VmBroken
    pub fn is_error(&self) -> bool {
        matches!(
            self,
            LuaVmResult::LuaError { .. } | LuaVmResult::VmBroken {}
        )
    }

    /// Logs an error in the case of a error lua vm result
    pub async fn log_error(
        &self,
        template_name: &str,
        guild_id: serenity::all::GuildId,
        pool: &sqlx::PgPool,
        serenity_context: &serenity::all::Context,
    ) -> Result<(), silverpelt::Error> {
        match self {
            LuaVmResult::VmBroken {} => {
                log::error!("Lua VM is broken in template {}", template_name);
                crate::lang_lua::log_error(
                    guild_id,
                    pool,
                    serenity_context,
                    template_name,
                    "Lua VM has been marked as broken".to_string(),
                )
                .await?;
            }
            LuaVmResult::LuaError { ref err } => {
                log::error!("Lua error in template {}: {}", template_name, err);

                crate::lang_lua::log_error(
                    guild_id,
                    pool,
                    serenity_context,
                    template_name,
                    err.to_string(),
                )
                .await?;
            }
            _ => {}
        }

        Ok(())
    }
}

/// Map of template name to bytecode
///
/// Note that it is assumed for BytecodeCache to be uniquely made per server
pub struct BytecodeCache(scc::HashMap<String, (Vec<u8>, u64)>);

impl BytecodeCache {
    /// Create a new bytecode cache
    pub fn new() -> Self {
        BytecodeCache(scc::HashMap::new())
    }
}

impl Deref for BytecodeCache {
    type Target = scc::HashMap<String, (Vec<u8>, u64)>;

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

    lua.set_app_data(thread_tracker.clone());

    let scheduler = Scheduler::new(TaskManager::new(lua.clone(), Rc::new(thread_tracker)));

    scheduler.attach();

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

pub(crate) fn create_guild_state(
    last_execution_time: Arc<atomicinstant::AtomicInstant>,
    guild_id: GuildId,
    pool: sqlx::PgPool,
    serenity_context: serenity::all::Context,
    shard_messenger: serenity::all::ShardMessenger,
    reqwest_client: reqwest::Client,
) -> Result<state::GuildState, silverpelt::Error> {
    Ok(state::GuildState {
        pool,
        guild_id,
        serenity_context,
        shard_messenger,
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
    template: &crate::Template,
    bytecode_cache_ref: &BytecodeCache,
) -> Result<Vec<u8>, LuaError> {
    // Check if the source hash matches the expected source hash
    let mut hasher = std::hash::DefaultHasher::new();
    template.content.hash(&mut hasher);
    let cur_hash = hasher.finish();

    let existing_bycode = bytecode_cache_ref.read(&template.name, |_, v| {
        if v.1 == cur_hash {
            Some(v.0.clone())
        } else {
            None
        }
    });

    if let Some(Some(bytecode)) = existing_bycode {
        return Ok(bytecode);
    }

    let bytecode = create_lua_compiler().compile(&template.content)?;

    let _ = bytecode_cache_ref
        .insert_async(template.name.clone(), (bytecode.clone(), cur_hash))
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
    shard_messenger: serenity::all::ShardMessenger,
    reqwest_client: reqwest::Client,
) -> Result<ArLua, silverpelt::Error> {
    let Some(mut vm) = VMS.get(&guild_id) else {
        let vm = vm_manager::create_lua_vm(
            guild_id,
            pool,
            serenity_context,
            shard_messenger,
            reqwest_client,
        )
        .await?;
        if let Err((_, alt_vm)) = VMS.insert_async(guild_id, vm.clone()).await {
            return Ok(alt_vm);
        }
        return Ok(vm);
    };

    if vm.broken.load(std::sync::atomic::Ordering::Acquire) {
        let new_vm = vm_manager::create_lua_vm(
            guild_id,
            pool,
            serenity_context,
            shard_messenger,
            reqwest_client,
        )
        .await?;
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
    shard_messenger: serenity::all::ShardMessenger,
    reqwest_client: reqwest::Client,
) -> Result<FireBenchmark, silverpelt::Error> {
    // Get_lua_vm
    let pool_a = pool.clone();
    let guild_id_a = guild_id;
    let serenity_context_a = serenity_context.clone();
    let reqwest_client_a = reqwest_client.clone();

    let start = std::time::Instant::now();
    let _ = get_lua_vm(
        guild_id_a,
        pool_a,
        serenity_context_a,
        shard_messenger.clone(),
        reqwest_client_a,
    )
    .await?;
    let get_lua_vm = start.elapsed().as_micros();

    let new_map = scc::HashMap::new();
    let start = std::time::Instant::now();
    let _ = new_map.insert_async(1, 1).await;
    let hashmap_insert_time = start.elapsed().as_micros();

    // Exec simple with wait

    let pt = crate::Template {
        pragma: crate::TemplatePragma::default(),
        content: "return 1".to_string(),
        ..Default::default()
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
            serde_json::Value::Null,
            None,
        ),
        ParseCompileState {
            serenity_context: serenity_context_a,
            shard_messenger: shard_messenger.clone(),
            reqwest_client: reqwest_client_a,
            guild_id: guild_id_a,
            pool: pool_a,
        },
        pt.into(),
    )
    .await?
    .wait()
    .await?
    .to_response()?;

    let exec_simple = start.elapsed().as_micros();

    if n != 1 {
        return Err(format!("Expected 1, got {}", n).into());
    }

    // Exec simple with no wait
    let pt = crate::Template {
        pragma: crate::TemplatePragma::default(),
        content: "return 1".to_string(),
        ..Default::default()
    };

    let pool_a = pool.clone();
    let guild_id_a = guild_id;
    let serenity_context_a = serenity_context.clone();
    let reqwest_client_a = reqwest_client.clone();

    let start = std::time::Instant::now();
    execute(
        event::Event::new(
            "Benchmark".to_string(),
            "Benchmark".to_string(),
            "Benchmark".to_string(),
            serde_json::Value::Null,
            None,
        ),
        ParseCompileState {
            serenity_context: serenity_context_a,
            shard_messenger: shard_messenger.clone(),
            reqwest_client: reqwest_client_a,
            guild_id: guild_id_a,
            pool: pool_a,
        },
        pt.into(),
    )
    .await?;
    let exec_no_wait = start.elapsed().as_micros();

    // Exec simple with wait
    let pt = crate::Template {
        pragma: crate::TemplatePragma::default(),
        content: "error('MyError')\nreturn 1".to_string(),
        ..Default::default()
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
            serde_json::Value::Null,
            None,
        ),
        ParseCompileState {
            serenity_context: serenity_context_a,
            shard_messenger: shard_messenger.clone(),
            reqwest_client: reqwest_client_a,
            guild_id: guild_id_a,
            pool: pool_a,
        },
        pt.into(),
    )
    .await?
    .wait()
    .await?;
    let exec_error = start.elapsed().as_micros();

    match err {
        LuaVmResult::LuaError { err } => {
            if !err.contains("MyError") {
                return Err(format!("Expected MyError, got {}", err).into());
            }
        }
        _ => {
            return Err("Expected error, got success".into());
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
    pub shard_messenger: serenity::all::ShardMessenger,
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
    template: Arc<crate::Template>,
) -> Result<RenderTemplateHandle, silverpelt::Error> {
    let lua = get_lua_vm(
        state.guild_id,
        state.pool,
        state.serenity_context,
        state.shard_messenger,
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
        .send((LuaVmAction::Exec { template, event }, tx))
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

    /// Waits for the template to render, then logs an error if the result is an error
    pub async fn wait_and_log_error(
        self,
        template_name: &str,
        guild_id: serenity::all::GuildId,
        pool: &sqlx::PgPool,
        serenity_context: &serenity::all::Context,
    ) -> Result<LuaVmResult, silverpelt::Error> {
        let res = self.wait().await?;
        res.log_error(template_name, guild_id, pool, serenity_context)
            .await?;
        Ok(res)
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
}

/// Helper method to get guild template and log error
pub async fn log_error(
    guild_id: serenity::all::GuildId,
    pool: &sqlx::PgPool,
    serenity_context: &serenity::all::Context,
    template_name: &str,
    e: String,
) -> Result<(), silverpelt::Error> {
    log::error!("Lua thread error: {}: {}", template_name, e);

    let Ok(template) = crate::cache::get_guild_template(guild_id, template_name, pool).await else {
        return Err("Failed to get template data for error reporting".into());
    };

    dispatch_error(serenity_context, &e, guild_id, &template).await
}

/// Dispatches an error to a channel
pub async fn dispatch_error(
    ctx: &serenity::all::Context,
    error: &str,
    guild_id: serenity::all::GuildId,
    template: &crate::Template,
) -> Result<(), silverpelt::Error> {
    let data = ctx.data::<silverpelt::data::Data>();

    if let Some(error_channel) = template.error_channel {
        let Some(channel) = sandwich_driver::channel(
            &ctx.cache,
            &ctx.http,
            &data.reqwest,
            Some(guild_id),
            error_channel,
        )
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

        guild_channel
            .send_message(
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

    Ok(())
}
