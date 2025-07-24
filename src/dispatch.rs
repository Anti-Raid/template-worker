use std::collections::HashMap;
use std::sync::Arc;

use crate::data::Data;
use crate::templatingrt::cache::{get_templates_with_event, get_templates_with_event_scoped, get_templates_by_name};
use crate::templatingrt::{IntoResponse, KhronosValueResponse};
use crate::templatingrt::{fire, execute, CreateGuildState, LuaVmAction, template::Template};
use antiraid_types::ar_event::AntiraidEvent;
use indexmap::IndexMap;
use khronos_runtime::primitives::event::CreateEvent;
use khronos_runtime::utils::khronos_value::KhronosValue;
use serenity::all::{Context, IEvent, GuildId};

pub async fn discord_event_dispatch(
    event: IEvent,
    serenity_context: &Context,
) -> Result<(), crate::Error> {
    if event.ty == "GUILD_CREATE" {
        // Ignore guild create events
        return Ok(());
    }

    let data = serenity_context.data::<Data>();

    let guild_id = match event.sandwich_edt {
        Some(ref edt) => {
            if let Some(user_id) = edt.user_id {
                if user_id == data.current_user.id {
                    return Ok(());
                }
            }

            match edt.guild_id {
                Some(guild_id) => guild_id,
                None => return Ok(()), // No guild ID, nothing to do
            }
        },
        None => return Ok(()), // No EventDispatchIdentifier from Sandwich-Daemon, nothing to do
    };


    dispatch(
        serenity_context,
        &data,
        CreateEvent::new_raw_value(
            "Discord".to_string(),
            if event.ty.as_str() == "MESSAGE_CREATE" {
                "MESSAGE".to_string() // Message events are called MESSAGE and not MESSAGE_CREATE in AntiRaid for backwards compatibility
            } else {
                event.ty
            },
            event.data,
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
    ))
}

/// Dispatch without waiting for a response
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

    fire(
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
    .await
}

/// Dispatches a template event to all templates, waiting for as many responses as possible and returning it
pub async fn dispatch_to_and_wait<T: IntoResponse>(
    ctx: &Context,
    data: &Data,
    event: CreateEvent,
    guild_id: GuildId,
    wait_timeout: std::time::Duration,
    templates: Vec<Arc<Template>>,
) -> Result<HashMap<String, DispatchResult<T>>, crate::Error> {
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
            templates,
        },
    )
    .await?;

    let result_handle = handle.wait_timeout(wait_timeout).await?;

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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum DispatchResult<T> {
    Ok(T),
    Err(String),
}

#[allow(dead_code)]
impl DispatchResult<KhronosValue> {
    pub fn into_khronos_value(self) -> Result<KhronosValue, crate::Error> {
        match self {
            DispatchResult::Ok(value) => Ok(value),
            DispatchResult::Err(e) => Err(e.into()),
        }
    }
}

impl DispatchResult<KhronosValueResponse> {
    pub fn into_khronos_value(self) -> Result<KhronosValue, crate::Error> {
        match self {
            DispatchResult::Ok(value) => Ok(value.0),
            DispatchResult::Err(e) => Err(e.into()),
        }
    }
}

/// Simple helper to convert a dispatch result of KhronosValueResponse into a KhronosValue
pub struct KhronosValueMapper(pub HashMap<String, DispatchResult<KhronosValueResponse>>);

impl KhronosValueMapper {
    pub fn into_khronos_value(self) -> Result<KhronosValue, crate::Error> {
        let mut result = IndexMap::with_capacity(self.0.len());
        for (key, value) in self.0 {
            result.insert(key, value.into_khronos_value()?);
        }
        Ok(KhronosValue::Map(result))
    }
}

/// Dispatches a template event to all templates, waiting for the response and returning it
pub async fn dispatch_and_wait<T: IntoResponse>(
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

    dispatch_to_and_wait(
        ctx,
        data,
        event,
        guild_id,
        wait_timeout,
        matching,
    ).await
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
pub async fn dispatch_scoped_and_wait<T: IntoResponse>(
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

    dispatch_to_and_wait(
        ctx,
        data,
        event,
        guild_id,
        wait_timeout,
        matching,
    ).await
}

/// Dispatches a template event to all templates, waiting for the response and returning it
pub async fn dispatch_to_template_and_wait<T: IntoResponse>(
    ctx: &Context,
    data: &Data,
    event: CreateEvent,
    guild_id: GuildId,
    wait_timeout: std::time::Duration,
    template_name: &str,
) -> Result<HashMap<String, DispatchResult<T>>, crate::Error> {
    let matching = get_templates_by_name(guild_id, template_name).await;
    if matching.is_empty() {
        return Ok(HashMap::new());
    };

    dispatch_to_and_wait(
        ctx,
        data,
        event,
        guild_id,
        wait_timeout,
        matching,
    ).await
}
