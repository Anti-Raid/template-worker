use crate::CONFIG;
use crate::templatingrt::primitives::ctxprovider::TemplateContextProvider;
use crate::templatingrt::state::GuildState;
use crate::templatingrt::template::{ConstructedFS, Template};
use crate::templatingrt::MAX_TEMPLATES_EXECUTION_TIME;
use crate::templatingrt::MAX_TEMPLATES_RETURN_WAIT_TIME;
use crate::templatingrt::MAX_TEMPLATE_MEMORY_USAGE;
use khronos_runtime::primitives::event::CreateEvent;
use khronos_runtime::primitives::event::Event;
use khronos_runtime::rt::KhronosIsolate;
use khronos_runtime::rt::KhronosRuntime;
use khronos_runtime::rt::KhronosRuntimeInterruptData;
use khronos_runtime::rt::KhronosRuntimeManager;
use khronos_runtime::rt::RuntimeCreateOpts;
use khronos_runtime::utils::pluginholder::PluginSet;
use khronos_runtime::utils::threadlimitmw::ThreadLimiter;
use khronos_runtime::require::FilesystemWrapper;
use khronos_runtime::TemplateContext;
use mlua::prelude::*;
use std::rc::Rc;
use std::sync::Arc;
use super::client::{LuaVmAction, LuaVmResult};
use tokio::sync::oneshot::Sender;
use crate::templatingrt::sandwich_config;
use crate::templatingrt::cache::{get_all_guild_templates, get_guild_template};

impl LuaVmAction {
    pub async fn handle(
        self,
        tis_ref: KhronosRuntimeManager,
        gs: Rc<GuildState>,
        callback: Sender<Vec<(String, LuaVmResult)>>,
    ) {
        match self {
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
                        result_val: serde_json::Value::Number(
                            used.into(),
                        ),
                    },
                )]);
            }
            LuaVmAction::SetMemoryLimit { limit } => {
                let result = match tis_ref
                    .runtime()
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

                let _ = callback.send(vec![(
                    "_".to_string(),
                    LuaVmResult::Ok {
                        result_val: serde_json::Value::Null,
                    },
                )]);
            }
            LuaVmAction::Panic {} => {
                panic!("Panic() called");
            }
        };
    }
}

impl LuaVmResult {
    pub async fn log_error_and_warn(self, guild_state: &GuildState, template: &Template) -> Self {
        match self.log_error(guild_state, template).await {
            Ok(()) => self,
            Err(e) => {
                log::error!("Error logging error: {:?}", e);
                self
            }
        }
    }

    pub async fn log_error(&self, guild_state: &GuildState, template: &Template) -> Result<(), crate::Error> {
        let error = match self {
            LuaVmResult::Ok { .. } => return Ok(()),
            LuaVmResult::LuaError { err } => format!("```lua\n{}```", err.replace('`', "\\`")),
            LuaVmResult::VmBroken {} => format!("VM marked as broken!")
        };

        if let Some(error_channel) = template.error_channel {
            let Some(channel) = sandwich_driver::channel(
                &guild_state.serenity_context.cache,
                &guild_state.serenity_context.http,
                &guild_state.reqwest_client,
                Some(template.guild_id),
                error_channel,
                &sandwich_config(),
            )
            .await?
            else {
                return Ok(());
            };

            let Some(guild_channel) = channel.guild() else {
                return Ok(());
            };

            if guild_channel.guild_id != template.guild_id {
                return Ok(());
            }

            guild_channel
                .send_message(
                    &guild_state.serenity_context.http,
                    serenity::all::CreateMessage::new()
                        .embed(
                            serenity::all::CreateEmbed::new()
                                .title("Error executing template")
                                .field("Error", error, false)
                                .field("Template", template.name.clone(), false),
                        )
                        .components(vec![serenity::all::CreateActionRow::Buttons(
                            vec![serenity::all::CreateButton::new_link(
                                &CONFIG.meta.support_server_invite,
                            )
                            .label("Support Server")]
                            .into(),
                        )]),
                )
                .await?;
        }

        Ok(())
    }
}

/// Configures the khronos runtime.
pub(super) fn configure_runtime_manager() -> LuaResult<KhronosRuntimeManager>
{
    let mut rt = KhronosRuntime::new(
        ThreadLimiter::new(10000),
        RuntimeCreateOpts {
            disable_scheduler_lib: false,
            disable_task_lib: false,
        },
        Some(|_a: &Lua, b: &KhronosRuntimeInterruptData| {
            let Some(last_execution_time) = b.last_execution_time else {
                return Ok(LuaVmState::Continue);
            };

            if last_execution_time.elapsed() >= MAX_TEMPLATES_EXECUTION_TIME {
                return Ok(LuaVmState::Yield);
            }

            Ok(LuaVmState::Continue)
        }),
        None::<(fn(&Lua, LuaThread) -> Result<(), mlua::Error>, fn() -> ())>
    )?;

    rt.load_plugins({
        let mut pset = PluginSet::new();
        pset.add_default_plugins::<TemplateContextProvider>();
        pset
    })?;

    rt.set_memory_limit(MAX_TEMPLATE_MEMORY_USAGE)?;

    rt.sandbox()?;

    Ok(KhronosRuntimeManager::new(rt))
}

/// Helper method to dispatch an event to a template
pub async fn dispatch_event_to_template(
    template: Arc<Template>,
    event: Event,
    manager: KhronosRuntimeManager,
    guild_state: Rc<GuildState>,
) -> LuaVmResult {
    if manager.runtime().is_broken() {
        return (LuaVmResult::VmBroken {}).log_error_and_warn(&guild_state, &template).await;
    }

    // Get or create a subisolate
    let sub_isolate = if let Some(sub_isolate) = manager.get_sub_isolate(&template.name) {
        sub_isolate
    } else {
        let mut attempts = 0;
        let sub_isolate = loop {
            // It may take a few attempts to create a subisolate successfully
            // due to ongoing Lua VM operations
            match KhronosIsolate::new_subisolate(
                manager.runtime().clone(),
                {
                    match template.ready_fs {
                        Some(ConstructedFS::Memory(ref fs)) => {
                            FilesystemWrapper::new(fs.clone())
                        },
                        Some(ConstructedFS::Overlay(ref fs)) => {
                            FilesystemWrapper::new(fs.clone())
                        },
                        None => {
                            return LuaVmResult::LuaError {
                                err: format!("Template {} does not have a ready filesystem", template.name),
                            };
                        }
                    }
                },
            ) {
                Ok(isolate) => {
                    break isolate;
                }
                Err(e) => {
                    log::error!("Failed to create subisolate: {}. This is an internal bug that should not happen", e);
                    attempts += 1;
                    if attempts >= 20 {
                        return LuaVmResult::LuaError {
                            err: format!("Failed to create subisolate: {}. This is an internal bug that should not happen", e),
                        };
                    }

                    // Wait a bit before retrying
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    // Check if the runtime is broken
                    if manager.runtime().is_broken() {
                        return (LuaVmResult::VmBroken {}).log_error_and_warn(&guild_state, &template).await;
                    }
                }
            }
        };

        log::info!("Created subisolate for template {}", template.name);
        manager.add_sub_isolate(template.name.clone(), sub_isolate.clone());

        sub_isolate
    };

    // Now, create the template context that should be passed to the template
    let provider = TemplateContextProvider::new(
        guild_state.clone(),
        template.clone(),
        manager
    );

    let template_context = TemplateContext::new(provider);

    let spawn_result = match sub_isolate
        .spawn_asset("/init.luau", "/init.luau", template_context, event)
        .await
    {
        Ok(sr) => sr,
        Err(e) => {
            return (LuaVmResult::LuaError { err: e.to_string() }).log_error_and_warn(&guild_state, &template).await;
        }
    };

    let json_value = match spawn_result.into_serde_json_value(&sub_isolate) {
        Ok(v) => v,
        Err(e) => {
            return (LuaVmResult::LuaError {
                err: format!("Failed to convert result to JSON: {}", e),
            }).log_error_and_warn(&guild_state, &template).await;
        }
    };

    LuaVmResult::Ok {
        result_val: json_value,
    }
}

pub async fn dispatch_event_to_multiple_templates(
    templates: Arc<Vec<Arc<Template>>>,
    event: CreateEvent,
    manager: &KhronosRuntimeManager,
    guild_state: Rc<GuildState>,
) -> Vec<(String, LuaVmResult)> {
    log::debug!("Dispatching event to {} templates", templates.len());

    let mut set = tokio::task::JoinSet::new();
    for template in templates.iter().filter(|t| t.should_dispatch(&event)) {
        let template = template.clone();
        let manager_ref = manager.clone();
        let gs = guild_state.clone();
        let event = Event::from_create_event(&event);
        set.spawn_local(async move {
            let name = template.name.clone();
            let result = dispatch_event_to_template(template, event, manager_ref, gs).await;

            (name, result)
        });
    }

    let mut results = Vec::with_capacity(templates.len());
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