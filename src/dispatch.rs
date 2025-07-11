use std::collections::HashMap;

use crate::data::Data;
use crate::templatingrt::cache::{get_templates_with_event, get_templates_with_event_scoped};
use crate::templatingrt::{execute, CreateGuildState, LuaVmAction};
use antiraid_types::ar_event::AntiraidEvent;
use khronos_runtime::primitives::event::CreateEvent;
use serenity::all::{Context, FullEvent, GuildId, Interaction};

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
) -> Result<(), crate::Error> {
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
pub fn parse_event(event: &AntiraidEvent) -> Result<CreateEvent, crate::Error> {
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
) -> Result<(), crate::Error> {
    let matching = get_templates_with_event(guild_id, &event).await;
    if matching.is_empty() {
        if event.name() == "INTERACTION_CREATE" {
            log::debug!("No templates for event: {}", event.name());
        }
        return Ok(());
    };

    execute(
        guild_id,
        CreateGuildState {
            serenity_context: ctx.clone(),
            pool: data.pool.clone(),
            reqwest_client: data.reqwest.clone(),
            object_store: data.object_store.clone(),
        },
        LuaVmAction::DispatchEvent {
            event,
            templates: matching,
        },
    )
    .await?;

    Ok(())
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum DispatchResult<T> {
    Ok(T),
    Err(String),
}

/// Dispatches a template event to all templates, waiting for the response and returning it
pub async fn dispatch_and_wait<T: serde::de::DeserializeOwned>(
    ctx: &Context,
    data: &Data,
    event: CreateEvent,
    guild_id: GuildId,
    wait_timeout: std::time::Duration,
) -> Result<HashMap<String, DispatchResult<T>>, crate::Error> {
    let matching = get_templates_with_event(guild_id, &event).await;
    if matching.is_empty() {
        return Ok(HashMap::new());
    };

    let handle = execute(
        guild_id,
        CreateGuildState {
            serenity_context: ctx.clone(),
            pool: data.pool.clone(),
            reqwest_client: data.reqwest.clone(),
            object_store: data.object_store.clone(),
        },
        LuaVmAction::DispatchEvent {
            event,
            templates: matching,
        },
    )
    .await?;

    let result_handle = match handle.wait_timeout(wait_timeout).await {
        Ok(Some(action)) => action,
        Ok(None) => return Err("Timed out while waiting for response".into()),
        Err(e) => return Err(e.to_string().into()),
    };

    let mut results = HashMap::with_capacity(result_handle.results.len());

    for result in result_handle.results {
        let name = result.template_name.clone();

        if let Some(e) = result.lua_error() {
            results.insert(name, DispatchResult::Err(e.to_string()));
            continue;
        }

        match result.into_response_without_types::<T>() {
            Ok(value) => {
                results.insert(name, DispatchResult::Ok(value));
            }
            Err(e) => {
                results.insert(name, DispatchResult::Err(e.to_string()));
            }
        }
    }

    Ok(results)
}

#[allow(dead_code)]
pub async fn dispatch_scoped(
    ctx: &Context,
    data: &Data,
    event: CreateEvent,
    scopes: &[String],
    guild_id: GuildId,
) -> Result<(), crate::Error> {
    let matching = get_templates_with_event_scoped(guild_id, &event, scopes).await;
    if matching.is_empty() {
        log::debug!("No templates for event: {}", event.name());
        return Ok(());
    };

    execute(
        guild_id,
        CreateGuildState {
            serenity_context: ctx.clone(),
            pool: data.pool.clone(),
            reqwest_client: data.reqwest.clone(),
            object_store: data.object_store.clone(),
        },
        LuaVmAction::DispatchEvent {
            event,
            templates: matching,
        },
    )
    .await?;

    Ok(())
}

/// Dispatches a template event to all templates, waiting for the response and returning it
pub async fn dispatch_scoped_and_wait<T: serde::de::DeserializeOwned>(
    ctx: &Context,
    data: &Data,
    event: CreateEvent,
    scopes: &[String],
    guild_id: GuildId,
    wait_timeout: std::time::Duration,
) -> Result<HashMap<String, DispatchResult<T>>, crate::Error> {
    let matching = get_templates_with_event_scoped(guild_id, &event, scopes).await;
    if matching.is_empty() {
        return Ok(HashMap::new());
    };

    let handle = execute(
        guild_id,
        CreateGuildState {
            serenity_context: ctx.clone(),
            pool: data.pool.clone(),
            reqwest_client: data.reqwest.clone(),
            object_store: data.object_store.clone(),
        },
        LuaVmAction::DispatchEvent {
            event,
            templates: matching,
        },
    )
    .await?;

    let result_handle = match handle.wait_timeout(wait_timeout).await {
        Ok(Some(action)) => action,
        Ok(None) => return Err("Timed out while waiting for response".into()),
        Err(e) => return Err(e.to_string().into()),
    };

    let mut results = HashMap::with_capacity(result_handle.results.len());

    for result in result_handle.results {
        let name = result.template_name.clone();

        if let Some(e) = result.lua_error() {
            results.insert(name, DispatchResult::Err(e.to_string()));
            continue;
        }

        match result.into_response_without_types::<T>() {
            Ok(value) => {
                results.insert(name, DispatchResult::Ok(value));
            }
            Err(e) => {
                results.insert(name, DispatchResult::Err(e.to_string()));
            }
        }
    }

    Ok(results)
}
