use async_trait::async_trait;
use khronos_runtime::primitives::event::CreateEvent;
use serenity::all::{EventHandler, IEvent};
use serenity::gateway::client::Context;
use crate::{data::Data, worker::workervmmanager::Id};

pub struct EventFramework {}

#[async_trait]
impl EventHandler for EventFramework {
    async fn dispatch(&self, ctx: &Context, event: IEvent) {
        if event.ty == "GUILD_CREATE" {
            // Ignore guild create events
            return;
        }

        match discord_event_dispatch(event, ctx).await {
            Ok(_) => {}
            Err(e) => {
                log::error!("Error dispatching event: {:?}", e);
            }
        }
    }
}

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