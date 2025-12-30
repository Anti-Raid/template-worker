use crate::{data::Data, worker::workervmmanager::Id};
use crate::events::AntiraidEvent;
use khronos_runtime::primitives::event::CreateEvent;
use serenity::all::{Context, IEvent};

pub async fn discord_event_dispatch(
    event: IEvent,
    serenity_context: &Context,
) -> Result<(), crate::Error> {
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
    .dispatch_event_nowait(
        Id::GuildId(guild_id),
        CreateEvent::new_raw_value(
            if event.ty.as_str() == "MESSAGE_CREATE" {
                "MESSAGE".to_string() // Message events are called MESSAGE and not MESSAGE_CREATE in AntiRaid for backwards compatibility
            } else {
                event.ty
            },
            None,
            event.data,
        ),
    )
    .await?;

    Ok(())
}

/// Parses an antiraid event into a template event
pub fn parse_event(event: &AntiraidEvent) -> Result<CreateEvent, crate::Error> {
    Ok(CreateEvent::new(
        event.to_string(),
        None,
        event.to_value()?,
    ))
}
