use std::time::Duration;

use serenity::{
    all::{shard_id, ShardId},
    futures::FutureExt,
};
use silverpelt::{ar_event::AntiraidEvent, data::Data, punishments::Punishment, stings::Sting};

use crate::{
    dispatch::{dispatch, parse_event},
    temporary_punishments::handle_expired_punishment,
};

pub async fn punishment_expiry_task(
    ctx: &serenity::all::client::Context,
) -> Result<(), silverpelt::Error> {
    pub async fn event_listener(
        guild_id: serenity::all::GuildId,
        data: &Data,
        event: AntiraidEvent,
        serenity_context: &serenity::all::Context,
    ) -> Result<(), silverpelt::Error> {
        let tevent = parse_event(&event)?;

        dispatch(serenity_context, data, tevent, guild_id).await?;

        if let AntiraidEvent::PunishmentExpire(ref punishment) = event {
            handle_expired_punishment(data, serenity_context, punishment).await?;
        }

        Ok(())
    }

    let data = ctx.data::<silverpelt::data::Data>();
    let pool = &data.pool;

    let punishments = Punishment::get_expired(pool).await?;

    let mut set = tokio::task::JoinSet::new();

    let shard_count = templating::shard_count()?;
    let shards = templating::shard_ids()?;

    for punishment in punishments {
        let guild_id = punishment.guild_id;

        // Ensure shard id
        let shard_id = ShardId(shard_id(guild_id, shard_count));

        if !shards.contains(&shard_id) {
            continue;
        }

        // Dispatch event
        let punishment_id = punishment.id;
        let event = AntiraidEvent::PunishmentExpire(punishment);

        // Spawn task to dispatch event
        let data = data.clone(); // Cloned for flagging is_handled
        let ctx = ctx.clone();
        set.spawn(async move {
            match event_listener(guild_id, &data, event, &ctx).await {
                Ok(()) => {
                    // Mark the punishment as handled
                    let _ = sqlx::query!(
                        "UPDATE punishments SET is_handled = true WHERE id = $1",
                        punishment_id
                    )
                    .execute(&data.pool)
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
                    .execute(&data.pool)
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
    pub async fn event_listener(
        guild_id: serenity::all::GuildId,
        data: &Data,
        event: AntiraidEvent,
        serenity_context: &serenity::all::Context,
    ) -> Result<(), silverpelt::Error> {
        let tevent = parse_event(&event)?;

        dispatch(serenity_context, data, tevent, guild_id).await?;

        Ok(())
    }

    let data = ctx.data::<silverpelt::data::Data>();
    let pool = &data.pool;

    let stings = Sting::get_expired(pool).await?;

    let mut set = tokio::task::JoinSet::new();

    let shard_count = templating::shard_count()?;
    let shards = templating::shard_ids()?;

    for sting in stings {
        let guild_id = sting.guild_id;

        // Ensure shard id
        let shard_id = ShardId(shard_id(guild_id, shard_count));

        if !shards.contains(&shard_id) {
            continue;
        }

        // Dispatch event
        let sting_id = sting.id;
        let event = AntiraidEvent::StingExpire(sting);

        // Spawn task to dispatch event
        let data = data.clone(); // Cloned for flagging is_handled
        let ctx = ctx.clone();
        set.spawn(async move {
            match event_listener(guild_id, &data, event, &ctx).await {
                Ok(()) => {
                    // Mark the punishment as handled
                    let _ = sqlx::query!(
                        "UPDATE stings SET is_handled = true WHERE id = $1",
                        sting_id
                    )
                    .execute(&data.pool)
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
                    .execute(&data.pool)
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
            run: Box::new(move |ctx| stings_expiry_task(ctx).boxed()),
        },
        botox::taskman::Task {
            name: "punishment_expiry",
            description: "Check for expired punishments and dispatch the required event",
            enabled: true,
            duration: Duration::from_secs(60),
            run: Box::new(move |ctx| punishment_expiry_task(ctx).boxed()),
        },
    ]
}
