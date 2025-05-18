use std::collections::HashMap;

use crate::templatingrt::cache::{has_templates, has_templates_with_event};
use crate::templatingrt::{execute, LuaVmAction, ParseCompileState};
use antiraid_types::ar_event::AntiraidEvent;
use khronos_runtime::primitives::event::CreateEvent;
use serenity::all::{Context, FullEvent, GuildId, Interaction};
use silverpelt::data::Data;

#[inline]
const fn not_audit_loggable_event() -> &'static [&'static str] {
    &[
        "CACHE_READY",         // Internal
        "RATELIMIT",           // Internal
        "GUILD_CREATE",        // Internal
        "GUILD_MEMBERS_CHUNK", // Internal
    ]
}

pub async fn discord_event_dispatch(
    event: &FullEvent,
    serenity_context: &Context,
) -> Result<(), silverpelt::Error> {
    let data = serenity_context.data::<Data>();

    let Some(guild_id) = gwevent::core::get_event_guild_id(event) else {
        return Ok(());
    };

    let event_snake_name = event.snake_case_name();
    if not_audit_loggable_event().contains(&event_snake_name) {
        return Ok(());
    }

    let user_id = gwevent::core::get_event_user_id(event);

    let event_data = match event {
        FullEvent::GuildAuditLogEntryCreate { .. } => serde_json::to_value(event)?,
        FullEvent::InteractionCreate { interaction } => {
            match interaction {
                Interaction::Ping(_) => return Ok(()),
                Interaction::Command(_) | Interaction::Autocomplete(_) => {
                    let mut value = serde_json::to_value(interaction)?;

                    // Inject in type
                    if let serde_json::Value::Object(ref mut map) = value {
                        let typ: u8 = interaction.kind().0;
                        map.insert("type".to_string(), serde_json::Value::Number(typ.into()));
                    }

                    serde_json::json!({
                        "InteractionCreate": {
                            "interaction": value
                        }
                    })
                }
                _ => {
                    let mut value = serde_json::to_value(interaction)?; // Allow Component+Modal interactions to freely passed through

                    // Inject in type
                    if let serde_json::Value::Object(ref mut map) = value {
                        let typ: u8 = interaction.kind().0;
                        map.insert("type".to_string(), serde_json::Value::Number(typ.into()));
                    }

                    serde_json::json!({
                        "InteractionCreate": {
                            "interaction": value
                        }
                    })
                }
            }
        }
        // Ignore ourselves as well as interaction creates that are reserved
        _ => {
            if let Some(user_id) = user_id {
                if user_id == serenity_context.cache.current_user().id {
                    return Ok(());
                }
            }

            serde_json::to_value(event)?
        }
    };

    dispatch(
        serenity_context,
        &data,
        CreateEvent::new(
            "Discord".to_string(),
            event.snake_case_name().to_uppercase(),
            event_data,
            user_id.map(|u| u.to_string()),
        ),
        guild_id,
    )
    .await
}

/// Parses an antiraid event into a template event
pub fn parse_event(event: &AntiraidEvent) -> Result<CreateEvent, silverpelt::Error> {
    Ok(CreateEvent::new(
        "AntiRaid".to_string(),
        event.to_string(),
        event.to_value()?,
        event.author(),
    ))
}

pub async fn dispatch(
    ctx: &Context,
    data: &Data,
    event: CreateEvent,
    guild_id: GuildId,
) -> Result<(), silverpelt::Error> {
    if !has_templates_with_event(guild_id, &event).await {
        if event.name() == "INTERACTION_CREATE" {
            log::debug!("No templates for event: {}", event.name());
        }
        return Ok(());
    };

    let res = execute(
        ParseCompileState {
            serenity_context: ctx.clone(),
            pool: data.pool.clone(),
            reqwest_client: data.reqwest.clone(),
            object_store: data.object_store.clone(),
            guild_id,
        },
        LuaVmAction::DispatchEvent { event },
    )
    .await?;

    let serenity_context = ctx.clone();

    tokio::task::spawn(async move {
        res.wait_and_log_error(guild_id, &serenity_context)
            .await
            .map_err(|e| {
                log::error!("Error while waiting for template: {}", e);
            })
    });

    Ok(())
}

pub async fn dispatch_to_template(
    ctx: &Context,
    data: &Data,
    event: CreateEvent,
    guild_id: GuildId,
    template_name: String,
) -> Result<(), silverpelt::Error> {
    if !has_templates(guild_id) {
        return Ok(());
    };

    let res = execute(
        ParseCompileState {
            serenity_context: ctx.clone(),
            pool: data.pool.clone(),
            reqwest_client: data.reqwest.clone(),
            object_store: data.object_store.clone(),
            guild_id,
        },
        LuaVmAction::DispatchTemplateEvent { event, template_name },
    )
    .await?;

    let serenity_context = ctx.clone();

    tokio::task::spawn(async move {
        res.wait_and_log_error(guild_id, &serenity_context)
            .await
            .map_err(|e| {
                log::error!("Error while waiting for template [template event]: {}", e);
            })
    });

    Ok(())
}

/// Dispatches a template event to all templates, waiting for the response and returning it
pub async fn dispatch_and_wait(
    ctx: &Context,
    data: &Data,
    event: CreateEvent,
    guild_id: GuildId,
    wait_timeout: std::time::Duration,
) -> Result<HashMap<String, serde_json::Value>, silverpelt::Error> {
    if !has_templates(guild_id) {
        return Ok(HashMap::new());
    };

    let handle = execute(
        ParseCompileState {
            serenity_context: ctx.clone(),
            pool: data.pool.clone(),
            reqwest_client: data.reqwest.clone(),
            object_store: data.object_store.clone(),
            guild_id,
        },
        LuaVmAction::DispatchEvent { event },
    )
    .await?;

    let result_handle = match handle.wait_timeout(wait_timeout).await {
        Ok(Some(action)) => action,
        Ok(None) => return Err("Timed out while waiting for response".into()),
        Err(e) => return Err(e.to_string().into()),
    };

    let mut results = HashMap::with_capacity(result_handle.results.len());

    for result in result_handle.results {
        if let Err(e) = result.log_error(guild_id, ctx).await {
            log::error!("Error while waiting for template: {}", e);
            continue;
        }

        let name = result.template_name.clone();
        if let Ok(value) = result.into_response::<serde_json::Value>() {
            results.insert(name, value);
        }
    }

    Ok(results)
}

#[allow(dead_code)]
/// Dispatches a template event to all templates, waiting for the response and returning it
pub async fn dispatch_to_template_and_wait(
    ctx: &Context,
    data: &Data,
    event: CreateEvent,
    guild_id: GuildId,
    template_name: String,
    wait_timeout: std::time::Duration,
) -> Result<HashMap<String, serde_json::Value>, silverpelt::Error> {
    if !has_templates(guild_id) {
        return Ok(HashMap::new());
    };

    let handle = execute(
        ParseCompileState {
            serenity_context: ctx.clone(),
            pool: data.pool.clone(),
            reqwest_client: data.reqwest.clone(),
            object_store: data.object_store.clone(),
            guild_id,
        },
        LuaVmAction::DispatchTemplateEvent { event, template_name },
    )
    .await?;

    let result_handle = match handle.wait_timeout(wait_timeout).await {
        Ok(Some(action)) => action,
        Ok(None) => return Err("Timed out while waiting for response".into()),
        Err(e) => return Err(e.to_string().into()),
    };

    let mut results = HashMap::with_capacity(result_handle.results.len());

    for result in result_handle.results {
        if let Err(e) = result.log_error(guild_id, ctx).await {
            log::error!("Error while waiting for template: {}", e);
            continue;
        }

        let name = result.template_name.clone();
        if let Ok(value) = result.into_response::<serde_json::Value>() {
            results.insert(name, value);
        }
    }

    Ok(results)
}