pub mod ctx;
pub mod event;
pub mod primitives_docs;
pub mod samples;
pub(crate) mod state;

mod plugins;
pub use plugins::PLUGINS;

mod handler;
pub use handler::handle_event;

use crate::atomicinstant;
use crate::{MAX_TEMPLATES_EXECUTION_TIME, MAX_TEMPLATE_LIFETIME, MAX_TEMPLATE_MEMORY_USAGE};
use mlua::prelude::*;
use moka::future::Cache;
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

#[cfg(feature = "thread_proc")]
mod thread_proc;

static VMS: LazyLock<Cache<GuildId, ArLua>> =
    LazyLock::new(|| Cache::builder().time_to_idle(MAX_TEMPLATE_LIFETIME).build());

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
    /// Stop the Lua VM entirely
    Stop {},
    /// Returns the memory usage of the Lua VM
    GetMemoryUsage {},
    /// Set the memory limit of the Lua VM
    SetMemoryLimit { limit: usize },
}

pub enum LuaVmResult {
    Ok { result_val: serde_json::Value },
    LuaError { err: String },
    VmBroken {},
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
/// Note that the Lua VM is not directly exposed due to thread safety issues
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
    })
}

pub(crate) fn create_lua_vm_userdata(
    last_execution_time: Arc<atomicinstant::AtomicInstant>,
    guild_id: GuildId,
    pool: sqlx::PgPool,
    serenity_context: serenity::all::Context,
    reqwest_client: reqwest::Client,
    shard_messenger: serenity::all::ShardMessenger,
) -> Result<state::LuaUserData, silverpelt::Error> {
    Ok(state::LuaUserData {
        pool,
        guild_id,
        shard_messenger,
        serenity_context,
        reqwest_client,
        kv_constraints: state::LuaKVConstraints::default(),
        kv_ratelimits: Rc::new(state::LuaRatelimits::new_kv_rl()?),
        actions_ratelimits: Rc::new(state::LuaRatelimits::new_action_rl()?),
        sting_ratelimits: Rc::new(state::LuaRatelimits::new_stings_rl()?),
        last_execution_time,
    })
}

/// Create a new Lua VM complete with sandboxing and modules pre-loaded
///
/// Note that callers should instead call the render_template functions
///
/// As such, this function is private and should not be used outside of this module
async fn create_lua_vm(
    guild_id: GuildId,
    pool: sqlx::PgPool,
    serenity_context: serenity::all::Context,
    reqwest_client: reqwest::Client,
) -> Result<ArLua, silverpelt::Error> {
    #[cfg(feature = "thread_proc")]
    thread_proc::lua_thread_impl(
        guild_id,
        pool,
        shard_messenger_for_guild(&serenity_context, guild_id).await?,
        serenity_context,
        reqwest_client,
    )
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
    match VMS.get(&guild_id).await {
        Some(vm) => {
            if vm.broken.load(std::sync::atomic::Ordering::Acquire) {
                let vm = create_lua_vm(guild_id, pool, serenity_context, reqwest_client).await?;
                VMS.insert(guild_id, vm.clone()).await;
                return Ok(vm);
            }
            Ok(vm.clone())
        }
        None => {
            let vm = create_lua_vm(guild_id, pool, serenity_context, reqwest_client).await?;
            VMS.insert(guild_id, vm.clone()).await;
            Ok(vm)
        }
    }
}

#[derive(Clone)]
pub struct ParseCompileState {
    pub serenity_context: serenity::all::Context,
    pub reqwest_client: reqwest::Client,
    pub guild_id: GuildId,
    pub template: crate::Template,
    pub pragma: crate::TemplatePragma,
    pub template_content: String,
    pub pool: sqlx::PgPool,
}

/// Render a template
pub async fn render_template(
    event: event::Event,
    state: ParseCompileState,
) -> Result<(), silverpelt::Error> {
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

    let (tx, _rx) = tokio::sync::oneshot::channel();

    lua.handle
        .send((
            LuaVmAction::Exec {
                template: state.template,
                content: state.template_content,
                pragma: state.pragma,
                event,
            },
            tx,
        ))
        .map_err(|e| format!("Could not send data to Lua thread: {}", e))?;

    Ok(())
}

async fn shard_messenger_for_guild(
    serenity_context: &serenity::all::Context,
    guild_id: serenity::all::GuildId,
) -> Result<serenity::all::ShardMessenger, crate::Error> {
    let data = serenity_context.data::<silverpelt::data::Data>();

    let guild_shard_count = data.props.shard_count().await?;
    let guild_shard_count =
        std::num::NonZeroU16::new(guild_shard_count).ok_or("No shards available")?;
    let guild_shard_id = serenity::all::utils::shard_id(guild_id, guild_shard_count);
    let guild_shard_id = serenity::all::ShardId(guild_shard_id);

    if serenity_context.shard_id != guild_shard_id {
        return data.props.shard_messenger(guild_shard_id).await;
    }

    Ok(serenity_context.shard.clone())
}
