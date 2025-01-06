use serenity::all::{Context, FullEvent, GuildId, Interaction};
use silverpelt::ar_event::AntiraidEvent;
use silverpelt::data::Data;
use templating::{get_all_guild_templates, CreateEvent};

use crate::serenitystore::shard_messenger_for_guild;

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
) -> Result<(), silverpelt::Error> {
    let data = serenity_context.data::<Data>();

    let Some(guild_id) = gwevent::core::get_event_guild_id(event) else {
        return Ok(());
    };

    let event_snake_name = event.snake_case_name();
    if not_audit_loggable_event().contains(&event_snake_name) {
        return Ok(());
    }

    let user_id = gwevent::core::get_event_user_id(event);

    match event {
        FullEvent::GuildAuditLogEntryCreate { .. } => {}
        FullEvent::InteractionCreate { interaction } => {
            match interaction {
                Interaction::Ping(_) => return Ok(()),
                Interaction::Command(i) | Interaction::Autocomplete(i) => {
                    if limits::command_name_limits::RESERVED_COMMAND_NAMES
                        .contains(&i.data.name.as_str())
                    {
                        return Ok(());
                    }
                }
                _ => {} // Allow Component+Modal interactions to freely passed through
            }
        }
        // Ignore ourselves as well as interaction creates that are reserved
        _ => {
            if let Some(user_id) = user_id {
                if user_id == serenity_context.cache.current_user().id {
                    return Ok(());
                }
            }
        }
    }

    dispatch(
        serenity_context,
        &data,
        CreateEvent::new(
            "Discord".to_string(),
            event.snake_case_name().to_uppercase(),
            serde_json::to_value(event)?,
            user_id.map(|u| u.to_string()),
        ),
        guild_id,
    )
    .await
}

/// Parses an antiraid event into a template event
pub fn parse_event(event: &AntiraidEvent) -> Result<CreateEvent, silverpelt::Error> {
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
) -> Result<(), silverpelt::Error> {
    let templates = get_all_guild_templates(guild_id, &data.pool).await?;

    for template in templates.iter().filter(|template| {
        template.events.contains(&event.name().to_string())
            || template.events.contains(&event.base_name().to_string())
    }) {
        log::info!("Dispatching event: {} to {}", event.name(), template.name);

        match templating::execute(
            event.clone(),
            templating::ParseCompileState {
                serenity_context: ctx.clone(),
                shard_messenger: crate::serenitystore::shard_messenger_for_guild(guild_id)?,
                pool: data.pool.clone(),
                reqwest_client: data.reqwest.clone(),
                guild_id,
            },
            template.clone(),
        )
        .await
        {
            Ok(handle) => {
                let template_name = template.name.clone();
                let pool = data.pool.clone();
                let serenity_context = ctx.clone();
                tokio::task::spawn(async move {
                    handle
                        .wait_and_log_error(&template_name, guild_id, &pool, &serenity_context)
                        .await
                        .map_err(|e| {
                            log::error!("Error while waiting for template: {}", e);
                        })
                });
            }
            Err(e) => {
                templating::dispatch_error(ctx, &e.to_string(), guild_id, template).await?;
            }
        }
    }
    Ok(())
}

/// Dispatches a template event to all templates, waiting for the response and returning it
pub async fn dispatch_and_wait(
    ctx: &Context,
    data: &Data,
    event: CreateEvent,
    guild_id: GuildId,
    wait_timeout: std::time::Duration,
) -> Result<Vec<serde_json::Value>, silverpelt::Error> {
    let templates = get_all_guild_templates(guild_id, &data.pool).await?;

    let mut local_set = tokio::task::JoinSet::new();
    for template in templates.iter().filter(|template| {
        template.events.contains(&event.name().to_string())
            || template.events.contains(&event.base_name().to_string())
    }) {
        log::info!("Dispatching event: {} to {}", event.name(), template.name);

        match templating::execute(
            event.clone(),
            templating::ParseCompileState {
                serenity_context: ctx.clone(),
                shard_messenger: shard_messenger_for_guild(guild_id)?,
                pool: data.pool.clone(),
                reqwest_client: data.reqwest.clone(),
                guild_id,
            },
            template.clone(),
        )
        .await
        {
            Ok(handle) => {
                let template = template.name.clone();
                local_set.spawn(async move {
                    match handle.wait_timeout(wait_timeout).await {
                        Ok(Some(action)) => action
                            .into_response::<serde_json::Value>()
                            .map_err(|e| (e, template)),
                        Ok(None) => Err(("Timed out while waiting for response".into(), template)),
                        Err(e) => Err((e.to_string().into(), template)),
                    }
                });
            }
            Err(e) => return Err(e),
        }
    }

    let mut results = Vec::with_capacity(local_set.len());

    while let Some(result) = local_set.join_next().await {
        let result = result?;
        match result {
            Ok(r) => results.push(r),
            Err((e, template_name)) => {
                local_set.abort_all();

                while (local_set.join_next().await).is_some() {
                    // Drain the rest of the results
                }

                // Try logging error
                if let Err(e) =
                    templating::log_error(guild_id, &data.pool, ctx, &template_name, e.to_string())
                        .await
                {
                    log::error!("Error while logging error: {}", e);
                }

                templating::dispatch_error(
                    ctx,
                    &e.to_string(),
                    guild_id,
                    &templates[results.len()],
                )
                .await
                .ok();

                return Err(e);
            }
        }
    }

    Ok(results)
}
