use super::client::LuaVmResult;
use crate::templatingrt::cache::get_all_guild_templates;
use crate::templatingrt::primitives::assetmanager::TemplateAssetManager;
use crate::templatingrt::primitives::ctxprovider::TemplateContextProvider;
use crate::templatingrt::state::GuildState;
use crate::templatingrt::state::Ratelimits;
use crate::templatingrt::template::Template;
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
use khronos_runtime::TemplateContext;
use mlua::prelude::*;
use serenity::all::GuildId;
use silverpelt::templates::LuaKVConstraints;
use std::rc::Rc;
use std::sync::Arc;

/// Configures the khronos runtime.
pub(super) fn configure_runtime_manager() -> LuaResult<KhronosRuntimeManager<TemplateAssetManager>>
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
        //None,
    )?;

    rt.lua().set_memory_limit(MAX_TEMPLATE_MEMORY_USAGE)?;

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
pub(super) async fn dispatch_event_to_template(
    template: Arc<Template>,
    event: Event,
    manager: &KhronosRuntimeManager<TemplateAssetManager>,
    guild_state: Rc<GuildState>,
) -> LuaVmResult {
    if manager.runtime().is_broken() {
        return LuaVmResult::VmBroken {};
    }

    // Get or create a subisolate
    let sub_isolate = if let Some(sub_isolate) = manager.get_sub_isolate(&template.name) {
        sub_isolate
    } else {
        let sub_isolate = match KhronosIsolate::new_subisolate(
            manager.runtime().clone(),
            TemplateAssetManager::new(template.clone()),
            {
                let mut pset = PluginSet::new();
                pset.add_default_plugins::<TemplateContextProvider>();
                pset
            },
        ) {
            Ok(sub_isolate) => sub_isolate,
            Err(e) => {
                return LuaVmResult::LuaError {
                    err: format!("Failed to create subisolate: {}", e),
                };
            }
        };

        manager.add_sub_isolate(template.name.clone(), sub_isolate.clone());

        sub_isolate
    };

    // Now, create the template context that should be passed to the template
    let provider = TemplateContextProvider {
        guild_state,
        template_data: template.clone(),
        isolate: sub_isolate.clone(),
    };

    let template_context = TemplateContext::new(provider);

    let spawn_result = match sub_isolate
        .spawn_asset("init.luau", "init.luau", template_context, event)
        .await
    {
        Ok(sr) => sr,
        Err(e) => {
            return LuaVmResult::LuaError { err: e.to_string() };
        }
    };

    let json_value = match spawn_result.into_serde_json_value(&sub_isolate) {
        Ok(v) => v,
        Err(e) => {
            return LuaVmResult::LuaError {
                err: format!("Failed to convert result to JSON: {}", e),
            };
        }
    };

    LuaVmResult::Ok {
        result_val: json_value,
    }
}

pub(super) async fn dispatch_event_to_multiple_templates(
    templates: Arc<Vec<Arc<Template>>>,
    event: CreateEvent,
    manager: &KhronosRuntimeManager<TemplateAssetManager>,
    guild_state: Rc<GuildState>,
) -> Vec<(String, LuaVmResult)> {
    let mut set = tokio::task::JoinSet::new();
    for template in templates.iter().filter(|t| t.should_dispatch(&event)) {
        let template = template.clone();
        let manager_ref = manager.clone();
        let gs = guild_state.clone();
        let event = Event::from_create_event(&event);
        set.spawn_local(async move {
            let name = template.name.clone();
            let result = dispatch_event_to_template(template, event, &manager_ref, gs).await;

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

pub(super) async fn reset_vm_cache(
    guild_id: GuildId,
    runtime: &KhronosRuntimeManager<TemplateAssetManager>,
) {
    if let Some(templates) = get_all_guild_templates(guild_id).await {
        for template in templates.iter() {
            if let Some(vm) = runtime.get_sub_isolate(&template.name) {
                vm.asset_manager().set_template(template.clone());
            }
        }
    }
}
