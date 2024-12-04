use crate::{handle_event, lang_lua::ArLua, LuaVmAction, LuaVmResult, MAX_VM_THREAD_STACK_SIZE};
use serenity::all::GuildId;
use std::{panic::PanicHookInfo, sync::Arc};

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
    let bytecode_cache = Arc::new(scc::HashMap::new());

    let userdata = crate::lang_lua::create_lua_vm_userdata(
        last_execution_time.clone(),
        bytecode_cache.clone(),
        guild_id,
        pool,
        serenity_context,
        reqwest_client,
        shard_messenger,
    )?;

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

                let tis_ref = Arc::new(
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
