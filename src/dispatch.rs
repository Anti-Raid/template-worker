use std::collections::HashMap;

use crate::{data::Data, worker::workervmmanager::Id};
use crate::events::AntiraidEvent;
use crate::worker::workerdispatch::DispatchTemplateResult;
use khronos_runtime::primitives::event::CreateEvent;
use khronos_runtime::utils::khronos_value::KhronosValue;
use serenity::all::{Context, IEvent};

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

    data.worker
    .dispatch_event_to_templates_nowait(
        Id::GuildId(guild_id),
        CreateEvent::new_raw_value(
            "Discord".to_string(),
            if event.ty.as_str() == "MESSAGE_CREATE" {
                "MESSAGE".to_string() // Message events are called MESSAGE and not MESSAGE_CREATE in AntiRaid for backwards compatibility
            } else {
                event.ty
            },
            event.data,
        ),
    )
    .await?;

    Ok(())
}

/// Parses an antiraid event into a template event
pub fn parse_event(event: &AntiraidEvent) -> Result<CreateEvent, crate::Error> {
    Ok(CreateEvent::new(
        "AntiRaid".to_string(),
        event.to_string(),
        event.to_value()?,
    ))
}

/// Parses a DispatchTemplateResult into a HashMap of DispatchResult<T>'s
pub fn parse_response<T: IntoResponse>(response: DispatchTemplateResult) -> Result<HashMap<String, DispatchResult<T>>, crate::Error> {
    match response {
        DispatchTemplateResult::Ok(value) => {
            let mut results = HashMap::with_capacity(value.len());
            for (name, result) in value {
                let result = match result {
                    Ok(result) => result,
                    Err(e) => {
                        results.insert(name, DispatchResult::Err(e.to_string()));
                        continue;
                    },
                };
                match T::into_response_without_types(result) {
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
        DispatchTemplateResult::Err(e) => {
            Err(e.into())
        }
    }
}

#[allow(unused)]
pub trait IntoResponse
where Self: Sized {
    fn into_response(value: KhronosValue) -> Result<Self, crate::Error>;
    fn into_response_without_types(value: KhronosValue) -> Result<Self, crate::Error>;
}

impl<T: serde::de::DeserializeOwned> IntoResponse for T {
    fn into_response(value: KhronosValue) -> Result<Self, crate::Error> {
        value.into_value::<T>()
    }

    fn into_response_without_types(value: KhronosValue) -> Result<Self, crate::Error> {
        value.into_value_untyped::<T>()
    }
}

#[derive(Debug, Clone)]
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