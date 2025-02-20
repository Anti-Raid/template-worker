use super::client::{ArLuaHandle, LuaVmResult};
use super::core::{configure_lua_vm, create_guild_state, BytecodeCache};
use super::{client::ArLua, AtomicInstant};
use crate::templatingrt::cache::get_all_guild_templates;
use crate::templatingrt::vm_manager::client::LuaVmAction;
use crate::templatingrt::vm_manager::core::dispatch_event_to_template;
use crate::templatingrt::{MAX_TEMPLATES_RETURN_WAIT_TIME, MAX_VM_THREAD_STACK_SIZE};
use khronos_runtime::primitives::event::Event;
use serenity::all::GuildId;
use std::rc::Rc;
use std::sync::atomic::AtomicBool;
use std::{panic::PanicHookInfo, sync::Arc};

#[allow(dead_code)]
pub async fn create_lua_vm(
    guild_id: GuildId,
    pool: sqlx::PgPool,
    serenity_context: serenity::all::Context,
    reqwest_client: reqwest::Client,
) -> Result<ArLua, silverpelt::Error> {
    let broken = Arc::new(AtomicBool::new(false));
    let last_execution_time = Arc::new(super::AtomicInstant::new(std::time::Instant::now()));

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<(
        LuaVmAction,
        tokio::sync::oneshot::Sender<Vec<(String, LuaVmResult)>>,
    )>();

    let broken_ref = broken.clone();
    let last_execution_time_ref = last_execution_time.clone();

    std::thread::Builder::new()
        .name(format!("lua-vm-{}", guild_id))
        .stack_size(MAX_VM_THREAD_STACK_SIZE)
        .spawn(move || {
            let gs = Rc::new(
                create_guild_state(
                    last_execution_time_ref.clone(),
                    guild_id,
                    pool,
                    serenity_context,
                    reqwest_client,
                )
                .expect("Failed to create Lua VM userdata"),
            );

            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();

            let local = tokio::task::LocalSet::new();

            local.block_on(&rt, async {
                // Catch panics
                fn panic_catcher(
                    guild_id: GuildId,
                    broken_ref: Arc<AtomicBool>,
                ) -> Box<dyn Fn(&PanicHookInfo<'_>) + 'static + Sync + Send> {
                    Box::new(move |_| {
                        log::error!("Lua thread panicked: {}", guild_id);
                        broken_ref.store(true, std::sync::atomic::Ordering::Release);
                    })
                }

                let bytecode_cache = BytecodeCache::new();

                let tis_ref = Rc::new(
                    match configure_lua_vm(broken_ref, last_execution_time_ref, bytecode_cache) {
                        Ok(tis) => tis,
                        Err(e) => {
                            log::error!("Failed to configure Lua VM: {}", e);
                            panic!("Failed to configure Lua VM");
                        }
                    },
                );

                super::perthreadpanichook::set_hook(panic_catcher(
                    guild_id,
                    tis_ref.broken.clone(),
                ));

                while let Some((action, callback)) = rx.recv().await {
                    let tis_ref = tis_ref.clone();
                    let gs = gs.clone();

                    local.spawn_local(async move {
                        match action {
                            LuaVmAction::DispatchEvent { event } => {
                                let Some(templates) = get_all_guild_templates(guild_id).await
                                else {
                                    return;
                                };

                                let mut set = tokio::task::JoinSet::new();
                                for template in
                                    templates.iter().filter(|t| t.should_dispatch(&event))
                                {
                                    let template = template.clone();
                                    let tis_ref = tis_ref.clone();
                                    let gs = gs.clone();
                                    let event = Event::from_create_event(&event);
                                    set.spawn_local(async move {
                                        let name = template.name.clone();
                                        let result = dispatch_event_to_template(
                                            template, event, &tis_ref, gs,
                                        )
                                        .await;

                                        (name, result)
                                    });
                                }

                                let mut results = Vec::new();
                                while let Ok(Some(result)) = tokio::time::timeout(
                                    MAX_TEMPLATES_RETURN_WAIT_TIME,
                                    set.join_next(),
                                )
                                .await
                                {
                                    match result {
                                        Ok((name, result)) => {
                                            results.push((name, result));
                                        }
                                        Err(e) => {
                                            log::error!(
                                                "Failed to dispatch event to template: {}",
                                                e
                                            );
                                        }
                                    }
                                }

                                // Send back to the caller
                                let _ = callback.send(results);
                            }
                            LuaVmAction::DispatchInlineEvent { event, template } => {
                                let event = Event::from_create_event(&event);
                                let name = template.name.clone();
                                let result =
                                    dispatch_event_to_template(template, event, &tis_ref, gs).await;

                                // Send back to the caller
                                let _ = callback.send(vec![(name, result)]);
                            }
                            LuaVmAction::Stop {} => {
                                // Mark VM as broken
                                tis_ref
                                    .broken
                                    .store(true, std::sync::atomic::Ordering::Release);

                                let _ = callback.send(vec![(
                                    "_".to_string(),
                                    LuaVmResult::Ok {
                                        result_val: serde_json::Value::Null,
                                    },
                                )]);
                            }
                            LuaVmAction::GetMemoryUsage {} => {
                                let used = tis_ref.lua.used_memory();

                                let _ = callback.send(vec![(
                                    "_".to_string(),
                                    LuaVmResult::Ok {
                                        result_val: serde_json::Value::Number(used.into()),
                                    },
                                )]);
                            }
                            LuaVmAction::SetMemoryLimit { limit } => {
                                let result = match tis_ref.lua.set_memory_limit(limit) {
                                    Ok(limit) => LuaVmResult::Ok {
                                        result_val: serde_json::Value::Number(limit.into()),
                                    },
                                    Err(e) => LuaVmResult::LuaError { err: e.to_string() },
                                };

                                let _ = callback.send(vec![("_".to_string(), result)]);
                            }
                        };
                    });
                }
            });
        })?;

    Ok(ArLua::ThreadPerGuild(PerThreadLuaHandle {
        last_execution_time,
        handle: tx,
        broken,
    }))
}

#[derive(Clone)]
pub struct PerThreadLuaHandle {
    /// The last execution time of the Lua VM
    pub last_execution_time: Arc<AtomicInstant>,

    #[allow(clippy::type_complexity)]
    /// The thread handle for the Lua VM
    pub handle: tokio::sync::mpsc::UnboundedSender<(
        LuaVmAction,
        tokio::sync::oneshot::Sender<Vec<(String, LuaVmResult)>>,
    )>,

    /// Is the VM broken/needs to be remade
    pub broken: Arc<std::sync::atomic::AtomicBool>,
}

impl ArLuaHandle for PerThreadLuaHandle {
    fn broken(&self) -> bool {
        self.broken.load(std::sync::atomic::Ordering::Acquire)
    }

    fn set_broken(&self) {
        self.broken
            .store(true, std::sync::atomic::Ordering::Release);
    }

    fn last_execution_time(&self) -> std::time::Instant {
        self.last_execution_time
            .load(std::sync::atomic::Ordering::Acquire)
    }

    fn send_action(
        &self,
        action: LuaVmAction,
        callback: tokio::sync::oneshot::Sender<Vec<(String, LuaVmResult)>>,
    ) -> Result<(), khronos_runtime::Error> {
        self.handle.send((action, callback))?;
        Ok(())
    }
}
