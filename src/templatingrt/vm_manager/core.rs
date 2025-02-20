use super::client::LuaVmResult;
use super::AtomicInstant;
use crate::templatingrt::primitives::ctxprovider::TemplateContextProvider;
use crate::templatingrt::state::GuildState;
use crate::templatingrt::state::Ratelimits;
use crate::templatingrt::template::Template;
use crate::templatingrt::MAX_TEMPLATES_EXECUTION_TIME;
use crate::templatingrt::MAX_TEMPLATES_RETURN_WAIT_TIME;
use crate::templatingrt::MAX_TEMPLATE_MEMORY_USAGE;
use khronos_runtime::primitives::event::CreateEvent;
use khronos_runtime::primitives::event::Event;
use khronos_runtime::utils::pluginholder::PluginSet;
use khronos_runtime::utils::prelude::setup_prelude;
use khronos_runtime::utils::proxyglobal::proxy_global;
use khronos_runtime::TemplateContext;
use mlua::prelude::*;
use mlua_scheduler::TaskManager;
use mlua_scheduler_ext::feedbacks::ThreadTracker;
use mlua_scheduler_ext::traits::IntoLuaThread;
use mlua_scheduler_ext::Scheduler;
use serenity::all::GuildId;
use silverpelt::templates::LuaKVConstraints;
use std::hash::Hash;
use std::hash::Hasher;
use std::ops::Deref;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::LazyLock;
use std::time::Duration;

pub static PLUGIN_SET: LazyLock<PluginSet> = LazyLock::new(|| {
    let mut plugins = PluginSet::new();
    plugins.add_default_plugins::<TemplateContextProvider>();
    plugins
});

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

pub(super) struct ArLuaThreadInnerState {
    /// The Lua VM
    pub lua: Lua,

    /// Last execution time
    pub last_execution_time: Arc<AtomicInstant>,

    /// The bytecode cache maps template to (bytecode, source hash)
    pub bytecode_cache: BytecodeCache,

    /// Is the VM broken/needs to be remade
    pub broken: Arc<std::sync::atomic::AtomicBool>,

    /// Stores the servers global table
    pub global_table: mlua::Table,

    #[allow(dead_code)]
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
    lua.set_compiler(compiler);

    let global_tab = proxy_global(&lua)?;

    // Prelude code providing some basic functions directly to the Lua VM
    setup_prelude(&lua, global_tab.clone())?;

    // Override require function for plugin support and increased security
    lua.globals().set(
        "require",
        lua.create_function(|this, module: String| PLUGIN_SET.require(this, module))?,
    )?;

    // Also create the mlua scheduler in the app data
    let thread_tracker = ThreadTracker::new();

    pub struct ThreadLimiter {
        pub thread_limit: usize,
        pub threads: std::cell::RefCell<usize>,
    }

    impl mlua_scheduler_ext::feedbacks::ThreadAddMiddleware for ThreadLimiter {
        fn on_thread_add(
            &self,
            _label: &str,
            _creator: &mlua::Thread,
            _thread: &mlua::Thread,
        ) -> mlua::Result<()> {
            let mut threads = self.threads.borrow_mut();
            if *threads >= self.thread_limit {
                return Err(mlua::Error::external("Thread limit reached"));
            }

            *threads += 1;

            Ok(())
        }
    }

    lua.set_app_data(thread_tracker.clone());

    let combined = mlua_scheduler_ext::feedbacks::ThreadAddMiddlewareFeedback::new(
        thread_tracker,
        ThreadLimiter {
            thread_limit: 10000,
            threads: std::cell::RefCell::new(0),
        },
    );

    let scheduler = Scheduler::new(TaskManager::new(
        lua.clone(),
        Rc::new(combined),
        Duration::from_millis(1),
    ));

    scheduler.attach();

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

    let broken_ref = broken.clone();
    let last_execution_time_ref = last_execution_time.clone();
    // Create an interrupt to limit the execution time of a template
    lua.set_interrupt(move |_| {
        if last_execution_time_ref
            .load(std::sync::atomic::Ordering::Acquire)
            .elapsed()
            >= MAX_TEMPLATES_EXECUTION_TIME
        {
            return Ok(LuaVmState::Yield);
        }

        if broken_ref.load(std::sync::atomic::Ordering::Acquire) {
            return Ok(LuaVmState::Yield);
        }

        Ok(LuaVmState::Continue)
    });

    Ok(ArLuaThreadInnerState {
        lua,
        bytecode_cache,
        last_execution_time,
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
    reqwest_client: reqwest::Client,
) -> Result<GuildState, silverpelt::Error> {
    Ok(GuildState {
        pool,
        guild_id,
        serenity_context,
        reqwest_client,
        kv_constraints: LuaKVConstraints::default(),
        ratelimits: Rc::new(Ratelimits::new()?),
        last_execution_time,
    })
}

/// Helper method to fetch a template from bytecode or compile it if it doesnt exist in bytecode cache
pub(crate) async fn resolve_template_to_bytecode(
    template: &Template,
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

/// Helper method to dispatch an event to a template
pub(super) async fn dispatch_event_to_template(
    template: Arc<Template>,
    event: Event,
    tis_ref: &ArLuaThreadInnerState,
    guild_state: Rc<GuildState>,
) -> LuaVmResult {
    if tis_ref.broken.load(std::sync::atomic::Ordering::Acquire) {
        return LuaVmResult::VmBroken {};
    }

    tis_ref.last_execution_time.store(
        std::time::Instant::now(),
        std::sync::atomic::Ordering::Release,
    );

    // Check bytecode cache first, compile template if not found
    let template_bytecode =
        match resolve_template_to_bytecode(&template, &tis_ref.bytecode_cache).await {
            Ok(bytecode) => bytecode,
            Err(e) => {
                return LuaVmResult::LuaError { err: e.to_string() };
            }
        };

    let thread = match tis_ref
        .lua
        .load(&template_bytecode)
        .set_name(&template.name)
        .set_mode(mlua::ChunkMode::Binary) // Ensure auto-detection never selects binary mode
        .set_environment(tis_ref.global_table.clone())
        .into_lua_thread(&tis_ref.lua)
    {
        Ok(f) => f,
        Err(e) => {
            // Mark memory error'd VMs as broken automatically to avoid user grief/pain
            if let LuaError::MemoryError(_) = e {
                // Mark VM as broken
                tis_ref
                    .broken
                    .store(true, std::sync::atomic::Ordering::Release);
            }

            return LuaVmResult::LuaError { err: e.to_string() };
        }
    };

    // Now, create the template context that should be passed to the template
    let provider = TemplateContextProvider {
        guild_state,
        template_data: template,
        global_table: tis_ref.global_table.clone(),
    };

    let template_context = TemplateContext::new(provider);

    let scheduler = tis_ref
        .lua
        .app_data_ref::<mlua_scheduler_ext::Scheduler>()
        .unwrap();

    let args = match (event, template_context).into_lua_multi(&tis_ref.lua) {
        Ok(f) => f,
        Err(e) => {
            // Mark memory error'd VMs as broken automatically to avoid user grief/pain
            if let LuaError::MemoryError(_) = e {
                // Mark VM as broken
                tis_ref
                    .broken
                    .store(true, std::sync::atomic::Ordering::Release);
            }

            return LuaVmResult::LuaError { err: e.to_string() };
        }
    };

    let Ok(value) = scheduler.spawn_thread_and_wait("Exec", thread, args).await else {
        return LuaVmResult::LuaError {
            err: "Failed to spawn thread".to_string(),
        };
    };

    let json_value = if let Some(Ok(values)) = value {
        match values.len() {
            0 => serde_json::Value::Null,
            1 => {
                let value = values.into_iter().next().unwrap();

                match tis_ref.lua.from_value::<serde_json::Value>(value) {
                    Ok(v) => v,
                    Err(e) => {
                        return LuaVmResult::LuaError { err: e.to_string() };
                    }
                }
            }
            _ => {
                let mut arr = Vec::with_capacity(values.len());

                for v in values {
                    match tis_ref.lua.from_value::<serde_json::Value>(v) {
                        Ok(v) => arr.push(v),
                        Err(e) => {
                            return LuaVmResult::LuaError { err: e.to_string() };
                        }
                    }
                }

                serde_json::Value::Array(arr)
            }
        }
    } else if let Some(Err(e)) = value {
        return LuaVmResult::LuaError { err: e.to_string() };
    } else {
        serde_json::Value::String("No response".to_string())
    };

    LuaVmResult::Ok {
        result_val: json_value,
    }
}

pub(super) async fn dispatch_event_to_multiple_templates(
    templates: Arc<Vec<Arc<Template>>>,
    event: CreateEvent,
    tis_ref: Rc<ArLuaThreadInnerState>,
    guild_state: Rc<GuildState>,
) -> Vec<(String, LuaVmResult)> {
    let mut set = tokio::task::JoinSet::new();
    for template in templates.iter().filter(|t| t.should_dispatch(&event)) {
        let template = template.clone();
        let tis_ref = tis_ref.clone();
        let gs = guild_state.clone();
        let event = Event::from_create_event(&event);
        set.spawn_local(async move {
            let name = template.name.clone();
            let result = dispatch_event_to_template(template, event, &tis_ref, gs).await;

            (name, result)
        });
    }

    let mut results = Vec::new();
    while let Ok(Some(result)) =
        tokio::time::timeout(MAX_TEMPLATES_RETURN_WAIT_TIME, set.join_next()).await
    {
        match result {
            Ok((name, result)) => {
                results.push((name, result));
            }
            Err(e) => {
                log::error!("Failed to dispatch event to template: {}", e);
            }
        }
    }

    results
}
