use super::client::ArLua;
use super::client::{ArLuaHandle, LuaVmResult, VMS};
use super::core::{
    create_guild_state, dispatch_event_to_multiple_templates, dispatch_event_to_template,
};
use crate::templatingrt::cache::{get_guild_template, get_all_guild_templates};
use crate::templatingrt::vm_manager::client::LuaVmAction;
use crate::templatingrt::vm_manager::core::configure_runtime_manager;
use crate::templatingrt::MAX_VM_THREAD_STACK_SIZE;
use khronos_runtime::primitives::event::Event;
use serenity::all::GuildId;
use std::rc::Rc;

#[allow(dead_code)]
pub async fn create_lua_vm(
    guild_id: GuildId,
    pool: sqlx::PgPool,
    serenity_context: serenity::all::Context,
    reqwest_client: reqwest::Client,
) -> Result<ArLua, silverpelt::Error> {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<(
        LuaVmAction,
        tokio::sync::oneshot::Sender<Vec<(String, LuaVmResult)>>,
    )>();

    std::thread::Builder::new()
        .name(format!("lua-vm-{}", guild_id))
        .stack_size(MAX_VM_THREAD_STACK_SIZE)
        .spawn(move || {
            let gs = Rc::new(
                create_guild_state(guild_id, pool, serenity_context, reqwest_client)
                    .expect("Failed to create Lua VM userdata"),
            );

            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to create tokio runtime");

            let local = tokio::task::LocalSet::new();

            local.block_on(&rt, async {
                let tis_ref = match configure_runtime_manager() {
                    Ok(tis) => tis,
                    Err(e) => {
                        log::error!("Failed to configure Lua VM: {}", e);
                        panic!("Failed to configure Lua VM");
                    }
                };

                tis_ref.set_on_broken(Box::new(move || {
                    VMS.remove(&guild_id);
                }));

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
                            LuaVmAction::DispatchTemplateEvent { event, template_name } => {
                                let event = Event::from_create_event(&event);
                                let Some(template) = get_guild_template(guild_id, &template_name).await else {
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
                                let result =
                                    dispatch_event_to_template(template, event, tis_ref, gs).await;

                                // Send back to the caller
                                let _ = callback.send(vec![(name, result)]);
                            }
                            LuaVmAction::Stop {} => {
                                // Mark VM as broken
                                if let Err(e) = tis_ref.runtime().mark_broken(true) {
                                    log::error!("Failed to mark VM as broken: {}", e);
                                }

                                let _ = callback.send(vec![(
                                    "_".to_string(),
                                    LuaVmResult::Ok {
                                        result_val: serde_json::Value::Null,
                                    },
                                )]);
                            }
                            LuaVmAction::GetMemoryUsage {} => {
                                let used = tis_ref.runtime().memory_usage();

                                let _ = callback.send(vec![(
                                    "_".to_string(),
                                    LuaVmResult::Ok {
                                        result_val: serde_json::Value::Number(used.into()),
                                    },
                                )]);
                            }
                            LuaVmAction::SetMemoryLimit { limit } => {
                                let result = match tis_ref.runtime().set_memory_limit(limit) {
                                    Ok(limit) => LuaVmResult::Ok {
                                        result_val: serde_json::Value::Number(limit.into()),
                                    },
                                    Err(e) => LuaVmResult::LuaError { err: e.to_string() },
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
            });
        })?;

    Ok(ArLua::ThreadPerGuild(PerThreadLuaHandle { handle: tx }))
}

#[derive(Clone)]
pub struct PerThreadLuaHandle {
    #[allow(clippy::type_complexity)]
    /// The thread handle for the Lua VM
    pub handle: tokio::sync::mpsc::UnboundedSender<(
        LuaVmAction,
        tokio::sync::oneshot::Sender<Vec<(String, LuaVmResult)>>,
    )>,
}

impl ArLuaHandle for PerThreadLuaHandle {
    fn send_action(
        &self,
        action: LuaVmAction,
        callback: tokio::sync::oneshot::Sender<Vec<(String, LuaVmResult)>>,
    ) -> Result<(), khronos_runtime::Error> {
        self.handle.send((action, callback))?;
        Ok(())
    }
}
