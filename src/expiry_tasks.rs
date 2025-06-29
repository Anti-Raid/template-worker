use crate::data::Data;
use antiraid_types::ar_event::AntiraidEvent;
use std::time::Duration;

use crate::dispatch::{dispatch_scoped_and_wait, parse_event};
use crate::templatingrt::cache::{extend_key_expiry, get_all_expired_keys, remove_key_expiry};
use crate::templatingrt::MAX_TEMPLATES_RETURN_WAIT_TIME;

const EXPIRY_TICK_TIME: Duration = Duration::from_secs(5);
const EXTEND_EXPIRY_BY: Duration = Duration::from_secs(60 * 60); // 1 hour

pub async fn key_expiry_task(ctx: serenity::all::client::Context) -> ! {
    pub async fn event_listener(
        guild_id: serenity::all::GuildId,
        scopes: &[String],
        data: &Data,
        event: AntiraidEvent,
        serenity_context: &serenity::all::Context,
    ) -> Result<(), crate::Error> {
        let tevent = parse_event(&event)?;

        log::info!(
            "Dispatching key expiry event: {} and scopes {:?}",
            tevent.name(),
            scopes
        );

        dispatch_scoped_and_wait::<serde_json::Value>(
            serenity_context,
            data,
            tevent,
            scopes,
            guild_id,
            MAX_TEMPLATES_RETURN_WAIT_TIME,
        )
        .await?;

        Ok(())
    }

    let data = ctx.data::<crate::data::Data>();
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
                        let new_expiry = chrono::Utc::now() + EXTEND_EXPIRY_BY;
                        match extend_key_expiry(guild_id, &expired_task.id, new_expiry, &data.pool)
                            .await
                        {
                            Ok(_) => {}
                            Err(e) => {
                                log::error!("Error removing scheduled execution: {:?}", e);
                            }
                        }

                        log::error!("Error in scheduled_executions_task: {:?}", e);
                    }
                }
            });
        }

        tokio::time::sleep(EXPIRY_TICK_TIME).await;
    }
}
