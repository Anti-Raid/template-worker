use crate::{
    handle_event,
    lang_lua::{ArLua, BytecodeCache, XRc},
    LuaVmAction, LuaVmResult, MAX_VM_THREAD_STACK_SIZE,
};
use mlua_scheduler::{taskmgr::SchedulerFeedback, TaskManager};
use mlua_scheduler_ext::{
    feedbacks::{MultipleSchedulerFeedback, ThreadTracker},
    Scheduler,
};
use serenity::all::GuildId;
use std::{panic::PanicHookInfo, rc::Rc, sync::Arc, time::Duration};

#[derive(Clone)]
pub struct ThreadErrorTracker {
    pub tracker: ThreadTracker,
}

impl ThreadErrorTracker {
    /// Creates a new thread error tracker
    pub fn new(tracker: ThreadTracker) -> Self {
        Self { tracker }
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
            let inner = tm.inner.clone();
            tokio::task::spawn_local(async move {
                log::error!("Lua thread error: {}: {}", template_name, e);

                let user_data = inner
                    .lua
                    .app_data_ref::<crate::lang_lua::state::LuaUserData>()
                    .unwrap();

                let Ok(template) = crate::cache::get_guild_template(
                    user_data.guild_id,
                    &template_name,
                    &user_data.pool,
                )
                .await
                else {
                    log::error!("Failed to get template data for error reporting");
                    return;
                };

                if let Err(e) = crate::dispatch_error(
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
    }
}

pub fn lua_thread_impl(
    guild_id: GuildId,
    pool: sqlx::PgPool,
    shard_messenger: serenity::all::ShardMessenger,
    serenity_context: serenity::all::Context,
    reqwest_client: reqwest::Client,
) -> Result<ArLua, silverpelt::Error> {
    let broken = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let last_execution_time = Arc::new(crate::atomicinstant::AtomicInstant::new(
        std::time::Instant::now(),
    ));

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<(
        LuaVmAction,
        tokio::sync::oneshot::Sender<LuaVmResult>,
    )>();

    let broken_ref = broken.clone();
    let last_execution_time_ref = last_execution_time.clone();

    std::thread::Builder::new()
        .name(format!("lua-vm-{}", guild_id))
        .stack_size(MAX_VM_THREAD_STACK_SIZE)
        .spawn(move || {
            let userdata = crate::lang_lua::create_lua_vm_userdata(
                last_execution_time_ref.clone(),
                guild_id,
                pool,
                serenity_context,
                reqwest_client,
                shard_messenger,
            )
            .expect("Failed to create Lua VM userdata");

            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();

            let local = tokio::task::LocalSet::new();

            local.block_on(&rt, async {
                // Catch panics
                fn panic_catcher(
                    guild_id: GuildId,
                    broken_ref: Arc<std::sync::atomic::AtomicBool>,
                ) -> Box<dyn Fn(&PanicHookInfo<'_>) + 'static + Sync + Send> {
                    Box::new(move |_| {
                        log::error!("Lua thread panicked: {}", guild_id);
                        broken_ref.store(true, std::sync::atomic::Ordering::Release);
                    })
                }

                let bytecode_cache = BytecodeCache::new();

                let tis_ref = XRc::new(
                    match crate::lang_lua::configure_lua_vm(
                        broken_ref,
                        last_execution_time_ref,
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

                // Also create the mlua scheduler in the app data
                let thread_tracker = ThreadTracker::new();
                let ter = ThreadErrorTracker::new(thread_tracker.clone());

                let scheduler_feedback = MultipleSchedulerFeedback::new(vec![
                    Box::new(thread_tracker.clone()),
                    Box::new(ter.clone()),
                ]);

                tis_ref.lua.set_app_data(thread_tracker);
                tis_ref.lua.set_app_data(ter);

                let scheduler = Scheduler::new(TaskManager::new(
                    tis_ref.lua.clone(),
                    Rc::new(scheduler_feedback),
                ));

                scheduler.attach(&tis_ref.lua);

                // Start the scheduler in a tokio task
                let broken_sched_ref = tis_ref.broken.clone();
                local.spawn_local(async move {
                    match scheduler.run(Duration::from_millis(1)).await {
                        Ok(_) => {
                            log::info!("Lua scheduler exited. This should not happen.");

                            // If the scheduler exited, the Lua VM is broken
                            broken_sched_ref.store(true, std::sync::atomic::Ordering::Release);
                        }
                        Err(e) => {
                            log::error!("Lua scheduler exited with error: {}", e);

                            // If the scheduler exited, the Lua VM is broken
                            broken_sched_ref.store(true, std::sync::atomic::Ordering::Release);
                        }
                    }
                });

                super::perthreadpanichook::set_hook(panic_catcher(
                    guild_id,
                    tis_ref.broken.clone(),
                ));

                while let Some((action, callback)) = rx.recv().await {
                    let tis_ref = tis_ref.clone();
                    local.spawn_local(async move {
                        let result = handle_event(action, &tis_ref).await;

                        let _ = callback.send(result);
                    });
                }
            });
        })?;

    Ok(ArLua {
        last_execution_time,
        handle: tx,
        broken,
    })
}
