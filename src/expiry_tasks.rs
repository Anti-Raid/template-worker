use antiraid_types::ar_event::AntiraidEvent;
use silverpelt::data::Data;
use std::time::Duration;

use crate::dispatch::{dispatch_scoped, parse_event};
use crate::templatingrt::cache::{get_all_expired_keys, remove_key_expiry};

const EXPIRY_TICK_TIME: Duration = Duration::from_secs(5);

pub async fn key_expiry_task(ctx: serenity::all::client::Context) -> ! {
    pub async fn event_listener(
        guild_id: serenity::all::GuildId,
        scopes: &[String],
        data: &Data,
        event: AntiraidEvent,
        serenity_context: &serenity::all::Context,
    ) -> Result<(), silverpelt::Error> {
        let tevent = parse_event(&event)?;

        log::info!(
            "Dispatching key expiry event: {} and scopes {:?}",
            tevent.name(),
            scopes
        );
        dispatch_scoped(serenity_context, data, tevent, scopes, guild_id).await?;

        Ok(())
    }

    let data = ctx.data::<silverpelt::data::Data>();
    let mut set = tokio::task::JoinSet::new();
    loop {
        for (guild_id, expired_task) in get_all_expired_keys() {
            let event = AntiraidEvent::KeyExpiry(antiraid_types::ar_event::KeyExpiryEvent {
                id: expired_task.id.clone(),
                key: expired_task.key.clone(),
                scopes: expired_task.scopes.clone(),
            });

            let ctx = ctx.clone();
            let data = data.clone();

            set.spawn(async move {
                match event_listener(guild_id, &expired_task.scopes, &data, event, &ctx).await {
                    Ok(_) => {
                        log::info!("Expiring key: {}", expired_task.key);
                        match remove_key_expiry(guild_id, &expired_task.id, &data.pool).await {
                            Ok(_) => {}
                            Err(e) => {
                                log::error!("Error removing scheduled execution: {:?}", e);
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("Error in scheduled_executions_task: {:?}", e);
                    }
                }
            });
        }

        tokio::time::sleep(EXPIRY_TICK_TIME).await;
    }
}
