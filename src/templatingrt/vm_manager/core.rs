use super::client::LuaVmResult;
use crate::templatingrt::primitives::ctxprovider::TemplateContextProvider;
use crate::templatingrt::state::GuildState;
use crate::templatingrt::state::Ratelimits;
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
use khronos_runtime::utils::require_v2::FilesystemWrapper;
use khronos_runtime::TemplateContext;
use mlua::prelude::*;
use serenity::all::GuildId;
use silverpelt::templates::LuaKVConstraints;
use std::rc::Rc;
use std::sync::Arc;
use std::cell::RefCell;
use std::collections::HashMap;
use crate::templatingrt::log_error;
use khronos_runtime::rt::manager::IsolateData;
use serde_json::json;

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

pub(super) fn create_guild_state(
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
    })
}

/// Helper method to dispatch an event to a template
pub async fn dispatch_event_to_template(
    template: Arc<Template>,
    event: Event,
    manager: KhronosRuntimeManager,
    guild_state: Rc<GuildState>,
) -> LuaVmResult {
    if manager.runtime().is_broken() {
        return LuaVmResult::VmBroken {};
    }

    // Get or create a subisolate
    if let Some(sub_isolate_data) = manager.get_sub_isolate(&template.name) {
        sub_isolate_data.event_channel.0.send_async(event).await;
    } else {
        let sub_isolate = KhronosIsolate::new_subisolate(
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
        );

        let sub_isolate = match sub_isolate {
            Ok(isolate) => isolate,
            Err(e) => {
                log::error!("Failed to create subisolate: {}", e);
                return LuaVmResult::LuaError { err: e.to_string() };
            }
        };

        log::info!("Created subisolate for template {}", template.name);
        let event_channel = flume::unbounded();
        manager.add_sub_isolate(template.name.clone(), IsolateData {
            isolate: sub_isolate.clone(),
            event_channel: event_channel.clone(),
        });

        // Now, create the template context that should be passed to the template
        let template_name = template.name.to_string();

        let provider = TemplateContextProvider::new(
            guild_state.clone(),
            template,
            manager,
            event_channel.1.clone()
        );

        let template_context = TemplateContext::new(provider);

        // Start the sub-isolate in a spawn loop
        let guild_state_ref = guild_state.clone();
        if let Err(e) = sub_isolate.spawn_loop("/init.luau".to_string(), None, template_context, move |_, e| {
            let template_name = template_name.clone();
            let guild_state_ref = guild_state_ref.clone();
            async move {
                if let Err(e) = log_error(
                    guild_state_ref.guild_id,
                    &guild_state_ref.serenity_context,
                    &template_name,
                    e.to_string()
                )
                .await {
                    log::error!("Failed to log error: {}", e);
                }

                tokio::time::sleep(std::time::Duration::from_secs(2)).await;

                Ok(())
            }
        })
        .await {
            log::error!("Failed to spawn subisolate: {}", e);
            return LuaVmResult::LuaError { err: e.to_string() };
        }
        if let Err(e) = event_channel.0.send_async(event).await {
            log::error!("Failed to send event to subisolate: {}", e);
            return LuaVmResult::LuaError { err: e.to_string() };
        }
    };

    // Templates are long lived and as such do not have result_vals
    LuaVmResult::Ok {
        result_val: json!({}),
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