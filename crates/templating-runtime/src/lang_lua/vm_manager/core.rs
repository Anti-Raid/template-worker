use super::AtomicInstant;
use crate::lang_lua::plugins;
use crate::lang_lua::primitives::CreateEvent;
use crate::lang_lua::state::GuildState;
use crate::lang_lua::state::Ratelimits;
use crate::MAX_TEMPLATES_EXECUTION_TIME;
use crate::MAX_TEMPLATE_MEMORY_USAGE;
use mlua::prelude::*;
use mlua_scheduler::TaskManager;
use mlua_scheduler_ext::feedbacks::ThreadTracker;
use mlua_scheduler_ext::Scheduler;
use serenity::all::GuildId;
use silverpelt::templates::LuaKVConstraints;
use std::hash::Hash;
use std::hash::Hasher;
use std::ops::Deref;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::LazyLock;

// Vm creation strategies
#[cfg(feature = "thread_proc")]
use super::threadperguild_strategy::create_lua_vm;
#[cfg(feature = "threadpool_proc")]
use super::threadpool_strategy::create_lua_vm;

/// VM cache
static VMS: LazyLock<scc::HashMap<GuildId, ArLua>> = LazyLock::new(scc::HashMap::new);

#[derive(serde::Serialize, serde::Deserialize)]
#[allow(dead_code)]
pub enum LuaVmAction {
    /// Execute a template
    Exec {
        template: Arc<crate::Template>,
        event: CreateEvent,
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
pub(crate) struct ArLua {
    /// The last execution time of the Lua VM
    pub last_execution_time: Arc<AtomicInstant>,

    /// The thread handle for the Lua VM
    pub handle: tokio::sync::mpsc::UnboundedSender<(
        LuaVmAction,
        tokio::sync::oneshot::Sender<LuaVmResult>,
    )>,

    /// Is the VM broken/needs to be remade
    pub broken: Arc<std::sync::atomic::AtomicBool>,
}

pub(super) struct ArLuaThreadInnerState {
    /// The Lua VM
    pub lua: Lua,

    /// The bytecode cache maps template to (bytecode, source hash)
    pub bytecode_cache: BytecodeCache,

    /// Is the VM broken/needs to be remade
    pub broken: Arc<std::sync::atomic::AtomicBool>,

    /// Stores the servers global table
    pub global_table: mlua::Table,

    /// The scheduler
    pub scheduler: Scheduler,
}

pub(super) fn create_lua_compiler() -> mlua::Compiler {
    mlua::Compiler::new()
        .set_optimization_level(2)
        .set_type_info_level(1)
}

/// Configures a raw Lua VM. Note that userdata is not set in this function
pub(super) fn configure_lua_vm(
    broken: Arc<std::sync::atomic::AtomicBool>,
    last_execution_time: Arc<AtomicInstant>,
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

pub(super) fn create_guild_state(
    last_execution_time: Arc<AtomicInstant>,
    guild_id: GuildId,
    pool: sqlx::PgPool,
    serenity_context: serenity::all::Context,
    shard_messenger: serenity::all::ShardMessenger,
    reqwest_client: reqwest::Client,
) -> Result<GuildState, silverpelt::Error> {
    Ok(GuildState {
        pool,
        guild_id,
        serenity_context,
        shard_messenger,
        reqwest_client,
        kv_constraints: LuaKVConstraints::default(),
        ratelimits: Rc::new(Ratelimits::new()?),
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
pub(crate) async fn get_lua_vm(
    guild_id: GuildId,
    pool: sqlx::PgPool,
    serenity_context: serenity::all::Context,
    shard_messenger: serenity::all::ShardMessenger,
    reqwest_client: reqwest::Client,
) -> Result<ArLua, silverpelt::Error> {
    let Some(mut vm) = VMS.get(&guild_id) else {
        let vm = create_lua_vm(
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
        let new_vm = create_lua_vm(
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
