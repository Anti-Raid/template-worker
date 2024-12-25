use std::time::Duration;

use serenity::futures::FutureExt;

pub async fn punishment_expiry_task(
    ctx: &serenity::all::client::Context,
) -> Result<(), silverpelt::Error> {
    let data = ctx.data::<silverpelt::data::Data>();
    let pool = &data.pool;

    let punishments = silverpelt::punishments::Punishment::get_expired(pool).await?;

    let mut set = tokio::task::JoinSet::new();

    let shard_count = templating::shard_count()?;
    let shards = templating::shard_ids()?;

    for punishment in punishments {
        let guild_id = punishment.guild_id;

        // Ensure shard id
        let shard_id = serenity::all::ShardId(serenity::utils::shard_id(guild_id, shard_count));

        if !shards.contains(&shard_id) {
            continue;
        }

        // Dispatch event
        let punishment_id = punishment.id;
        let event = silverpelt::ar_event::AntiraidEvent::PunishmentExpire(punishment);

        let event_handler_context = silverpelt::ar_event::EventHandlerContext {
            event,
            guild_id,
            data: data.clone(),
            serenity_context: ctx.clone(),
        };

        // Spawn task to dispatch event
        let pool = data.pool.clone(); // Cloned for flagging is_handled
        set.spawn(async move {
            match crate::dispatch::event_listener(event_handler_context).await {
                Ok(()) => {
                    // Mark the punishment as handled
                    let _ = sqlx::query!(
                        "UPDATE punishments SET is_handled = true WHERE id = $1",
                        punishment_id
                    )
                    .execute(&pool)
                    .await;
                }
                Err(e) => {
                    log::error!("Error in punishment_expiry_task: {:?}", e);
                    // Mark the punishment as handled
                    let _ = sqlx::query!(
                        "UPDATE punishments SET is_handled = true, handle_log = $2 WHERE id = $1",
                        punishment_id,
                        serde_json::json!({
                            "error": format!("{:?}", e),
                        })
                    )
                    .execute(&pool)
                    .await;
                }
            }
        });
    }

    while let Some(res) = set.join_next().await {
        match res {
            Ok(()) => {}
            Err(e) => {
                log::error!("Error in punishment_expiry_task: {}", e);
            }
        }
    }

    Ok(())
}

pub async fn stings_expiry_task(
    ctx: &serenity::all::client::Context,
) -> Result<(), silverpelt::Error> {
    let data = ctx.data::<silverpelt::data::Data>();
    let pool = &data.pool;

    let stings = silverpelt::stings::Sting::get_expired(pool).await?;

    let mut set = tokio::task::JoinSet::new();

    let shard_count = templating::shard_count()?;
    let shards = templating::shard_ids()?;

    for sting in stings {
        let guild_id = sting.guild_id;

        // Ensure shard id
        let shard_id = serenity::all::ShardId(serenity::utils::shard_id(guild_id, shard_count));

        if !shards.contains(&shard_id) {
            continue;
        }

        // Dispatch event
        let sting_id = sting.id;
        let event = silverpelt::ar_event::AntiraidEvent::StingExpire(sting);

        let event_handler_context = silverpelt::ar_event::EventHandlerContext {
            event,
            guild_id,
            data: data.clone(),
            serenity_context: ctx.clone(),
        };

        // Spawn task to dispatch event
        let pool = data.pool.clone(); // Cloned for flagging is_handled
        set.spawn(async move {
            match crate::dispatch::event_listener(event_handler_context).await {
                Ok(()) => {
                    // Mark the punishment as handled
                    let _ = sqlx::query!(
                        "UPDATE stings SET is_handled = true WHERE id = $1",
                        sting_id
                    )
                    .execute(&pool)
                    .await;
                }
                Err(e) => {
                    log::error!("Error in stings_expiry_task: {:?}", e);
                    // Mark the punishment as handled
                    let _ = sqlx::query!(
                        "UPDATE stings SET is_handled = true, handle_log = $2 WHERE id = $1",
                        sting_id,
                        serde_json::json!({
                            "error": format!("{:?}", e),
                        })
                    )
                    .execute(&pool)
                    .await;
                }
            }
        });
    }

    while let Some(res) = set.join_next().await {
        match res {
            Ok(()) => {}
            Err(e) => {
                log::error!("Error in sting_expiry_task: {}", e);
            }
        }
    }

    Ok(())
}

pub fn tasks() -> Vec<botox::taskman::Task> {
    vec![
        botox::taskman::Task {
            name: "sting_expiry",
            description: "Check for expired stings and dispatch the required event",
            enabled: true,
            duration: Duration::from_secs(60),
            run: Box::new(move |ctx| crate::expiry_tasks::stings_expiry_task(ctx).boxed()),
        },
        botox::taskman::Task {
            name: "punishment_expiry",
            description: "Check for expired punishments and dispatch the required event",
            enabled: true,
            duration: Duration::from_secs(60),
            run: Box::new(move |ctx| crate::expiry_tasks::punishment_expiry_task(ctx).boxed()),
        },
    ]
}
