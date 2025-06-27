use super::client::{LuaVmAction, LuaVmResult};
use crate::templatingrt::primitives::ctxprovider::TemplateContextProvider;
use crate::templatingrt::state::GuildState;
use crate::templatingrt::template::Template;
use crate::templatingrt::MAX_TEMPLATES_EXECUTION_TIME;
use crate::templatingrt::MAX_TEMPLATES_RETURN_WAIT_TIME;
use crate::templatingrt::MAX_TEMPLATE_MEMORY_USAGE;
use crate::CONFIG;
use khronos_runtime::primitives::event::CreateEvent;
use khronos_runtime::primitives::event::Event;
use khronos_runtime::require::FilesystemWrapper;
use khronos_runtime::rt::mlua::prelude::*;
use khronos_runtime::rt::CreatedKhronosContext;
use khronos_runtime::rt::KhronosIsolate;
use khronos_runtime::rt::KhronosRuntime;
use khronos_runtime::rt::KhronosRuntimeInterruptData;
use khronos_runtime::rt::RuntimeCreateOpts;
use khronos_runtime::rt::{IsolateData, KhronosRuntimeManager as Krm};
use std::rc::Rc;
use std::sync::Arc;
use tokio::sync::oneshot::Sender;

pub type KhronosRuntimeManager = Krm<CreatedKhronosContext>;

impl LuaVmAction {
    pub async fn handle(
        self,
        tis_ref: KhronosRuntimeManager,
        gs: Rc<GuildState>,
        callback: Sender<Vec<(String, LuaVmResult)>>,
    ) {
        match self {
            LuaVmAction::DispatchEvent { event, templates } => {
                if event.name() == "INTERACTION_CREATE" {
                    log::info!("Found templates: {} {}", event.name(), templates.len());
                }

                let _ = callback.send(
                    dispatch_event_to_multiple_templates(templates, event, &tis_ref, gs).await,
                );
            }
            LuaVmAction::DispatchInlineEvent { event, template } => {
                let event = Event::from_create_event(&event);
                let name = template.name.clone();
                let result = dispatch_event_to_template(template, event, tis_ref, gs).await;

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

    pub async fn log_error(
        &self,
        guild_state: &GuildState,
        template: &Template,
    ) -> Result<(), crate::Error> {
        let error = match self {
            LuaVmResult::Ok { .. } => return Ok(()),
            LuaVmResult::LuaError { err } => format!("```lua\n{}```", err.replace('`', "\\`")),
            LuaVmResult::VmBroken {} => format!("VM marked as broken!"),
        };

        if let Some(error_channel) = template.error_channel {
            let Some(channel) = crate::sandwich::channel(
                &guild_state.serenity_context.cache,
                &guild_state.serenity_context.http,
                &guild_state.reqwest_client,
                Some(template.guild_id),
                error_channel,
            )
            .await?
            else {
                // Send to main server
                crate::CONFIG
                    .meta
                    .default_error_channel
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
pub(super) fn configure_runtime_manager() -> LuaResult<KhronosRuntimeManager> {
    let mut rt = KhronosRuntime::new(
        RuntimeCreateOpts {
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
        None::<(fn(&Lua, LuaThread) -> Result<(), LuaError>, fn() -> ())>,
    )?;

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
        return (LuaVmResult::VmBroken {})
            .log_error_and_warn(&guild_state, &template)
            .await;
    }

    // Get or create a subisolate
    let (sub_isolate, created_context) = if let Some(sub_isolate) =
        manager.get_sub_isolate(&template.name)
    {
        (sub_isolate.isolate, sub_isolate.data)
    } else {
        let mut attempts = 0;
        let sub_isolate = loop {
            // It may take a few attempts to create a subisolate successfully
            // due to ongoing Lua VM operations
            match KhronosIsolate::new_subisolate(
                manager.runtime().clone(),
                FilesystemWrapper::new(template.content.0.clone()),
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
                        return (LuaVmResult::VmBroken {})
                            .log_error_and_warn(&guild_state, &template)
                            .await;
                    }
                }
            }
        };

        log::info!("Created subisolate for template {}", template.name);

        let provider = TemplateContextProvider::new(guild_state.clone(), template.clone());

        let created_context = match sub_isolate.create_context(provider) {
            Ok(ctx) => ctx,
            Err(e) => {
                return (LuaVmResult::LuaError { err: e.to_string() })
                    .log_error_and_warn(&guild_state, &template)
                    .await
            }
        };

        let iso_data = IsolateData {
            isolate: sub_isolate.clone(),
            data: created_context.clone(),
        };

        manager.add_sub_isolate(template.name.clone(), iso_data);

        (sub_isolate, created_context)
    };

    let spawn_result = match sub_isolate
        .spawn_asset("/init.luau", "/init.luau", created_context, event)
        .await
    {
        Ok(sr) => sr,
        Err(e) => {
            return (LuaVmResult::LuaError { err: e.to_string() })
                .log_error_and_warn(&guild_state, &template)
                .await;
        }
    };

    let json_value = match spawn_result.into_serde_json_value(&sub_isolate) {
        Ok(v) => v,
        Err(e) => {
            return (LuaVmResult::LuaError {
                err: format!("Failed to convert result to JSON: {}", e),
            })
            .log_error_and_warn(&guild_state, &template)
            .await;
        }
    };

    LuaVmResult::Ok {
        result_val: json_value,
    }
}

pub async fn dispatch_event_to_multiple_templates(
    templates: Vec<Arc<Template>>,
    event: CreateEvent,
    manager: &KhronosRuntimeManager,
    guild_state: Rc<GuildState>,
) -> Vec<(String, LuaVmResult)> {
    log::debug!("Dispatching event to {} templates", templates.len());

    let mut set = tokio::task::JoinSet::new();
    let t_len = templates.len();
    for template in templates {
        let manager_ref = manager.clone();
        let gs = guild_state.clone();
        let event = Event::from_create_event(&event);
        set.spawn_local(async move {
            let name = template.name.clone();
            let result = dispatch_event_to_template(template, event, manager_ref, gs).await;

            (name, result)
        });
    }

    let mut results = Vec::with_capacity(t_len);
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
