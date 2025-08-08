use crate::dispatch::discord_event_dispatch;
use async_trait::async_trait;
use serenity::all::{EventHandler, IEvent};
use serenity::gateway::client::Context;

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
